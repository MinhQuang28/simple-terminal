# Repodeck review: UI/UX, tính năng, hiệu năng

> Cập nhật 2026-10-21: các mục đánh dấu **[đã sửa]** đã vào code; **[một phần]** còn việc. Tóm tắt ở mục 6.

Ngày: 2026-10-21. Phạm vi: toàn bộ `src/` sau đợt làm lại sidebar, theme Warp Dark, Markdown preview. Review đọc code (không profile runtime). Bổ sung cho `warp-comparison.md`, không lặp lại các mục đã có ở đó.

## 1. Hiệu năng

### Nguồn repaint liên tục
- Idle đã được gate đúng: không có `request_repaint()` vô điều kiện, chỉ có timer 60 s (`app.rs:324`).
- **Spinner 12.5 Hz** (`widgets.rs:170`, 80 ms) là nguồn tốn CPU chính. Spinner hiện khi `tab_busy` đúng: agent `Working` (kéo dài nhiều phút), title `◐/◑`, hoặc có output trong 1.5 s (`session.rs:194`). Sidebar vẽ spinner cho mọi tab của mọi repo, nên một phiên Claude đang chạy ở pane ẩn vẫn làm cả cửa sổ repaint 12.5 Hz.
- Cursor blink ~1.9 Hz khi pane focus (`view_chrome.rs:87`).
- Output PTY: 60 Hz cho pane hiện, 1 Hz cho pane ẩn (`repaint.rs:11`).

### Cao
1. **[một phần]** **Terminal snapshot + layout lại toàn bộ mỗi frame** (`snapshot.rs:47-72`, `render.rs:147-197`). Mỗi frame cấp phát `Vec<Cell>` ~0.5 MB cho 200×50 và dựng lại mọi `LayoutJob`. Cộng với spinner 12.5 Hz hoặc output 60 Hz, split nhiều pane là nặng. Sửa: dirty flag theo session (bump khi Wakeup/resize/scroll/selection/theme), cache `Frame` và `Vec<(Pos2, Arc<Galley>, Color32)>`, replay bằng `painter.galley` khi sạch. Tối thiểu: tái dùng buffer `cells.clear()`.
2. **[một phần]** **Editor highlight lại cả file mỗi frame / mỗi phím** (`editor/view.rs:73-83`). `egui_extras::highlight` hash toàn bộ text (tới 200 kB) mỗi frame; mỗi lần gõ chạy syntect đồng bộ cả file. Gutter dựng `String` mới mỗi frame; `desired_rows(lines)` không ảo hoá. Sửa: cache `LayoutJob` trên `Buffer` theo generation counter + theme/font; highlight theo dòng hoặc nền; cache gutter theo số dòng.
3. **[đã sửa]** **Diff view dựng LayoutJob 20k dòng mỗi frame, chạy `git diff` đồng bộ trên UI thread** (`diff_view.rs:66-87`, `diff.rs:25-65`, gọi từ `workspace.rs:71`). Chạy lại mỗi khi snapshot GitStatus đổi (tới 2 s/lần khi file churn). Thêm: `status: *const GitStatus` (`diff.rs:14`) là ABA hazard, Arc giải phóng rồi cấp lại cùng địa chỉ sẽ trả diff cũ. Sửa: cache galley theo theme/font; chạy git diff trên worker, hiện text cũ tới khi có kết quả; key bằng generation tăng dần.
4. **[một phần]** **Files panel clone toàn bộ listing mỗi frame, list đồng bộ** (`files_panel.rs:87` `to_vec()`, `file_tree.rs:57-62`). Lần đầu mở thư mục chạy `WalkBuilder` trên UI thread; mọi thư mục đang mở list lại mỗi 3 s dù không đổi. Sửa: `Arc<[Entry]>`; `ScrollArea::show_rows`; invalidate từ watcher `notify` sẵn có thay vì TTL 3 s.

### Trung bình
5. **[đã sửa]** Spinner: giảm xuống ~150 ms, không request repaint cho spinner của tab có pane ẩn (`widgets.rs:160-183`, `sidebar_rows.rs`, `tab_bar.rs:139`).
6. 6-8 lần `term.lock()` mỗi pane mỗi frame (`view.rs:100,113,122,159`, `scroll.rs:90,98,179`, `view_chrome.rs:33`, `find.rs:116`), tranh chấp với PTY reader. Lấy lock một lần, đọc mode/display_offset/history_size cùng `Frame::capture`.
7. `tab_title/tab_busy/tab_status` tính 2-3 lần mỗi tab mỗi frame ở sidebar và tab bar; `task_summary` cấp phát `to_lowercase` mỗi lần (`tab_title.rs:28-38`). Tính một `TabView { title, busy, status }` mỗi tab mỗi frame.
8. Find: `count_matches` quét toàn bộ scrollback trên UI thread mỗi phím; `visible_matches` chạy regex mỗi frame (`find.rs:117-141`). Debounce và cache theo (query, display_offset, generation).
9. ⌘-hover link: `stat` mỗi frame khi giữ ⌘ (`links.rs:112`). Cache theo (row, col, hash nội dung).
10. Git fallback poll chạy `git status` tuần tự mọi repo mỗi 60 s trên một thread (`git_watcher.rs:105-120`). Rải đều theo repo; chỉ poll repo không có watcher.
11. Markdown preview parse và layout lại toàn bộ mỗi frame (`editor/markdown.rs:10`). Chấp nhận được khi idle; nặng khi spinner/blink đang chạy.
12. Claude history: clone `Vec<Session>` mỗi frame khi popup mở, spawn thread không join mỗi 15 s đọc cả transcript (`claude_history.rs:44-67,110-134`). Trả `Arc<[Session]>`, một worker, đọc từ cuối file.

### Thấp
13. **[đã sửa]** `visible_panes()` dựng HashSet hai lần mỗi frame (`app.rs:253,313`).
14. `FITS_CELL` clone `FontFamily` và hash tên family mỗi cell non-ASCII (`render.rs:99-111`); key theo `(char, bold)`.
15. `decorations::paint` chạy trước early-out cho ô trống (`render.rs:151`).
16. Glyph box-drawing cắt text run (`render.rs:160`); đẩy vào shape list riêng.
17. `TerminalStyle::new` cấp phát `FontFamily::Name` mỗi pane mỗi frame; `workspace.rs:57` clone branch mỗi frame.
18. `ui.input(|i| i.events.clone())` clone cả event vec mỗi frame (`view.rs:158`).
19. `ViewState` clone ra/vào egui data mỗi pane mỗi frame (`view.rs:83,147`); giữ trong `TerminalSession`.
20. 2 `fs::metadata` mỗi session mỗi 250 ms (`session_pool.rs:118`, `event_log.rs:43`); ổn tới ~50 pane.
21. `fonts()` clone cả `Vec<FontEntry>` mỗi frame khi combo mở (`font_catalog.rs:63`, `typography.rs:39`).
22. Font cài hai lần lúc khởi động (`app.rs:72` rồi `sync_font`), atlas dựng lại.
23. Palette dựng lại items và lowercase mọi label mỗi frame (`palette.rs:25-28,101-150`).
24. Pane context menu lock term để `selection_to_string`/`last_output` mỗi frame khi mở (`pane_chrome.rs:21,27`).
25. **[đã sửa]** `Cargo.toml` chưa có `[profile.release]`: thêm `lto = "thin"`, `codegen-units = 1`, `panic = "abort"`.

Thứ tự nên làm: 1 + 5 (loại phần lớn CPU steady-state), rồi 2 + 3, rồi 4, rồi 6-9.

## 2. UX

1. **[đã sửa]** **Xoá không hỏi, không undo.** `RemoveProject` / `RemoveRepo` (`app_actions.rs:24-44`) kill ngay mọi terminal và phiên Claude từ một click menu. Đóng tab có agent `Working` chỉ chặn khi file chưa lưu. Sửa: `Dialog::Confirm` liệt kê "N terminal, M agent đang chạy"; `Dialog::CloseBusyTab`; undo stack cho repo đã xoá.
2. **[đã sửa]** **⌘Q / đóng cửa sổ mất file chưa lưu.** `on_exit` (`app.rs:275`) chỉ lưu state; không xử lý `close_requested`. Sửa: `ViewportCommand::CancelClose` + `Dialog::UnsavedChanges` dạng "tất cả file".
3. **[đã sửa]** **⌘W hai nghĩa, ba nhãn khác nhau.** Tab terminal: đóng pane; tab tài liệu: đóng tab (`app_shortcuts.rs:70-77`). Nhãn "Close pane ⌘W" / "Close tab ⌘W" ở `pane_chrome.rs:37`, `sidebar.rs:131`, `workspace.rs:177`. Sửa: ⌘W = đóng pane (đóng tab khi pane cuối), ⇧⌘W = đóng tab.
4. **[đã sửa]** **Trạng thái attention không tự xoá khi đã xem.** `Done/NeedsInput/Error` chỉ xoá khi gõ phím (`session.rs:166`, `agent_events.rs:121`). Dock badge đếm cả agent đã đọc; pill "Done" nằm trên tab đang xem. Không có "nhảy tới tab cần chú ý". Sửa: xoá `Done` khi pane hiện và cửa sổ focus ~2 s; giữ `NeedsInput/NeedsApproval` tới khi gõ; thêm `Action::FocusNextAttention` (⌃⌘N) + palette.
5. **[một phần]** **Nhiều shortcut vô hình.** Không có trong palette (`palette.rs:98-129`): ⌘⌥←→↑↓, ⌘⌥⇧←→, ⌘⇧[ ], ⌘1-9, ⌘+/-, ⌘↑/↓, ⌘F, Rename tab, Close tab, Resume session, Restart pane. Palette ghi "Show files ⌘B" nhưng ⌘B là toggle. Palette dùng `tab.title` thô thay vì `tab_title()` nên bỏ qua tên đã đổi và tóm tắt task. Không có menu bar macOS (`main.rs:56-66`). Sửa: mỗi `Action` một dòng palette; `tab_title()`; menu bar tối thiểu.
6. **[một phần]** **Chỉ báo trạng thái trên tab bar chỉ bằng màu, ẩn khi hover** (`tab_bar.rs:135-146`), không tooltip. Dirty dot trắng ở tab bar nhưng vàng ở editor (`editor/view.rs:36`). ⬆/⬇ và chữ trạng thái Changes chỉ khác hue. Sửa: tooltip `status.label()`, không ẩn khi hover, dùng pill như sidebar, một màu dirty.
7. **[đã sửa]** **Lỗi git/OS chỉ log, không hiện.** `create_worktree` thất bại đóng dialog im lặng (`app_open_folder.rs:121`); `RevealRepo`, `open_externally`, `notify` tương tự. `DiffCache` trả stderr làm nội dung diff (`diff.rs:55`). Sửa: giữ dialog mở với dòng lỗi; toast lỗi ở status bar.
8. **[một phần]** **Thiếu context menu**: tab chỉ Rename/Close; Files panel không có; project row không có chuột phải; editor, diff không có; pane menu thiếu Paste / Select all / Clear scrollback.
9. **[một phần]** **Focus**: (a) `OpenFind` set flag toàn cục, chỉ terminal focus mới tiêu thụ; khi tab hiện là file/diff, flag rò và find bar bật ở terminal nào focus sau (`app_actions.rs:99`, `view.rs:86`). (b) Esc trong Settings cần hai lần khi ô text đang focus (`settings/mod.rs:68`). (c) `focus_active_pane()` cho diff tab trỏ `tab.id` nhưng diff view không đăng ký widget id đó. (d) Không điều hướng phím trong sidebar, Files, Changes, History.
10. **[đã sửa]** **Dialog**: nút OK ghi "Save" cho New project / New worktree; hint "e.g. ego-cart-app" dùng cả cho Rename tab; `UnsavedChanges` không có nút mặc định (Enter không làm gì); tên nhánh worktree chỉ thay space, không `git check-ref-format`.
11. **Click trong sidebar không nhất quán**: click project toggle collapse, click repo select; chevron project là label, chevron repo là button; "★" trông như nút favorite nhưng không làm gì; double-click vùng trống tab strip không mở tab.
12. **[một phần]** **Trùng/empty state**: hai nút "New project" cùng hiện (header và footer); project không có repo không hiện dòng "Add repositories…"; `+` tab bar và pill không tooltip; `ago` không tooltip giờ tuyệt đối.

## 3. Thiếu so với workflow Warp (đã xác nhận không có trong code)

1. **[đã sửa trong app]** Click notification → focus pane (`attention.rs:5-16` không mang pane id, không handler).
2. **[đã sửa]** Unread theo tab cho shell thường (build xong ở tab nền không có dấu); `is_busy()` chỉ là cửa sổ 1.5 s.
3. **[đã sửa]** Worktree chỉ có add (`app_open_folder.rs:92-126`): không remove/prune, không list worktree sẵn có, không nhận biết repo là worktree (`.git` là file) để offer xoá folder.
4. Không tìm kiếm chéo: palette chỉ command + tab title; không fuzzy open file (⌘P), không search scrollback nhiều pane, không filter History, không filter Files.
5. Tab không chuyển giữa repo (`app_panes.rs:51-56` chỉ reorder trong repo; drag tab chỉ trục x).
6. **[một phần]** Split luôn là Shell ở repo root (`app_panes.rs:73`), không kế thừa cwd pane hiện; không split Claude; không swap/break-out; không resize bằng phím.
7. **[một phần]** cwd shell không lưu qua restart (`session.cwd()` đọc mỗi frame ở `workspace.rs:140` nhưng không ghi vào `pane.cwd`); `closed_tabs` không persist.
8. Không drag-and-drop trong app (không `dnd_drag_source` ở đâu): không kéo file từ Files vào terminal/Claude, không kéo tab vào sidebar.
9. Link chỉ tồn tại khi giữ ⌘ (`view_chrome.rs:26`); OSC 8 không gạch chân; không chuột phải Open / Copy link.
10. **[một phần]** Copy/paste: pane menu thiếu Paste / Select all / Clear scrollback; `Event::Cut` xử lý như copy (`view.rs:199`); không cảnh báo paste nhiều dòng.
11. Editor không có find/replace, go-to-line, word wrap (`editor/view.rs:81` wrap vô hạn).
12. Files panel và diff view chỉ đọc: Files không new/rename/delete/reveal/copy path/open terminal here, không auto-expand tới file đang mở; Diff luôn `git diff HEAD` nên staged/unstaged không phân biệt, không stage/discard/hunk click → mở file tại dòng; Changes không stage checkbox/commit.

## 4. Polish

1. Thang cỡ chữ tuỳ tiện (≥12 cỡ: 15/14/13.5/13/12.5/12/11.5/11/10.5/16/24). Gom thành `TextRole { Title, Body, Secondary, Caption }`.
2. Chiều cao hàng khác nhau: project/repo 32, branch 28, tab sidebar 26, files 28, history 30, settings nav 34, tab bar 38. Chuẩn 28 cho list row, 32 cho group header.
3. Màu muted/border tính 4 kiểu (0.6 / 0.55 / 0.8 / 0.85; border 0.12 / 0.25 / `theme.border()`). Chỉ giữ `muted()`, `muted_strong()`, `border()`.
4. Màu hard-code bỏ qua theme (status hex ở `files_panel.rs:146`, đỏ ở `agent_status.rs:11`, `LIGHT_RED` ở editor/workspace, cam `AGENT`); vàng `0xe5c07b` gần như vô hình trên theme sáng. Lấy từ `theme.ansi[]`.
5. Fill chọn/hover và bo góc khác nhau theo surface (r4/5/6/7/8/10/12/14). Chuẩn 2 bán kính: 6 control, 12 container; một màu selection.
6. Tab strip không xử lý tràn: title cắt theo 28 ký tự không theo pixel, strip là `horizontal` thường nên nhiều tab đẩy `+` và nút history ra ngoài. Co title theo chiều rộng hoặc `ScrollArea::horizontal` + menu "⋯".
7. Ưu tiên cắt chữ: cụm phải của branch row cố định nên ở sidebar 200 px tên nhánh mất trước; status bar không `truncate` nên chồng khi hẹp; palette không truncate label dài.
8. Mono/proportional lẫn lộn cùng vai trò: icon ">_" proportional ở sidebar nhưng mono ở settings; chevron files `.monospace()` còn sidebar proportional; ô shell program không `.code_editor()` trong khi command khác có.
9. Glyph "›" / "⌄" lệch baseline nên toggle thấy giật. Vẽ chevron bằng vector như `file_icon`.
10. Offset chrome khác nhau: `TRAFFIC_LIGHTS_HEIGHT` 34 (sidebar) vs 40 (settings nav); files panel top 44 ≠ sidebar 34+8. Nội dung nhảy ~6 px khi vào Settings. Một hằng `CHROME_TOP`.
11. Icon không đồng nhất: stroke 1.2-1.5, hit box 14/18/22/28; nút × pane có frame còn icon khác không. Chuẩn stroke 1.4, glyph 16, hit 28.
12. Số thứ tự tab "1 " luôn hiện, tốn ~12 px mỗi tab; Warp chỉ hiện khi giữ ⌘.

## 5. Top 10 nên làm trước

1. Cache frame/galley terminal + giảm spinner (Hiệu năng 1, 5).
2. Confirm khi xoá project/repo và đóng tab có agent đang chạy; undo cho repo đã xoá (UX 1).
3. Chặn ⌘Q khi còn file chưa lưu (UX 2).
4. Attention: tự xoá `Done` khi đã xem, `FocusNextAttention`, click notification → focus pane (UX 4, Gap 1-2).
5. Sửa ⌘W / ⇧⌘W và nhãn (UX 3).
6. Palette đủ mọi action, dùng `tab_title()`, menu bar tối thiểu (UX 5).
7. Editor và diff cache highlight/galley; `git diff` chạy nền; sửa ABA ở `DiffCache` (Hiệu năng 2-3).
8. Files panel: `Arc` listing, ảo hoá, invalidate từ watcher; thêm context menu và hành động file (Hiệu năng 4, Gap 12).
9. Diff: click hunk → mở file tại dòng, staged/unstaged, stage/discard (Gap 12).
10. Lưu cwd shell qua restart; split kế thừa cwd và cho phép Claude (Gap 6-7).

## 6. Đã sửa ngày 2026-10-21

Hiệu năng
- Spinner 80 → 150 ms (`widgets.rs`). Frame terminal tái dùng buffer cell (`Frame::capture_into`, `session.frame`) thay vì cấp phát mỗi frame; chưa cache galley theo dirty flag.
- Diff: `git diff` chạy trên thread `git-diff`, text cũ hiện tới khi có kết quả, "Computing diff…" lần đầu; galley cache theo (text, font, theme, ppp) trong `DiffCache::galley_slot`; ABA sửa bằng `GitStatus.generation` tăng dần từ watcher (`diff.rs`, `git_watcher.rs`).
- Editor: gutter cache theo số dòng (`Buffer::gutter`). Highlight vốn đã qua cache của `egui_extras` (hash text mỗi frame); chưa highlight theo dòng.
- Files panel: listing là `Arc<[Entry]>`, không `to_vec()` mỗi frame; vẫn TTL 3 s, chưa ảo hoá.
- `visible_panes()` tính một lần mỗi frame. `[profile.release]` lto thin + codegen-units 1.

UX
- Xoá project/repo có terminal hoặc Claude đang chạy → `Dialog::Confirm` liệt kê "N terminal và M Claude session (k working)"; đóng tab/pane có Claude đang Working → hỏi. Chưa có undo repo.
- ⌘Q/đóng cửa sổ khi còn file chưa lưu → `CancelClose` + dialog Save all / Quit without saving / Cancel.
- ⌘W = đóng pane (tab khi là pane cuối), ⇧⌘W = đóng tab (`Action::CloseActiveTab`); nhãn menu sidebar đổi theo.
- "Done" tự xoá sau 2 s khi pane hiện và cửa sổ focus (`TerminalSession::note_viewed`); `NeedsInput/NeedsApproval` giữ tới khi gõ. ⌃⌘N = `FocusNextAttention` (cũng trong palette).
- Palette: thêm Focus pane ←→↑↓, Move tab, Next/Previous tab, Find, Rename tab, Close tab, Restart pane, Split with Claude, Next attention, Mark notifications read, Toggle files panel, Worktree setup/remove; tab dùng `tab_title()`. Chưa có menu bar macOS, chưa có Resume session trong palette.
- Lỗi git/OS (worktree add/remove, Reveal, external editor) → inbox notice + log thay vì chỉ log. Diff vẫn hiện stderr làm nội dung.
- Pane menu thêm Paste / Select all / Clear scrollback / Split with Claude. Chưa có menu cho Files, project row, editor.
- `OpenFind` chỉ set cờ khi tab hiện là terminal (hết rò sang terminal khác).
- Dialog: nút OK đúng ngữ cảnh (Create/Rename), hint riêng, Enter = Save ở UnsavedChanges và xác nhận ở Confirm.
- Footer sidebar bỏ "+" trùng; project rỗng có dòng "Add repositories…". "+" tab bar có min width menu.
- Tab bar: tooltip tên trạng thái khi hover; chấm trạng thái không ẩn khi hover (trừ khi trỏ vào ×).

Thiếu so với Warp
- Inbox thông báo trong app: click → focus pane. Khi cửa sổ bị che kín/ẩn, eframe không chạy `ui`; `App::logic` giờ vẫn bơm PTY và giao notice nên thông báo hệ thống không bị trễ tới lúc cửa sổ hiện lại (`app.rs`). Click notification hệ thống vẫn chưa focus (notify-rust không có callback trên macOS).
- Lệnh dài xong ở tab nền và OSC 9/777 vào inbox + Notification Center (giới hạn 1 thông báo hệ thống / pane / 3 s).
- Worktree: nhận biết worktree (`.git` file trỏ `worktrees/`), tag "merged", Remove worktree (+ xoá nhánh nếu merged), `.worktreeinclude`, setup command per repo. Chưa list worktree sẵn có / prune.
- Split kế thừa cwd pane hiện và cho phép Claude. cwd shell lưu vào `pane.cwd` khi đổi (qua shell hook). `closed_tabs` vẫn chưa persist.

Còn mở (ưu tiên tiếp): Perf 1 (dirty flag + cache galley terminal), 6-9; UX 5 (menu bar), 8 (context menu Files/editor), 11; Gap 4, 5, 8, 9, 11, 12; Polish 1-12.
