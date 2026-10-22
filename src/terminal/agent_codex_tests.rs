//! A real Codex turn through an agent pane. Ignored by default: it starts the Codex TUI, spends
//! ChatGPT quota, and needs `codex` logged in plus `cargo build` done so `repodeck` is on the
//! pane's PATH. Run with `cargo test codex_turn -- --ignored --nocapture`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use eframe::egui;
use uuid::Uuid;

use super::{Launch, SessionConfig, TerminalSession};
use crate::agent_events::{AgentEvents, AgentStatus};
use crate::agent_launch::with_hooks;
use crate::model::{AgentRenderer, CursorShape};
use crate::theme::Theme;

const PROMPT: &str = "Reply with exactly the word pong.";

fn screen(session: &TerminalSession) -> String {
    let term = session.term.lock();
    let end = Point::new(Line(term.screen_lines() as i32 - 1), Column(term.columns() - 1));
    term.bounds_to_string(Point::new(Line(0), Column(0)), end)
}

fn wait(
    session: &mut TerminalSession,
    secs: u64,
    mut done: impl FnMut(&mut TerminalSession) -> bool,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        session.process_events();
        if done(session) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
#[ignore = "starts the real Codex TUI and spends ChatGPT quota"]
fn a_codex_turn_in_an_agent_pane_reports_its_prompt_and_reply() {
    let ctx = egui::Context::default();
    let theme = Theme::default();
    let bin_dir = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    assert!(bin_dir.join("repodeck").is_file(), "run `cargo build` first: {}", bin_dir.display());
    let cwd = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let command = with_hooks(&format!("export PATH=\"{}:$PATH\"; codex", bin_dir.display()));
    assert!(command.contains("repodeck run-codex codex"), "{command}");
    let pane = Uuid::new_v4();
    let agent = AgentEvents::create(pane);
    let config = SessionConfig {
        pane,
        cwd,
        launch: Launch::Agent(&command),
        shell: "",
        cursor: CursorShape::Block,
        scrollback_lines: 1000,
        theme: &theme,
        agent_renderer: AgentRenderer::Classic,
        agent_hide_api_keys: true,
    };
    let mut session = TerminalSession::spawn(config, agent, Arc::default(), &ctx).unwrap();
    session.resize(120, 40);

    // The TUI runs no SessionStart hook until the first turn, so wait for its composer instead.
    let ready = wait(&mut session, 45, |s| screen(s).contains("Ask Codex"));
    assert!(ready, "Codex composer not shown within 45 s; screen:\n{}", screen(&session));
    std::thread::sleep(Duration::from_secs(2));
    // Keys arriving in a burst count as a paste, whose Enter only adds a line; pause before it.
    session.send_input(PROMPT.as_bytes().to_vec());
    wait(&mut session, 1, |_| false);
    session.send_input(b"\r".to_vec());

    let mut changes = Vec::new();
    let finished = wait(&mut session, 120, |s| {
        changes.extend(s.take_status_change());
        changes.last().is_some_and(|(status, _)| *status == AgentStatus::Done)
    });
    if !finished {
        let events = std::fs::read_to_string(session.agent.as_ref().unwrap().path()).unwrap_or_default();
        panic!("no Stop within 120 s; changes {changes:?}; events {events:?}; screen:\n{}", screen(&session));
    }
    assert_eq!(changes.first().map(|(s, _)| *s), Some(AgentStatus::Working), "{changes:?}");
    assert_eq!(changes.last().map(|(_, m)| m.as_str()), Some("pong"), "{changes:?}");
    assert_eq!(session.title(), Some(PROMPT), "tab title follows the prompt");
    println!("status changes: {changes:?}\nscreen:\n{}", screen(&session));
    session.send_input(b"\x03\x03".to_vec());
}
