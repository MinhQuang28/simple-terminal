use eframe::egui::{self, Checkbox, Id, Key, Modal, RichText, TextEdit};
use uuid::Uuid;

use super::Action;

pub enum Dialog {
    NewProject {
        name: String,
    },
    RenameProject {
        id: Uuid,
        name: String,
    },
    UnsavedChanges {
        repo: Uuid,
        tab: Uuid,
        file_name: String,
    },
    RenameTab {
        repo: Uuid,
        tab: Uuid,
        name: String,
    },
    NewWorktree {
        repo: Uuid,
        branch: String,
    },
    RemoveWorktree {
        repo: Uuid,
        name: String,
        branch: String,
        delete_branch: bool,
    },
    RepoSetup {
        repo: Uuid,
        command: String,
    },
    /// Asks before `action`, which runs unchecked once confirmed.
    Confirm {
        title: String,
        body: String,
        ok: &'static str,
        action: Action,
    },
    /// Quitting with unsaved files.
    QuitUnsaved {
        count: usize,
    },
    EditPrompt {
        index: Option<usize>,
        name: String,
        text: String,
    },
}

pub enum DialogResult {
    CreateProject(String),
    RenameProject(Uuid, String),
    SaveAndClose { repo: Uuid, tab: Uuid },
    DiscardAndClose { repo: Uuid, tab: Uuid },
    RenameTab { repo: Uuid, tab: Uuid, name: String },
    CreateWorktree { repo: Uuid, branch: String },
    RemoveWorktree { repo: Uuid, delete_branch: bool },
    SetRepoSetup { repo: Uuid, command: String },
    Run(Action),
    SaveAllAndQuit,
    QuitWithoutSaving,
    SavePrompt { index: Option<usize>, name: String, text: String },
}

/// Shows the open dialog. Returns `(close, result)`.
pub fn show(ctx: &egui::Context, dialog: &mut Dialog) -> (bool, Option<DialogResult>) {
    let mut close = false;
    let mut result = None;
    let modal = Modal::new(Id::unique("repodeck-dialog")).show(ctx, |ui| {
        ui.set_width(380.0);
        match dialog {
            Dialog::Confirm { title, body, ok, action } => {
                ui.heading(title.as_str());
                ui.add_space(8.0);
                ui.label(body.as_str());
                ui.add_space(10.0);
                let (confirmed, cancel) = buttons(ui, ok);
                let enter = ui.input(|i| i.key_pressed(Key::Enter));
                result = (confirmed || enter).then(|| DialogResult::Run(action.clone()));
                close = result.is_some() || cancel;
            }
            Dialog::QuitUnsaved { count } => {
                ui.heading("Unsaved changes");
                ui.add_space(8.0);
                let files =
                    if *count == 1 { "1 file has".to_string() } else { format!("{count} files have") };
                ui.label(format!("{files} unsaved changes. Save before quitting?"));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Save all and quit").clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                        result = Some(DialogResult::SaveAllAndQuit);
                    }
                    if ui.button("Quit without saving").clicked() {
                        result = Some(DialogResult::QuitWithoutSaving);
                    }
                    close = ui.button("Cancel").clicked();
                });
                close |= result.is_some();
            }
            Dialog::EditPrompt { index, name, text } => {
                ui.heading(if index.is_some() { "Edit prompt" } else { "New prompt" });
                ui.add_space(8.0);
                let name_edit =
                    ui.add(TextEdit::singleline(name).hint_text("Name").desired_width(f32::INFINITY));
                if index.is_none() && name.is_empty() {
                    name_edit.request_focus();
                }
                ui.add_space(6.0);
                ui.add(
                    TextEdit::multiline(text)
                        .hint_text("Prompt text; pasted into the pane as is.")
                        .desired_rows(6)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                let (ok, cancel) = buttons(ui, "Save");
                let valid = !name.trim().is_empty() && !text.trim().is_empty();
                result = (ok && valid).then(|| DialogResult::SavePrompt {
                    index: *index,
                    name: name.trim().to_string(),
                    text: text.trim_end().to_string(),
                });
                close = result.is_some() || cancel;
            }
            Dialog::NewProject { name } => {
                let (submitted, cancel) = name_form(ui, "New project", "e.g. Client work", "Create", name);
                result = submitted.map(DialogResult::CreateProject);
                close = result.is_some() || cancel;
            }
            Dialog::RenameProject { id, name } => {
                let (submitted, cancel) = name_form(ui, "Rename project", "", "Rename", name);
                result = submitted.map(|n| DialogResult::RenameProject(*id, n));
                close = result.is_some() || cancel;
            }
            Dialog::RenameTab { repo, tab, name } => {
                let (submitted, cancel) = name_form(ui, "Rename tab", "", "Rename", name);
                result = submitted.map(|name| DialogResult::RenameTab { repo: *repo, tab: *tab, name });
                close = result.is_some() || cancel;
            }
            Dialog::NewWorktree { repo, branch } => {
                ui.label(RichText::new("Creates a branch and checks it out in a sibling folder.").weak());
                let (submitted, cancel) =
                    name_form(ui, "New worktree", "e.g. feature/login", "Create", branch);
                result = submitted.map(|branch| DialogResult::CreateWorktree { repo: *repo, branch });
                close = result.is_some() || cancel;
            }
            Dialog::RemoveWorktree { repo, name, branch, delete_branch } => {
                ui.heading("Remove worktree");
                ui.add_space(8.0);
                ui.label(format!(
                    "Deletes the folder {name} and closes its tabs. Uncommitted changes are lost."
                ));
                ui.add_space(6.0);
                let label =
                    if branch.is_empty() { "its branch".to_string() } else { format!("branch {branch}") };
                ui.add(Checkbox::new(delete_branch, format!("Also delete {label} (only if merged)")));
                ui.add_space(10.0);
                let (ok, cancel) = buttons(ui, "Remove");
                result =
                    ok.then_some(DialogResult::RemoveWorktree { repo: *repo, delete_branch: *delete_branch });
                close = ok || cancel;
            }
            Dialog::RepoSetup { repo, command } => {
                ui.label(
                    RichText::new(
                        "Runs in the first terminal of each new worktree, before the shell prompt.",
                    )
                    .weak(),
                );
                let edit =
                    TextEdit::singleline(command).hint_text("e.g. npm install").desired_width(f32::INFINITY);
                ui.heading("Worktree setup");
                ui.add_space(8.0);
                let edit = ui.add(edit.code_editor());
                edit.request_focus();
                let submit = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.add_space(8.0);
                let (ok, cancel) = buttons(ui, "Save");
                result = (ok || submit)
                    .then(|| DialogResult::SetRepoSetup { repo: *repo, command: command.trim().to_string() });
                close = result.is_some() || cancel;
            }
            Dialog::UnsavedChanges { repo, tab, file_name } => {
                ui.heading("Unsaved changes");
                ui.add_space(8.0);
                ui.label(format!("Save changes to {file_name} before closing?"));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                        result = Some(DialogResult::SaveAndClose { repo: *repo, tab: *tab });
                    }
                    if ui.button("Don't save").clicked() {
                        result = Some(DialogResult::DiscardAndClose { repo: *repo, tab: *tab });
                    }
                    close = ui.button("Cancel").clicked();
                });
                close |= result.is_some();
            }
        }
    });
    close |= modal.should_close();
    (close, result)
}

/// Returns the trimmed name once submitted, plus whether the dialog was cancelled.
fn name_form(
    ui: &mut egui::Ui,
    heading: &str,
    hint: &str,
    ok_label: &str,
    name: &mut String,
) -> (Option<String>, bool) {
    ui.heading(heading);
    ui.add_space(8.0);
    let edit = ui.add(TextEdit::singleline(name).hint_text(hint).desired_width(f32::INFINITY));
    edit.request_focus();
    let submit = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    ui.add_space(8.0);
    let (ok, cancel) = buttons(ui, ok_label);
    let name = name.trim().to_string();
    (((ok || submit) && !name.is_empty()).then_some(name), cancel)
}

fn buttons(ui: &mut egui::Ui, ok_label: &str) -> (bool, bool) {
    ui.horizontal(|ui| {
        let ok = ui.button(ok_label).clicked();
        let cancel = ui.button("Cancel").clicked();
        (ok, cancel)
    })
    .inner
}
