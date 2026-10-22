use std::path::Path;
use std::sync::Arc;

use eframe::egui::{
    Align, Align2, FontId, Frame, Galley, Id, Key, Label, Layout, Popup, PopupCloseBehavior, RichText, Sense,
    TextEdit, Ui, pos2, vec2,
};
use uuid::Uuid;

use super::Action;
use super::dialog_kit::{self as kit, Tone};
use super::widgets::{AGENT, accent, muted, right_aligned};

/// A note the user attached to a line of a diff, to send to Claude in one go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub line: usize,
    pub text: String,
}

const GUTTER_WIDTH: f32 = 18.0;
const EDITOR_WIDTH: f32 = 360.0;

/// Column left of the diff: hover a row for "+", click it to write a comment; commented rows keep
/// a dot. Rows are uniform because the diff never wraps.
pub fn gutter(ui: &mut Ui, galley: &Arc<Galley>, file: &Path, comments: &mut Vec<Comment>) {
    let rows = galley.rows.len().max(1);
    let row_height = galley.size().y / rows as f32;
    let (rect, response) = ui.allocate_exact_size(vec2(GUTTER_WIDTH, galley.size().y), Sense::click());
    let painter = ui.painter();
    let center_of = |line: usize| pos2(rect.center().x, rect.top() + (line as f32 + 0.5) * row_height);
    for comment in comments.iter() {
        painter.circle_filled(center_of(comment.line), 3.0, AGENT);
    }
    let hovered = response.hover_pos().map(|p| ((p.y - rect.top()) / row_height).floor().max(0.0) as usize);
    let editing_id = Id::unique(("diff-comment-line", file));
    let mut editing: Option<usize> = ui.data(|d| d.get_temp(editing_id));
    if let Some(line) = hovered.filter(|l| *l < rows) {
        if editing != Some(line) && !comments.iter().any(|c| c.line == line) {
            painter.text(center_of(line), Align2::CENTER_CENTER, "+", FontId::proportional(14.0), accent(ui));
        }
        if response.clicked() {
            editing = Some(line);
            ui.data_mut(|d| d.insert_temp(editing_id, line));
        }
    }
    let Some(line) = editing else { return };
    let anchor = eframe::egui::Rect::from_center_size(center_of(line), vec2(GUTTER_WIDTH, row_height));
    let text_id = Id::unique(("diff-comment-text", file));
    let popup = Popup::new(editing_id.with("popup"), ui.ctx().clone(), anchor, ui.layer_id())
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .width(EDITOR_WIDTH);
    let shown = popup.show(|ui| {
        let mut text: String = ui.data(|d| d.get_temp(text_id)).unwrap_or_default();
        let existing = comments.iter().position(|c| c.line == line);
        if let Some(index) = existing.filter(|_| text.is_empty()) {
            text = comments[index].text.clone();
        }
        ui.label(RichText::new(format!("Comment on line {}", line + 1)).color(muted(ui)).size(11.5));
        let edit = ui.add(TextEdit::multiline(&mut text).desired_rows(3).desired_width(f32::INFINITY));
        edit.request_focus();
        let submit = ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter));
        let footer = kit::footer(ui, &[("Add", Tone::Primary)], false);
        if footer.action.is_some() || submit {
            let text = text.trim().to_string();
            match (existing, text.is_empty()) {
                (Some(index), true) => {
                    comments.remove(index);
                }
                (Some(index), false) => comments[index].text = text,
                (None, false) => comments.push(Comment { line, text }),
                (None, true) => {}
            }
            comments.sort_by_key(|c| c.line);
        }
        let done = footer.action.is_some() || submit || footer.cancel;
        ui.data_mut(|d| d.insert_temp(text_id, text));
        done
    });
    let closed = shown.is_none_or(|r| r.inner);
    if closed {
        ui.data_mut(|d| {
            d.remove_temp::<usize>(editing_id);
            d.remove_temp::<String>(text_id);
        });
    }
}

/// Lists the comments under the diff with a button that sends them all to Claude as one prompt.
pub fn list(
    ui: &mut Ui,
    repo: Uuid,
    relative: &Path,
    text: &str,
    comments: &mut Vec<Comment>,
    actions: &mut Vec<Action>,
) {
    if comments.is_empty() {
        return;
    }
    let lines: Vec<&str> = text.lines().collect();
    Frame::new().inner_margin(vec2(14.0, 8.0)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let count = comments.len();
            let label = if count == 1 { "1 comment".to_string() } else { format!("{count} comments") };
            ui.label(RichText::new(label).strong().size(12.5));
            right_aligned(ui, |ui| {
                if ui.button("Send comments to Claude").clicked() {
                    actions.push(Action::SendToAgent { repo, text: prompt(relative, &lines, comments) });
                    comments.clear();
                }
                if ui.button("Clear").clicked() {
                    comments.clear();
                }
            });
        });
        let mut remove = None;
        for (index, comment) in comments.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if ui.small_button("×").clicked() {
                    remove = Some(index);
                }
                let excerpt = lines.get(comment.line).copied().unwrap_or_default().trim();
                let excerpt: String = excerpt.chars().take(48).collect();
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let head = format!("L{} · {excerpt}", comment.line + 1);
                    ui.add(
                        Label::new(RichText::new(head).monospace().size(11.5).color(muted(ui))).truncate(),
                    );
                    ui.add(Label::new(RichText::new(&comment.text).size(12.5)).truncate());
                });
            });
        }
        if let Some(index) = remove {
            comments.remove(index);
        }
    });
}

fn prompt(relative: &Path, lines: &[&str], comments: &[Comment]) -> String {
    let mut out = format!("Review comments on the diff of {} (against HEAD):\n\n", relative.display());
    for (n, comment) in comments.iter().enumerate() {
        let excerpt = lines.get(comment.line).copied().unwrap_or_default();
        out.push_str(&format!(
            "{}. Line: `{excerpt}`\n   > {}\n\n",
            n + 1,
            comment.text.replace('\n', "\n   > ")
        ));
    }
    out.push_str("Please address each comment and tell me what you changed.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_quotes_each_commented_line() {
        let lines = ["@@ -1 +1 @@", "-old", "+new"];
        let comments = [Comment { line: 2, text: "why?".into() }];
        let text = prompt(Path::new("src/a.rs"), &lines, &comments);
        assert!(text.contains("1. Line: `+new`\n   > why?"), "{text}");
    }
}
