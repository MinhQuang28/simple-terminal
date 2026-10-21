use std::path::{Path, PathBuf};
use std::process::Command;

use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::finder_service::FolderRequest;
use crate::model::{PaneKind, Project, Repo};

/// Project that collects folders opened from Finder that aren't part of any project yet.
const FINDER_PROJECT: &str = "Finder";

impl RepodeckApp {
    /// Opens a new tab at `path`, inside the repo that contains it or a newly added one.
    pub(crate) fn open_folder(&mut self, request: FolderRequest) {
        let FolderRequest { path, kind } = request;
        if !path.is_dir() {
            log::warn!("Ignoring Services request for non-folder {}", path.display());
            return;
        }
        let repo_id = match self.repo_containing(&path) {
            Some(id) => id,
            None => self.add_repo_to(None, &path),
        };
        let at_root = self.state.repo(repo_id).is_some_and(|r| r.path == path);
        self.new_tab(repo_id, kind, (!at_root).then_some(path));
        self.dirty = true;
    }

    /// Adds folders dropped from Finder as repos and selects the first; a repo without tabs gets a
    /// shell. Files are ignored.
    pub(crate) fn add_dropped_repos(&mut self, project: Option<Uuid>, paths: &[PathBuf]) {
        let mut first = None;
        for path in paths.iter().filter(|p| p.is_dir()) {
            let id = self.add_repo_to(project, path);
            first.get_or_insert(id);
        }
        let Some(repo_id) = first else { return };
        self.state.selected_repo = Some(repo_id);
        if self.state.repo(repo_id).is_some_and(|r| r.tabs.is_empty()) {
            self.new_tab(repo_id, PaneKind::Shell, None);
        }
        self.focus_active_pane();
        self.dirty = true;
    }

    /// The innermost known repo whose directory contains `path`.
    fn repo_containing(&self, path: &Path) -> Option<Uuid> {
        self.state
            .projects
            .iter()
            .flat_map(|p| &p.repos)
            .filter(|r| path.starts_with(&r.path))
            .max_by_key(|r| r.path.components().count())
            .map(|r| r.id)
    }

    /// Adds the git root of `path` (or `path` itself) to `project`, falling back to the selected
    /// repo's project, then the first project, then a "Finder" project. Returns the repo, which
    /// may already have existed there.
    pub(crate) fn add_repo_to(&mut self, project: Option<Uuid>, path: &Path) -> Uuid {
        let root = git_root(path).unwrap_or_else(|| path.to_path_buf());
        let selected_project = self
            .state
            .selected_repo
            .and_then(|id| self.state.projects.iter().find(|p| p.repos.iter().any(|r| r.id == id)))
            .map(|p| p.id);
        let target = project.or(selected_project).or_else(|| self.state.projects.first().map(|p| p.id));
        let index = match target.and_then(|id| self.state.projects.iter().position(|p| p.id == id)) {
            Some(index) => index,
            None => {
                self.state.projects.push(Project::new(FINDER_PROJECT.into()));
                self.state.projects.len() - 1
            }
        };
        let project = &mut self.state.projects[index];
        project.collapsed = false;
        if let Some(existing) = project.repos.iter().find(|r| r.path == root) {
            return existing.id;
        }
        let repo = Repo::from_path(root);
        let id = repo.id;
        project.repos.push(repo);
        self.sync_git_paths();
        id
    }
}

fn git_root(path: &Path) -> Option<PathBuf> {
    let output = Command::new("git").arg("-C").arg(path).args(["rev-parse", "--show-toplevel"]).output();
    match output {
        Ok(out) if out.status.success() => {
            let root = String::from_utf8(out.stdout).ok()?;
            Some(PathBuf::from(root.trim_end()))
        }
        Ok(_) => None,
        Err(err) => {
            log::warn!("Failed to run git in {}: {err}", path.display());
            None
        }
    }
}
