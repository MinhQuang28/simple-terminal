//! A repo's local branches, for the top bar's branch menu, and switching between them.

use std::path::Path;

use crate::git_commit::git;

/// Local branch names, the most recently committed to first. (`lstrip`, not `short`: a tag
/// sharing a branch's name would make `short` write `heads/name`.)
pub fn local(repo: &Path) -> Result<Vec<String>, String> {
    let out =
        git(repo, &["for-each-ref", "--sort=-committerdate", "--format=%(refname:lstrip=2)", "refs/heads"])?;
    Ok(out.lines().map(str::to_string).filter(|b| !b.is_empty()).collect())
}

/// Checks out `branch`, or with `create` starts it at the current commit; git refuses, and says
/// why, when local changes would be overwritten or the name is taken or invalid.
pub fn switch(repo: &Path, branch: &str, create: bool) -> Result<(), String> {
    // Branch names can't start with "-", but one read from elsewhere must not pass as an option.
    if branch.starts_with('-') || branch.is_empty() {
        return Err(format!("Not a branch name: {branch}"));
    }
    let args: &[&str] =
        if create { &["switch", "--create", branch] } else { &["switch", "--no-guess", branch] };
    git(repo, args).map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    #[test]
    fn lists_and_switches_branches() {
        let dir = std::env::temp_dir().join(format!("repodeck-branch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let ok = Command::new("git").arg("-C").arg(&dir).args(args).output().unwrap().status.success();
            assert!(ok, "git {args:?}");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "one"]);
        run(&["branch", "feature"]);

        let mut branches = local(&dir).unwrap();
        branches.sort();
        assert_eq!(branches, ["feature", "main"]);
        switch(&dir, "feature", false).unwrap();
        assert_eq!(git(&dir, &["branch", "--show-current"]).unwrap(), "feature");
        assert!(switch(&dir, "missing", false).is_err());
        assert!(switch(&dir, "--detach", false).is_err());
        switch(&dir, "fix/new", true).unwrap();
        assert_eq!(git(&dir, &["branch", "--show-current"]).unwrap(), "fix/new");
        assert!(switch(&dir, "main", true).is_err(), "taken");
        assert!(switch(&dir, "bad name", true).is_err(), "invalid");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
