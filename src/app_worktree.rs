use uuid::Uuid;

use crate::app::{Job, RepodeckApp};
use crate::model::PaneKind;
use crate::worktree;

impl RepodeckApp {
    /// Checks `branch` out in `<repo>-<branch>` next to the repo (creating the branch if needed)
    /// in the background, then adds it to the same project and opens a shell there that first
    /// runs the repo's setup command.
    pub(crate) fn create_worktree(&mut self, repo_id: Uuid, branch: &str) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let (source, setup) = (repo.path.clone(), repo.setup_command.clone());
        let branch = branch.trim().replace(' ', "-");
        let folder = format!("{}-{}", repo.name, branch.replace('/', "-"));
        let path = source.parent().map_or_else(|| source.join(&folder), |p| p.join(&folder));
        let project =
            self.state.projects.iter().find(|p| p.repos.iter().any(|r| r.id == repo_id)).map(|p| p.id);
        self.spawn_job("worktree-add", move || {
            let result = worktree::add(&source, &path, &branch);
            Box::new(move |app: &mut RepodeckApp| match result {
                Ok(warnings) => {
                    for warning in warnings {
                        app.report_error("Worktree include skipped", warning);
                    }
                    let id = app.add_repo_to(project, &path);
                    if let Some(new) = app.state.repo_mut(id) {
                        new.setup_command = setup.clone();
                    }
                    app.state.selected_repo = Some(id);
                    let pane = app.new_tab(id, PaneKind::Shell, None);
                    if let Some(pane) = pane.filter(|_| !setup.is_empty()) {
                        app.pool.set_startup(pane, setup);
                    }
                    app.dirty = true;
                }
                Err(err) => app.report_error(&format!("Worktree {branch} not created"), err),
            }) as Job
        });
    }

    /// Deletes the worktree (and optionally its merged branch) in the background; on success
    /// drops it from its project and closes its tabs.
    pub(crate) fn remove_worktree(&mut self, repo_id: Uuid, delete_branch: bool) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let (path, name) = (repo.path.clone(), repo.name.clone());
        let Some(main) = worktree::main_repo(&path) else {
            self.report_error("Not a linked worktree", path.display().to_string());
            return;
        };
        self.spawn_job("worktree-remove", move || {
            let result = worktree::remove(&main, &path, delete_branch);
            Box::new(move |app: &mut RepodeckApp| {
                if let Err(err) = &result {
                    app.report_error(&format!("Removing worktree {name}"), err.clone());
                }
                // A kept branch still means the folder is gone.
                if result.is_ok() || result.as_ref().is_err_and(|e| e.starts_with("worktree removed")) {
                    app.forget_worktree(repo_id, &main);
                }
            }) as Job
        });
    }

    fn forget_worktree(&mut self, repo_id: Uuid, main: &std::path::Path) {
        for project in &mut self.state.projects {
            project.repos.retain(|r| r.id != repo_id);
        }
        if self.state.selected_repo == Some(repo_id) {
            self.state.selected_repo =
                self.state.projects.iter().flat_map(|p| &p.repos).find(|r| r.path == main).map(|r| r.id);
        }
        self.after_structure_change();
        self.focus_active_pane();
        self.dirty = true;
    }
}
