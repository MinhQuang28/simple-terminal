//! The "Add project" dialog: a name and where the project's first repository comes from — a
//! folder already on disk, a new repository, or a clone.

use std::path::{Path, PathBuf};

use eframe::egui::{Button, Color32, CornerRadius, Label, RichText, Stroke, Ui, vec2};

use super::design::{self, RADIUS_ROW, TEXT_BODY, TEXT_SECONDARY};
use super::dialog_kit::{self as kit, Tone};
use super::dialogs::DialogResult;
use super::widgets::{muted, right_aligned};

const SOURCE_HEIGHT: f32 = 32.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Pick,
    Create,
    Clone,
}

/// Where a new project's repository comes from, once the form is valid.
pub enum ProjectSource {
    /// A folder already on disk.
    Folder(PathBuf),
    /// A new git repository `name` inside `parent`.
    Create { parent: PathBuf, name: String },
    /// A clone of `url` inside `parent`.
    Clone { url: String, parent: PathBuf },
}

pub struct AddProject {
    name: String,
    source: Source,
    /// The folder to add, or the one a new repository or clone goes in.
    folder: Option<PathBuf>,
    /// Where new repositories and clones go unless another folder is chosen.
    parent: Option<PathBuf>,
    repo_name: String,
    url: String,
    error: Option<String>,
}

impl AddProject {
    /// `parent` is offered as the place for a new repository or a clone.
    pub fn new(parent: Option<PathBuf>) -> Self {
        Self {
            name: String::new(),
            source: Source::Pick,
            folder: None,
            parent,
            repo_name: String::new(),
            url: String::new(),
            error: None,
        }
    }

    /// Returns `(close, result)`.
    pub fn show(&mut self, ui: &mut Ui) -> (bool, Option<DialogResult>) {
        kit::title(ui, "Add project");
        let name = kit::field(ui, &mut self.name, "Project name (the folder's when empty)", false);
        kit::focus_initially(ui, &name);
        self.source_picker(ui);
        match self.source {
            Source::Pick => {
                caption(ui, "Directory");
                folder_row(ui, &mut self.folder, "Select a directory");
            }
            Source::Create => {
                caption(ui, "Repository name");
                kit::field(ui, &mut self.repo_name, "e.g. my-app", true);
                caption(ui, "Location");
                folder_row(ui, &mut self.parent, "Select where to create it");
            }
            Source::Clone => {
                caption(ui, "Repository URL");
                kit::field(ui, &mut self.url, "https://… or git@…", true);
                caption(ui, "Location");
                folder_row(ui, &mut self.parent, "Select where to clone it");
            }
        }
        if let Some(error) = &self.error {
            ui.label(RichText::new(error).size(TEXT_SECONDARY).color(design::danger(ui)));
        }
        let footer = kit::footer(ui, &[("Create", Tone::Primary)], true);
        let mut result = None;
        if footer.action.is_some() {
            match self.source() {
                Ok(source) => {
                    result = Some(DialogResult::AddProject { name: self.name.trim().to_string(), source });
                }
                Err(error) => self.error = Some(error),
            }
        }
        (result.is_some() || footer.cancel, result)
    }

    /// Three equal buttons, the chosen one filled.
    fn source_picker(&mut self, ui: &mut Ui) {
        let options = [
            (Source::Pick, "Pick directory"),
            (Source::Create, "Create repository"),
            (Source::Clone, "Clone repository"),
        ];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let width = (ui.available_width() - 16.0) / 3.0;
            for (source, label) in options {
                let selected = self.source == source;
                let fill =
                    if selected { ui.visuals().widgets.active.weak_bg_fill } else { Color32::TRANSPARENT };
                let ink = if selected { ui.visuals().text_color() } else { muted(ui) };
                let line = if selected { ink.gamma_multiply(0.5) } else { design::hairline(ui) };
                let button = Button::new(RichText::new(label).size(TEXT_SECONDARY).color(ink))
                    .fill(fill)
                    .stroke(Stroke::new(1.0, line))
                    .corner_radius(CornerRadius::same(RADIUS_ROW))
                    .min_size(vec2(width, SOURCE_HEIGHT));
                if ui.add(button).clicked() {
                    self.source = source;
                    self.error = None;
                }
            }
        });
    }

    /// The form as a source, or what is still missing.
    fn source(&self) -> Result<ProjectSource, String> {
        let parent = || self.parent.clone().ok_or_else(|| "Choose a location.".to_string());
        match self.source {
            Source::Pick => {
                self.folder.clone().map(ProjectSource::Folder).ok_or("Choose a directory.".into())
            }
            Source::Create => {
                let name = self.repo_name.trim();
                if name.is_empty() || name.starts_with(['.', '-']) || name.contains(['/', '\\']) {
                    return Err("Enter a repository name without slashes.".into());
                }
                let parent = parent()?;
                free(&parent.join(name))?;
                Ok(ProjectSource::Create { parent, name: name.to_string() })
            }
            Source::Clone => {
                let url = self.url.trim();
                let Some(name) = clone_name(url) else { return Err("Enter the repository's URL.".into()) };
                let parent = parent()?;
                free(&parent.join(name))?;
                Ok(ProjectSource::Clone { url: url.to_string(), parent })
            }
        }
    }
}

/// The folder a clone of `url` lands in: its last path segment without `.git`. None for text
/// git would read as an option or that names no repository.
pub fn clone_name(url: &str) -> Option<&str> {
    if url.starts_with('-') || url.contains(char::is_whitespace) {
        return None;
    }
    let trimmed = url.trim_end_matches('/');
    let last = trimmed.rsplit(['/', ':']).next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    (!name.is_empty() && !name.starts_with('.') && last.len() < trimmed.len()).then_some(name)
}

fn free(path: &Path) -> Result<(), String> {
    if path.exists() { Err(format!("{} already exists.", path.display())) } else { Ok(()) }
}

fn caption(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(text).size(TEXT_SECONDARY).color(muted(ui)));
}

/// The chosen folder, or `hint`, with the button that opens the system folder picker.
fn folder_row(ui: &mut Ui, folder: &mut Option<PathBuf>, hint: &str) {
    ui.horizontal(|ui| {
        right_aligned(ui, |ui| {
            if kit::button(ui, "Choose", Tone::Secondary).clicked()
                && let Some(picked) = rfd::FileDialog::new().set_title(hint).pick_folder()
            {
                *folder = Some(picked);
            }
            ui.with_layout(eframe::egui::Layout::left_to_right(eframe::egui::Align::Center), |ui| {
                let text = match folder {
                    Some(path) => RichText::new(path.display().to_string()).size(TEXT_BODY),
                    None => RichText::new(hint).size(TEXT_BODY).color(muted(ui)),
                };
                ui.add(Label::new(text).truncate().selectable(false));
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::clone_name;

    #[test]
    fn clone_folder_comes_from_the_url() {
        assert_eq!(clone_name("https://github.com/acme/widgets.git"), Some("widgets"));
        assert_eq!(clone_name("git@github.com:acme/widgets.git"), Some("widgets"));
        assert_eq!(clone_name("https://example.com/acme/widgets/"), Some("widgets"));
        assert_eq!(clone_name("--upload-pack=evil"), None);
        assert_eq!(clone_name("widgets"), None);
        assert_eq!(clone_name(""), None);
    }
}
