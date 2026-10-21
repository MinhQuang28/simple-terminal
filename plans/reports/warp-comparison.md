# Repodeck vs Warp: hiệu năng và tính năng cho lập trình viên

Ngày: 2026-10-21. Nguồn: đọc toàn bộ `src/` của Repodeck (~5.850 dòng) và mã Warp tại
`../warp` (app/, crates/warpui*, crates/warp_terminal). Đường dẫn Warp tính từ thư mục `warp/`.

## 1. Hiệu năng: Warp làm gì, Repodeck đang làm gì

| Chủ đề | Warp | Repodeck hiện tại | Khoảng cách |
|---|---|---|---|
| Đọc PTY | Thread riêng, buffer 256 KiB, parse tối đa 64 KiB mỗi lần giữ lock, `try_lock` rồi đọc tiếp nếu UI đang giữ, `FairMutexGuard::bump()` nhường lock sau mỗi chunk (`crates/warp_terminal/src/local_tty/event_loop.rs:201-287`) | alacritty EventLoop nguyên bản: 1 MiB buffer, 64 KiB mỗi lock, `lease()` ưu tiên thread PTY | Gần tương đương. Khác ở chỗ Repodeck giữ lock suốt quá trình layout chữ (`terminal/view.rs:71-75`) |
| Gộp vẽ lại | Kênh wakeup riêng, `throttle` 60 Hz gộp mọi wakeup trong chu kỳ thành một (`app/src/throttle.rs`), `notify()` chỉ đánh dấu view đó bẩn; AppKit gộp `setNeedsDisplay` thành một lần vẽ mỗi vsync | `RepaintGate` 60 Hz mỗi session, nhưng mỗi gate xin vẽ lại **toàn cửa sổ**; session ẩn vẫn xin 2 Hz mỗi session | N session bận tạo frame lệch pha nhau; session ẩn vẫn gây vẽ |
| Vẽ lưới | Không shaping: tra `char → glyph` qua `CellGlyphCache`, vẽ thẳng vào glyph atlas trên GPU (`app/src/terminal/grid_renderer.rs:466-889`). Box-drawing, block, powerline vẽ bằng rect/path, không dùng font (`grid_renderer/box_drawing.rs`) | Một `LayoutJob` cho mỗi run ASCII cùng màu; **mỗi ô non-ASCII là một galley riêng** (`terminal/render.rs:203-210`). Box-drawing của Claude Code (─│╭╮) tạo hàng trăm galley mỗi frame | Điểm nóng lớn nhất |
| Cache layout | Text layout cache 2 thế hệ, giữ đúng 1 frame (`crates/warpui_core/src/text_layout.rs`); cache metric font và `char→glyph` | Dựa hoàn toàn vào `GalleyCache` của epaint: vẫn hash toàn bộ `LayoutJob` mỗi frame; đổi cỡ chữ là miss toàn bộ | Chấp nhận được nếu giảm số galley |
| Tab ẩn | Không nằm trong cây element, không layout, không paint (`app/src/workspace/view.rs:22335`); vẫn parse PTY | Không vẽ nhưng vẫn drain event mỗi frame và xin vẽ lại 2 Hz/session | Bỏ xin vẽ lại từ session ẩn |
| Scrollback | 50.000 dòng, lưu dạng flat + RLE thuộc tính (`crates/warp_terminal/src/model/grid/flat_storage/`) | 10.000 dòng, cell đầy đủ của alacritty | Ổn cho hiện tại |
| Git | **Không poll**: fs watcher `notify` trên `.git/refs`, `.git/HEAD`, worktree, debounce 500 ms, throttle 5 s, bỏ qua thay đổi gitignored, chạy `git` ở thread nền và hủy lần chạy cũ (`app/src/code_review/git_repo_model/local.rs:93-170`) | Thread nền chạy `git status` tuần tự cho **mọi** repo mỗi 4 s, kể cả project đang thu gọn | Watcher + chỉ repo đang hiện |
| IO chặn UI | File tree build ở thread nền, lazy theo thư mục, giới hạn 200k file | Liệt kê file tree trên UI thread mỗi 3 s; `stat` mỗi frame khi editor mở; encode PNG, ghi state.json, `git rev-parse` đều đồng bộ | Đưa xuống thread nền |
| Idle | Blink chỉ khi editor focus và cửa sổ active; `repaint_after` một timer gần nhất mỗi cửa sổ; `build_scene` thoát sớm nếu không view nào bẩn | Blink chạy cả khi cửa sổ không focus; spinner 25 fps vẽ lại toàn cửa sổ khi có agent bận; timer 30 s; `track_activity` quét mọi pane mỗi frame | Giảm vẽ khi rảnh |

### Việc nên làm, theo thứ tự lợi ích / công sức

1. **Vẽ box-drawing và block bằng hình** (U+2500–257F, U+2580–259F, powerline E0B0–E0B3) thay vì font. Giải quyết ngay điểm nóng với giao diện Claude Code. Tham khảo `grid_renderer/box_drawing.rs`.
2. **Gộp non-ASCII vào run**: thay vì một galley mỗi ô, gom ô non-ASCII cùng kiểu vào một `LayoutJob` theo hàng, hoặc cache galley theo `(hash nội dung hàng, style)` qua các frame.
3. **Chụp snapshot dưới lock rồi vẽ ngoài lock**: copy các ô nhìn thấy vào `Vec` trong vài trăm micro giây, thả lock, rồi layout/paint. Thread PTY không còn bị chặn bởi layout chữ.
4. **Một gate vẽ lại toàn app**: session ẩn chỉ cập nhật trạng thái bận/tiêu đề, không xin vẽ. Sidebar đọc trạng thái ở nhịp 1 Hz là đủ.
5. **Git watcher**: `notify` trên `.git/HEAD`, `.git/refs/`, `.git/index` và worktree; debounce 500 ms, throttle 5 s; chỉ theo dõi repo đang hiện trong sidebar; chạy `git status` ở thread nền.
6. **IO nền**: file tree, `stat` của editor (dùng watcher hoặc 1 s), ghi state.json, encode PNG, `git rev-parse` chuyển sang thread nền.
7. **Idle**: dừng blink khi cửa sổ mất focus; spinner 12 fps; bỏ timer 30 s; `track_activity` chạy 4 Hz thay vì mỗi frame; `visible_panes()` không cấp phát `HashSet` mỗi frame.
8. Nhỏ hơn: `git.status()` clone toàn bộ `GitStatus` nhiều lần mỗi frame (`ui/sidebar.rs:61,123`, `app_layout.rs:20,48`); `AppState::repo()` tìm tuyến tính; tab bar dựng lại format/galley/icon mỗi frame.

## 2. Tính năng cho lập trình viên: Warp có, Repodeck chưa có

Chỉ liệt kê tính năng **làm được offline** (không cần server Warp). Thứ tự theo giá trị với app lấy Claude Code làm trung tâm.

### Nhóm A: bảng điều khiển agent (giá trị cao nhất, hợp định vị Repodeck)

- **Trạng thái agent qua hook, không đoán từ tiêu đề.** Warp cài plugin Claude Code (`warpdotdev/claude-code-warp`) để hook `SessionStart`, `PromptSubmit`, `ToolComplete`, `Stop`, `PermissionRequest`, `QuestionAsked`, `NeedsInput`, `IdlePrompt` phát **OSC 777** vào PTY; `CLIAgentSessionsModel` dịch thành InProgress / Success / Failed / Blocked (`app/src/terminal/cli_agent_sessions/`). Repodeck có thể làm tương tự bằng hooks trong `~/.claude/settings.json` in `\e]777;notify;<json>\a`, alacritty bắt OSC 777 rồi gán pill trạng thái cho tab: *Đang chạy / Xong / Cần duyệt / Cần nhập / Lỗi*. Thay thế hoàn toàn heuristics `tab_title.rs`.
- **Hộp thư thông báo + thông báo macOS**: gửi khi agent xong, cần duyệt, hoặc lỗi, **chỉ khi người dùng đang nhìn chỗ khác** (pane khác hoặc app khác); bấm vào nhảy đúng pane (`app/src/ai/agent_management/notifications/`). Thêm badge Dock đếm số agent đang chờ.
- **Sidebar như dashboard**: mỗi tab agent hiện pill trạng thái, nhánh git, `+N −M` diff của worktree (`app/src/workspace/view/vertical_tabs.rs`).
- **Tab config theo worktree**: tạo `git worktree` + nhánh mới rồi mở Claude trong đó, để chạy nhiều agent song song trên cùng repo không giẫm nhau (`app/src/tab_configs/`, `new_worktree_modal.rs`).
- **Diff viewer có comment gửi cho agent**: so với HEAD hoặc merge-base nhánh chính, comment từng dòng rồi gộp thành prompt gửi Claude (`app/src/code_review/`, `cli_agent.rs:531-599`). Repodeck đã có tab Changes, đây là bước tiếp theo tự nhiên.
- **Ô nhập giàu trên agent TUI**: editor nhiều dòng, chip ảnh đính kèm, menu slash/skill nổi trên Claude Code (`app/src/terminal/input/cli_agent.rs`). Công sức lớn, để sau.

### Nhóm B: terminal hằng ngày

- **Link**: nhận diện URL và `path:line:col` tồn tại tương đối cwd, ⌘-click mở trong editor ngoài (VS Code, Cursor, Zed, JetBrains) hoặc editor nội bộ đúng dòng; OSC 8 hyperlink (`app/src/terminal/view/link_detection.rs`).
- **Tìm trong terminal** (⌘F) với regex, phân biệt hoa thường.
- **Panes**: split lồng nhau với divider kéo được, zoom pane (⇧⌘Enter), điều hướng pane (⌘[ ⌘]), kéo size bằng phím, làm mờ pane không active.
- **Tabs**: đổi tên, kéo sắp xếp, màu tab theo thư mục, mở lại tab vừa đóng (undo-close stack), gửi input tới mọi pane.
- **Command palette** (⌘K): action, tab, repo, file, session Claude, kèm mục gần đây; keybinding tùy chỉnh có kiểm tra xung đột với PTY.
- **Shell integration nhẹ**: hook precmd/preexec của zsh phát OSC 133 để có *blocks*: ⌘↑/↓ nhảy giữa lệnh, copy output lệnh trước, tô màu exit code, thông báo "lệnh dài đã xong" theo ngưỡng thời gian. Warp bootstrap bằng heredoc + `eval` (`app/assets/bundled/bootstrap/zsh.sh`).
- **Tìm file toàn cục** bằng ripgrep trong panel Files.

### Nhóm C: emulation còn thiếu (ảnh hưởng trực tiếp Claude Code)

- **Mouse click/drag reporting**: hiện chỉ gửi wheel. Claude Code fullscreen bật `?1000/1002/1003` để click "Jump to bottom", click link, chọn text bằng chuột trong TUI. Cần gửi press/release/drag khi chương trình bật mouse mode (giữ selection nội bộ khi ấn Shift).
- **Trả lời OSC 10/11** (màu nền/chữ): Claude Code hỏi để chọn theme sáng/tối; hiện bị bỏ qua (`terminal/session.rs:209-214`).
- **Focus reporting** `?1004`: Claude Code bật để biết cửa sổ focus.
- **Italic, undercurl, underline màu**, bell (âm hoặc nhấp nháy), OSC 9/777 notify, kitty keyboard protocol, Option-as-Meta.
- **Scrollback cấu hình được**, copy-on-select, lưu kích thước cửa sổ.

## 3. Lộ trình đề xuất

1. **Hiệu năng (1–2 ngày)**: mục 1.1 → 1.7 ở trên. Đo trước/sau bằng frame time khi Claude Code vẽ lại toàn màn hình.
2. **Agent dashboard (2–3 ngày)**: OSC 777 từ hooks, pill trạng thái, thông báo macOS + badge, mouse click reporting, trả lời OSC 10/11, focus reporting.
3. **Năng suất (theo từng tính năng)**: link + mở editor, ⌘F, palette, pane model mới, quản lý tab, diff viewer, worktree tab.

## 4. Trạng thái thực hiện (2026-10-21)

Đã làm toàn bộ lộ trình trên, trừ các mục ghi rõ bên dưới.

### Hiệu năng (xong)
- Box-drawing/block/powerline vẽ bằng hình (`terminal/box_drawing.rs`, bảng `box_arms.rs`); non-ASCII vừa ô gộp vào run (`render.rs`, cache `FITS_CELL`).
- Snapshot ô dưới lock rồi vẽ ngoài lock (`terminal/snapshot.rs`).
- `RepaintGate` dùng chung mọi session: lane hiện 60 Hz, lane ẩn 1 Hz (`terminal/repaint.rs`).
- Git: fs watcher `notify` trên worktree + `.git/{index,HEAD,refs,packed-refs}`, debounce 500 ms, tối thiểu 2 s, fallback 60 s, `--no-optional-locks` (`git_watcher.rs`); trạng thái trả về `Arc` (không clone mỗi frame).
- File tree liệt kê ở thread nền sau lần đầu; editor `stat` 1 s/lần; `state.json` ghi ở thread riêng (`store::Saver`).
- Idle: không blink khi cửa sổ mất focus, spinner 12 fps, timer 60 s.

### Agent dashboard (xong)
- Hook Claude Code qua `claude --settings "$REPODECK_AGENT_HOOKS"` (tự chèn vào lệnh `claude`), hook `(cat; echo) >> $REPODECK_AGENT_EVENTS`; tail file theo pane (`agent_events.rs`, `event_log.rs`). Trạng thái: Working / Done / Your turn / Approve? / Error → pill ở sidebar, chấm màu ở tab bar.
- `session_id` từ hook được lưu vào pane để tab khôi phục đúng hội thoại (`--resume`).
- Thông báo macOS (`notify-rust`) khi pane không hiện hoặc cửa sổ mất focus; badge Dock đếm agent chờ (`attention.rs`). *Chưa có*: bấm thông báo nhảy đúng pane (notify-rust trên macOS không có callback).
- Mouse reporting SGR/X10 (press/release/motion, Shift giữ chọn cục bộ) (`terminal/mouse.rs`); trả lời OSC 10/11/4 theo theme; focus in/out (mode 1004); undercurl/double underline; bell nháy viền. *Chưa có*: italic (egui không tổng hợp nghiêng).

### Năng suất (xong)
- ⌘-click URL mở trình duyệt; đường dẫn `file:line:col` (và OSC 8) mở editor trong app tại đúng dòng, hoặc lệnh ngoài cấu hình ở Settings → Terminal → Links (`terminal/links.rs`).
- ⌘F tìm regex (smart case), Enter/⇧Enter, đếm kết quả, tô sáng (`terminal/find.rs`).
- ⌘K command palette với fuzzy search lệnh + chuyển tab/repo (`ui/palette.rs`).
- Split lồng nhau, kéo divider, zoom pane ⇧⌘↩, điều hướng ⌘⌥←→↑↓ (`pane_layout.rs`).
- Tab: đổi tên (menu chuột phải), kéo sắp xếp, ⌘⇧[ ], ⌘⌥⇧←→ di chuyển, ⌥⌘T mở lại tab đã đóng.
- Diff viewer trong Changes (bấm file), nút *Send to Claude* dán diff vào pane Claude của repo (`diff.rs`, `ui/diff_view.rs`).
- Worktree mới từ menu repo / palette (`git worktree add`, thêm repo cùng project).
- Shell integration zsh (ZDOTDIR wrapper, `shell_integration.rs`): mốc prompt (⌘↑/⌘↓), thông báo lệnh > 15 s, badge `exit N`, *Copy last output*, cwd thật cho link. *Chưa có*: bash/fish; mốc prompt lệch khi history đầy (10k dòng) do alacritty không đếm dòng đã trôi.
- Settings: scrollback, Option as Meta, copy-on-select, lưu kích thước cửa sổ (`persist_window`). *Chưa có*: tùy biến keybinding.
