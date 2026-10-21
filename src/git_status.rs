use std::path::Path;
use std::process::Command;

use crate::worktree;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    /// One-letter status such as `M`, `A`, `D`, `R` or `U` (untracked).
    pub status: char,
    /// Path relative to the repository root.
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitStatus {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub changed: u32,
    pub files: Vec<ChangedFile>,
    /// A linked worktree whose commits are all in the default branch, so it can be removed.
    pub merged: bool,
    /// Increases each time the watcher sees a different status, so caches keyed on it never
    /// confuse a recycled allocation for an unchanged snapshot.
    pub generation: u64,
}

/// Runs `git status` for the repository at `path`; `None` when it isn't a repository or git
/// isn't installed. Optional locks are skipped so a status check never rewrites the index,
/// which would otherwise wake the file watcher again.
pub fn read_status(path: &Path) -> Option<GitStatus> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["--no-optional-locks", "status", "--porcelain=v2", "--branch"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut status = parse_porcelain(&String::from_utf8_lossy(&output.stdout));
    status.merged = worktree::is_worktree(path) && worktree::is_merged(path);
    Some(status)
}

fn parse_porcelain(out: &str) -> GitStatus {
    let mut status = GitStatus::default();
    for line in out.lines() {
        if let Some(head) = line.strip_prefix("# branch.head ") {
            status.branch = head.to_string();
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            let mut parts = ab.split_whitespace();
            status.ahead = parts.next().and_then(|a| a.trim_start_matches('+').parse().ok()).unwrap_or(0);
            status.behind = parts.next().and_then(|b| b.trim_start_matches('-').parse().ok()).unwrap_or(0);
        } else if let Some(file) = parse_entry(line) {
            status.changed += 1;
            status.files.push(file);
        }
    }
    status
}

/// Parses a porcelain v2 entry line (`1`, `2`, `u` or `?`); ignored files return `None`.
fn parse_entry(line: &str) -> Option<ChangedFile> {
    let (kind, rest) = line.split_once(' ')?;
    // Field counts before the path, per `git status --porcelain=v2`.
    let (xy, path) = match kind {
        "?" => return Some(ChangedFile { status: 'U', path: rest.to_string() }),
        "1" => (rest.get(..2)?, rest.splitn(8, ' ').nth(7)?),
        "2" => (rest.get(..2)?, rest.splitn(9, ' ').nth(8)?.split('\t').next()?),
        "u" => (rest.get(..2)?, rest.splitn(10, ' ').nth(9)?),
        _ => return None,
    };
    let status = if kind == "u" { '!' } else { xy.chars().find(|c| *c != '.').unwrap_or('M') };
    Some(ChangedFile { status, path: path.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_ahead_behind_and_changes() {
        let out = "# branch.oid abc\n# branch.head main\n# branch.upstream origin/main\n\
                   # branch.ab +2 -1\n1 .M N... 100644 100644 100644 a b src/x.rs\n? new.txt\n";
        let s = parse_porcelain(out);
        assert_eq!((s.branch.as_str(), s.ahead, s.behind, s.changed), ("main", 2, 1, 2));
        let files: Vec<_> = s.files.iter().map(|f| (f.status, f.path.as_str())).collect();
        assert_eq!(files, [('M', "src/x.rs"), ('U', "new.txt")]);
    }

    #[test]
    fn parses_renames_with_spaces() {
        let line = "2 R. N... 100644 100644 100644 a b R100 new name.rs\told.rs";
        let file = parse_entry(line).unwrap();
        assert_eq!((file.status, file.path.as_str()), ('R', "new name.rs"));
    }

    #[test]
    fn detached_head_without_upstream() {
        let s = parse_porcelain("# branch.oid abc\n# branch.head (detached)\n");
        assert_eq!(s.branch, "(detached)");
        assert_eq!((s.ahead, s.behind, s.changed), (0, 0, 0));
    }
}
