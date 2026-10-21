# Khảo sát: app cùng loại có tính năng AI/agent nâng cao gì

Ngày: 2026-10-21. Nguồn: trang chính thức và docs của Warp, Claude Code desktop, Conductor, cmux, Emdash, Nimbalyst, danh sách awesome-agent-orchestrators (270 công cụ). Chỉ ghi tính năng có trong tài liệu, không suy đoán.

## 1. Bức tranh chung

Thị trường "terminal / workspace cho coding agent" chia 3 lớp:
- **Terminal gốc + agent** (Warp, cmux, Calyx, agterm): terminal thật, sidebar dọc theo workspace, badge "cần bạn", notification mailbox, API cho agent hook.
- **Workspace per-task trên worktree** (Conductor, Emdash, Nimbalyst, Claude Code desktop): mỗi task một worktree, diff review có comment, PR + CI ngay trong app, lịch chạy định kỳ, nhận issue từ Linear/Jira/GitHub.
- **Orchestrator** (Claude Squad, agent-deck, gastown, ralph-*): nhiều agent chạy nền, mailbox giữa agent, task board, merge queue.

Repodeck đang ở lớp 1 với một phần lớp 2 (worktree add, diff view, hooks status). Những tính năng dưới đây là thứ các app kia có mà Repodeck chưa có.

## 2. Tính năng theo nhóm

### A. Trạng thái và thông báo của agent
| Tính năng | Ai có | Repodeck |
|---|---|---|
| Notification mailbox: panel chuông, lọc All / Unread / Errors, mở tab là đánh dấu đã đọc | Warp | **Có** (2026-10-21): chuông ở top bar, popup All/Unread, Mark all read, click → focus pane, xem pane = đã đọc |
| Toast trong app cho 3 trạng thái: xong, cần input, lỗi (tối đa 2 toast) | Warp | Chưa, chỉ macOS notification |
| Notification ring quanh pane cần chú ý, nhận OSC 9/99/777 và lệnh CLI `notify` | cmux | **Có một phần**: nhận OSC 9 và OSC 777 từ mọi CLI, lệnh `repodeck notify <text>` có sẵn trên PATH của pane; chưa có ring quanh pane |
| "Needs you" sidebar: nhóm session đang chờ người dùng lên đầu | Claude Code desktop | Chưa |
| Approval inbox: gom mọi permission prompt của mọi agent vào một chỗ để duyệt | Calyx, octomux, tuios | **Có**: mục Approvals trong popup chuông (badge cam), hiện "Bash: npm test", Allow/Deny gửi Enter/Esc vào pane |
| Push lên điện thoại khi cần duyệt | Claude Code desktop, agent-session-manager (ntfy) | Chưa |

### B. Worktree và vòng đời task
| Tính năng | Ai có | Repodeck |
|---|---|---|
| Một click tạo workspace = worktree + branch + terminal + diff + checks | Conductor, Emdash, Nimbalyst | Có worktree add, phần còn lại rời rạc |
| Setup script / run script per repo (cài deps, chạy dev server khi tạo worktree) | Conductor, Emdash | **Có**: menu repo → "Worktree setup…", chạy trong terminal đầu tiên của worktree mới |
| `.worktreeinclude`: copy file gitignored (.env) sang worktree mới | Claude Code desktop | **Có** |
| Auto-archive session khi PR merge/close; xoá worktree kèm | Claude Code desktop | **Có một phần**: tag "merged" trên branch row, "Remove worktree…" (tuỳ chọn xoá nhánh đã merge); chưa tự động theo PR |
| Kanban tự chuyển cột In Progress / Waiting / Review theo trạng thái agent | Nimbalyst, Vibe Kanban | Chưa |

### C. Review, PR, CI trong app
| Tính năng | Ai có | Repodeck |
|---|---|---|
| Diff file-by-file, click dòng để comment, ⌘Enter gửi mọi comment cho agent sửa | Claude Code desktop, agent-manager, alas | **Có** (2026-10-21): gutter trái của diff, click dòng → ghi chú (⌘↩ lưu), danh sách comment + "Send comments to Claude" gộp một prompt |
| `/code-review` ra danh sách findings, nút "Fix this one" / "Apply fixes" | Claude Code desktop | Chưa |
| PR status + CI check trong app; Auto-fix khi CI đỏ; Merge when ready | Claude Code desktop, Conductor, Emdash | **Có một phần**: badge `#N` trên branch row màu theo CI (poll `gh pr view` 90 s), hover liệt kê check, click mở PR; menu repo "Create pull request…", "Fix failing CI with Claude" (gửi `gh run view --log-failed`). Chưa có merge |
| Accept / reject từng hunk thay đổi của agent inline | Nimbalyst | Chưa |

### D. Skills, prompts, MCP
| Tính năng | Ai có | Repodeck |
|---|---|---|
| Library: prompt tái dùng, cài skill, sync MCP server sang các agent | Emdash, Warp Drive | **Có một phần**: tab Library ở panel phải: prompt (thêm/sửa/xoá, click dán vào pane, ⇧⌘P chọn qua palette), skill từ `~/.claude/skills` và `.claude/skills` với bật/tắt (đổi tên SKILL.md), MCP từ `.mcp.json` và `~/.claude.json`. Chưa cài skill từ xa |
| UI Customize: connectors (MCP) có wizard, skills hiện trong menu `/`, plugin browser bật/tắt | Claude Code desktop | Chưa |
| Profiles: bộ quy tắc + quyền mặc định, chọn theo tác vụ | Warp | Chưa |
| Skill dạng YAML/lockfile để cài lặp lại | skillfold, agent-runbook | Chưa |

### E. Lịch chạy và intake
| Tính năng | Ai có | Repodeck |
|---|---|---|
| Recurring runs: bug sweep, release prep, review định kỳ, có lịch sử run + diff stats | Emdash, Claude Code desktop (scheduled tasks) | Chưa |
| Nhận issue từ Linear / Jira / GitHub / GitLab / Notion / Asana làm context | Emdash, Contrabass, cyrus | Chưa |
| Agent chạy theo sự kiện Slack / GitHub Actions (Oz) | Warp | Chưa |

### F. Browser và preview
| Tính năng | Ai có | Repodeck |
|---|---|---|
| Browser pane cạnh terminal, agent chụp màn hình / click / inspect để tự verify | Claude Code desktop, cmux, Emdash | Chưa |
| `.claude/launch.json` khai báo dev server, nút start/stop | Claude Code desktop | Chưa |
| Visual editor cho markdown, mockup, diagram, CSV, data model | Nimbalyst | Chỉ có Markdown preview |

### G. Điều khiển và tự động hoá
| Tính năng | Ai có | Repodeck |
|---|---|---|
| CLI + socket API để script hoặc để hook của agent điều khiển app (mở tab, notify, focus) | cmux, agterm, Rove | **Có**: socket `repodeck.sock` trong data dir; `repodeck open <dir>`, `claude <dir>`, `focus [pane]`, `list`, `notify`; pane có `$REPODECK_PANE`, `$REPODECK_SOCKET` |
| Remote control từ web / mobile, session tiếp tục khi đóng app | Claude Code desktop, Happy, Conductor Cloud | Chưa |
| Chạy agent qua SSH trên máy khác, vẫn có worktree | Emdash, Claude Code desktop, clave | Chưa |
| Side chat `/btw` (⌘;) hỏi phụ không làm bẩn context chính | Claude Code desktop | Chưa |
| Cross-session messaging: agent liệt kê / đọc / nhắn session khác | Claude Code desktop, hcom, ReevesAgents | Chưa |
| Usage / cost: token và tiền theo session, quota gauge | Helicon, termany, clave, Warp (credit) | **Có một phần**: status bar hiện ↑ input ↓ output của phiên Claude đang xem (đọc transcript JSONL, tăng dần); chưa quy ra tiền |
| Session snapshot per-turn, rewind, audit ledger | Agent Console | Chưa |

## 3. Gợi ý cho Repodeck, theo chi phí và giá trị

### Nhỏ, giá trị cao — đã làm 2026-10-21
Cả 5 mục dưới đã có trong code (`inbox.rs`, `terminal/osc_notify.rs`, `worktree.rs`, `app_worktree.rs`, `claude_usage.rs`). Còn thiếu: ring quanh pane, auto-archive theo PR, quy đổi tiền.
1. **Notification mailbox** trong sidebar: panel chuông, danh sách sự kiện từ hook (Done / Needs input / Error) theo thời gian, lọc Unread, click → focus pane, mở tab = đã đọc. Dữ liệu đã có trong `agent_events.rs`.
2. **Nhận OSC 9 / 777** để mọi CLI (Codex, Aider, script build) đều báo "cần chú ý", không chỉ Claude Code. Thêm lệnh `repodeck notify` cho script.
3. **Setup script per repo** chạy khi tạo worktree (copy `.env`, `npm i`); đọc `.worktreeinclude` giống Claude Code desktop.
4. **Remove worktree** + auto-archive khi branch đã merge.
5. **Usage per session**: đọc transcript JSONL của Claude Code đã có sẵn (`claude_history.rs`) để tính token in/out và hiện cạnh tab.

### Vừa — đã làm 2026-10-21
Cả 5 mục (`ui/diff_comments.rs`, `pr_status.rs` + `app_pr.rs`, `library.rs` + `ui/library_panel.rs`, `socket_api.rs`, approvals trong `ui/inbox_panel.rs`). Thiếu: merge PR trong app, cài skill từ registry.
6. **Diff review có comment**: click dòng → ghi chú, nút "Send comments to Claude" gộp thành một prompt gửi vào pane agent của repo.
7. **PR + CI trong app** qua `gh pr status` và `gh run list`: badge trên branch row, nút "Open PR", "Auto-fix CI" gửi log lỗi cho Claude.
8. **Library**: prompt snippet (⌘⇧P chèn vào pane), danh sách skill trong `~/.claude/skills` và project `.claude/skills` với bật/tắt, danh sách MCP từ `.mcp.json`.
9. **Socket API / CLI**: `repodeck open <path>`, `repodeck focus <pane>`, `repodeck notify`, để hook và script bên ngoài điều khiển app.
10. **Approval inbox**: gom PermissionRequest của mọi pane, duyệt từ một panel thay vì nhảy từng tab.

### Lớn (tuần trở lên)
11. Browser pane nhúng (WKWebView qua objc2) để agent preview và chụp màn hình.
12. Scheduled runs: cron chạy `claude -p` với prompt lưu sẵn trong worktree, lịch sử run + diff stats.
13. Issue intake từ GitHub/Linear: chọn issue → tạo worktree + mở Claude với nội dung issue.
14. Remote control qua web/mobile.

## 4. Nguồn
- Claude Code desktop: https://code.claude.com/docs/en/desktop
- Warp agent notifications: https://docs.warp.dev/agents/capabilities/agent-notifications/ ; Agents in Warp: https://docs.warp.dev/getting-started/readme/agents-in-warp ; Running multiple agents: https://docs.warp.dev/guides/agent-workflows/running-multiple-agents-at-once-with-warp/
- Conductor: https://www.conductor.build và https://www.conductor.build/docs/guides/git-worktrees/run-claude-code-with-git-worktrees
- cmux: https://vibecoding.app/tools/cmux
- Emdash: https://emdash.com/
- Nimbalyst: https://www.sitepoint.com/nimbalyst-the-visual-workspace-for-building-with-claude-code-and-codex/
- Danh sách 270 orchestrator: https://github.com/andyrewlee/awesome-agent-orchestrators
- Tổng hợp: https://munderdiffl.in/blog/best-claude-code-multi-agent-tools/ , https://dev.to/stravukarl/best-tools-for-managing-parallel-ai-coding-agents-in-2026-14l8
