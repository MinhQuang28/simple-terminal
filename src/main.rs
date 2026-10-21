mod agent_events;
mod app;
mod app_actions;
mod app_files;
mod app_layout;
mod app_notices;
mod app_open_folder;
mod app_panes;
mod app_pr;
mod app_resume_session;
mod app_shortcuts;
mod app_worktree;
mod attention;
mod claude_history;
mod claude_usage;
mod clipboard_image;
mod diff;
mod editor;
mod event_log;
mod file_drag;
mod file_tree;
mod finder_service;
mod font_catalog;
mod git_status;
mod git_watcher;
mod inbox;
mod library;
mod model;
mod pane_layout;
mod pr_status;
mod session_pool;
mod shell_events;
mod shell_integration;
mod socket_api;
mod store;
mod symbol_fallback_fonts;
mod tab_title;
mod terminal;
mod theme;
mod theme_presets;
mod ui;
mod worktree;

use eframe::egui;

/// Drops variables a parent Claude Code session sets for its children. Otherwise, when Repodeck is
/// launched from such a session, agents in its terminals think they are nested and stop saving
/// transcripts (so `claude --continue` finds nothing). Settings the user exports in their shell
/// profile come back because terminals run login shells.
fn drop_parent_agent_env() {
    let inherited: Vec<_> = std::env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| name.to_str().is_some_and(|n| n.starts_with("CLAUDE") || n == "AI_AGENT"))
        .collect();
    for name in inherited {
        // SAFETY: called first in `main`, before any other thread exists to read the environment.
        unsafe { std::env::remove_var(name) };
    }
}

const CLI_USAGE: &str =
    "usage: repodeck notify <text> | open <folder> | claude <folder> | focus [pane] | list";

/// Command-line mode, used by scripts and hooks inside panes. `notify` emits an OSC 9 sequence
/// on stdout, which Repodeck (and most other terminals) show as a notification for the pane it
/// runs in; the rest talk to the running app over its socket.
fn run_cli() -> bool {
    use std::io::Write;
    let mut args = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned());
    let Some(verb) = args.next() else { return false };
    let rest: String = args.collect::<Vec<_>>().join(" ").chars().filter(|c| !c.is_control()).collect();
    let command = match verb.as_str() {
        "notify" => {
            let mut out = std::io::stdout().lock();
            let _ = write!(out, "\x1b]9;{rest}\x07");
            let _ = out.flush();
            return true;
        }
        "open" | "claude" => {
            let folder = if rest.is_empty() { ".".to_string() } else { rest };
            let path = std::fs::canonicalize(&folder).unwrap_or_else(|_| folder.into());
            format!("{verb} {}", path.display())
        }
        "focus" => {
            let pane =
                if rest.is_empty() { std::env::var(socket_api::PANE_VAR).unwrap_or_default() } else { rest };
            format!("focus {pane}")
        }
        "list" => "list".to_string(),
        "-h" | "--help" | "help" => {
            println!("{CLI_USAGE}");
            return true;
        }
        _ => return false,
    };
    match socket_api::send(&command) {
        Ok(reply) if reply.starts_with("error:") => {
            eprintln!("{reply}");
            std::process::exit(1);
        }
        Ok(reply) => {
            if !reply.is_empty() {
                println!("{reply}");
            }
        }
        Err(err) => {
            eprintln!("repodeck: {err}");
            std::process::exit(1);
        }
    }
    true
}

fn main() -> eframe::Result {
    if run_cli() {
        return Ok(());
    }
    if std::env::args_os().len() > 1 {
        eprintln!("{CLI_USAGE}");
        std::process::exit(2);
    }
    drop_parent_agent_env();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Repodeck")
            .with_app_id("dev.repodeck.app")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([760.0, 480.0])
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false),
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native("Repodeck", options, Box::new(|cc| Ok(Box::new(app::RepodeckApp::new(cc)))))
}
