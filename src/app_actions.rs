use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use eframe::egui;
use uuid::Uuid;

use crate::agent_events::AgentStatus;
use crate::app::RepodeckApp;
use crate::editor::{BarKind, EditorBar};
use crate::finder_service::FolderRequest;
use crate::library;
use crate::model::{PaneKind, Project, Repo, SidePanel, Tab};
use crate::trash;
use crate::ui::Action;
use crate::ui::dialogs::Dialog;
use crate::ui::palette::Palette;

impl RepodeckApp {
    pub(crate) fn apply(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::OpenNewProject => self.dialog = Some(Dialog::NewProject { name: String::new() }),
            Action::OpenRenameProject(id) => {
                if let Some(p) = self.state.projects.iter().find(|p| p.id == id) {
                    self.dialog = Some(Dialog::RenameProject { id, name: p.name.clone() });
                }
            }
            Action::OpenSettings => self.settings_open = !self.settings_open,
            Action::RemoveProject(id) => {
                let Some(project) = self.state.projects.iter().find(|p| p.id == id) else { return };
                let panes = project.repos.iter().flat_map(|r| &r.tabs).flat_map(|t| &t.panes).map(|p| p.id);
                if let Some(body) = self.running_summary(panes).filter(|_| !self.confirmed) {
                    let title = format!("Remove project {}?", project.name);
                    let dialog = Dialog::Confirm { title, body, ok: "Remove", action };
                    self.dialog = Some(dialog);
                    return;
                }
                self.state.projects.retain(|p| p.id != id);
                self.after_structure_change();
            }
            Action::ToggleProject(id) => {
                if let Some(p) = self.state.projects.iter_mut().find(|p| p.id == id) {
                    p.collapsed = !p.collapsed;
                }
            }
            Action::SetSidebarView(view) => self.state.sidebar_view = view,
            Action::SwitchBranch { repo, branch, create } => {
                let Some(path) = self.state.repo(repo).map(|r| r.path.clone()) else { return };
                // Git sees those files clean, but saving them later would write this branch's
                // text over the other one's.
                let unsaved = self.buffers.dirty_under(&path);
                if unsaved > 0 && !create {
                    let files = if unsaved == 1 { "file" } else { "files" };
                    let detail = format!("Save or revert {unsaved} unsaved {files} in this repo first.");
                    crate::ui::toast::post(ctx, format!("Didn't switch to {branch}: {detail}"));
                    self.report_error(&format!("Didn't switch to {branch}"), detail);
                    return;
                }
                self.spawn_job("git-switch", move || {
                    let result = crate::git_branch::switch(&path, &branch, create);
                    Box::new(move |app: &mut RepodeckApp| match result {
                        Ok(()) => crate::ui::toast::post(&app.ctx, format!("Switched to {branch}")),
                        Err(err) => {
                            // The click must answer where it was made; the inbox keeps the details.
                            let why = crate::git_branch::failure_summary(&err);
                            crate::ui::toast::post(&app.ctx, format!("Couldn't switch to {branch}: {why}"));
                            app.report_error(&format!("Couldn't switch to {branch}"), err);
                        }
                    })
                });
                return;
            }
            Action::CommitAll { repo, message } => {
                let Some(path) = self.state.repo(repo).map(|r| r.path.clone()) else { return };
                self.spawn_job("git-commit", move || {
                    let result = crate::git_commit::commit_all(&path, &message);
                    Box::new(move |app: &mut RepodeckApp| match result {
                        Ok(summary) => crate::ui::toast::post(&app.ctx, format!("Committed {summary}")),
                        Err(err) => {
                            crate::ui::commit_box::restore(&app.ctx, repo, message);
                            app.report_error("Commit failed", err);
                        }
                    })
                });
                return;
            }
            Action::GitSync { repo, sync } => {
                let Some(path) = self.state.repo(repo).map(|r| r.path.clone()) else { return };
                self.spawn_job("git-sync", move || {
                    let result = sync.run(&path);
                    Box::new(move |app: &mut RepodeckApp| {
                        crate::ui::commit_box::sync_finished(&app.ctx, repo);
                        app.git.wake();
                        match result {
                            Ok(()) => crate::ui::toast::post(&app.ctx, sync.done().to_string()),
                            Err(err) => {
                                let why = err.lines().last().unwrap_or_default().to_string();
                                let title = format!("Couldn't {}", sync.verb());
                                crate::ui::toast::post(&app.ctx, format!("{title}: {why}"));
                                app.report_error(&title, err);
                            }
                        }
                    })
                });
                return;
            }
            Action::Peek(pane) => {
                self.peek = Some(pane);
                return;
            }
            Action::ClosePeek => {
                self.peek = None;
                self.focus_active_pane();
                return;
            }
            Action::GoToPane(pane) => {
                self.peek = None;
                self.focus_pane(pane);
            }
            Action::TogglePinTab { repo, tab } => {
                if let Some(tab) =
                    self.state.repo_mut(repo).and_then(|r| r.tabs.iter_mut().find(|t| t.id == tab))
                {
                    tab.pinned = !tab.pinned;
                }
            }
            Action::ToggleSettleTab { repo, tab } => {
                if let Some(tab) =
                    self.state.repo_mut(repo).and_then(|r| r.tabs.iter_mut().find(|t| t.id == tab))
                {
                    tab.settled = !tab.settled;
                }
            }
            Action::ToggleRepoTabs(id) => {
                if let Some(repo) = self.state.repo_mut(id) {
                    repo.tabs_collapsed = !repo.tabs_collapsed;
                }
            }
            Action::AddRepos(project) => self.add_repos(project),
            Action::RemoveRepo(id) => {
                let Some(repo) = self.state.repo(id) else { return };
                let panes = repo.tabs.iter().flat_map(|t| &t.panes).map(|p| p.id);
                if let Some(body) = self.running_summary(panes).filter(|_| !self.confirmed) {
                    let title = format!("Remove {} from the project?", repo.name);
                    self.dialog = Some(Dialog::Confirm { title, body, ok: "Remove", action });
                    return;
                }
                for p in &mut self.state.projects {
                    p.repos.retain(|r| r.id != id);
                }
                self.after_structure_change();
            }
            Action::RevealRepo(id) => {
                if let Some(repo) = self.state.repo(id)
                    && let Err(err) = Command::new("open").arg(&repo.path).spawn()
                {
                    self.report_error("Could not reveal in Finder", err.to_string());
                }
                return;
            }
            Action::RevealPath(path) => {
                if let Err(err) = Command::new("open").arg("-R").arg(&path).spawn() {
                    self.report_error("Could not reveal in Finder", err.to_string());
                }
                return;
            }
            Action::TrashPath { repo, ref path } => {
                if !self.confirmed {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let title = format!("Move {name} to the Trash?");
                    let body = "Tabs open on it close; the file can be put back from the Finder.".into();
                    self.dialog = Some(Dialog::Confirm { title, body, ok: "Move to Trash", action });
                    return;
                }
                self.trash_path(repo, path);
            }
            Action::SelectRepo(id) => {
                self.state.selected_repo = Some(id);
                self.focus_active_pane();
            }
            Action::SelectTab { repo, tab } => {
                self.state.selected_repo = Some(repo);
                if let Some(r) = self.state.repo_mut(repo) {
                    r.activate(tab);
                    // Opening a settled tab brings it back.
                    r.tabs.iter_mut().filter(|t| t.id == tab).for_each(|t| t.settled = false);
                }
                self.focus_active_pane();
            }
            Action::NewTab { repo, kind } => {
                // Within the repo being worked in, a terminal opens where the active pane is. Agents
                // stay at the root, where the repo's conversation history is kept.
                let here = self.state.selected_repo == Some(repo) && kind == PaneKind::Shell;
                let cwd = if here { self.active_pane_cwd() } else { None };
                self.new_tab(repo, kind, cwd);
            }
            Action::RunPreset { repo, preset } => self.run_preset(repo, preset),
            Action::CloseTab { repo, tab } => {
                let Some(t) = self.state.repo(repo).and_then(|r| r.tabs.iter().find(|t| t.id == tab)) else {
                    return;
                };
                let file = t.file.clone();
                let working = t.panes.iter().map(|p| p.id);
                if let Some(body) = self.working_summary(working).filter(|_| !self.confirmed) {
                    let title = "Close tab?".to_string();
                    self.dialog = Some(Dialog::Confirm { title, body, ok: "Close", action });
                    return;
                }
                match file {
                    Some(path) if self.buffers.is_dirty(&path) => {
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                        self.dialog = Some(Dialog::UnsavedChanges { repo, tab, file_name });
                        return;
                    }
                    _ => self.close_tab(repo, tab),
                }
            }
            Action::DiscardAndCloseTab { repo, tab } => self.close_tab(repo, tab),
            Action::CloseActiveTab => {
                let Some(repo) = self.state.selected_repo.and_then(|id| self.state.repo(id)) else { return };
                match repo.active_tab {
                    Some(tab) => self.apply(Action::CloseTab { repo: repo.id, tab }, ctx),
                    None => return,
                }
            }
            Action::FocusNextAttention => {
                if !self.focus_next_attention() {
                    return;
                }
            }
            Action::OpenFile { repo, path } => self.open_file(repo, path),
            Action::OpenFileAt { repo, path, line, col } => {
                let command = self.state.settings.external_editor.trim().to_string();
                if command.is_empty() {
                    self.open_file(repo, path);
                    self.pending_goto = Some((line.unwrap_or(1) as usize, col.unwrap_or(1) as usize));
                } else {
                    if let Err(err) = open_externally(&command, &path, line, col) {
                        self.report_error("Could not run the external editor", err);
                    }
                    return;
                }
            }
            Action::SaveActiveFile => {
                if let Some(path) = self.active_file() {
                    self.buffers.save(&path);
                }
                return;
            }
            Action::ResumeSession { repo, session, title } => self.resume_session(repo, session, title),
            Action::MoveRepo { repo, project, index } => {
                if !self.state.projects.iter().any(|p| p.id == project) {
                    return;
                }
                let moved = self.state.projects.iter_mut().find_map(|p| {
                    let index = p.repos.iter().position(|r| r.id == repo)?;
                    Some(p.repos.remove(index))
                });
                let Some(moved) = moved else { return };
                if let Some(target) = self.state.projects.iter_mut().find(|p| p.id == project) {
                    target.collapsed = false;
                    target.repos.insert(index.min(target.repos.len()), moved);
                }
            }
            Action::AddDroppedRepos { project, paths } => self.add_dropped_repos(project, &paths),
            Action::OpenFolderTab(path) => {
                self.open_folder(FolderRequest { path, kind: PaneKind::Shell });
            }
            // Only the Code view has a sidebar to fold; the terminals keep the project sidebar.
            Action::ToggleSidePanel if self.code_view() => {
                self.state.show_code_panel = !self.state.show_code_panel;
            }
            Action::ToggleSidePanel => return,
            Action::GoToDefinition { repo, word, peek } => {
                self.go_to_definition(repo, word, peek);
                return;
            }
            Action::ClosePeekDefinition => {
                self.peek_definition = None;
                self.focus_active_pane();
                return;
            }
            Action::FormatActiveFile => {
                self.format_active_file();
                return;
            }
            Action::ToggleSplitEditor => self.toggle_split_editor(),
            Action::OpenSearch => {
                let Some(repo) = self.selected_repo_mut() else { return };
                repo.show_code(true);
                self.state.code_panel = SidePanel::Search;
                self.state.show_code_panel = true;
                self.search.focus = true;
            }
            Action::ShowCode(code) => {
                let Some(repo) = self.selected_repo_mut() else { return };
                repo.show_code(code);
                self.focus_active_pane();
            }
            Action::OpenQuickOpen => self.palette = Some(Palette::files()),
            Action::OpenFind => {
                if let Some(path) = self.active_file() {
                    self.buffers.get(&path).bar = Some(EditorBar::new(BarKind::Find));
                    return;
                }
                let terminal_tab = self
                    .selected_repo_mut()
                    .and_then(|r| r.active_tab_mut())
                    .is_some_and(|t| !t.is_document());
                if !terminal_tab {
                    return;
                }
                self.focus_active_pane();
                crate::terminal::view::request_find(ctx);
            }
            Action::OpenGoToLine => {
                if let Some(path) = self.active_file() {
                    self.buffers.get(&path).bar = Some(EditorBar::new(BarKind::GoToLine));
                }
                return;
            }
            // The changes and library live in the Code view's sidebar, so asking for them goes there.
            Action::SetSidePanel(panel) => {
                let Some(repo) = self.selected_repo_mut() else { return };
                repo.show_code(true);
                (self.state.code_panel, self.state.show_code_panel) = (panel, true);
            }
            Action::ToggleIgnoredFiles => self.state.show_ignored_files = !self.state.show_ignored_files,
            Action::Split { dir, kind } => {
                self.split(dir, kind);
            }
            Action::ToggleZoom
            | Action::FocusNeighbor(_)
            | Action::SwapNeighbor(_)
            | Action::ResizeSplit { .. }
            | Action::OpenRenameTab { .. }
            | Action::MoveTab { .. }
            | Action::MoveTabToRepo { .. }
            | Action::SelectAdjacentTab(_) => {
                if !self.apply_pane_action(action, ctx) {
                    return;
                }
            }
            Action::ReopenClosedTab => {
                let Some((repo_id, tab)) = self.state.closed_tabs.pop() else { return };
                let Some(repo) = self.state.repo_mut(repo_id) else { return };
                let id = tab.id;
                repo.tabs.push(tab);
                repo.activate(id);
                self.state.selected_repo = Some(repo_id);
                self.focus_active_pane();
            }
            Action::OpenDiff { repo, path } => self.open_diff(repo, path),
            Action::SendToAgent { repo, text } => self.send_to_agent(repo, text),
            Action::NewWorktreeAgent(repo) => self.create_worktree(repo, None, None, None),
            Action::OpenNewWorktree(repo) => {
                self.dialog = Some(Dialog::NewWorktree { repo, branch: String::new() });
                return;
            }
            Action::OpenRemoveWorktree(repo) => {
                self.open_remove_worktree(repo);
                return;
            }
            Action::OpenWorktreeList(repo) => {
                self.open_worktree_list(repo);
                return;
            }
            Action::OpenRepoSetup(repo) => {
                let Some(r) = self.state.repo(repo) else { return };
                self.dialog = Some(Dialog::RepoSetup { repo, command: r.setup_command.clone() });
                return;
            }
            Action::OpenNotice(id) => self.open_notice(id),
            Action::MarkNoticesRead => {
                self.inbox.mark_all_read();
                return;
            }
            Action::ClearNotices => {
                self.inbox.clear();
                return;
            }
            Action::OpenUrl(url) => {
                if let Err(err) = Command::new("open").arg(&url).spawn() {
                    self.report_error("Could not open the link", err.to_string());
                }
                return;
            }
            Action::CreatePullRequest(repo) => {
                self.create_pull_request(repo);
                return;
            }
            Action::AutoFixCi(repo) => {
                self.auto_fix_ci(repo);
                return;
            }
            Action::InsertPrompt(text) => {
                self.insert_prompt(text);
                return;
            }
            Action::OpenEditPrompt(index) => {
                let prompt =
                    index.and_then(|i| self.state.settings.prompts.get(i)).cloned().unwrap_or_default();
                self.dialog = Some(Dialog::EditPrompt { index, name: prompt.name, text: prompt.text });
                return;
            }
            Action::DeletePrompt(index) => {
                if index < self.state.settings.prompts.len() {
                    self.state.settings.prompts.remove(index);
                }
            }
            Action::SetSkillEnabled { file, enabled } => {
                if let Err(err) = library::set_skill_enabled(&file, enabled) {
                    self.report_error("Could not change the skill", format!("{}: {err}", file.display()));
                }
                // Shows the toggle at once instead of after the next rescan.
                self.library.invalidate();
                return;
            }
            Action::AnswerApproval { pane, allow } => {
                if let Some(session) = self.pool.get_mut(pane) {
                    session.answer_approval(allow);
                }
                return;
            }
            Action::FocusPane { tab, pane } => {
                let tab = self
                    .state
                    .projects
                    .iter_mut()
                    .flat_map(|p| &mut p.repos)
                    .flat_map(|r| &mut r.tabs)
                    .find(|t| t.id == tab);
                match tab {
                    Some(tab) if tab.active_pane != Some(pane) => tab.active_pane = Some(pane),
                    _ => return,
                }
            }
            Action::CloseActivePane => {
                let target = self
                    .selected_repo_mut()
                    .and_then(|r| r.active_tab_mut())
                    .and_then(|t| Some((t.id, t.active_pane?)));
                match target {
                    Some((tab, pane)) => self.apply(Action::ClosePane { tab, pane }, ctx),
                    None => return,
                }
            }
            Action::ClosePane { tab, pane } => {
                if let Some(body) = self.working_summary([pane].into_iter()).filter(|_| !self.confirmed) {
                    let title = "Close pane?".to_string();
                    self.dialog = Some(Dialog::Confirm { title, body, ok: "Close", action });
                    return;
                }
                self.close_pane(tab, pane);
            }
            Action::RestartPane(pane) => {
                self.pool.restart(pane);
                self.pending_focus = Some(pane);
                return;
            }
        }
        self.dirty = true;
        ctx.request_repaint();
    }

    pub(crate) fn create_project(&mut self, name: String) {
        let project = Project::new(name);
        let id = project.id;
        self.state.projects.push(project);
        self.add_repos(id);
    }

    fn add_repos(&mut self, project_id: Uuid) {
        let Some(paths) = rfd::FileDialog::new().set_title("Add repositories").pick_folders() else {
            return;
        };
        let Some(project) = self.state.projects.iter_mut().find(|p| p.id == project_id) else { return };
        let mut first_new = None;
        for path in paths {
            if project.repos.iter().any(|r| r.path == path) {
                continue;
            }
            let repo = Repo::from_path(path);
            first_new.get_or_insert(repo.id);
            project.repos.push(repo);
        }
        project.collapsed = false;
        if let Some(repo) = first_new {
            self.new_tab(repo, PaneKind::Shell, None);
        }
        self.sync_git_paths();
        self.dirty = true;
    }

    /// Opens and focuses a new tab in the repo; returns its pane.
    pub(crate) fn new_tab(&mut self, repo_id: Uuid, kind: PaneKind, cwd: Option<PathBuf>) -> Option<Uuid> {
        let pane = self.add_tab(repo_id, kind, cwd)?;
        let repo = self.state.repo_mut(repo_id)?;
        if let Some(tab) = repo.tabs.last().map(|t| t.id) {
            repo.activate(tab);
        }
        self.state.selected_repo = Some(repo_id);
        self.pending_focus = Some(pane);
        Some(pane)
    }

    /// Adds a tab to `repo` without switching to it; returns its pane.
    pub(crate) fn add_tab(&mut self, repo_id: Uuid, kind: PaneKind, cwd: Option<PathBuf>) -> Option<Uuid> {
        let mut tab = Tab::new(kind);
        tab.panes[0].cwd = cwd;
        let pane = tab.panes[0].id;
        self.state.repo_mut(repo_id)?.tabs.push(tab);
        self.pool.mark_fresh(pane);
        self.dirty = true;
        Some(pane)
    }

    /// Moves `path` to the Trash, closing the tabs open on it or inside it, and re-lists its folder.
    fn trash_path(&mut self, repo_id: Uuid, path: &Path) {
        if let Err(err) = trash::move_to_trash(path) {
            self.report_error("Could not move to the Trash", format!("{}: {err}", path.display()));
            return;
        }
        let open: Vec<Uuid> = self
            .state
            .repo(repo_id)
            .map(|r| {
                r.tabs
                    .iter()
                    .filter(|t| t.file.as_deref().is_some_and(|f| f.starts_with(path)))
                    .map(|t| t.id)
                    .collect()
            })
            .unwrap_or_default();
        for tab in open {
            self.close_tab(repo_id, tab);
        }
        if let Some(parent) = path.parent() {
            self.file_tree.forget(parent);
        }
    }

    /// Whether the selected repo shows its Code view.
    pub(crate) fn code_view(&self) -> bool {
        self.state.selected_repo.and_then(|id| self.state.repo(id)).is_some_and(|r| r.code_view)
    }

    pub(crate) fn selected_repo_mut(&mut self) -> Option<&mut Repo> {
        let id = self.state.selected_repo?;
        self.state.repo_mut(id)
    }

    /// Focuses the active terminal pane, or the editor for file tabs.
    pub(crate) fn focus_active_pane(&mut self) {
        self.pending_focus = self
            .selected_repo_mut()
            .and_then(|r| r.active_tab_mut())
            .and_then(|t| if t.is_document() { Some(t.id) } else { t.active_pane });
    }

    /// Kills sessions of removed panes and clears selections that no longer exist.
    pub(crate) fn after_structure_change(&mut self) {
        let live: HashSet<Uuid> = self.state.all_pane_ids().collect();
        self.pool.retain(&live);
        if let Some(id) = self.state.selected_repo
            && self.state.repo(id).is_none()
        {
            self.state.selected_repo = None;
        }
        self.sync_git_paths();
    }
}

impl RepodeckApp {
    /// What would be lost by killing `panes`: "2 terminals and 1 Claude session (1 working)".
    pub(crate) fn running_summary(&self, panes: impl Iterator<Item = Uuid>) -> Option<String> {
        let (mut shells, mut agents, mut working) = (0, 0, 0);
        for pane in panes {
            let Some(session) = self.pool.get(pane) else { continue };
            match session.agent_status() {
                Some(status) => {
                    agents += 1;
                    working += usize::from(status == AgentStatus::Working);
                }
                None => shells += 1,
            }
        }
        if shells + agents == 0 {
            return None;
        }
        let plural =
            |n: usize, one: &str, many: &str| if n == 1 { one.to_string() } else { many.to_string() };
        let mut parts = Vec::new();
        if shells > 0 {
            parts.push(format!("{shells} {}", plural(shells, "terminal", "terminals")));
        }
        if agents > 0 {
            let mut part = format!("{agents} Claude {}", plural(agents, "session", "sessions"));
            if working > 0 {
                part.push_str(&format!(" ({working} working)"));
            }
            parts.push(part);
        }
        Some(format!("This closes {}.", parts.join(" and ")))
    }

    /// A warning when any of `panes` has an agent mid-task.
    fn working_summary(&self, panes: impl Iterator<Item = Uuid>) -> Option<String> {
        let working = panes.filter(|p| self.pool.agent_status(*p) == Some(AgentStatus::Working)).count();
        (working > 0).then(|| "Claude is still working here; closing it loses the conversation.".to_string())
    }
}

/// Runs the user's editor command with `{path}`, `{line}` and `{col}` filled in.
fn open_externally(template: &str, path: &Path, line: Option<u32>, col: Option<u32>) -> Result<(), String> {
    let command = template
        .replace("{path}", &path.display().to_string())
        .replace("{line}", &line.unwrap_or(1).to_string())
        .replace("{col}", &col.unwrap_or(1).to_string());
    Command::new("/bin/sh").args(["-lc", &command]).spawn().map(|_| ()).map_err(|e| format!("{command}: {e}"))
}
