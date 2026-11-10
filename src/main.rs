/// The app bundle's identifier, as in Cargo.toml's bundle metadata; macOS keys permissions and
/// icon caches by it.
pub const BUNDLE_ID: &str = "co.egohub.repodeck";

mod agent_events;
mod agent_launch;
mod app;
mod app_actions;
mod app_code;
mod app_files;
mod app_layout;
#[cfg(target_os = "macos")]
mod app_menu;
mod app_notices;
mod app_open_folder;
mod app_panes;
mod app_pr;
mod app_resume_session;
mod app_shortcuts;
mod app_split;
mod app_worktree;
mod attention;
mod bench;
mod claude_history;
mod claude_status_line;
mod cli;
mod clipboard_image;
mod code_format;
mod codex_launch;
mod definitions;
mod diff;
mod editor;
mod event_log;
mod file_drag;
mod file_icon_files;
mod file_icons;
mod file_index;
mod file_tree;
mod finder_service;
mod font_catalog;
mod font_install;
mod git_branch;
mod git_commit;
mod git_ignore;
mod git_status;
mod git_sync;
mod git_watcher;
mod inbox;
mod keymap;
mod keymap_hint;
mod library;
mod library_cache;
mod model;
mod notification_permission;
#[cfg(target_os = "macos")]
mod notification_post;
mod pane_layout;
mod pr_status;
mod program_icon_files;
mod programs;
mod repo_view;
mod session_pool;
mod shell_env;
mod shell_events;
mod shell_integration;
mod socket_api;
mod socket_commands;
mod store;
mod symbol_fallback_fonts;
mod tab_title;
mod terminal;
mod text_search;
mod text_search_runner;
mod theme;
mod theme_presets;
mod trash;
mod ui;
mod window_activity;
mod worktree;
mod worktree_branch_name;
mod worktree_include;

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

fn main() -> eframe::Result {
    if cli::run() {
        return Ok(());
    }
    if std::env::args_os().len() > 1 {
        eprintln!("{}", cli::USAGE);
        std::process::exit(2);
    }
    drop_parent_agent_env();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Repodeck")
            // An empty icon stops eframe from replacing the bundle's Dock icon with its own.
            .with_icon(egui::IconData::default())
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
