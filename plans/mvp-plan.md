# Repodeck MVP

Native Rust terminal (eframe/egui 0.37 + alacritty_terminal 0.26) organised as
Projects → Repos → Terminal tabs, persisted across restarts.

## Scope
1. Sidebar: projects (add/rename/remove), repos per project (folder picker), tabs per repo.
2. Terminal tabs per repo, cwd = repo path; split panes (right/down) inside a tab.
3. Git status per repo (branch, ahead/behind, dirty count) polled in background via `git` CLI.
4. Agent tabs: run the configured agent command (default `claude`) in the repo;
   restored agent tabs run the resume command (default `claude --continue`).
5. State saved as JSON in the platform data dir; sessions respawn lazily on restore.

## Layout
- `model.rs` persisted data, `store.rs` load/save, `git_status.rs` watcher
- `terminal/` session (PTY + Term), colors, key input, grid view widget
- `ui/` sidebar, workspace (tab bar + panes), dialogs
- `app.rs` wiring, shortcuts, save-on-change

## Out of scope (next)
Mouse reporting for TUIs, hyperlinks, search, themes UI, git worktrees per branch.
