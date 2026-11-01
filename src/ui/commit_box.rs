//! Commit message field and button above the Changes list.

use eframe::egui::{Button, Context, Id, TextEdit, Ui};

use super::Action;
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

pub fn show(ui: &mut Ui, repo: &Repo, changes: usize, actions: &mut Vec<Action>) {
    let id = message_id(repo.id);
    let mut message: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    ui.add(
        TextEdit::multiline(&mut message)
            .hint_text("Commit message")
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    let ready = !message.trim().is_empty();
    let commit = Button::new(format!("Commit {changes} change{}", if changes == 1 { "" } else { "s" }));
    if ui.add_enabled(ready, commit).on_hover_text("Stages every change, then commits").clicked() {
        actions.push(Action::CommitAll { repo: repo.id, message: message.trim().to_string() });
        message.clear();
    }
    ui.data_mut(|d| d.insert_temp(id, message));
    ui.add_space(6.0);
}
