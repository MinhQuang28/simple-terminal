use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use uuid::Uuid;

use super::{Launch, SessionConfig, TerminalSession};
use crate::model::{AgentRenderer, CursorShape};
use crate::theme::Theme;

#[test]
fn osc_notifications_from_the_child_reach_the_session() {
    let ctx = egui::Context::default();
    let theme = Theme::default();
    let cwd = std::env::temp_dir();
    let script = "printf 'hello\\n\\033]9;Build done\\007\\033]777;notify;Tests;2 failed\\033\\\\'; exit";
    let config = SessionConfig {
        pane: Uuid::new_v4(),
        cwd: &cwd,
        launch: Launch::Command(script),
        shell: "/bin/sh",
        cursor: CursorShape::Block,
        scrollback_lines: 100,
        theme: &theme,
        agent_renderer: AgentRenderer::default(),
        agent_hide_api_keys: false,
    };
    let mut session = TerminalSession::spawn(config, None, Arc::default(), &ctx).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while got.len() < 2 && Instant::now() < deadline {
        session.process_events();
        got.extend(session.take_notifications());
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(got, ["Build done", "Tests: 2 failed"]);
}

#[test]
fn input_typed_after_a_pasted_image_lands_after_its_path() {
    use alacritty_terminal::grid::Dimensions;
    use alacritty_terminal::index::{Column, Line, Point};

    let ctx = egui::Context::default();
    let theme = Theme::default();
    let cwd = std::env::temp_dir();
    let config = SessionConfig {
        pane: Uuid::new_v4(),
        cwd: &cwd,
        launch: Launch::Command("cat"),
        shell: "/bin/sh",
        cursor: CursorShape::Block,
        scrollback_lines: 100,
        theme: &theme,
        agent_renderer: AgentRenderer::default(),
        agent_hide_api_keys: false,
    };
    let mut session = TerminalSession::spawn(config, None, Arc::default(), &ctx).unwrap();
    let image = egui::ColorImage::filled([2, 2], egui::Color32::RED);
    session.paste_image(Arc::new(image), &ctx);
    session.send_input(b"after\n".to_vec());

    let screen = |session: &TerminalSession| {
        let term = session.term.lock();
        let end = Point::new(Line(term.screen_lines() as i32 - 1), Column(term.columns() - 1));
        term.bounds_to_string(Point::new(Line(0), Column(0)), end)
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    while !screen(&session).contains("after") && Instant::now() < deadline {
        session.process_events();
        std::thread::sleep(Duration::from_millis(20));
    }
    let text = screen(&session);
    let (path, after) = (text.find(".png").expect("image path pasted"), text.find("after").unwrap());
    assert!(path < after, "{text}");
}

/// Claude Code only wraps its frames in synchronized updates (DEC 2026) when the terminal
/// reports the mode as supported; without that every frame can be shown half drawn.
#[test]
fn the_terminal_reports_synchronized_updates_as_supported() {
    let ctx = egui::Context::default();
    let theme = Theme::default();
    let cwd = std::env::temp_dir();
    let script = "stty raw -echo min 0 time 30; printf '\\033[?2026$p'; \
        r=$(dd bs=64 count=1 2>/dev/null | od -An -c | tr -d ' \\n'); stty sane; \
        printf '\\033]9;R=%s\\007' \"$r\"; exit";
    let config = SessionConfig {
        pane: Uuid::new_v4(),
        cwd: &cwd,
        launch: Launch::Command(script),
        shell: "/bin/sh",
        cursor: CursorShape::Block,
        scrollback_lines: 100,
        theme: &theme,
        agent_renderer: AgentRenderer::default(),
        agent_hide_api_keys: false,
    };
    let mut session = TerminalSession::spawn(config, None, Arc::default(), &ctx).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while got.is_empty() && Instant::now() < deadline {
        session.process_events();
        got.extend(session.take_notifications());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(got, ["R=033[?2026;2$y"]);
}

#[test]
fn the_classic_renderer_reaches_agent_panes_through_the_environment() {
    let ctx = egui::Context::default();
    let theme = Theme::default();
    let cwd = std::env::temp_dir();
    let pane = Uuid::new_v4();
    let script = "printf '\\033]9;NF=%s\\007' \"${CLAUDE_CODE_NO_FLICKER-unset}\"; exit";
    let config = SessionConfig {
        pane,
        cwd: &cwd,
        launch: Launch::Agent(script),
        shell: "/bin/sh",
        cursor: CursorShape::Block,
        scrollback_lines: 100,
        theme: &theme,
        agent_renderer: AgentRenderer::Classic,
        agent_hide_api_keys: true,
    };
    let agent = crate::agent_events::AgentEvents::create(pane);
    let mut session = TerminalSession::spawn(config, agent, Arc::default(), &ctx).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while got.is_empty() && Instant::now() < deadline {
        session.process_events();
        got.extend(session.take_notifications());
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(got, ["NF=0"]);
}
