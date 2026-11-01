use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use eframe::egui;
use uuid::Uuid;

use crate::agent_events::{AgentEvents, AgentStatus};
use crate::agent_launch::{with_first_prompt, with_hooks};
use crate::claude_history::{has_transcript, is_session_id};
use crate::claude_status_line::StatusLine;
use crate::model::{Pane, PaneKind, Settings};
use crate::terminal::{Launch, Presence, RepaintGate, SessionConfig, TerminalSession};

/// Live terminal sessions keyed by pane id, spawned on first display.
#[derive(Default)]
pub struct SessionPool {
    sessions: HashMap<Uuid, TerminalSession>,
    errors: HashMap<Uuid, String>,
    /// Panes created during this run; panes missing here were restored from disk.
    fresh: HashSet<Uuid>,
    /// Commands to run before the shell in panes that haven't started yet.
    startup: HashMap<Uuid, String>,
    repaint: Arc<RepaintGate>,
}

impl SessionPool {
    pub fn mark_fresh(&mut self, pane: Uuid) {
        self.fresh.insert(pane);
    }

    /// Runs `command` in a shell pane once it starts, then leaves the shell open; for a new agent
    /// pane, it is the first prompt the agent gets.
    pub fn set_startup(&mut self, pane: Uuid, command: String) {
        self.startup.insert(pane, command);
    }

    pub fn get_or_spawn(
        &mut self,
        pane: &Pane,
        cwd: &Path,
        settings: &Settings,
        ctx: &egui::Context,
    ) -> Result<&mut TerminalSession, String> {
        let id = pane.id;
        if let Some(err) = self.errors.get(&id) {
            return Err(err.clone());
        }
        if !self.sessions.contains_key(&id) {
            // Only resume when the transcript is actually present under this cwd; otherwise
            // `claude --resume` aborts with "No conversation found with session ID", so fall
            // through to the restore/fresh command instead.
            let resume = pane
                .session
                .as_deref()
                .filter(|s| is_session_id(s) && has_transcript(cwd, s))
                // A preset's own command (Claude with flags) resumes with those flags.
                .map(|s| {
                    format!("{} --resume {s}", pane.command.as_deref().unwrap_or(&settings.agent_command))
                });
            let command = match (&pane.kind, resume) {
                (PaneKind::Shell, _) => self.startup.get(&id).or(pane.command.as_ref()).cloned(),
                (PaneKind::Agent, Some(command)) => Some(command),
                // A preset's agent starts afresh on restore too: only Claude's resume is known.
                (PaneKind::Agent, None) if pane.command.is_some() => pane.command.clone(),
                (PaneKind::Agent, None) if self.fresh.contains(&id) => Some(settings.agent_command.clone()),
                (PaneKind::Agent, None) => Some(settings.agent_resume_command.clone()),
            };
            // A terminal tab gets events too, dormant, for an agent started from its shell.
            let agent = if pane.kind == PaneKind::Agent && command.is_some() {
                AgentEvents::create(id)
            } else {
                AgentEvents::create_dormant(id)
            };
            let command = command.map(|c| if agent.is_some() { with_hooks(&c) } else { c });
            // Added after the hooks so nothing in the prompt is read as part of the command line.
            let task = (pane.kind == PaneKind::Agent && self.fresh.contains(&id))
                .then(|| self.startup.get(&id))
                .flatten();
            let command = match (command, task) {
                (Some(command), Some(task)) => Some(with_first_prompt(&command, task)),
                (command, _) => command,
            };
            let launch = match (&pane.kind, command.as_deref()) {
                (_, None) => Launch::LoginShell,
                (PaneKind::Agent, Some(command)) => Launch::Agent(command),
                (PaneKind::Shell, Some(command)) => Launch::Command(command),
            };
            let config = SessionConfig {
                pane: id,
                cwd,
                launch,
                shell: &settings.shell,
                cursor: settings.cursor_shape,
                scrollback_lines: settings.scrollback_lines,
                theme: &settings.theme,
                agent_renderer: settings.agent_renderer,
                agent_hide_api_keys: settings.agent_hide_api_keys,
            };
            let spawned = TerminalSession::spawn(config, agent, self.repaint.clone(), ctx);
            match spawned {
                Ok(session) => {
                    self.sessions.insert(id, session);
                    self.startup.remove(&id);
                }
                Err(err) => {
                    let msg = format!("Failed to start terminal in {}: {err}", cwd.display());
                    log::error!("{msg}");
                    self.errors.insert(id, msg.clone());
                    return Err(msg);
                }
            }
        }
        Ok(self.sessions.get_mut(&id).expect("session inserted above"))
    }

    pub fn get(&self, pane: Uuid) -> Option<&TerminalSession> {
        self.sessions.get(&pane)
    }

    pub fn get_mut(&mut self, pane: Uuid) -> Option<&mut TerminalSession> {
        self.sessions.get_mut(&pane)
    }

    pub fn agent_status(&self, pane: Uuid) -> Option<AgentStatus> {
        self.sessions.get(&pane)?.agent_status()
    }

    /// The most recent subscription limits any agent reported; they're account-wide.
    pub fn latest_limits(&self) -> Option<StatusLine> {
        self.sessions
            .values()
            .filter_map(TerminalSession::agent_status_line)
            .filter(StatusLine::has_limits)
            .max_by_key(|s| s.reported_at)
    }

    /// Number of agents stopped and waiting on the user.
    pub fn attention_count(&self) -> usize {
        self.sessions.values().filter(|s| s.agent_status().is_some_and(AgentStatus::needs_attention)).count()
    }

    /// Kills the session so the next display spawns a fresh one.
    pub fn restart(&mut self, pane: Uuid) {
        self.sessions.remove(&pane);
        self.errors.remove(&pane);
        self.fresh.insert(pane);
    }

    pub fn apply_settings(&self, settings: &Settings) {
        for session in self.sessions.values() {
            session.apply_settings(settings.cursor_shape, settings.scrollback_lines);
        }
    }

    /// Drops sessions whose panes no longer exist in the model.
    pub fn retain(&mut self, live: &HashSet<Uuid>) {
        self.sessions.retain(|id, _| live.contains(id));
        self.errors.retain(|id, _| live.contains(id));
        self.fresh.retain(|id| live.contains(id));
        self.startup.retain(|id, _| live.contains(id));
    }

    /// Drains PTY events of sessions not currently on screen so their channels don't grow
    /// unbounded and terminal queries still get answered.
    /// While the window is minimized or covered (`window_shown` false) no pane is on screen.
    pub fn pump_background(&mut self, visible: &HashSet<Uuid>, window_shown: bool, ctx: &egui::Context) {
        for (id, session) in &mut self.sessions {
            let on_screen = window_shown && visible.contains(id);
            // An on-screen pane's view sets how closely it is watched when it draws.
            if !on_screen {
                session.set_presence(Presence::Hidden);
            } else if session.presence() == Presence::Hidden {
                session.set_presence(Presence::Visible);
            }
            if !on_screen && let Some(text) = session.process_events() {
                ctx.copy_text(text);
            }
        }
    }
}
