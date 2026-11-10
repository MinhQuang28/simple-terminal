use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::worktree_include::copy_includes;

/// Linked worktrees keep a `.git` file pointing into the main repository's `worktrees` directory
/// (submodules have such a file too, pointing into `modules`).
pub fn is_worktree(path: &Path) -> bool {
    fs::read_to_string(path.join(".git"))
        .is_ok_and(|content| content.starts_with("gitdir:") && content.contains("/worktrees/"))
}

/// Checks `branch` out at `path`; a new branch starts from the default branch (see
/// [`default_branch`]) rather than whatever `source` has checked out, without tracking it.
/// Copies the include files over and returns messages for those that couldn't be copied.
pub fn add(source: &Path, path: &Path, branch: &str) -> Result<Vec<String>, String> {
    let path = path.to_string_lossy();
    let base = default_branch(source);
    let mut args: Vec<&str> = vec!["worktree", "add"];
    if branch_exists(source, branch) {
        args.extend([path.as_ref(), branch]);
    } else {
        args.extend(["--no-track", "-b", branch, path.as_ref()]);
        args.extend(base.as_deref());
    }
    run(source, &args)?;
    Ok(copy_includes(source, Path::new(path.as_ref())))
}

/// The branch new work starts from and merges into: `origin/HEAD` (e.g. `origin/main`), else a
/// local `main` or `master`.
pub fn default_branch(path: &Path) -> Option<String> {
    git(path, &["rev-parse", "--abbrev-ref", "origin/HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "origin/HEAD")
        .or_else(|| ["main", "master"].into_iter().find(|b| branch_exists(path, b)).map(String::from))
}

/// The branch checked out at `path`, or `None` on a detached HEAD.
pub fn current_branch(path: &Path) -> Option<String> {
    git(path, &["symbolic-ref", "--quiet", "--short", "HEAD"]).map(|s| s.trim().to_string())
}

/// Renames `from` to `to`, which must not exist yet.
pub fn rename_branch(path: &Path, from: &str, to: &str) -> Result<(), String> {
    run(path, &["branch", "-m", from, to])
}

/// Whether `name` is a valid new branch name that isn't taken.
pub fn branch_name_free(path: &Path, name: &str) -> bool {
    let valid = Command::new("git")
        .args(["check-ref-format", "--branch", name])
        .output()
        .is_ok_and(|o| o.status.success());
    valid && !branch_exists(path, name)
}

/// The main checkout of the linked worktree at `path`, read from its `.git` file
/// (`gitdir: <main>/.git/worktrees/<name>`) without running git.
pub fn main_path(path: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(path.join(".git")).ok()?;
    let gitdir = content.strip_prefix("gitdir:")?.trim();
    gitdir.rsplit_once("/.git/worktrees/").map(|(main, _)| PathBuf::from(main))
}

/// The main repository a linked worktree belongs to.
pub fn main_repo(path: &Path) -> Option<PathBuf> {
    let out = git(path, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    Path::new(out.trim_end()).parent().map(Path::to_path_buf)
}

/// Whether `HEAD` has commits and all of them are already in the default branch. A branch still
/// sitting on the default tip doesn't count.
pub fn is_merged(path: &Path) -> bool {
    let Some(default) = default_branch(path) else { return false };
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

/// One checkout of a repository, as `git worktree list` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkout {
    pub path: PathBuf,
    /// `None` on a detached HEAD.
    pub branch: Option<String>,
    /// The repository's own folder rather than a linked worktree.
    pub main: bool,
}

/// Every checkout of the repository `path` belongs to, the main one first. Bare repositories
/// and worktrees whose folder is gone are left out.
pub fn list(path: &Path) -> Vec<Checkout> {
    let out = git(path, &["worktree", "list", "--porcelain"]).unwrap_or_default();
    let mut checkouts = Vec::new();
    for (index, block) in out.split("\n\n").enumerate() {
        let field = |name: &str| block.lines().find_map(|l| l.strip_prefix(name));
        let Some(dir) = field("worktree ") else { continue };
        if block.lines().any(|l| l == "bare" || l.starts_with("prunable")) {
            continue;
        }
        let branch = field("branch ").map(|b| b.trim_start_matches("refs/heads/").to_string());
        checkouts.push(Checkout { path: PathBuf::from(dir), branch, main: index == 0 });
    }
    checkouts
}

/// Commits of a detached `HEAD` at `path` that no branch, remote or tag holds, so removing the
/// worktree would leave them unreachable. Zero when a branch is checked out: it keeps them.
pub fn stray_commits(path: &Path) -> u32 {
    if current_branch(path).is_some() {
        return 0;
    }
    git(path, &["rev-list", "--count", "HEAD", "--not", "--branches", "--remotes", "--tags"])
        .and_then(|count| count.trim().parse().ok())
        .unwrap_or(0)
}

/// Deletes the worktree at `path` from `main`, then its branch when asked and fully merged.
/// Without `force` git refuses a worktree with modified or untracked files.
pub fn remove(main: &Path, path: &Path, delete_branch: bool, force: bool) -> Result<(), String> {
    let branch = git(path, &["rev-parse", "--abbrev-ref", "HEAD"]).map(|b| b.trim().to_string());
    let path = path.to_string_lossy();
    let mut args = vec!["worktree", "remove"];
    args.extend(force.then_some("--force"));
    args.push(&path);
    run(main, &args)?;
    match branch.filter(|b| delete_branch && b != "HEAD") {
        Some(branch) => run(main, &["branch", "-d", &branch])
            .map_err(|err| format!("worktree removed, but branch {branch} was kept: {err}")),
        None => Ok(()),
    }
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
    fn lists_checkouts_and_refuses_to_remove_a_dirty_one_unforced() {
        let root = temp_dir().canonicalize().unwrap();
        let (main, linked, loose) = (root.join("main"), root.join("linked"), root.join("loose"));
        fs::create_dir_all(&main).unwrap();
        let ok = |dir: &Path, args: &[&str]| run(dir, args).unwrap();
        ok(&main, &["init", "-q", "-b", "main"]);
        ok(&main, &["config", "user.email", "test@example.com"]);
        ok(&main, &["config", "user.name", "Test"]);
        ok(&main, &["config", "commit.gpgsign", "false"]);
        ok(&main, &["commit", "-q", "--allow-empty", "-m", "first"]);
        ok(&main, &["worktree", "add", "-q", "-b", "feat", linked.to_str().unwrap()]);
        ok(&main, &["worktree", "add", "-q", "--detach", loose.to_str().unwrap()]);

        let found = list(&linked);
        let shape: Vec<_> = found.iter().map(|c| (c.path.as_path(), c.branch.as_deref(), c.main)).collect();
        assert_eq!(
            shape,
            [
                (main.as_path(), Some("main"), true),
                (linked.as_path(), Some("feat"), false),
                (loose.as_path(), None, false)
            ]
        );

        assert_eq!(stray_commits(&loose), 0);
        ok(&loose, &["commit", "-q", "--allow-empty", "-m", "nowhere"]);
        assert_eq!(stray_commits(&loose), 1);
        ok(&linked, &["commit", "-q", "--allow-empty", "-m", "kept by feat"]);
        assert_eq!(stray_commits(&linked), 0);

        fs::write(linked.join("new.txt"), "x").unwrap();
        assert!(remove(&main, &linked, false, false).is_err(), "git keeps a dirty worktree");
        assert!(linked.exists());
        remove(&main, &linked, false, true).unwrap();
        assert!(!linked.exists());
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
        assert_eq!(main_path(&root), Some(PathBuf::from("/main")));
        fs::remove_dir_all(root).unwrap();
    }
}
