use eframe::egui::{self, Checkbox, Ui};
use uuid::Uuid;

use super::Action;
use super::dialog_kit::{self as kit, Tone};

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
    let modal = kit::modal(ctx, |ui| match dialog {
        Dialog::Confirm { title, body, ok, action } => {
            kit::title(ui, title);
            kit::description(ui, body);
            let footer = kit::footer(ui, &[(ok, Tone::Destructive)], true);
            result = footer.action.map(|_| DialogResult::Run(action.clone()));
            close = result.is_some() || footer.cancel;
        }
        Dialog::QuitUnsaved { count } => {
            kit::title(ui, "Unsaved changes");
            let files = if *count == 1 { "1 file has".to_string() } else { format!("{count} files have") };
            kit::description(ui, &format!("{files} unsaved changes. Save before quitting?"));
            let footer = kit::footer(
                ui,
                &[("Save all and quit", Tone::Primary), ("Quit without saving", Tone::Secondary)],
                true,
            );
            result = match footer.action {
                Some(0) => Some(DialogResult::SaveAllAndQuit),
                Some(_) => Some(DialogResult::QuitWithoutSaving),
                None => None,
            };
            close = result.is_some() || footer.cancel;
        }
        Dialog::UnsavedChanges { repo, tab, file_name } => {
            kit::title(ui, "Unsaved changes");
            kit::description(ui, &format!("Save changes to {file_name} before closing?"));
            let footer = kit::footer(ui, &[("Save", Tone::Primary), ("Don't save", Tone::Secondary)], true);
            result = match footer.action {
                Some(0) => Some(DialogResult::SaveAndClose { repo: *repo, tab: *tab }),
                Some(_) => Some(DialogResult::DiscardAndClose { repo: *repo, tab: *tab }),
                None => None,
            };
            close = result.is_some() || footer.cancel;
        }
        Dialog::EditPrompt { index, name, text } => {
            kit::title(ui, if index.is_some() { "Edit prompt" } else { "New prompt" });
            let name_edit = kit::field(ui, name, "Name", false);
            if index.is_none() && name.is_empty() {
                name_edit.request_focus();
            }
            kit::text_area(ui, text, "Prompt text; pasted into the pane as is.", 6);
            let footer = kit::footer(ui, &[("Save", Tone::Primary)], false);
            let valid = !name.trim().is_empty() && !text.trim().is_empty();
            result = (footer.action.is_some() && valid).then(|| DialogResult::SavePrompt {
                index: *index,
                name: name.trim().to_string(),
                text: text.trim_end().to_string(),
            });
            close = result.is_some() || footer.cancel;
        }
        Dialog::NewProject { name } => {
            let (submitted, cancel) = name_form(ui, "New project", None, "e.g. Client work", "Create", name);
            result = submitted.map(DialogResult::CreateProject);
            close = result.is_some() || cancel;
        }
        Dialog::RenameProject { id, name } => {
            let (submitted, cancel) = name_form(ui, "Rename project", None, "", "Rename", name);
            result = submitted.map(|n| DialogResult::RenameProject(*id, n));
            close = result.is_some() || cancel;
        }
        Dialog::RenameTab { repo, tab, name } => {
            let (submitted, cancel) = name_form(ui, "Rename tab", None, "", "Rename", name);
            result = submitted.map(|name| DialogResult::RenameTab { repo: *repo, tab: *tab, name });
            close = result.is_some() || cancel;
        }
        Dialog::NewWorktree { repo, branch } => {
            let about = Some("Creates a branch and checks it out in a sibling folder.");
            let (submitted, cancel) =
                name_form(ui, "New worktree", about, "e.g. feature/login", "Create", branch);
            result = submitted.map(|branch| DialogResult::CreateWorktree { repo: *repo, branch });
            close = result.is_some() || cancel;
        }
        Dialog::RemoveWorktree { repo, name, branch, delete_branch } => {
            kit::title(ui, "Remove worktree");
            kit::description(
                ui,
                &format!("Deletes the folder {name} and closes its tabs. Uncommitted changes are lost."),
            );
            let label = if branch.is_empty() { "its branch".to_string() } else { format!("branch {branch}") };
            ui.add(Checkbox::new(delete_branch, format!("Also delete {label} (only if merged)")));
            let footer = kit::footer(ui, &[("Remove", Tone::Destructive)], true);
            result = footer
                .action
                .map(|_| DialogResult::RemoveWorktree { repo: *repo, delete_branch: *delete_branch });
            close = result.is_some() || footer.cancel;
        }
        Dialog::RepoSetup { repo, command } => {
            kit::title(ui, "Worktree setup");
            kit::description(ui, "Runs in the first terminal of each new worktree, before the shell prompt.");
            kit::field(ui, command, "e.g. npm install", true).request_focus();
            let footer = kit::footer(ui, &[("Save", Tone::Primary)], true);
            result = footer
                .action
                .map(|_| DialogResult::SetRepoSetup { repo: *repo, command: command.trim().to_string() });
            close = result.is_some() || footer.cancel;
        }
    });
    close |= modal.should_close();
    (close, result)
}

/// A title, optional description and one name field. Returns the trimmed name once submitted,
/// plus whether the dialog was cancelled.
fn name_form(
    ui: &mut Ui,
    heading: &str,
    about: Option<&str>,
    hint: &str,
    ok_label: &str,
    name: &mut String,
) -> (Option<String>, bool) {
    kit::title(ui, heading);
    if let Some(about) = about {
        kit::description(ui, about);
    }
    kit::field(ui, name, hint, false).request_focus();
    let footer = kit::footer(ui, &[(ok_label, Tone::Primary)], true);
    let name = name.trim().to_string();
    ((footer.action.is_some() && !name.is_empty()).then_some(name), footer.cancel)
}
