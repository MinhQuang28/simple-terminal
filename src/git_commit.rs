//! Committing every change in a repo from the Changes panel.

use std::path::Path;
use std::process::Command;

/// Stages everything (new files too, minus gitignored ones) and commits it with `message`;
/// returns git's one-line summary, or what it printed when the commit (or a hook) failed.
pub fn commit_all(repo: &Path, message: &str) -> Result<String, String> {
    // What was staged before, to put back if the commit doesn't happen; absent mid-merge.
    let staged = git(repo, &["write-tree"]).ok();
    git(repo, &["add", "--all"])?;
    match git(repo, &["commit", "--message", message]) {
        Ok(out) => Ok(out.lines().next().unwrap_or_default().to_string()),
        Err(err) => {
            if let Some(tree) = staged {
                let _ = git(repo, &["read-tree", &tree]);
            }
            Err(err)
        }
    }
}

/// Runs git in `repo`; its stdout, or what it printed on failure.
pub(crate) fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    // A failing hook may print to either stream.
    Err(if stderr.is_empty() { stdout } else { stderr })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commits_new_and_changed_files() {
        let dir = std::env::temp_dir().join(format!("repodeck-commit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            assert!(Command::new("git").arg("-C").arg(&dir).args(args).output().unwrap().status.success())
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        run(&["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        let summary = commit_all(&dir, "first").unwrap();
        assert!(summary.contains("first"), "{summary}");
        assert!(commit_all(&dir, "nothing to commit").is_err());

        // A failed commit leaves the index as it was: here, a staged file and an unstaged one.
        std::fs::write(dir.join("b.txt"), "b").unwrap();
        std::fs::write(dir.join("c.txt"), "c").unwrap();
        run(&["add", "b.txt"]);
        run(&["config", "core.hooksPath", "hooks"]);
        std::fs::create_dir_all(dir.join("hooks")).unwrap();
        let hook = dir.join("hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\necho rejected >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        assert_eq!(commit_all(&dir, "second").unwrap_err(), "rejected");
        let out = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["diff", "--cached", "--name-only"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "b.txt");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
