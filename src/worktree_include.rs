//! Copies gitignored files a new worktree needs (`.env`, local config) from the main checkout.
//! `.worktreeinclude` at the repo root uses `.gitignore` syntax (`.env*`, `!.env.example`,
//! `secrets/**/*.json`); only paths Git ignores are copied, once, never over an existing file.
//! Ignored folders no pattern points into, such as `node_modules/`, are not scanned.

use std::fs;
use std::path::Path;
use std::process::Command;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

const INCLUDE_FILE: &str = ".worktreeinclude";

/// Copies what `src`'s include file selects into `dst`. Returns a message per path that
/// couldn't be copied; a missing include file copies nothing.
pub fn copy_includes(src: &Path, dst: &Path) -> Vec<String> {
    let Ok(list) = fs::read_to_string(src.join(INCLUDE_FILE)) else { return Vec::new() };
    let mut errors = Vec::new();
    let mut builder = GitignoreBuilder::new(src);
    for line in list.lines() {
        if let Err(err) = builder.add_line(None, line) {
            errors.push(format!("{INCLUDE_FILE}: {err}"));
        }
    }
    let rules = match builder.build() {
        Ok(rules) => rules,
        Err(err) => return vec![format!("{INCLUDE_FILE}: {err}")],
    };
    let prefixes = anchored_prefixes(&list);
    for entry in ignored_entries(src) {
        let rel = entry.trim_end_matches('/');
        let is_dir = entry.ends_with('/');
        let wanted = rules.matched(rel, is_dir).is_ignore();
        let points_inside = is_dir && prefixes.iter().any(|p| p.starts_with(&entry));
        if wanted || points_inside {
            copy_matching(&rules, src, dst, Path::new(rel), &mut errors);
        }
    }
    errors
}

/// Paths Git ignores under `root`, relative to it; a folder ignored as a whole is listed once,
/// with a trailing `/`, without its contents.
fn ignored_entries(root: &Path) -> Vec<String> {
    let args = ["ls-files", "-z", "--others", "--ignored", "--exclude-standard", "--directory"];
    let Ok(out) = Command::new("git").arg("-C").arg(root).args(args).output() else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    out.stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

/// The literal folder path each anchored pattern starts with (`secrets/` for `secrets/**/*.json`),
/// so an ignored folder is scanned only when a pattern names something inside it.
fn anchored_prefixes(list: &str) -> Vec<String> {
    list.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .filter_map(|l| {
            let l = l.trim_start_matches('/');
            let literal = &l[..l.find(['*', '?', '[']).unwrap_or(l.len())];
            literal.contains('/').then(|| literal.to_string())
        })
        .collect()
}

/// Copies `rel` from `src` to `dst`, descending into folders and keeping the files the rules
/// select; skips symlinks and anything already at the destination.
fn copy_matching(rules: &Gitignore, src: &Path, dst: &Path, rel: &Path, errors: &mut Vec<String>) {
    let from = src.join(rel);
    let Ok(meta) = fs::symlink_metadata(&from) else { return };
    if meta.file_type().is_symlink() {
        return;
    }
    if meta.is_dir() {
        let Ok(entries) = fs::read_dir(&from) else { return };
        for entry in entries.flatten() {
            copy_matching(rules, src, dst, &rel.join(entry.file_name()), errors);
        }
        return;
    }
    let to = dst.join(rel);
    if to.exists() || !rules.matched_path_or_any_parents(rel, false).is_ignore() {
        return;
    }
    if let Err(err) = copy_path(&from, &to) {
        errors.push(format!("{}: {err}", rel.display()));
    }
}

/// Copies a file, or a folder with everything in it, creating missing parent folders.
pub fn copy_path(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_path(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(from, to).map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("repodeck-inc-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        assert!(Command::new("git").arg("-C").arg(&dir).args(["init", "-q"]).status().unwrap().success());
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn copies_ignored_matches_with_gitignore_syntax() {
        let (src, dst) = (repo(), repo());
        write(&src, ".gitignore", ".env*\nsecrets/\nnode_modules/\nlocal.txt\nkeep.txt\n");
        write(&src, ".env", "A=1");
        write(&src, ".env.local", "B=2");
        write(&src, ".env.example", "X");
        write(&src, "secrets/a/key.json", "{}");
        write(&src, "secrets/a/key.pem", "pem");
        write(&src, "node_modules/pkg/.env", "nm");
        write(&src, "tracked.env", "not ignored");
        write(&src, "keep.txt", "src");
        write(&dst, "keep.txt", "dst");
        write(
            &src,
            INCLUDE_FILE,
            "# secrets\n.env*\n!.env.example\nsecrets/**/*.json\nkeep.txt\ntracked.env\n",
        );

        assert!(copy_includes(&src, &dst).is_empty());
        assert_eq!(fs::read_to_string(dst.join(".env")).unwrap(), "A=1");
        assert_eq!(fs::read_to_string(dst.join(".env.local")).unwrap(), "B=2");
        assert!(!dst.join(".env.example").exists(), "negated");
        assert!(dst.join("secrets/a/key.json").exists());
        assert!(!dst.join("secrets/a/key.pem").exists(), "not matched");
        assert!(!dst.join("node_modules").exists(), "ignored folder no pattern points into");
        assert!(!dst.join("tracked.env").exists(), "not gitignored");
        assert_eq!(fs::read_to_string(dst.join("keep.txt")).unwrap(), "dst");
        fs::remove_dir_all(src).unwrap();
        fs::remove_dir_all(dst).unwrap();
    }

    #[test]
    fn a_listed_folder_is_copied_whole() {
        let (src, dst) = (repo(), repo());
        write(&src, ".gitignore", "config/local/\n");
        write(&src, "config/local/dev.json", "{}");
        write(&src, INCLUDE_FILE, "config/local\n");
        assert!(copy_includes(&src, &dst).is_empty());
        assert_eq!(fs::read_to_string(dst.join("config/local/dev.json")).unwrap(), "{}");
        fs::remove_dir_all(src).unwrap();
        fs::remove_dir_all(dst).unwrap();
    }
}
