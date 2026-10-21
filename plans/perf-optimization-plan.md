# Phương án tối ưu hiệu năng Repodeck

Ngày: 2026-10-22. Dựa trên: đo đạc trong PR #1 (`perf/session-load-performance`), rà soát toàn bộ `src/`, và các phiên làm việc trước (warp → repodeck): mục tiêu lặp lại nhiều nhất là **chat/scroll Claude mượt như Warp**, kế đến là paste ảnh, kéo file từ Finder, icon trạng thái tab, usage 5 h/7 ngày, lịch sử theo worktree.

Nguyên tắc: chỉ tối ưu chỗ đã đo thấy tốn; không cache thêm cho phần vẽ terminal (0.6 ms/frame, không đáng).

## 1. Đã vào PR #1

| # | Việc | Hiệu quả đo được |
|---|---|---|
| 1 | Một `try_lock` terminal mỗi frame, giữ frame cũ ≤ 50 ms khi PTY đang parse | p50 chờ lock 1.4 ms → 0 |
| 2 | Trả lời query terminal (XTVERSION, DECRQM, OSC 11…) ngay, không qua throttle | pane ẩn: trễ ≤ 1 s → 0 |
| 3 | Paste ảnh encode PNG ở luồng nền, phím gõ xếp hàng sau đường dẫn ảnh | UI đứng 0.3–1 s → 0 |
| 4 | Pane Claude chạy `zsh -c` trên env đã chụp lúc mở app | 0.35–0.4 s/phiên → 0 |
| 5 | Lịch sử Claude: cache tiêu đề theo size+mtime, `Arc<[Session]>` | hết đọc lại 262 MB mỗi 15 s |
| 6 | Spinner 60 Hz, "Starting Claude…" khi pane trống | — |
| 7 | **`logic` chạy trước `ui` mỗi frame** (eframe 0.37): mọi quét pane chạy 2 lần và báo cáo bị tiêu thụ với `watching=false` → thông báo macOS bắn cả khi đang nhìn pane | sửa: bookkeeping chỉ ở `logic`, 1 lần/frame |
| 8 | Find bar đếm lại toàn bộ scrollback mỗi frame khi ô mất focus/không khớp | sửa |

## 2. Xung đột / trùng lặp còn lại (theo mức ảnh hưởng)

**Ảnh hưởng hiệu năng thấy được**
- **Spinner giữ cả cửa sổ ở 60 Hz** khi *bất kỳ* tab nào busy, kể cả pane ẩn (`sidebar_rows.rs:78,178`, `tab_bar.rs:140`). "Busy" = hook `Working` ∨ glyph `◐/◑` ∨ có output 1.5 s gần nhất. Shell chạy `tail -f`/dev server cũng tính. Chi phí ~4–6 % một nhân khi Claude làm việc nhiều phút.
- **Library panel đọc file hệ thống mỗi frame** (`library_panel.rs:36,45` → `library.rs`): `read_dir` 2 thư mục skills, `read_to_string` từng `SKILL.md`, parse `~/.claude.json`. Chỉ khi panel Library mở.
- **Files panel** không ảo hoá hàng, mỗi thư mục mở list lại mỗi 3 s dù `notify` đã có.
- **Editor**: syntect highlight cả file mỗi lần gõ, hash toàn văn mỗi frame; Markdown preview parse lại mỗi frame.
- Pane menu / palette / repo menu đọc `.git` hoặc lock terminal mỗi frame khi mở (`pane_chrome.rs:31,37`, `palette.rs:126`, `sidebar_rows.rs:277`).

**Trùng trạng thái**
- Trạng thái agent có 3 nguồn OR nhau (hook, glyph title, cửa sổ output) + 2 override cục bộ (`note_user_input`, `acknowledge_done`). Hook chỉ gắn khi token lệnh đúng bằng `claude` (`agent_events.rs:62-74`); `agent_command=/path/claude` hoặc `npx …` → status `Idle` mãi, "Starting Claude…" treo tới 30 s.
- Thông báo: hook `Notification` ("Your turn") và OSC 9 từ chính Claude ("Notification") cùng rơi vào inbox, label khác nhau nên không dedupe → 2 notice. `repodeck notify` CLI in OSC 9 ra stdout của hook (bị nuốt) thay vì dùng socket.
- Usage và History cùng đọc `~/.claude/projects/<repo>` nhưng cache riêng, và đều key theo `repo.path`: agent mở ở thư mục con / sau `cd` không có usage, không có history.
- 3 cơ chế phát hiện thay đổi file: `notify` (git), TTL 3 s (file tree), stat 1 s (editor). Diff tab chỉ làm mới khi *danh sách* file git đổi, sửa tiếp file đã modified → diff cũ.
- 2 hàng đợi paste: `pending_paste` (app, 1 slot, timer 2.5 s cố định) và `session.queued` (ảnh). `insert_prompt`, Finder drop, `answer_approval` ghi thẳng, có thể vượt lên trước đường dẫn ảnh.
- `PrWatcher::set_paths` không dedupe; mỗi lần đóng tab reset timer 90 s → poll PR bị dời vô hạn khi đóng tab liên tục.
- Font cài 2 lần lúc khởi động (atlas dựng lại 2 lần).

## 3. Lộ trình

### Giai đoạn A — nhỏ, chắc, làm ngay (≈ 1 ngày)
1. Spinner: 60 Hz cho tab đang hiện; tab có pane ẩn 10 Hz. Bỏ `is_busy()` khỏi điều kiện spinner của pane Shell không có agent (chỉ giữ cho Agent).
2. Library: cache theo `(repo, mtime thư mục)` hoặc TTL 2 s; đọc ở luồng nền như file tree.
3. Hook agent: so khớp `claude` theo basename của token đầu (`/x/claude`, `claude@…` qua npx) để status và "Starting Claude…" đúng.
4. Dedupe thông báo theo `(pane, 3 s)` bất kể label; CLI `repodeck notify` đi qua socket.
5. `PrWatcher::set_paths` dedupe như `GitWatcher`; không reset timer khi path không đổi.
6. Font: bỏ `apply` trong `new`, chỉ `sync_font`.
7. Pane/palette/repo menu: tính `is_worktree`, `selection_to_string`, `last_output` một lần khi mở menu.

### Giai đoạn B — đúng với lịch sử yêu cầu (≈ 2–3 ngày)
8. **Cuộn Claude mượt**: thêm setting "Claude renderer: alt-screen (NO_FLICKER) / scrollback". Chế độ scrollback: Repodeck đặt `CLAUDE_CODE_NO_FLICKER=0` cho pane Agent, cuộn theo pixel trên scrollback của Repodeck (đã có), không còn phụ thuộc Claude vẽ lại. Đánh đổi: Claude có thể nhấp nháy khi resize; cần thử trên session dài trước khi đổi mặc định.
9. **Usage 5 h / 7 ngày** từ JSON statusLine của Claude thay cho cộng token transcript: bỏ `claude_usage.rs` (stat 30 s + đọc file), hiển thị % hạn mức đúng gói Pro/Max.
10. **Key usage/history theo cwd của pane** (fallback `repo.path`), và **lịch sử theo worktree** (gộp transcript của repo chính khi pane ở worktree).
11. Gộp `pending_paste` vào `session.queued`: mọi ghi do người dùng (prompt, drop, approval) đi qua một hàng đợi; bỏ timer 2.5 s, thay bằng "gửi khi hook `SessionStart` tới".
12. Diff tab: làm mới theo mtime file đang xem; file tree invalidate từ `notify` thay vì TTL.

### Giai đoạn C — chỉ khi dùng editor nhiều
13. Editor: highlight theo dòng + cache `LayoutJob` theo generation; Markdown preview cache theo hash văn bản.
14. Files panel: `ScrollArea::show_rows`.

### Không làm
- Cache frame terminal theo dirty flag, tối ưu parser: đã đo không đáng (0.6 ms/frame, 107 MB/s).
- Nới giới hạn 40 dòng/s khi chuyển wheel → phím mũi tên: Claude sẽ hiện cảnh báo "scroll wheel is sending arrow keys" (đã gặp trong phiên trước).

## 4. Cần bạn quyết
- Giai đoạn B mục 8: thử `CLAUDE_CODE_NO_FLICKER=0` trên một session dài để chọn mặc định.
- Spinner pane ẩn 10 Hz (tiết kiệm CPU) hay giữ 60 Hz (mượt tuyệt đối).
- Có bỏ hẳn cách đếm token từ transcript khi đã có statusLine không.
