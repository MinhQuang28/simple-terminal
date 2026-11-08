//! The extra cursors around egui's single-cursor `TextEdit`: typing, deleting and moving keys are
//! taken before the editor sees them and applied at every cursor, and the extra cursors are drawn
//! over the text.

use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::text_edit::TextEditOutput;
use eframe::egui::{Color32, Event, Id, Key, Rect, Stroke, TextEdit, Ui, pos2, vec2};

use super::multi_cursor::{self, Dir, Edit, Sel};

/// The editor's selection as (anchor, cursor).
pub fn primary(ui: &Ui, id: Id) -> Sel {
    let state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    state.cursor.char_range().map_or((0, 0), |r| (r.secondary.index.into(), r.primary.index.into()))
}

pub fn set_primary(ui: &Ui, id: Id, sel: Sel) {
    let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(sel.0), CCursor::new(sel.1))));
    TextEdit::store_state(ui.ctx(), id, state);
}

/// With extra cursors, takes this frame's typing and cursor keys from the input and applies them
/// at every cursor. Escape drops the extras. Returns whether the text changed.
pub fn take_input(ui: &Ui, id: Id, text: &mut String, extra: &mut Vec<Sel>) -> bool {
    if extra.is_empty() || !ui.memory(|m| m.has_focus(id)) {
        return false;
    }
    let mut edits = Vec::new();
    let mut escape = false;
    ui.input_mut(|i| {
        i.events.retain(|event| match edit_for(event) {
            Some(edit) => {
                edits.push(edit);
                false
            }
            None if matches!(event, Event::Key { key: Key::Escape, pressed: true, .. }) => {
                escape = true;
                false
            }
            None => true,
        })
    });
    if escape {
        extra.clear();
        return false;
    }
    if edits.is_empty() {
        return false;
    }
    let mut cursors = vec![primary(ui, id)];
    cursors.extend(extra.iter().copied());
    let mut changed = false;
    for edit in &edits {
        if let Some(new_text) = multi_cursor::apply(text, &mut cursors, edit) {
            *text = new_text;
            changed = true;
        }
    }
    // The first cursor stays egui's; the rest are ours.
    set_primary(ui, id, cursors[0]);
    *extra = cursors[1..].to_vec();
    changed
}

fn edit_for(event: &Event) -> Option<Edit> {
    match event {
        Event::Text(text) | Event::Paste(text) => Some(Edit::Insert(text.clone())),
        Event::Key { key, pressed: true, modifiers, .. } if !modifiers.command && !modifiers.ctrl => {
            let extend = modifiers.shift;
            let to = |dir| Some(Edit::Move { dir, extend });
            match key {
                Key::Backspace if !modifiers.alt => Some(Edit::Backspace),
                Key::Delete if !modifiers.alt => Some(Edit::Delete),
                Key::Enter => Some(Edit::Newline),
                Key::Tab if !extend => Some(Edit::Insert("\t".into())),
                Key::ArrowLeft if !modifiers.alt => to(Dir::Left),
                Key::ArrowRight if !modifiers.alt => to(Dir::Right),
                Key::ArrowUp if !modifiers.alt => to(Dir::Up),
                Key::ArrowDown if !modifiers.alt => to(Dir::Down),
                Key::Home => to(Dir::Home),
                Key::End => to(Dir::End),
                _ => None,
            }
        }
        // ⌘← and ⌘→ go to the line's ends on macOS.
        Event::Key { key: Key::ArrowLeft, pressed: true, modifiers, .. } if modifiers.mac_cmd => {
            Some(Edit::Move { dir: Dir::Home, extend: modifiers.shift })
        }
        Event::Key { key: Key::ArrowRight, pressed: true, modifiers, .. } if modifiers.mac_cmd => {
            Some(Edit::Move { dir: Dir::End, extend: modifiers.shift })
        }
        _ => None,
    }
}

/// Draws the extra cursors and their selections over the editor's text.
pub fn paint(ui: &Ui, output: &TextEditOutput, extra: &[Sel], color: Color32) {
    let painter = ui.painter().with_clip_rect(output.text_clip_rect);
    let origin = output.galley_pos.to_vec2();
    let galley = &output.galley;
    for &(anchor, head) in extra {
        let (lo, hi) = (anchor.min(head), anchor.max(head));
        if lo != hi {
            let start = galley.pos_from_cursor(CCursor::new(lo));
            let end = galley.pos_from_cursor(CCursor::new(hi));
            if (start.min.y - end.min.y).abs() < 1.0 {
                let rect = Rect::from_min_max(start.min, pos2(end.min.x, start.max.y)).translate(origin);
                painter.rect_filled(rect, 2.0, color.gamma_multiply(0.3));
            } else {
                // Across rows: to the end of the first, whole rows between, into the last.
                let right = galley.rect.right().max(start.min.x + 4.0);
                painter.rect_filled(
                    Rect::from_min_max(start.min, pos2(right, start.max.y)).translate(origin),
                    0.0,
                    color.gamma_multiply(0.3),
                );
                let middle =
                    Rect::from_min_max(pos2(galley.rect.left(), start.max.y), pos2(right, end.min.y));
                painter.rect_filled(middle.translate(origin), 0.0, color.gamma_multiply(0.3));
                painter.rect_filled(
                    Rect::from_min_max(pos2(galley.rect.left(), end.min.y), end.max).translate(origin),
                    0.0,
                    color.gamma_multiply(0.3),
                );
            }
        }
        let caret = galley.pos_from_cursor(CCursor::new(head)).translate(origin);
        let top = caret.min + vec2(0.0, 0.0);
        painter.line_segment([top, pos2(top.x, caret.max.y)], Stroke::new(2.0, color));
    }
}
