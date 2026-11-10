//! Pushing and pulling the current branch from the Source Control panel.

use std::path::Path;

use crate::git_commit::git;
use crate::git_status::GitStatus;

/// What would bring the branch and its remote counterpart back in step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sync {
    Push,
    Pull,
    /// First push of a branch with no upstream, to the named remote.
    Publish(String),
}

impl Sync {
    /// The next step for `status`, if any. Pulling comes first: a push is refused while behind.
    pub fn of(status: &GitStatus) -> Option<Self> {
        if status.behind > 0 {
            Some(Self::Pull)
        } else if status.ahead > 0 {
            Some(Self::Push)
        } else {
            status.publish_to.clone().map(Self::Publish)
        }
    }

    /// e.g. "↑  Push 2 commits".
    pub fn label(&self, status: &GitStatus) -> String {
        let commits = |n: u32| if n == 1 { "1 commit".to_string() } else { format!("{n} commits") };
        match self {
            Self::Push => format!("↑  Push {}", commits(status.ahead)),
            Self::Pull => format!("↓  Pull {}", commits(status.behind)),
            Self::Publish(_) => "↑  Publish Branch".into(),
        }
    }

    /// Shown on the button while git runs.
    pub fn running(&self) -> &'static str {
        match self {
            Self::Push => "Pushing…",
            Self::Pull => "Pulling…",
            Self::Publish(_) => "Publishing…",
        }
    }

    /// For "Couldn't push: …".
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Pull => "pull",
            Self::Publish(_) => "publish",
        }
    }

    /// Past tense for the toast, e.g. "Pushed".
    pub fn done(&self) -> &'static str {
        match self {
            Self::Push => "Pushed",
            Self::Pull => "Pulled",
            Self::Publish(_) => "Published",
        }
    }

    /// Runs it in `repo`; the error is what git printed. A pull merges or rebases as the user's
    /// git config says, without opening an editor for the merge message.
    pub fn run(&self, repo: &Path) -> Result<(), String> {
        match self {
            Self::Push => git(repo, &["push"]),
            Self::Pull => git(repo, &["pull", "--no-edit"]),
            Self::Publish(remote) => git(repo, &["push", "--set-upstream", remote, "HEAD"]),
        }
        .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn picks_the_next_step() {
        let status = |ahead, behind, publish_to: Option<&str>| GitStatus {
            ahead,
            behind,
            publish_to: publish_to.map(str::to_owned),
            ..GitStatus::default()
        };
        assert_eq!(Sync::of(&status(0, 0, None)), None);
        assert_eq!(Sync::of(&status(2, 0, None)), Some(Sync::Push));
        assert_eq!(Sync::of(&status(2, 1, None)), Some(Sync::Pull));
        assert_eq!(Sync::of(&status(0, 0, Some("origin"))), Some(Sync::Publish("origin".into())));
        assert_eq!(Sync::Push.label(&status(1, 0, None)), "↑  Push 1 commit");
        assert_eq!(Sync::Pull.label(&status(0, 3, None)), "↓  Pull 3 commits");
    }

    #[test]
    fn publishes_pushes_and_pulls() {
        let root = std::env::temp_dir().join(format!("repodeck-sync-{}", uuid::Uuid::new_v4()));
        let (remote, work, other) = (root.join("remote.git"), root.join("work"), root.join("other"));
        std::fs::create_dir_all(&work).unwrap();
        let run = |dir: &Path, args: &[&str]| {
            let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
            assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        let identify = |dir: &Path| {
            run(dir, &["config", "user.email", "test@example.com"]);
            run(dir, &["config", "user.name", "Test"]);
            run(dir, &["config", "commit.gpgsign", "false"]);
        };
        run(&root, &["init", "-q", "--bare", "remote.git"]);
        run(&work, &["init", "-q", "-b", "main"]);
        identify(&work);
        run(&work, &["remote", "add", "origin", remote.to_str().unwrap()]);
        run(&work, &["commit", "-q", "--allow-empty", "-m", "first"]);

        let status = crate::git_status::read_status(&work).unwrap();
        assert_eq!(Sync::of(&status), Some(Sync::Publish("origin".into())));
        Sync::of(&status).unwrap().run(&work).unwrap();
        assert_eq!(Sync::of(&crate::git_status::read_status(&work).unwrap()), None);

        run(&work, &["commit", "-q", "--allow-empty", "-m", "second"]);
        let status = crate::git_status::read_status(&work).unwrap();
        assert_eq!(Sync::of(&status), Some(Sync::Push));
        Sync::Push.run(&work).unwrap();

        run(&root, &["clone", "-q", "-b", "main", remote.to_str().unwrap(), "other"]);
        identify(&other);
        run(&other, &["commit", "-q", "--allow-empty", "-m", "third"]);
        Sync::Push.run(&other).unwrap();
        run(&work, &["fetch", "-q"]);
        assert_eq!(Sync::of(&crate::git_status::read_status(&work).unwrap()), Some(Sync::Pull));
        Sync::Pull.run(&work).unwrap();
        assert_eq!(Sync::of(&crate::git_status::read_status(&work).unwrap()), None);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
