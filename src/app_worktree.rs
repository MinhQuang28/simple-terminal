use uuid::Uuid;

use crate::app::{Job, RepodeckApp};
use crate::claude_history;
use crate::model::PaneKind;
use crate::ui::dialogs::Dialog;
use crate::ui::worktree_dialogs::{OpenWorktree, RemoveWorktree};
use crate::worktree;
use crate::{shell_env, socket_commands, worktree_branch_name};

impl RepodeckApp {
    /// Checks `branch` out in `<repo>-<branch>` next to the main checkout (creating the branch
    /// from the default branch if needed) in the background, then adds it to the same project and
    /// opens a Claude tab there, plus a shell tab running the repo's setup command when it has
    /// one. The repo's Claude Code history is copied over so its conversations can be resumed
    /// there. Without a branch it makes a temporary one, renamed after Claude's first prompt.
    /// With a `task` (from the CLI) Claude starts on it in the background, leaving the view as is,
    /// and reports to the `parent` pane that asked for it.
    pub(crate) fn create_worktree(
        &mut self,
        repo_id: Uuid,
        branch: Option<&str>,
        task: Option<String>,
        parent: Option<Uuid>,
    ) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let (source, setup) = (repo.path.clone(), repo.setup_command.clone());
        // A worktree made from a worktree still belongs to the main checkout.
        let owner = repo.worktree_of.unwrap_or(repo_id);
        let temp = branch.is_none();
        let branch = match branch {
            Some(branch) => branch.trim().replace(' ', "-"),
            None => worktree_branch_name::temp_name(),
        };
        let project =
            self.state.projects.iter().find(|p| p.repos.iter().any(|r| r.id == repo_id)).map(|p| p.id);
        self.spawn_job("worktree-add", move || {
            // A worktree of a worktree still lives next to, and copies from, the main checkout.
            let main = worktree::main_repo(&source).filter(|m| m.is_dir()).unwrap_or(source);
            let name = main.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let folder = format!("{name}-{}", branch.replace('/', "-"));
            let path = main.parent().map_or_else(|| main.join(&folder), |p| p.join(&folder));
            let result = worktree::add(&main, &path, &branch).map(|mut warnings| {
                if let Err(err) = claude_history::copy_transcripts(&main, &path) {
                    warnings.push(format!("Claude history not copied: {err}"));
                }
                warnings
            });
            Box::new(move |app: &mut RepodeckApp| match result {
                Ok(warnings) => {
                    for warning in warnings {
                        app.report_error("Worktree copy incomplete", warning);
                    }
                    let id = app.add_repo_to(project, &path);
                    if let Some(new) = app.state.repo_mut(id) {
                        new.setup_command = setup.clone();
                        new.temp_branch = temp.then(|| branch.clone());
                        new.worktree_of = Some(owner).filter(|o| *o != id);
                    }
                    // The setup command gets its own shell tab behind the Claude tab, started now
                    // since a tab in the background otherwise starts only once it is shown.
                    if !setup.is_empty()
                        && let Some(pane) = app.add_tab(id, PaneKind::Shell, None)
                    {
                        app.pool.set_startup(pane, setup);
                        if let Err(err) = socket_commands::start(app, pane) {
                            app.report_error("Worktree setup did not start", err);
                        }
                    }
                    match task {
                        Some(task) => {
                            let pane = app.add_tab(id, PaneKind::Agent, None);
                            if let Some(pane) = pane {
                                app.set_pane_parent(pane, parent);
                                app.pool.set_startup(pane, task);
                                if let Err(err) = socket_commands::start(app, pane) {
                                    app.report_error("Claude did not start in the worktree", err);
                                }
                            }
                        }
                        None => {
                            app.new_tab(id, PaneKind::Agent, None);
                        }
                    }
                    app.dirty = true;
                }
                Err(err) => app.report_error(&format!("Worktree {branch} not created"), err),
            }) as Job
        });
    }

    /// Renames the worktree's temporary branch after the task `prompt` describes, unless it has
    /// been switched away from meanwhile; the folder keeps its name, since Claude runs in it.
    pub(crate) fn name_worktree_branch(&mut self, repo_id: Uuid, temp: String, prompt: String) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let path = repo.path.clone();
        let env = shell_env::get(&shell_env::program(&self.state.settings.shell)).cloned();
        self.spawn_job("worktree-name", move || {
            let result = match worktree::current_branch(&path) {
                Some(current) if current == temp => {
                    worktree_branch_name::for_prompt(&path, &prompt, env.as_ref())
                        .ok_or_else(|| "no usable name".to_string())
                        .and_then(|name| worktree::rename_branch(&path, &temp, &name).map(|_| name))
                }
                _ => return Box::new(|_: &mut RepodeckApp| {}) as Job,
            };
            Box::new(move |app: &mut RepodeckApp| match result {
                Ok(name) => log::info!("Worktree branch {temp} renamed to {name}"),
                Err(err) => app.report_error(&format!("Branch {temp} kept its name"), err),
            }) as Job
        });
    }

    /// Asks before removing the worktree, saying what would be lost with it.
    pub(crate) fn open_remove_worktree(&mut self, repo_id: Uuid) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        // Read now rather than from the watcher, whose snapshot may trail the last edit.
        let status = crate::git_status::read_status(&repo.path);
        let branch = worktree::current_branch(&repo.path).unwrap_or_default();
        self.dialog = Some(Dialog::RemoveWorktree(RemoveWorktree {
            repo: repo_id,
            name: repo.name.clone(),
            branch,
            changes: status.map_or(0, |s| s.files.len()),
            stray_commits: worktree::stray_commits(&repo.path),
            delete_branch: false,
        }));
    }

    /// Lists every checkout of the repo, to open one made elsewhere or closed earlier.
    pub(crate) fn open_worktree_list(&mut self, repo_id: Uuid) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let checkouts = worktree::list(&repo.path);
        if checkouts.is_empty() {
            crate::ui::toast::post(&self.ctx, format!("{} is not a git repository", repo.name));
            return;
        }
        let repo_name = checkouts[0].path.file_name().map(|n| n.to_string_lossy().into_owned());
        self.dialog = Some(Dialog::OpenWorktree(OpenWorktree {
            repo: repo_id,
            repo_name: repo_name.unwrap_or_else(|| repo.name.clone()),
            checkouts,
            open: self.state.projects.iter().flat_map(|p| &p.repos).map(|r| r.path.clone()).collect(),
            filter: String::new(),
            selected: 0,
        }));
    }

    /// Selects the checkout at `path`, first adding it to `from`'s project (nested under the
    /// main checkout, with the same setup command) when it isn't in the sidebar yet.
    pub(crate) fn open_worktree(&mut self, from: Uuid, path: &std::path::Path) {
        let known = self.state.projects.iter().flat_map(|p| &p.repos).find(|r| r.path == path).map(|r| r.id);
        let id = known.unwrap_or_else(|| {
            let project =
                self.state.projects.iter().find(|p| p.repos.iter().any(|r| r.id == from)).map(|p| p.id);
            let source = self.state.repo(from);
            let owner = source.map(|r| r.worktree_of.unwrap_or(from));
            let setup = source.map(|r| r.setup_command.clone()).unwrap_or_default();
            let id = self.add_repo_to(project, path);
            if let Some(new) = self.state.repo_mut(id) {
                new.setup_command = setup;
                new.worktree_of = owner.filter(|o| *o != id && worktree::is_worktree(path));
            }
            self.dirty = true;
            id
        });
        self.state.selected_repo = Some(id);
        self.focus_active_pane();
    }

    /// Deletes the worktree (and optionally its merged branch) in the background; on success
    /// drops it from its project and closes its tabs.
    pub(crate) fn remove_worktree(&mut self, repo_id: Uuid, delete_branch: bool, force: bool) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let (path, name) = (repo.path.clone(), repo.name.clone());
        let Some(main) = worktree::main_repo(&path) else {
            self.report_error("Not a linked worktree", path.display().to_string());
            return;
        };
        self.spawn_job("worktree-remove", move || {
            let result = worktree::remove(&main, &path, delete_branch, force);
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
