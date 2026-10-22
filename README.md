# Repodeck

A native, project-grouped terminal for macOS 12+ built for multi-repo work with AI coding agents.
It groups your repositories into projects, gives each repo its own tabs and split panes, and knows
when the Claude Code session in a pane is working, done, or waiting on you — so you can run several
agents side by side without babysitting each terminal. It drives the agents only through their own
CLI (`claude`), never calls an LLM API itself, and runs as one Rust process.

## Features

### Projects & terminals

- **Projects → repos → tabs → panes** — group related repositories, give each repo its own tabs,
  and split any tab into panes (⌘D right, ⇧⌘D down, ⇧⌘↩ to zoom one).
- **Fast, faithful terminal** — GPU-drawn grid on `alacritty_terminal`, true color, mouse reporting,
  clickable links, in-pane search (⌘F), box-drawing and powerline glyphs drawn as seamless shapes.
- **Shell integration** — knows each command's start, end and exit status; long commands report
  when they finish.
- **Command palette** (⌘K) — fuzzy-run any command or jump to any tab.
- **Files panel** (⌘B) with a built-in editor, syntax highlighting, and a rendered **Markdown preview**.
- **Paste images** straight into a pane; they are saved to a temp file whose path is typed for you.
- **Finder integration** — drop folders on the sidebar, or use Finder's Services menu
  ("New Repodeck Terminal at Folder").
- 11 built-in themes (Repodeck Dark, One Dark, Dracula, Catppuccin, Tokyo Night, Nord, Gruvbox,
  Solarized, GitHub Light…); any installed monospace font for the terminal and any installed
  font, at its own size, for the interface.

### Claude Code agents

- **Agent status at a glance** — each Claude pane shows *Working*, *Done*, *Your turn*, *Approve?*
  or *Error*, read from Claude Code hooks injected per launch with `claude --settings` (your global
  `~/.claude/settings.json` is never edited).
- **Inbox & notifications** — a bell lists every agent that finished or needs you; allow or deny
  permission prompts without switching panes; ⌃⌘N jumps to the next pane needing attention.
- **Subscription usage** — the status bar shows your Pro/Max **5-hour and weekly limits** and the
  active pane's context-window use, taken from Claude Code's own status line JSON. Your own
  `statusLine` command keeps working.
- **Resume past conversations** — the clock button lists the repo's Claude sessions; pick one to
  resume it in a new tab. Restarted agent panes resume where they left off.
- **Prompt library** — reusable prompts, plus the skills and MCP servers available in the repo.
- **Diff review** — view a file's changes, comment on lines, and send the diff with your notes to Claude.

### Git & GitHub

- **Branch and change count** per repo in the sidebar and status bar, refreshed on file changes.
- **Worktrees** — create a worktree for a branch in one step: gitignored files listed in
  `.worktreeinclude` are copied over, your setup command runs, and the repo's Claude history is
  copied so its conversations can be resumed there. Remove it (and its merged branch) when done.
- **Pull requests** — see the branch's PR and CI state (via `gh`), open a new PR, or hand a failed
  CI job's log to Claude.

### Scripting

Run `repodeck` inside any pane to talk to the app:

```sh
repodeck notify "Build finished"   # post to the inbox (and a system notification)
repodeck open ~/code/api           # open a folder in a new terminal tab
repodeck claude ~/code/api         # open a folder in a new Claude tab
repodeck focus "$REPODECK_PANE"    # bring a pane to the front
repodeck list                      # every pane: repo, path, tab, id, kind, agent status
```

Programs can also raise notifications with the OSC 9 / OSC 777 escape sequences.

## Requirements

- macOS 12 (Monterey) or later.
- [Claude Code](https://code.claude.com) (`claude`) on your `PATH` for agent tabs. Plain terminal
  tabs work without it.
- Optional: `git` for repo status and worktrees, [`gh`](https://cli.github.com) (logged in) for
  pull request and CI status.

## Install

> **Note for testers:** Repodeck is currently signed with a **local (non-notarized) certificate**
> — there is no Apple Developer ID behind it yet — so macOS will block the app on first launch.
> That warning is expected; the steps below get past it. If you'd rather not trust a pre-built
> binary, build from source instead (no Gatekeeper steps needed).

### Option 1 — Download the pre-built app (3 steps)

**Step 1 — Install.** Download `Repodeck.zip` from the
[latest release](https://github.com/MinhQuang28/simple-terminal/releases), unzip it, and drag
`Repodeck.app` into your **Applications** folder.

**Step 2 — Unblock.** Clear the Gatekeeper quarantine flag (one-time; a downloaded,
non-notarized app won't open without it):

```sh
xattr -dr com.apple.quarantine /Applications/Repodeck.app
```

Not comfortable with Terminal? Instead: double-click the app once (macOS blocks it), then
System Settings → Privacy & Security → scroll down → **Open Anyway** → open the app again.

**Step 3 — Open.** Launch Repodeck from Applications, click **+** (New project) in the sidebar and add a
repo folder, and open a terminal (⌘T) or a Claude session (⇧⌘T). macOS asks for notification
permission the first time an agent finishes, and for folder access when a program in a pane first
touches Desktop, Documents or Downloads.

### Option 2 — Build from source

Requires a recent stable Rust toolchain (edition 2024) and
[`cargo-bundle`](https://github.com/burtonageo/cargo-bundle) (`cargo install cargo-bundle`).

```sh
# One-time per machine: a stable signing identity so macOS privacy grants survive every
# rebuild (otherwise folder access is asked for again after each build).
script/setup-signing-cert.sh

# Build and sign target/release/bundle/osx/Repodeck.app
script/build-app.sh

# Or build and replace /Applications/Repodeck.app
script/build-app.sh --install
```

> **Check the signature before trusting a rebuild:** `build-app.sh` looks for the
> **"Repodeck Local Signing"** identity in the login keychain and prints
> `==> signing with stable local identity (…)` when it uses it. If it prints
> `==> ad-hoc signing` instead, the cert is missing on this machine — run
> `script/setup-signing-cert.sh` once and rebuild. Set `DEVELOPER_ID_APP` to sign with a real
> Developer ID instead. Verify what an installed build is signed with:
> `codesign -dvv /Applications/Repodeck.app 2>&1 | grep Authority`.

For a quick unbundled run while developing: `cargo run`.

## Usage

| Shortcut | Action                       | Shortcut | Action                     |
| -------- | ---------------------------- | -------- | -------------------------- |
| ⌘T       | New terminal                 | ⌘D       | Split pane right           |
| ⇧⌘T      | New Claude session           | ⇧⌘D      | Split pane down            |
| ⌘K       | Command palette              | ⇧⌘↩      | Zoom pane                  |
| ⌘F       | Find in terminal             | ⌥⌘←↑→↓   | Focus neighbouring pane    |
| ⇧⌘[ ⇧⌘]  | Previous / next tab          | ⌘W       | Close pane                 |
| ⌥⌘T      | Reopen closed tab            | ⇧⌘W      | Close tab                  |
| ⌃⌘N      | Next pane needing attention  | ⌘B       | Toggle files panel         |
| ⌘,       | Settings                     |          |                            |

**Settings** (⌘,) covers appearance (theme, ANSI palette), typography (terminal font and size,
a separate interface font and size, line height), notifications (on/off, sound, and whether
macOS allows them), terminal
behaviour (shell, cursor, scrollback, Option as Meta, copy on select) and the agent (the command
that starts new Claude sessions, and its renderer: **Fullscreen**, where Claude scrolls its own
transcript a line at a time, or **Classic**, where Claude writes into the pane's scrollback so it
scrolls smoothly, is searchable, and stays after Claude exits).

Per repo, a **setup command** runs in each new worktree, and a `.worktreeinclude` file at the repo
root lists gitignored paths (one per line, e.g. `.env`) to copy into new worktrees.

## Development

```sh
# Run the test suite
cargo test

# Lint and format
cargo clippy --all-targets
cargo fmt

# Regenerate the app icon (macos/AppIcon.icns)
swift script/make-icon.swift

# Package a release: build + zip + sha256 (local only)
script/package-release.sh

# Same, and publish the GitHub release + upload the zip
script/package-release.sh --publish
```

### Project layout

| Path                           | Purpose                                                          |
| ------------------------------ | ---------------------------------------------------------------- |
| `src/terminal/`                | Terminal panes: PTY session, rendering, input, search, scrolling |
| `src/ui/`                      | Sidebar, tab bar, inbox, palette, settings and other UI          |
| `src/editor/`                  | File editor and Markdown preview                                 |
| `src/agent_events.rs`          | Claude Code hook injection and agent status                      |
| `src/claude_status_line.rs`    | Subscription limits from Claude Code's status line               |
| `src/claude_history.rs`        | Past Claude conversations per repo                               |
| `src/worktree.rs`              | Git worktree creation and removal                                |
| `src/socket_api.rs`            | The `repodeck` CLI's socket                                      |
| `macos/`                       | Info.plist extensions, entitlements, app icon                    |
| `script/build-app.sh`          | Bundle, sign, and optionally install the `.app`                  |
| `script/setup-signing-cert.sh` | Create the stable local signing certificate                      |
| `script/package-release.sh`    | Build, zip, hash, and (optionally) publish a release             |
| `script/make-icon.swift`       | Draw the app icon                                                |

## License

Repodeck is **source-available** under the [PolyForm Noncommercial License 1.0.0](LICENSE.md):

- ✅ Free for **personal, noncommercial use** — install it, read the source, modify it, build it yourself, share it for free.
- ❌ **Commercial use is not permitted** — you may not copy this source into your own product, sell it, charge for access to it, or otherwise use it for commercial purposes without a separate license from the author.

Copyright © Ha Minh Quang.
