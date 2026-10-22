use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{self, Term};
use alacritty_terminal::tty::{self, Shell};
use alacritty_terminal::vte::ansi::{CursorShape as VteCursorShape, CursorStyle};
use anyhow::Result;
use eframe::egui;

use uuid::Uuid;

use alacritty_terminal::grid::Dimensions;

use super::colors;
use super::image_paste::Queued;
use super::input::paste_bytes;
use super::osc_notify::NotifyingPty;
pub use super::repaint::{EventProxy, RepaintGate};
use super::snapshot::Frame;
use crate::agent_events::{self, AgentEvents, AgentStatus};
use crate::claude_status_line::{self, StatusLine};
use crate::model::{AgentRenderer, CursorShape};
use crate::shell_env;
use crate::shell_events::ShellEvents;
use crate::shell_integration;
use crate::socket_api;
use crate::tab_title::{self, task_summary};
use crate::theme::Theme;
use crate::{agent_launch, store};

/// Output within this window counts as busy; typed echo within it doesn't.
const BUSY_WINDOW: Duration = Duration::from_millis(1500);
/// How long a finished agent's "Done" stays on a pane the user is looking at.
const DONE_LINGER: Duration = Duration::from_secs(2);

impl EventProxy {
    /// A terminal with no PTY behind it, for tests that drive it with escape sequences.
    #[cfg(test)]
    pub fn test_term(cols: usize, rows: usize) -> Term<EventProxy> {
        let (tx, _rx) = mpsc::channel();
        let proxy = EventProxy {
            tx,
            ctx: egui::Context::default(),
            repaint: Arc::default(),
            visible: Arc::default(),
        };
        Term::new(term_config(CursorShape::Block), &TermSize::new(cols, rows), proxy)
    }
}

/// What to run inside a new PTY.
pub enum Launch<'a> {
    LoginShell,
    /// Runs `command` through the user's shell, then drops into an interactive shell.
    Command(&'a str),
    /// Like `Command`, but started from the login environment captured at launch, when it's
    /// ready, instead of sourcing the user's startup files again first.
    Agent(&'a str),
}

pub struct SessionConfig<'a> {
    pub pane: Uuid,
    pub cwd: &'a Path,
    pub launch: Launch<'a>,
    /// Empty means the user's `$SHELL`.
    pub shell: &'a str,
    pub cursor: CursorShape,
    pub scrollback_lines: u32,
    pub theme: &'a Theme,
    pub agent_renderer: AgentRenderer,
    /// Drop the API key variables so Claude signs in with the user's subscription.
    pub agent_hide_api_keys: bool,
}

/// A running PTY child plus the terminal state parsed from its output.
pub struct TerminalSession {
    pub term: Arc<FairMutex<Term<EventProxy>>>,
    sender: EventLoopSender,
    events: Receiver<Event>,
    notifications: Receiver<String>,
    visible: Arc<AtomicBool>,
    size: (u16, u16),
    title: Option<String>,
    /// The agent's own title summary when the current turn's prompt was submitted; while the
    /// live title still says this, it is stale and the prompt names the tab instead.
    stale_summary: Option<String>,
    /// Title made from the prompt of the last finished turn whose live title went stale.
    prompt_title: Option<String>,
    pub exited: bool,
    pub last_output: Instant,
    last_user_input: Instant,
    pub(super) spawned_at: Instant,
    /// When the program last rang the bell, for a brief visual flash.
    pub bell_at: Option<Instant>,
    theme: Theme,
    /// Last captured grid contents; kept so its buffer is reused.
    pub frame: Frame,
    agent: Option<AgentEvents>,
    /// Which agent the pane was started with, for its icon and label.
    agent_kind: Option<agent_launch::Agent>,
    status_change: Option<(AgentStatus, String)>,
    /// When the user started looking at this pane while its agent reported "Done".
    done_seen_at: Option<Instant>,
    pub(super) shell: Option<ShellEvents>,
    /// Input waiting on a pasted image that is still being saved.
    pub(super) queued: VecDeque<Queued>,
}

impl TerminalSession {
    pub fn spawn(
        config: SessionConfig<'_>,
        agent: Option<AgentEvents>,
        repaint: Arc<RepaintGate>,
        ctx: &egui::Context,
    ) -> Result<Self> {
        let SessionConfig {
            pane,
            cwd,
            launch,
            shell,
            cursor,
            scrollback_lines,
            theme,
            agent_renderer,
            agent_hide_api_keys,
        } = config;
        let (tx, events) = mpsc::channel();
        let visible = Arc::new(AtomicBool::new(true));
        let proxy = EventProxy { tx, ctx: ctx.clone(), repaint, visible: visible.clone() };

        let (cols, rows) = (80, 24);
        let config = term::Config { scrolling_history: scrollback_lines as usize, ..term_config(cursor) };
        let term = Term::new(config, &TermSize::new(cols, rows), proxy.clone());
        let term = Arc::new(FairMutex::new(term));

        let shell_program = shell_env::program(shell);
        let captured = match launch {
            Launch::Agent(_) => shell_env::get(&shell_program),
            _ => None,
        };
        let shell = match launch {
            Launch::LoginShell if shell.is_empty() => None,
            Launch::LoginShell => Some(Shell::new(shell_program.clone(), vec!["-l".into()])),
            Launch::Command(command) | Launch::Agent(command) => {
                let script = format!("{command}; exec {shell_program} -l");
                let flags: &[&str] = if captured.is_some() { &["-c"] } else { &["-l", "-i", "-c"] };
                let args = flags.iter().map(|f| f.to_string()).chain([script]).collect();
                Some(Shell::new(shell_program.clone(), args))
            }
        };
        let mut env: HashMap<String, String> = captured.cloned().unwrap_or_default();
        env.extend([
            ("TERM".into(), "xterm-256color".into()),
            ("COLORTERM".into(), "truecolor".into()),
            ("TERM_PROGRAM".into(), "Repodeck".into()),
        ]);
        // Lets scripts in the pane run `repodeck notify …` without installing anything.
        if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)) {
            let path = env.get("PATH").cloned().unwrap_or_else(|| std::env::var("PATH").unwrap_or_default());
            env.insert("PATH".into(), format!("{}:{path}", dir.display()));
        }
        env.insert(socket_api::PANE_VAR.into(), pane.to_string());
        if let Some(socket) = socket_api::socket_path() {
            env.insert(socket_api::SOCKET_VAR.into(), socket.to_string_lossy().into_owned());
        }
        if let Some(agent) = &agent {
            env.insert(agent_events::EVENTS_VAR.into(), agent.path().to_string_lossy().into_owned());
            env.insert(
                agent_events::HOOKS_VAR.into(),
                agent_events::hooks_settings(cwd, agent_hide_api_keys),
            );
            if agent_hide_api_keys {
                for var in agent_events::API_KEY_VARS {
                    env.remove(*var);
                }
            }
            env.insert(
                claude_status_line::STATUS_VAR.into(),
                agent.status_line_path().to_string_lossy().into_owned(),
            );
            if let Some((key, value)) = agent_renderer.env() {
                env.insert(key.into(), value.into());
            }
            if let Launch::Agent(command) = launch
                && agent_launch::uses(command, agent_launch::Agent::OpenCode)
                && let Some(dir) = store::data_dir()
                && let Some(config) = agent_launch::opencode_config(
                    &dir,
                    env.get(agent_launch::OPENCODE_CONFIG_VAR).map(String::as_str),
                )
            {
                env.insert(agent_launch::OPENCODE_CONFIG_VAR.into(), config);
            }
        }
        let shell_events = ShellEvents::create(pane);
        if let Some(events) = &shell_events {
            env.extend(shell_integration::env(&shell_program, events.path()));
        }
        let options =
            tty::Options { shell, working_directory: Some(cwd.to_path_buf()), drain_on_exit: true, env };
        let window_size = window_size(cols as u16, rows as u16);
        let (notify_tx, notifications) = mpsc::channel();
        let pty = NotifyingPty::new(tty::new(&options, window_size, 0)?, notify_tx)?;
        let event_loop = EventLoop::new(term.clone(), proxy, pty, true, false)?;
        let sender = event_loop.channel();
        event_loop.spawn();

        Ok(Self {
            term,
            sender,
            events,
            notifications,
            visible,
            size: (cols as u16, rows as u16),
            title: None,
            exited: false,
            last_output: Instant::now(),
            last_user_input: Instant::now(),
            spawned_at: Instant::now(),
            bell_at: None,
            theme: theme.clone(),
            frame: Frame::default(),
            agent,
            agent_kind: match launch {
                Launch::Agent(command) => agent_launch::agent_of(command),
                _ => None,
            },
            status_change: None,
            stale_summary: None,
            prompt_title: None,
            done_seen_at: None,
            shell: shell_events,
            queued: VecDeque::new(),
        })
    }

    /// Applies the default cursor shape and scrollback size from settings.
    pub fn apply_settings(&self, cursor: CursorShape, scrollback_lines: u32) {
        let config = term::Config { scrolling_history: scrollback_lines as usize, ..term_config(cursor) };
        self.term.lock().set_options(config);
    }

    /// Whether the pane is on screen, which decides how eagerly output repaints the window.
    pub fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::Relaxed);
    }

    pub fn note_user_input(&mut self) {
        self.last_user_input = Instant::now();
        if let Some(agent) = &mut self.agent {
            agent.note_user_input();
        }
    }

    /// Colors reported back to programs that query the palette (OSC 10/11/4).
    pub fn sync_theme(&mut self, theme: &Theme) {
        if self.theme != *theme {
            self.theme = theme.clone();
        }
    }

    /// The pane is on screen in a focused window; a "Done" status clears once it has been seen
    /// for a moment.
    pub fn note_viewed(&mut self) {
        if self.agent_status() != Some(AgentStatus::Done) {
            self.done_seen_at = None;
            return;
        }
        let seen = *self.done_seen_at.get_or_insert_with(Instant::now);
        if seen.elapsed() >= DONE_LINGER
            && let Some(agent) = &mut self.agent
        {
            agent.acknowledge_done();
            self.done_seen_at = None;
        }
    }

    pub fn agent_status(&self) -> Option<AgentStatus> {
        self.agent.as_ref().map(AgentEvents::status)
    }

    pub fn agent_status_line(&self) -> Option<StatusLine> {
        self.agent.as_ref()?.status_line()
    }

    /// What the agent is waiting for permission to do, if anything.
    pub fn approval_request(&self) -> Option<&str> {
        self.agent.as_ref()?.request()
    }

    /// Answers the agent's permission prompt as the user would at the keyboard: Enter accepts
    /// the highlighted "Yes", Escape declines.
    pub fn answer_approval(&mut self, allow: bool) {
        self.write(if allow { b"\r".to_vec() } else { b"\x1b".to_vec() });
        self.note_user_input();
    }

    pub fn agent_session_id(&self) -> Option<&str> {
        self.agent.as_ref()?.session_id()
    }

    /// The agent the pane runs, when it was started as an agent pane.
    pub fn agent_kind(&self) -> Option<agent_launch::Agent> {
        self.agent_kind
    }

    /// The terminal title as the program set it.
    pub fn raw_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// The title to show: the program's, unless an agent left it stale across a turn, in which
    /// case the prompt that turn answered.
    pub fn title(&self) -> Option<&str> {
        let live = self.title.as_deref();
        match &self.prompt_title {
            Some(prompt) if live.and_then(task_summary) == self.stale_summary => Some(prompt),
            _ => live,
        }
    }

    /// The agent status if it changed since the last call, with the text that came with it.
    pub fn take_status_change(&mut self) -> Option<(AgentStatus, String)> {
        self.status_change.take()
    }

    /// Messages programs in the pane sent with OSC 9 or OSC 777 since the last call.
    pub fn take_notifications(&mut self) -> Vec<String> {
        self.notifications.try_iter().collect()
    }

    /// Producing output the user didn't just type (e.g. an agent working or a build running).
    pub fn is_busy(&self) -> bool {
        self.last_output.elapsed() < BUSY_WINDOW && self.last_user_input.elapsed() > BUSY_WINDOW
    }

    /// Sends `text` as a paste, bracketed when the program asked for that.
    pub fn paste(&self, text: &str) {
        let mode = *self.term.lock().mode();
        self.write(paste_bytes(text, mode));
    }

    pub fn write(&self, bytes: impl Into<Vec<u8>>) {
        let _ = self.sender.send(Msg::Input(bytes.into().into()));
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == 0 || rows == 0 || (cols, rows) == self.size {
            return;
        }
        self.size = (cols, rows);
        self.term.lock().resize(TermSize::new(cols as usize, rows as usize));
        let _ = self.sender.send(Msg::Resize(window_size(cols, rows)));
    }

    /// Drains PTY events; returns text the terminal asked to put on the clipboard.
    pub fn process_events(&mut self) -> Option<String> {
        let mut clipboard = None;
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Wakeup => self.last_output = Instant::now(),
                Event::Title(title) => self.title = Some(title),
                Event::ResetTitle => self.title = None,
                Event::PtyWrite(text) => self.write(text),
                Event::ClipboardStore(_, text) => clipboard = Some(text),
                Event::ColorRequest(index, format) => {
                    let rgb =
                        self.term.lock().colors()[index].or_else(|| colors::for_index(index, &self.theme));
                    if let Some(rgb) = rgb {
                        self.write(format(rgb));
                    }
                }
                Event::Bell => self.bell_at = Some(Instant::now()),
                Event::ClipboardLoad(_, _)
                | Event::TextAreaSizeRequest(_)
                | Event::MouseCursorDirty
                | Event::CursorBlinkingChange => {}
                Event::Exit | Event::ChildExit(_) => self.exited = true,
            }
        }
        self.flush_queued();
        if let Some(agent) = &mut self.agent
            && let Some(status) = agent.poll()
        {
            let live_summary = self.title.as_deref().and_then(task_summary);
            match status {
                AgentStatus::Working => self.stale_summary = live_summary,
                // The agent didn't retitle the terminal during the turn, so the prompt names it.
                AgentStatus::Done if live_summary == self.stale_summary => {
                    self.prompt_title = agent.prompt().map(tab_title::from_prompt);
                }
                AgentStatus::Done => self.prompt_title = None,
                _ => {}
            }
            self.status_change = Some((status, agent.message().unwrap_or_default().to_owned()));
        }
        if let Some(shell) = &mut self.shell {
            let term = &self.term;
            shell.poll(|| {
                let term = term.lock();
                term.grid().history_size() + term.grid().cursor.point.line.0.max(0) as usize
            });
        }
        clipboard
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Shutdown);
    }
}

fn term_config(cursor: CursorShape) -> term::Config {
    let shape = match cursor {
        CursorShape::Block => VteCursorShape::Block,
        CursorShape::Beam => VteCursorShape::Beam,
        CursorShape::Underline => VteCursorShape::Underline,
    };
    term::Config {
        scrolling_history: 10_000,
        default_cursor_style: CursorStyle { shape, blinking: false },
        ..Default::default()
    }
}

fn window_size(cols: u16, rows: u16) -> WindowSize {
    WindowSize { num_cols: cols, num_lines: rows, cell_width: 8, cell_height: 16 }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "agent_codex_tests.rs"]
mod agent_codex_tests;
