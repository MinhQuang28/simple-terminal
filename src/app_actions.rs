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
use crate::model::{PaneKind, Project, Repo, Tab};
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
            Action::SelectRepo(id) => {
                self.state.selected_repo = Some(id);
                self.focus_active_pane();
            }
            Action::SelectTab { repo, tab } => {
                self.state.selected_repo = Some(repo);
                if let Some(r) = self.state.repo_mut(repo) {
                    r.active_tab = Some(tab);
                }
                self.focus_active_pane();
            }
            Action::NewTab { repo, kind } => {
                self.new_tab(repo, kind, None);
            }
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
            Action::AddDroppedRepos { project, paths } => self.add_dropped_repos(project, &paths),
            Action::OpenFolderTab(path) => self.open_folder(FolderRequest { path, kind: PaneKind::Shell }),
            Action::ToggleSidePanel => self.state.show_side_panel = !self.state.show_side_panel,
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
            Action::SetSidePanel(panel) => {
                self.state.side_panel = panel;
                self.state.show_side_panel = true;
            }
            Action::Split { dir, kind } => self.split(dir, kind),
            Action::ToggleZoom
            | Action::FocusNeighbor(_)
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
                repo.active_tab = Some(tab.id);
                repo.tabs.push(tab);
                self.state.selected_repo = Some(repo_id);
                self.focus_active_pane();
            }
            Action::OpenDiff { repo, path } => self.open_diff(repo, path),
            Action::SendToAgent { repo, text } => self.send_to_agent(repo, text),
            Action::OpenNewWorktree(repo) => {
                self.dialog = Some(Dialog::NewWorktree { repo, branch: String::new() });
                return;
            }
            Action::OpenRemoveWorktree(repo) => {
                let Some(r) = self.state.repo(repo) else { return };
                let branch = self.git.status(&r.path).map(|s| s.branch.clone()).unwrap_or_default();
                let name = r.name.clone();
                self.dialog = Some(Dialog::RemoveWorktree { repo, name, branch, delete_branch: false });
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
        let mut tab = Tab::new(kind);
        tab.panes[0].cwd = cwd;
        let pane = tab.panes[0].id;
        let repo = self.state.repo_mut(repo_id)?;
        repo.active_tab = Some(tab.id);
        repo.tabs.push(tab);
        self.state.selected_repo = Some(repo_id);
        self.pool.mark_fresh(pane);
        self.pending_focus = Some(pane);
        Some(pane)
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

/// Removes the tab, activating its neighbour; returns it so it can be reopened.
pub(crate) fn close_tab(repo: &mut Repo, tab_id: Uuid) -> Option<Tab> {
    let index = repo.tabs.iter().position(|t| t.id == tab_id)?;
    let tab = repo.tabs.remove(index);
    if repo.active_tab == Some(tab_id) {
        let next = index.min(repo.tabs.len().saturating_sub(1));
        repo.active_tab = repo.tabs.get(next).map(|t| t.id);
    }
    Some(tab)
}
