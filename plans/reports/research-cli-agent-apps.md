# CLI-agent orchestrator apps: research for repodeck

Date: 2026-10-21. Scope: apps that drive Claude Code / Codex / OpenCode through their CLIs (BYO subscription, no own API client).
Method: WebSearch + WebFetch of vendor docs and repos. Aggregator/blog pages are flagged. Doc snapshots are ~mid-2026; re-check before shipping.

Tags: **[V]** verified on a primary doc/repo page fetched. **[S]** secondary (blog, aggregator, news, search snippet). **[U]** unverified or conflicting.

## 1. Key findings

1. Status detection converged on **vendor hook systems**, not output parsing: Claude Code hooks (33 events), Codex `notify` + hooks, OpenCode plugin events. [V]
2. Repodeck already uses the cleanest pattern found: inline `claude --settings "$REPODECK_AGENT_HOOKS"` (`src/agent_events.rs`). Most competitors (cmux, Emdash, Superset, Warp) write into global `~/.claude/settings.json` or agent config. [V]
3. Biggest gap: **no Codex or OpenCode adapter** in repodeck. `with_hooks` rewrites only `claude`. [V]
4. **Auth gap**: repodeck does not scrub `ANTHROPIC_API_KEY`. Claude Code uses that key over the subscription once approved. Cheap fix. [V]
5. **Usage gap**: `claude_usage.rs` sums transcript tokens. Claude's `statusLine` stdin JSON exposes official subscription `rate_limits.five_hour/seven_day.used_percentage` (Pro/Max). [V]
6. **Policy**: Anthropic's legal page bars third parties from routing Pro/Max OAuth on users' behalf or storing their tokens. Allowed: user runs the unmodified `claude` binary and logs in. Repodeck must never read `~/.claude/.credentials.json` or Keychain tokens. [V]
7. Repodeck has no HTTP/LLM client deps (`Cargo.toml`) and no `api.anthropic.com` / `api.openai.com` refs. "CLI-only" is true today but not enforced. [V]
8. Dead or changed: vibe-kanban sunsetting (announced 2026-04-10). Terragon = unmaintained snapshot of a shut-down product. Crystal deprecated, replaced by Nimbalyst. Do not model on these. [V]

## 2. App comparison

| App | Status (doc snapshot) | License | CLIs supported | Status signal | Auth / env handling | Isolation, resume |
|---|---|---|---|---|---|---|
| **Conductor** (conductor.build) | Active, Mac app; cloud tier added [S] | not verified [U] | Claude Code, Codex, Cursor, OpenCode [V] | not documented in fetched pages [U] | Not documented in fetched pages [U] | Workspace = git worktree + branch + terminal + diff + PR path [V] |
| **Crystal** | Deprecated, MIT, last release v0.3.5 (Feb 2026) [V] | MIT [V] | Claude Code, Codex [V] | n/a | n/a | Worktrees [V] |
| **Nimbalyst** (Crystal successor) | Active [S] | conflicting reports (MIT vs closed) [U] | Claude Code, Codex [S] | n/a [U] | n/a [U] | Worktrees optional, session kanban, iOS app [S] |
| **Claude Squad** (smtg-ai) | Active [V] | AGPL-3.0 [V] | claude (default), codex, aider, gemini, opencode [V] | not documented [V] | Codex path documents `OPENAI_API_KEY` [V] | tmux session per agent + git worktree; `cs -p`; `-y` autoyes (experimental) [V] |
| **Sculptor** (Imbue) | Beta/free [S] | not verified [U] | Claude Code; Codex via OpenAI key (Dec 2025 update) [S] | not documented in fetched pages [U] | Users bring Claude sub or API key [S] | Docker container per agent; Mac Apple Silicon + Linux [V] |
| **vibe-kanban** (BloopAI) | **Sunsetting** since 2026-04-10; local-only, community OSS [V] | Apache-2.0 [V] | Claude Code, Codex, Gemini CLI, Copilot, Amp, Cursor, OpenCode, Droid, CCR, Qwen Code [V] | not documented [V] | `DISABLE_WORKTREE_CLEANUP`, `MCP_HOST`, `MCP_PORT` [V] | Kanban card → worktree + agent [S] |
| **Superset** (superset-sh) | Active, desktop free [V] | Elastic License 2.0 [V] | Claude Code, Codex, OpenCode, Cursor Agent, Gemini, Copilot (tested) [V] | Hooks + wrappers in `~/.superset/bin`; start/finish/waiting; per-agent "Superset hooks" toggle [V] | `SUPERSET_HOME_DIR` (default `~/.superset`) [V]; also has built-in chat + BYO API providers (OpenRouter, Bedrock, Vertex) [V] | Worktree per workspace; `.superset/config.json` `setup`/`teardown`/`run` arrays [V] |
| **cmux** (manaflow-ai) | Active, native macOS (Swift/AppKit on libghostty) [V] | GPL-3.0-or-later (server parts BSL 1.1) [V] | Claude Code, OpenCode, Codex (via `cmux hooks setup codex`) [V] | OSC 9/99/777 + `cmux notify`; Claude hooks `Stop`/`SubagentStop` [V] | `CMUX_SOCKET_PATH`, `CMUX_TAB_ID`, `CMUX_PANEL_ID` in child shells [V] | Splits, sidebar (branch, PR, ports, last notice) [V]; `cmux claude-teams` [V] |
| **Warp** (3rd-party CLI agents) | Active; 16 CLIs listed [V] | closed [S] | Claude Code, Codex, OpenCode, Gemini, Cursor, Pi, Goose, etc. [V] | Notifications only for Claude Code, Codex, OpenCode [V]; detection mechanism not documented [V] | Agent owns auth [V] | Vertical tabs, toolbelt chips, rich input, review comments, Remote Control [V] |
| **Terragon** | Unmaintained snapshot of shut-down product [V] | Apache-2.0 [V] | Claude Code, Codex, Amp, Gemini [V] | n/a | Cloud sandboxes [V] | Container per agent [V] |
| **Opcode / Claudia** (getAsterisk; winfunc mirror) | Active-ish; releases "soon" per README snapshot [V] | AGPL-3.0 [V] | Claude Code [V] | Usage dashboard; reads `~/.claude/projects/` [V] | `claude` must be on PATH [V] | Checkpoints, timeline, fork [V]; MCP add via UI/JSON, import from Claude Desktop [S] |
| **CCManager** (kbwo) | Active, npm `ccmanager` [V] | MIT [V] | claude, gemini, codex, cursor-agent, copilot, cline, opencode, kimi, mcode, grok [V] | busy / waiting / idle; mechanism not documented [V][U] | Auto-approval via Claude Haiku judge (experimental) [V] | Worktrees, no tmux; copies Claude session data to new worktree [V] |
| **Zed external agents** (ACP) | Active [V] | not verified [U] | Claude (`claude-agent-acp`), Codex (`codex-acp`), OpenCode, Gemini CLI, Copilot, Cursor, Pi [V] | ACP structured events (not detailed here) [U] | Agent owns auth; Zed does not configure it [V] | Threads in Agent Panel; `agent_servers` `type: custom` [V] |
| **Emdash** (generalaction, YC W26) | Active [V] | Apache-2.0 [V] | 25-28 CLI agents incl. Claude Code, Codex, OpenCode, Amp, Gemini, Cursor [V] | Lifecycle hooks in 20 providers track working / awaiting / done [V] | Keys in OS keychain for remote; "does not send code or chats to Emdash servers" [V] | Worktree per task; SQLite state; SSH remote; Linear/Jira/GitHub issues; PR + CI [V] |
| **Agent Deck** (asheshgoplani) | Active, Go, tmux-based [V] | MIT [V] | Claude, Gemini, OpenCode, Codex, Copilot, Pi, others [V] | "Smart polling" + Codex notify hook + pi extension; Claude via statusLine/transcripts [V] | Config `$XDG_CONFIG_HOME/agent-deck/config.toml` [V] | Worktrees (`--worktree --new-branch`), setup/destruction scripts with consent [V]; MCP manager per session [V]; parent/child "conductor" [V] |

Not found in the fetched docs: any app that enforces "no direct LLM API" technically (env scrub, egress block, dependency ban). Emdash only states a data-handling policy [V].

## 3. Auth and policy constraints (BYO subscription)

### Claude Code auth precedence [V] (code.claude.com/docs/en/authentication)
1. Cloud provider vars (`CLAUDE_CODE_USE_BEDROCK|VERTEX|FOUNDRY`)
2. `ANTHROPIC_AUTH_TOKEN` (Bearer)
3. `ANTHROPIC_API_KEY`: interactive mode prompts once to approve; choice remembered. `-p` mode always uses it when present.
4. `apiKeyHelper` script output
5. `CLAUDE_CODE_OAUTH_TOKEN` (from `claude setup-token`, one year)
6. Anthropic profile / federation creds
7. Subscription OAuth from `/login`

- Doc: "If you have an active Claude subscription but also have `ANTHROPIC_API_KEY` set ... Claude Code uses the API key once you approve it." Fix: `unset ANTHROPIC_API_KEY`; check `/status`.
- `CLAUDE_CONFIG_DIR` gives a separate settings, session history and login per directory. Use for multi-account per project.
- Credential storage: macOS Keychain; fallback `~/.claude/.credentials.json` (0600). Do not read these.

### Anthropic legal terms [V] (code.claude.com/docs/en/legal-and-compliance)
- OAuth tokens: "intended exclusively for" Free/Pro/Max/Team/Enterprise purchasers, for Claude Code and native apps.
- Developers may not route requests through Pro/Max credentials on users' behalf, nor "collect, store, or intermediate Claude.ai credentials or session tokens".
- Binary must not be modified. Users may sign in to the unmodified binary with their own subscription.
- [S] Apr 2026 news: subscription limits stopped covering third-party harnesses (OpenClaw first; extra usage billed). Anthropic's own notice not fetched [U].

### Codex auth [S/V]
- Zed doc: Codex uses its native login (ChatGPT login, Codex or OpenAI API key) [V].
- `~/.codex/auth.json` and `OPENAI_API_KEY` precedence: not verified [U].
- `shell_environment_policy`: `ignore_default_excludes` defaults to `true` (names with KEY/SECRET/TOKEN are kept). Set `false` to strip them [V].

## 4. Vendor integration surface (cheat sheet)

### Claude Code [V] (code.claude.com/docs/en/hooks, /statusline)
- Hook events (33): `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `Notification`, `Stop`, `StopFailure`, `SubagentStop`, `SessionEnd`, `WorktreeCreate`, `WorktreeRemove`, `TeammateIdle`, `CwdChanged`, and 19 more.
- Common stdin fields: `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `permission_mode`, `agent_id`/`agent_type` (subagents).
- `Notification` matcher values: `permission_prompt`, `idle_prompt`, `auth_success`, `elicitation_dialog`, `agent_needs_input`, `agent_completed`, etc. Field `message` confirmed in example; `notification_type` [U].
- `Stop` stdin includes `last_assistant_message` [V].
- `PermissionRequest` output: `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"|"deny","updatedInput":{...}}}}`. Exit code 2 not honored here [V].
- Hook output may include `terminalSequence` (e.g., OSC 777) for desktop notify [V].
- Config files: `~/.claude/settings.json`, `.claude/settings.json`, `.claude/settings.local.json` [V]. Env: `CLAUDE_PROJECT_DIR`, `CLAUDE_ENV_FILE` (SessionStart only) [V].
- `statusLine`: `settings.json` key `{"type":"command","command":...,"refreshInterval"?}`. Stdin: `session_id`, `transcript_path`, `cwd`, `workspace.current_dir`, `workspace.project_dir`, `model`, `cost.total_cost_usd` (client-side estimate), `context_window.used_percentage`, `rate_limits.five_hour|seven_day.used_percentage` + `resets_at` (Pro/Max only, after first API response) [V].
- Transcripts: `~/.claude/projects/<encoded-cwd>/` [V via cmux/CCManager docs and repodeck `claude_history.rs`].

### Codex [V unless noted] (learn.chatgpt.com config-advanced)
- `notify` (top-level in `~/.codex/config.toml`): script receives one JSON arg. Fields: `type` (`agent-turn-complete`), `thread-id`, `turn-id`, `cwd`, `input-messages`, `last-assistant-message`. Project-local `.codex/config.toml` ignores `notify`.
- Fires only on turn completion. Approval prompts need TUI notifications: `[tui] notification_condition = "unfocused" | "always"` (Warp uses `always`) [V][S].
- `CODEX_HOME` defaults to `~/.codex`. Hooks load from `~/.codex/hooks.json`, `~/.codex/config.toml [hooks]`, and `<repo>/.codex/*` only when the project is trusted [V].
- Hook event names: [U]. Third-party: `SessionStart` and `Stop`, needs v0.114+ and a feature flag [S].
- Sessions: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`; SQLite index (`state_5.sqlite` or `state.db`, sources disagree) [S][U]. Resume: `codex resume --last`, `codex resume <id>` [S].
- `approval_policy`: `on-request` | `never` | `{granular={...}}`. `sandbox_mode`: `workspace-write` | `danger-full-access` [V].

### OpenCode [V] (opencode.ai/docs/plugins)
- Plugin dirs: `.opencode/plugins/` (project), `~/.config/opencode/plugins/` (global). npm plugins in `opencode.json` `plugin` array.
- Events: `session.idle`, `session.status`, `session.error`, `session.created`, `permission.asked`, `permission.replied`, `tool.execute.before|after`, `command.executed`, `file.edited`, `message.updated`.
- Plugin context: `project`, `directory`, `worktree`, `client`, `$`.
- CLI flags for serve/resume: not verified [U].

### Warp / cmux / CCManager / Superset / Conductor payloads
- Warp Claude plugin (`warp@claude-code-warp`): hooks `SessionStart`, `Stop`, `Notification` (`idle_prompt`, `agent_needs_input`), `PermissionRequest`, `UserPromptSubmit`, `PostToolUse`. Sends JSON via OSC 777 to `warp://cli-agent`. Needs `jq` [V]. Warp OpenCode: npm `@warp-dot-dev/opencode-warp` [S].
- cmux: `cmux notify --title T [--subtitle S] --body B`; OSC 99 (`ESC]99;...`) and OSC 777 (`ESC]777;notify;title;body BEL`). Hook env: `CMUX_NOTIFICATION_AGENT_KIND|CATEGORY|PENDING|IS_SUBAGENT`. Claude hook script `~/.claude/hooks/cmux-notify.sh` on `Stop`/`SubagentStop` in `~/.claude/settings.json`. `cmux hooks setup [codex | --agent opencode]`. `~/.config/cmux/cmux.json` → `terminal.autoResumeAgentSessions` [V].
- CCManager worktree post-create hook env: `CCMANAGER_WORKTREE_PATH`, `CCMANAGER_WORKTREE_BRANCH`, `CCMANAGER_GIT_ROOT`, `CCMANAGER_BASE_BRANCH`. Config `~/.config/ccmanager/config.json` key `worktreeHooks.post_creation`; per-repo `.ccmanager.json`; `.worktreeinclude`; `~/.config/ccmanager/sessions.json` for restore [V].
- Conductor env (workspace terminals, agents, scripts): `CONDUCTOR_WORKSPACE_NAME`, `CONDUCTOR_WORKSPACE_PATH`, `CONDUCTOR_ROOT_PATH`, `CONDUCTOR_DEFAULT_BRANCH` (local), `CONDUCTOR_PORT` (10-port range, local), `CONDUCTOR_IS_LOCAL`, `CONDUCTOR_SESSION_ID` (agent processes only) [V]. Config: `conductor.json` (older) vs `.conductor/settings.toml [scripts]` (newer) [S].
- Superset: `.superset/config.json` `setup`/`teardown`/`run` script arrays [V]. Env vars documented: `SUPERSET_API_KEY`, `SUPERSET_HOME_DIR`, `SUPERSET_HOME`, `SUPERSET_VERSION`, `GH_TOKEN`/`GITHUB_TOKEN` [V]. Hooks' own env var name: not documented [U].
- Agent Deck worktree scripts: `.agent-deck/worktree-setup.sh`, `.agent-deck/worktree-destruction.sh`; consent in `~/.local/share/agent-deck/worktree-script-consent.json`; `[worktree] run_repo_scripts = prompt|never|always` [V].
- Zed ACP: `zed: acp registry`; `agent_servers` custom `command`/`args`; `dev: open acp logs` [V]. ACP transport details: not fetched [U].

## 5. Ranked feature ideas for repodeck

Ranked by value / effort. "Repodeck today" = what the code does now, from grep of `src/`.

| # | Feature | Value | Effort | Repodeck today |
|---|---|---|---|---|
| 1 | Subscription-safe launch: strip `ANTHROPIC_API_KEY`/`ANTHROPIC_AUTH_TOKEN` from agent pane env; optional per-project `CLAUDE_CONFIG_DIR` | High | S | None [V] |
| 2 | Official subscription usage (5h / 7d %) via `statusLine` JSON, injected with the existing `--settings` payload | High | S-M | Token sums from transcripts [V] |
| 3 | Session history follows worktrees: copy Claude transcripts on worktree create; add Codex history reader | Med-High | S (Claude) / M (Codex) | Claude history by repo path only [V] |
| 4 | Codex adapter: `notify` (done), TUI `notification_condition` (waiting), hooks where available | High | M | Claude-only hooks [V] |
| 5 | OpenCode adapter via project/global plugin writing events to the same event file | High | M | None [V] |
| 6 | Show `last_assistant_message` (Stop) and `message` (Notification) in the inbox | Med | S | Status only, no text [V] |
| 7 | Worktree teardown script + `CONDUCTOR_PORT`-style port env + `CCMANAGER_*`-style env | Med | S | Setup script, `.worktreeinclude`, removal exist [V] |
| 8 | CI guard for "CLI-only": deny HTTP/LLM crates, grep for API hosts | Med (enforces differentiator) | XS | Clean today, not enforced [V] |
| 9 | In-app allow/deny via `PermissionRequest` hook (opt-in, never default auto-allow) | Med | M-H | Shows `Approve?` only [V] |
| 10 | Agent-facing control: documented CLI/MCP for spawning worktree tasks | Med | M | `REPODECK_SOCKET` + `repodeck` CLI exist [V] |
| 11 | Launch presets with per-agent args / model / effort | Low-Med | S | Single configurable agent command [V] |
| 12 | Per-session MCP toggling | Med | M-H | None [V] |
| 13 | ACP transport for agents that support it | High long-term | L-H | PTY only [V] |
| 14 | Parent/child orchestration ("conductor") | Med | H | None; defer (YAGNI) |

### Details for top items

**1. Subscription-safe launch.**
- Mechanism: prefix the agent command, e.g. `env -u ANTHROPIC_API_KEY -u ANTHROPIC_AUTH_TOKEN claude ...`. Apply after the user's zsh rc has run, since rc files can export the key. Hook point: `with_hooks` in `src/agent_events.rs`, call site `src/session_pool.rs:61`.
- Do not unset `CLAUDE_CODE_OAUTH_TOKEN` by default. It outranks `/login` [V].
- Competitor evidence: none found that scrubs env. Claude's docs prescribe `unset` [V].

**2. statusLine usage.**
- Pass a `statusLine` command in the inline settings JSON. The command writes `rate_limits` and `cost` to the event file or socket.
- Caveats: Pro/Max only; `rate_limits` appears after the first response; `cost.total_cost_usd` is an estimate at list price [V].
- Unverified: whether `--settings` `statusLine` overrides the user's own `statusLine` for that session [U]. Competitor: Agent Deck's `hooks install` wires `statusLine` into each account's `settings.json` [S].

**3. Session history across worktrees.**
- CCManager copies `~/.claude/projects/<src-encoded>/` to `<dst-encoded>/` on worktree create. Non-fatal on failure [V].
- Codex: sessions under `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`; resume with `codex resume <id>` [S/U].

**4. Codex adapter.**
- Options: (a) `notify` in `~/.codex/config.toml`, global and invasive; per-launch via `CODEX_HOME` breaks `auth.json` unless linked [U]; (b) `-c` config overrides, not verified [U]; (c) repo `.codex/hooks.json`, trusted projects only, pollutes git [V].
- Competitors: Agent Deck `agent-deck codex-hooks install` [S]; Warp sets `notification_condition = "always"` in `~/.codex/config.toml` [S]; cmux `cmux hooks setup codex` [S].

**5. OpenCode adapter.**
- Plugin file in `.opencode/plugins/` (project; exclude via `.git/info/exclude`) or `~/.config/opencode/plugins/` (global) [V].
- Events: `session.status` / `session.idle` (done), `permission.asked` (waiting), `session.error` (error) [V]. Map these to the existing `AgentStatus` enum.
- Competitor: Warp npm plugin `@warp-dot-dev/opencode-warp` via `opencode.json` `plugin` array [S].

**7. Teardown and env.**
- Mirror Superset's `teardown` array and Agent Deck's destruction script.
- Ask before running repo scripts. Agent Deck: `run_repo_scripts = prompt|never|always` with a consent file [V]. Keep this, since repo scripts are untrusted code.

**9. In-app approve / deny.**
- Mechanism: the `PermissionRequest` command hook blocks, waits for a decision from repodeck's socket, then prints `hookSpecificOutput.decision.behavior`. Exit 2 is not honored here [V].
- Risk: a blocked hook stalls the agent if repodeck is gone. Needs timeout → fall back to pane prompt.
- Competitor: CCManager's auto-approve uses a Claude Haiku verifier (experimental) [V]. Avoid as default.

## 6. Notes for decisions

- Keep the inline `--settings` hook injection. It is less invasive than cmux, Emdash, Superset, and Warp, which edit global agent config [V].
- Vendor hook names and payloads change often. Pin the minimum Claude/Codex versions and degrade to the PTY/OSC path when hooks are absent.
- Emdash's approach (marker-tagged global entries, removable) is the fallback if per-launch injection is impossible for an agent [V].

## 7. Unresolved questions

1. Does `claude --settings` honor `statusLine` and the `PermissionRequest` decision? Verify with `claude --help` and a test run [U].
2. Codex: `hooks.json` event names, minimum version, `-c` override support, and whether `OPENAI_API_KEY` beats ChatGPT login [U].
3. OpenCode: CLI flags for resume/session ID and config dir; ACP mode (`opencode acp`?) [U].
4. Claude `Notification` stdin field names beyond `message` (`notification_type`, `title`) [U].
5. Superset: worktree default path, hook env var names, and how status hooks are installed [U].
6. Conductor: which settings file current versions read (`conductor.json` vs `.conductor/settings.toml`); agent auth and notification docs not fetched [U].
7. Sculptor: current status and container sync details. Vendor sync-doc page returned 404 [U].
8. Nimbalyst license (MIT vs closed) [U]. Zed license [U].
9. Anthropic's Apr 2026 third-party harness billing change: primary notice not fetched [U].
10. Opcode usage-dashboard data source beyond `~/.claude/projects/` [U].

## 8. Sources

Primary (fetched):
- Claude Code hooks: https://code.claude.com/docs/en/hooks
- Claude Code auth: https://code.claude.com/docs/en/authentication
- Claude Code legal: https://code.claude.com/docs/en/legal-and-compliance
- Claude Code statusLine: https://code.claude.com/docs/en/statusline
- Codex config (advanced): https://learn.chatgpt.com/docs/config-file/config-advanced
- OpenCode plugins: https://opencode.ai/docs/plugins/
- Zed external agents: https://zed.dev/docs/ai/external-agents
- Warp CLI agents: https://docs.warp.dev/agents/cli-agents/overview.md
- Warp Claude Code: https://docs.warp.dev/agents/cli-agents/claude-code
- Warp Claude plugin repo: https://github.com/warpdotdev/claude-code-warp
- Conductor env vars: https://conductor.build/docs/reference/environment-variables
- Conductor docs: https://conductor.build/docs
- cmux notifications: https://cmux.com/docs/notifications
- cmux repo: https://github.com/manaflow-ai/cmux
- Superset agent status: https://docs.superset.sh/agent-status
- Superset env vars: https://docs.superset.sh/cli/env-vars
- Claude Squad README: https://github.com/smtg-ai/claude-squad/blob/main/README.md
- CCManager README: https://github.com/kbwo/ccmanager
- CCManager worktree hooks: https://github.com/kbwo/ccmanager/blob/main/docs/worktree-hooks.md
- Emdash repo: https://github.com/generalaction/emdash
- Emdash providers: https://emdash.com/docs/providers
- Agent Deck repo: https://github.com/asheshgoplani/agent-deck
- Crystal repo: https://github.com/stravu/crystal
- Vibe-kanban repo: https://github.com/BloopAI/vibe-kanban
- Vibe-kanban shutdown: https://vibekanban.com/blog/shutdown
- Terragon OSS: https://github.com/terragon-labs/terragon-oss
- Claudia/Opcode: https://github.com/getAsterisk/claudia
- Sculptor: https://imbue.com/sculptor
- Imbue features: https://docs.imbue.com/features

Secondary (used only where tagged [S]):
- Crystal → Nimbalyst: https://www.sitepoint.com/nimbalyst-the-visual-workspace-for-building-with-claude-code-and-codex/
- Conductor marketing / third-party: https://www.morphllm.com/conductor-ai-coding
- Claude Squad overview: https://deepwiki.com/smtg-ai/claude-squad
- Anthropic harness policy (news): https://venturebeat.com/ai/anthropic-cracks-down-on-unauthorized-claude-usage-by-third-party-harnesses/
- Codex hooks/notify (third-party): https://codex.danielvaughan.com/2026/04/13/codex-cli-notification-pipeline-osc9-hooks-alerts/
- Sculptor third-party review: https://devtune.ai/verticals/coding-agent-orchestration/sculptor/pricing
- Superset third-party overview: https://yuv.ai/blog/superset
- Codex config mirror: https://forge.lthn.ai/core/core-agent-ide/raw/branch/dev/docs/config.md
