use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Lists gitignored paths (relative to the repo root) to copy into new worktrees, one per line.
const INCLUDE_FILE: &str = ".worktreeinclude";

/// Linked worktrees keep a `.git` file pointing into the main repository's `worktrees` directory
/// (submodules have such a file too, pointing into `modules`).
pub fn is_worktree(path: &Path) -> bool {
    fs::read_to_string(path.join(".git"))
        .is_ok_and(|content| content.starts_with("gitdir:") && content.contains("/worktrees/"))
}

/// Checks `branch` out at `path` (creating it when it doesn't exist) and copies the source repo's
/// include files over. Returns messages for includes that couldn't be copied.
pub fn add(source: &Path, path: &Path, branch: &str) -> Result<Vec<String>, String> {
    let path = path.to_string_lossy();
    let args: Vec<&str> = if branch_exists(source, branch) {
        vec!["worktree", "add", &path, branch]
    } else {
        vec!["worktree", "add", "-b", branch, &path]
    };
    run(source, &args)?;
    Ok(copy_includes(source, Path::new(path.as_ref())))
}

/// The main repository a linked worktree belongs to.
pub fn main_repo(path: &Path) -> Option<PathBuf> {
    let out = git(path, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    Path::new(out.trim_end()).parent().map(Path::to_path_buf)
}

/// Whether `HEAD` has commits and all of them are already in the default branch (`origin/HEAD`,
/// else `main` or `master`). A branch still sitting on the default tip doesn't count.
pub fn is_merged(path: &Path) -> bool {
    let default = git(path, &["rev-parse", "--abbrev-ref", "origin/HEAD"])
        .map(|s| s.trim().to_string())
        .or_else(|| ["main", "master"].into_iter().find(|b| branch_exists(path, b)).map(String::from));
    let Some(default) = default else { return false };
    let tip = |rev: &str| git(path, &["rev-parse", rev]).map(|s| s.trim().to_string());
    if tip("HEAD").is_none_or(|head| Some(head) == tip(&default)) {
        return false;
    }
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["merge-base", "--is-ancestor", "HEAD", &default])
        .status()
        .is_ok_and(|s| s.success())
}

fn branch_exists(path: &Path, branch: &str) -> bool {
    git(path, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_some()
}

/// Deletes the worktree at `path` from `main`, then its branch when asked and fully merged.
pub fn remove(main: &Path, path: &Path, delete_branch: bool) -> Result<(), String> {
    let branch = git(path, &["rev-parse", "--abbrev-ref", "HEAD"]).map(|b| b.trim().to_string());
    run(main, &["worktree", "remove", "--force", &path.to_string_lossy()])?;
    match branch.filter(|b| delete_branch && b != "HEAD") {
        Some(branch) => run(main, &["branch", "-d", &branch])
            .map_err(|err| format!("worktree removed, but branch {branch} was kept: {err}")),
        None => Ok(()),
    }
}

/// Copies the paths listed in the source repo's include file into `dst`, skipping ones already
/// there. Returns a message per path that couldn't be copied.
pub fn copy_includes(src: &Path, dst: &Path) -> Vec<String> {
    let Ok(list) = fs::read_to_string(src.join(INCLUDE_FILE)) else { return Vec::new() };
    let mut errors = Vec::new();
    for line in list.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let relative = Path::new(line);
        if !is_safe_relative(relative) {
            errors.push(format!("{line}: must be a relative path inside the repo"));
            continue;
        }
        let (from, to) = (src.join(relative), dst.join(relative));
        if !from.exists() || to.exists() {
            continue;
        }
        if let Err(err) = copy_path(&from, &to) {
            errors.push(format!("{line}: {err}"));
        }
    }
    errors
}

fn is_safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

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

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(path).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn run(path: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git").arg("-C").arg(path).args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("repodeck-wt-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copies_listed_files_and_folders_without_overwriting() {
        let root = temp_dir();
        let (src, dst) = (root.join("src"), root.join("dst"));
        fs::create_dir_all(src.join("config/local")).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join(".env"), "A=1").unwrap();
        fs::write(src.join("config/local/dev.json"), "{}").unwrap();
        fs::write(src.join("keep.txt"), "src").unwrap();
        fs::write(dst.join("keep.txt"), "dst").unwrap();
        fs::write(src.join(INCLUDE_FILE), "# secrets\n.env\nconfig/local\nkeep.txt\nmissing\n\n").unwrap();

        assert!(copy_includes(&src, &dst).is_empty());
        assert_eq!(fs::read_to_string(dst.join(".env")).unwrap(), "A=1");
        assert_eq!(fs::read_to_string(dst.join("config/local/dev.json")).unwrap(), "{}");
        assert_eq!(fs::read_to_string(dst.join("keep.txt")).unwrap(), "dst");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_paths_escaping_the_repo() {
        let root = temp_dir();
        let (src, dst) = (root.join("src"), root.join("dst"));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join(INCLUDE_FILE), "../outside\n/etc/passwd\n").unwrap();
        let errors = copy_includes(&src, &dst);
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(!dst.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_plain_directory_is_not_a_worktree() {
        let root = temp_dir();
        assert!(!is_worktree(&root));
        fs::write(root.join(".git"), "gitdir: /main/.git/modules/sub").unwrap();
        assert!(!is_worktree(&root), "submodules are not worktrees");
        fs::write(root.join(".git"), "gitdir: /main/.git/worktrees/feat").unwrap();
        assert!(is_worktree(&root));
        fs::remove_dir_all(root).unwrap();
    }
}
