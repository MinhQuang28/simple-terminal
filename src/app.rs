use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use uuid::Uuid;

use crate::agent_events::AgentStatus;
use crate::app_notices::{PaneReport, Report};
use crate::attention;
use crate::claude_history::ClaudeHistory;
use crate::diff::DiffCache;
use crate::editor::Buffers;
use crate::file_drag::FileDrag;
use crate::file_tree::FileTree;
use crate::finder_service::{self, FolderRequest};
use crate::font_catalog::FontCatalog;
use crate::font_install::{self, FontChoice};
use crate::git_watcher::GitWatcher;
use crate::inbox::{Inbox, Source};
use crate::library::Prompt;
use crate::library_cache::LibraryCache;
use crate::model::{AppState, PaneKind, Tab, now_secs};
use crate::pr_status::PrWatcher;
use crate::session_pool::SessionPool;
use crate::shell_env;
use crate::socket_api;
use crate::store::{self, Saver};
use crate::tab_title::task_summary;
use crate::ui::Action;
use crate::ui::dialogs::{self, Dialog, DialogResult};
use crate::ui::palette::Palette;
use crate::ui::settings::{self, Page};

/// How long output must be idle before a tab's "last active" timestamp is persisted again.
const ACTIVITY_SAVE_GRANULARITY: u64 = 60;
/// Minimum gap between state writes, so dragging a settings slider doesn't hammer the disk.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// Work a background thread finished, applied to the app on the UI thread.
pub(crate) type Job = Box<dyn FnOnce(&mut RepodeckApp) + Send>;

/// Text waiting to be pasted into a pane that may still be starting.
pub struct PendingPaste {
    pub pane: Uuid,
    pub text: String,
    pub not_before: Instant,
}

pub struct RepodeckApp {
    pub(crate) state: AppState,
    pub(crate) pool: SessionPool,
    pub(crate) git: GitWatcher,
    pub(crate) prs: PrWatcher,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) palette: Option<Palette>,
    /// Most recently closed last, for “Reopen closed tab”.
    pub(crate) closed_tabs: Vec<(Uuid, Tab)>,
    pub(crate) pending_focus: Option<Uuid>,
    pub(crate) pending_goto: Option<(usize, usize)>,
    pub(crate) dirty: bool,
    pub(crate) settings_open: bool,
    pub(crate) buffers: Buffers,
    pub(crate) diffs: DiffCache,
    pub(crate) pending_paste: Option<PendingPaste>,
    pub(crate) file_tree: FileTree,
    pub(crate) claude_history: ClaudeHistory,
    pub(crate) library: LibraryCache,
    pub(crate) drag: FileDrag,
    pub(crate) ctx: egui::Context,
    pub(crate) inbox: Inbox,
    /// Set while an action confirmed through a dialog is applied, so it skips its own check.
    pub(crate) confirmed: bool,
    quit_confirmed: bool,
    settings_page: Page,
    fonts: FontCatalog,
    /// The font family currently installed in egui; `None` until the first install.
    applied_font: Option<(String, String, u32)>,
    last_save: Instant,
    saver: Saver,
    badge: usize,
    folder_requests: Receiver<FolderRequest>,
    jobs: Receiver<Job>,
    job_tx: Sender<Job>,
}

impl RepodeckApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        let app_ctx = ctx.clone();
        let state = store::load();
        shell_env::prefetch(shell_env::program(&state.settings.shell));
        state.settings.theme.apply_to(&ctx);
        // Install the bundled font immediately so the bold family exists before the first paint.
        // Bundled defaults until the font scan finds the chosen families.
        font_install::apply(&ctx, FontChoice { terminal: None, ui: None, ui_scale: 1.0 });
        let fonts = FontCatalog::scan(ctx.clone());
        let folder_requests = finder_service::register(&ctx);
        let claude_history = ClaudeHistory::new(ctx.clone());
        let library = LibraryCache::new(ctx.clone());
        let file_tree = FileTree::new(ctx.clone());
        let git = GitWatcher::spawn(ctx.clone());
        let prs = PrWatcher::spawn(ctx.clone());
        let (job_tx, jobs) = mpsc::channel();
        socket_api::listen(job_tx.clone(), ctx.clone());
        #[cfg(target_os = "macos")]
        crate::app_menu::install("Repodeck", job_tx.clone(), ctx.clone());
        let pending_focus = state
            .selected_repo
            .and_then(|id| state.repo(id))
            .and_then(|r| r.tabs.iter().find(|t| Some(t.id) == r.active_tab))
            .and_then(|t| t.active_pane);
        let app = Self {
            state,
            pool: SessionPool::default(),
            git,
            prs,
            dialog: None,
            palette: None,
            closed_tabs: Vec::new(),
            pending_focus,
            pending_goto: None,
            dirty: false,
            settings_open: false,
            buffers: Buffers::default(),
            diffs: DiffCache::new(ctx.clone()),
            pending_paste: None,
            file_tree,
            claude_history,
            library,
            drag: FileDrag::default(),
            ctx: app_ctx,
            inbox: Inbox::default(),
            confirmed: false,
            quit_confirmed: false,
            settings_page: Page::default(),
            fonts,
            applied_font: None,
            last_save: Instant::now(),
            saver: Saver::spawn(),
            badge: 0,
            folder_requests,
            jobs,
            job_tx,
        };
        app.sync_git_paths();
        app
    }

    /// Runs `work` on a named thread; the closure it returns is applied to the app afterwards.
    pub(crate) fn spawn_job(&self, name: &str, work: impl FnOnce() -> Job + Send + 'static) {
        let (tx, ctx) = (self.job_tx.clone(), self.ctx.clone());
        let spawned = thread::Builder::new().name(name.into()).spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
        if let Err(err) = spawned {
            log::error!("Failed to start {name}: {err}");
        }
    }

    /// Logs an error and files it in the inbox, since most of these happen off any pane.
    pub(crate) fn report_error(&mut self, title: &str, detail: String) {
        log::error!("{title}: {detail}");
        let nowhere = Source { repo: Uuid::nil(), tab: Uuid::nil(), pane: Uuid::nil() };
        self.inbox.push(nowhere, AgentStatus::Error, title, String::new(), detail);
    }

    pub(crate) fn sync_git_paths(&self) {
        let paths: Vec<_> =
            self.state.projects.iter().flat_map(|p| &p.repos).map(|r| r.path.clone()).collect();
        self.prs.set_paths(paths.clone());
        self.git.set_paths(paths);
    }

    fn show_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.as_mut() else { return };
        let (close, result) = dialogs::show(ctx, dialog);
        if close {
            self.dialog = None;
        }
        match result {
            Some(DialogResult::CreateProject(name)) => self.create_project(name),
            Some(DialogResult::RenameProject(id, name)) => {
                if let Some(p) = self.state.projects.iter_mut().find(|p| p.id == id) {
                    p.name = name;
                }
            }
            Some(DialogResult::SaveAndClose { repo, tab }) => {
                if let Some(path) =
                    self.state.repo(repo).and_then(|r| r.tabs.iter().find(|t| t.id == tab)?.file.clone())
                {
                    self.buffers.save(&path);
                }
                if !self.active_file().is_some_and(|p| self.buffers.is_dirty(&p)) {
                    self.apply(Action::DiscardAndCloseTab { repo, tab }, ctx);
                }
            }
            Some(DialogResult::DiscardAndClose { repo, tab }) => {
                self.apply(Action::DiscardAndCloseTab { repo, tab }, ctx);
            }
            Some(DialogResult::RenameTab { repo, tab, name }) => {
                if let Some(t) =
                    self.state.repo_mut(repo).and_then(|r| r.tabs.iter_mut().find(|t| t.id == tab))
                {
                    t.name = Some(name);
                }
            }
            Some(DialogResult::CreateWorktree { repo, branch }) => self.create_worktree(repo, &branch),
            Some(DialogResult::RemoveWorktree { repo, delete_branch }) => {
                self.remove_worktree(repo, delete_branch)
            }
            Some(DialogResult::Run(action)) => {
                self.confirmed = true;
                self.apply(action, ctx);
                self.confirmed = false;
            }
            Some(DialogResult::SaveAllAndQuit) => {
                self.buffers.save_all();
                if self.buffers.dirty_count() == 0 {
                    self.quit_confirmed = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Some(DialogResult::QuitWithoutSaving) => {
                self.quit_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(DialogResult::SavePrompt { index, name, text }) => {
                let prompts = &mut self.state.settings.prompts;
                match index.filter(|i| *i < prompts.len()) {
                    Some(i) => prompts[i] = Prompt { name, text },
                    None => prompts.push(Prompt { name, text }),
                }
            }
            Some(DialogResult::SetRepoSetup { repo, command }) => {
                if let Some(r) = self.state.repo_mut(repo) {
                    r.setup_command = command;
                }
            }
            None => return,
        }
        self.dirty = true;
    }

    fn show_settings(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let changes = settings::show(
            ui,
            &mut self.settings_open,
            &mut self.settings_page,
            &mut self.state.settings,
            &self.fonts,
        );
        if changes.theme {
            self.state.settings.theme.apply_to(&ctx);
        }
        if changes.terminal {
            self.pool.apply_settings(&self.state.settings);
        }
        if !self.settings_open {
            self.focus_active_pane();
        }
        self.dirty |= changes.any;
    }

    /// Installs the configured fonts once the system font scan has found them.
    fn sync_font(&mut self, ctx: &egui::Context) {
        let settings = &self.state.settings;
        let key =
            (settings.font_family.clone(), settings.ui_font_family.clone(), settings.ui_font_size.to_bits());
        if self.applied_font.as_ref() == Some(&key) {
            return;
        }
        let wanted = [&settings.font_family, &settings.ui_font_family];
        let fonts = match self.fonts.fonts() {
            Some(fonts) => fonts,
            None if wanted.iter().all(|f| f.is_empty()) => Vec::new(),
            None => return,
        };
        let find = |family: &String| {
            let entry = fonts.iter().find(|f| &f.family == family);
            if !family.is_empty() && entry.is_none() {
                log::warn!("Font {family:?} not installed; using the default");
            }
            entry
        };
        let choice = FontChoice {
            terminal: find(&settings.font_family),
            ui: find(&settings.ui_font_family),
            ui_scale: font_install::ui_scale(settings.ui_font_size),
        };
        font_install::apply(ctx, choice);
        self.applied_font = Some(key);
    }

    /// Persists recent PTY output as tab activity so the sidebar can show "5m ago", and agent task
    /// summaries so restored tabs keep their names before the agent sets a title again.
    fn track_activity(&mut self, visible: &HashSet<Uuid>, window_focused: bool, ctx: &egui::Context) {
        let now = now_secs();
        let mut reports = Vec::new();
        for repo in self.state.projects.iter_mut().flat_map(|p| &mut p.repos) {
            for tab in &mut repo.tabs {
                for pane in &mut tab.panes {
                    let Some(session) = self.pool.get_mut(pane.id) else { continue };
                    // Remember the conversation so a restored tab resumes exactly this one.
                    if let Some(id) =
                        session.agent_session_id().filter(|id| pane.session.as_deref() != Some(id))
                    {
                        pane.session = Some(id.to_string());
                        self.dirty = true;
                    }
                    if session.is_busy() {
                        // Repaint once the busy window lapses so spinners disappear.
                        ctx.request_repaint_after(Duration::from_millis(1600));
                    }
                    if window_focused && visible.contains(&pane.id) {
                        session.note_viewed();
                        if session.agent_status() == Some(AgentStatus::Done) {
                            ctx.request_repaint_after(Duration::from_secs(2));
                        }
                    }
                    // Remember where the shell went so a restored pane reopens there.
                    let cwd = session.cwd().filter(|c| pane.cwd.as_deref() != Some(*c) && *c != repo.path);
                    if let Some(cwd) = cwd.filter(|c| c.is_dir()).map(Path::to_path_buf) {
                        pane.cwd = Some(cwd);
                        self.dirty = true;
                    }
                    let source = Source { repo: repo.id, tab: tab.id, pane: pane.id };
                    let mut report = |report| {
                        let (repo_name, tab_title) = (repo.name.clone(), tab.title.clone());
                        reports.push(PaneReport { source, repo_name, tab_title, report });
                    };
                    if let Some(finished) = session.take_long_finished() {
                        report(Report::Command(finished));
                    }
                    if let Some(status) = session.take_status_change().filter(|s| s.needs_attention()) {
                        report(Report::Status(status));
                    }
                    for text in session.take_notifications() {
                        report(Report::Message(text));
                    }
                    let summary = (pane.kind == PaneKind::Agent)
                        .then(|| session.title.as_deref().and_then(task_summary))
                        .flatten();
                    if let Some(summary) = summary.filter(|s| *s != tab.title) {
                        tab.title = summary;
                        self.dirty = true;
                    }
                    let output_at = now.saturating_sub(session.last_output.elapsed().as_secs());
                    if output_at >= tab.last_active + ACTIVITY_SAVE_GRANULARITY {
                        tab.last_active = output_at;
                        self.dirty = true;
                    }
                }
            }
        }
        self.deliver(reports, window_focused, visible);
        let badge = self.pool.attention_count();
        if badge != self.badge {
            self.badge = badge;
            attention::set_badge(badge);
        }
    }

    /// Keeps the window open while files have unsaved changes, asking what to do with them.
    fn guard_quit(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.quit_confirmed {
            return;
        }
        let count = self.buffers.dirty_count();
        if count > 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Some(Dialog::QuitUnsaved { count });
        }
    }

    fn visible_panes(&self) -> HashSet<Uuid> {
        if self.settings_open {
            return HashSet::new();
        }
        self.state
            .selected_repo
            .and_then(|id| self.state.repo(id))
            .and_then(|r| r.tabs.iter().find(|t| Some(t.id) == r.active_tab))
            .map(|t| t.panes.iter().map(|p| p.id).collect())
            .unwrap_or_default()
    }
}

impl RepodeckApp {
    fn save(&mut self) {
        self.saver.save(&self.state);
        self.dirty = false;
        self.last_save = Instant::now();
    }

    fn save_when_due(&mut self, ctx: &egui::Context) {
        if !self.dirty {
            return;
        }
        if self.last_save.elapsed() >= SAVE_DEBOUNCE {
            self.save();
        } else {
            ctx.request_repaint_after(SAVE_DEBOUNCE);
        }
    }
}

impl eframe::App for RepodeckApp {
    fn on_exit(&mut self) {
        if self.dirty {
            self.save();
        }
        self.saver.flush();
    }

    /// Runs before `ui` on every frame, and alone while the window is hidden or fully covered, so
    /// agents and commands finishing in the meantime still notify right away. All per-pane
    /// bookkeeping lives here so it runs exactly once per frame.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(job) = self.jobs.try_recv() {
            job(self);
        }
        let visible = self.visible_panes();
        let window_focused = ctx.input(|i| i.focused && !i.viewport().minimized.unwrap_or(false));
        self.pool.pump_background(&visible, ctx);
        self.track_activity(&visible, window_focused, ctx);
        self.save_when_due(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.sync_font(&ctx);
        while let Ok(job) = self.jobs.try_recv() {
            job(self);
        }
        while let Ok(request) = self.folder_requests.try_recv() {
            self.open_folder(request);
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        self.drag = FileDrag::read(&ctx);
        self.guard_quit(&ctx);
        let mut actions = Vec::new();
        self.handle_shortcuts(&ctx, &mut actions);

        if self.settings_open {
            self.show_settings(ui);
        } else {
            self.layout(ui, &mut actions);
        }

        self.show_dialog(&ctx);
        if let Some(palette) = &mut self.palette
            && palette.show(&ctx, &self.state, &self.pool, &mut actions)
        {
            self.palette = None;
            self.focus_active_pane();
        }
        for action in actions {
            self.apply(action, &ctx);
        }
        self.flush_pending_paste();
        if self.pending_paste.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        self.save_when_due(&ctx);
        // Keep relative timestamps fresh without busy-looping; they only show whole minutes.
        ctx.request_repaint_after(Duration::from_secs(60));
    }
}
