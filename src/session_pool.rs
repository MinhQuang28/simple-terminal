use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use eframe::egui;
use uuid::Uuid;

use crate::agent_events::{AgentEvents, AgentStatus, with_hooks};
use crate::claude_history::is_session_id;
use crate::model::{Pane, PaneKind, Settings};
use crate::terminal::{Launch, RepaintGate, SessionConfig, TerminalSession};

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

    /// Runs `command` in the pane's shell once it starts, then leaves the shell open.
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
            let resume = pane
                .session
                .as_deref()
                .filter(|s| is_session_id(s))
                .map(|s| format!("{} --resume {s}", settings.agent_command));
            let command = match (&pane.kind, resume) {
                (PaneKind::Shell, _) => self.startup.get(&id).cloned(),
                (PaneKind::Agent, Some(command)) => Some(command),
                (PaneKind::Agent, None) if self.fresh.contains(&id) => Some(settings.agent_command.clone()),
                (PaneKind::Agent, None) => Some(settings.agent_resume_command.clone()),
            };
            let agent = (pane.kind == PaneKind::Agent && command.is_some())
                .then(|| AgentEvents::create(id))
                .flatten();
            let command = command.map(|c| if agent.is_some() { with_hooks(&c) } else { c });
            let launch = command.as_deref().map_or(Launch::LoginShell, Launch::Command);
            let config = SessionConfig {
                pane: id,
                cwd,
                launch,
                shell: &settings.shell,
                cursor: settings.cursor_shape,
                scrollback_lines: settings.scrollback_lines,
                theme: &settings.theme,
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
    pub fn pump_background(&mut self, visible: &HashSet<Uuid>, ctx: &egui::Context) {
        for (id, session) in &mut self.sessions {
            let on_screen = visible.contains(id);
            session.set_visible(on_screen);
            if !on_screen && let Some(text) = session.process_events() {
                ctx.copy_text(text);
            }
        }
    }
}
