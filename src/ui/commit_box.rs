//! Commit message field with the Commit and Push/Pull buttons, above the Changes list.

use eframe::egui::{
    Align2, Color32, Context, CornerRadius, FontId, Id, Key, Modifiers, Response, Sense, TextEdit, Ui, vec2,
};

use super::Action;
use super::widgets::accent;
use crate::git_status::GitStatus;
use crate::git_sync::Sync;
use crate::model::Repo;

/// Where the repo's unsent commit message is kept between frames.
pub fn message_id(repo: uuid::Uuid) -> Id {
    Id::unique(("commit-message", repo))
}

/// Puts a message back in the field after a commit that failed, unless a new one was started.
pub fn restore(ctx: &Context, repo: uuid::Uuid, message: String) {
    ctx.data_mut(|d| {
        let current: String = d.get_temp(message_id(repo)).unwrap_or_default();
        if current.trim().is_empty() {
            d.insert_temp(message_id(repo), message);
        }
    });
}

/// Marks the repo's push or pull as over, so its button can be pressed again.
pub fn sync_finished(ctx: &Context, repo: uuid::Uuid) {
    ctx.data_mut(|d| d.remove::<bool>(syncing_id(repo)));
}

fn syncing_id(repo: uuid::Uuid) -> Id {
    Id::unique(("git-syncing", repo))
}

/// The message field with full-width Commit and Push/Pull buttons, as in VS Code; ⌘Enter in the
/// field commits. With nothing to commit, the sync button takes Commit's place.
pub fn show(ui: &mut Ui, repo: &Repo, status: &GitStatus, actions: &mut Vec<Action>) {
    let id = message_id(repo.id);
    let edit_id = Id::unique(("commit-edit", repo.id));
    let changes = status.files.len();
    let mut message: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    // Taken before the field sees it, so ⌘Enter commits instead of adding a line.
    let focused = ui.memory(|m| m.has_focus(edit_id));
    let shortcut = focused && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
    ui.add(
        TextEdit::multiline(&mut message)
            .id(edit_id)
            .hint_text(format!("Message (⌘Enter to commit on \"{}\")", status.branch))
            .desired_rows(1)
            .desired_width(f32::INFINITY)
            .margin(vec2(8.0, 6.0)),
    );
    ui.add_space(6.0);
    let sync = Sync::of(status);
    if changes > 0 || sync.is_none() {
        let ready = changes > 0 && !message.trim().is_empty();
        let s = if changes == 1 { "" } else { "s" };
        let hint = match (changes, ready) {
            (0, _) => "Nothing to commit".into(),
            (_, true) => format!("Stages all {changes} change{s}, then commits"),
            _ => "Type a message first".into(),
        };
        let clicked = wide_button(ui, "✓  Commit", true, ready).on_hover_text(hint).clicked();
        if (clicked || shortcut) && ready {
            actions.push(Action::CommitAll { repo: repo.id, message: message.trim().to_string() });
            message.clear();
        }
    }
    ui.data_mut(|d| d.insert_temp(id, message));
    if let Some(sync) = sync {
        if changes > 0 {
            ui.add_space(6.0);
        }
        let busy = ui.data(|d| d.get_temp(syncing_id(repo.id)).unwrap_or(false));
        let label = if busy { sync.running().to_string() } else { sync.label(status) };
        if wide_button(ui, &label, changes == 0, !busy).clicked() && !busy {
            ui.data_mut(|d| d.insert_temp(syncing_id(repo.id), true));
            actions.push(Action::GitSync { repo: repo.id, sync });
        }
    }
    ui.add_space(8.0);
}

/// A panel-wide button with its label centred: accent-filled when `primary`, dimmed when not
/// `enabled`.
fn wide_button(ui: &mut Ui, label: &str, primary: bool, enabled: bool) -> Response {
    let fill = if primary { accent(ui) } else { ui.visuals().widgets.inactive.bg_fill };
    let color = if primary { Color32::WHITE } else { ui.visuals().text_color() };
    let dim = if enabled { 1.0 } else { 0.45 };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    let lit = if enabled && response.hovered() { fill.lerp_to_gamma(Color32::WHITE, 0.08) } else { fill };
    ui.painter().rect_filled(rect, CornerRadius::same(4), lit.gamma_multiply(dim));
    let font = FontId::proportional(13.0);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, color.gamma_multiply(dim));
    response
}
