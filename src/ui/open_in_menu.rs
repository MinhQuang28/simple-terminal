//! The top bar's "open in": the selected repo's folder in an editor, the Finder or Terminal.

use std::path::Path;

use eframe::egui::{Label, Response, RichText, Stroke, Ui, vec2};

use super::Action;
use super::design::{self, TEXT_BODY, TEXT_SECONDARY};
use super::program_icon;
use super::widgets::{icon_button_frame, muted, row_sized};
use crate::app_icon;
use crate::model::Repo;
use crate::programs;

const WIDTH: f32 = 290.0;
const ROW_HEIGHT: f32 = 34.0;
/// App icons carry their own margin, so they draw larger than a program tile to look its size.
const ICON_SIZE: f32 = 20.0;

/// Apps a folder opens in, as (label, name for `open -a`, bundle, icon key). Those not installed
/// are left out; the Finder and Terminal come with macOS.
const APPS: &[(&str, &str, &str, &str)] = &[
    ("VS Code", "Visual Studio Code", "/Applications/Visual Studio Code.app", "code"),
    ("Cursor", "Cursor", "/Applications/Cursor.app", "cursor"),
    ("Zed", "Zed", "/Applications/Zed.app", "zed"),
    ("Finder", "Finder", "/System/Library/CoreServices/Finder.app", "finder"),
    ("Terminal", "Terminal", "/System/Applications/Utilities/Terminal.app", "shell"),
    ("Xcode", "Xcode", "/Applications/Xcode.app", "swift"),
];

/// The button and, while open, its popup for `repo`.
pub fn show(ui: &mut Ui, repo: &Repo, actions: &mut Vec<Action>) {
    let response = button(ui).on_hover_text("Open this folder in…");
    design::popup(ui, &response, WIDTH, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(Label::new(RichText::new(&repo.name).size(TEXT_BODY).strong()).truncate());
                let path = RichText::new(short_path(&repo.path)).size(TEXT_SECONDARY).color(muted(ui));
                ui.add(Label::new(path).truncate());
            });
        });
        design::divider(ui);
        for (label, app, bundle, icon) in APPS.iter().filter(|(_, _, bundle, _)| Path::new(bundle).exists()) {
            let (response, ()) = row_sized(ui, false, 6.0, ROW_HEIGHT, |ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                // The app's own icon; a program tile stands in when macOS gives none.
                if !app_icon::show(ui, ICON_SIZE, bundle) {
                    let program =
                        programs::get(icon).or(programs::get("program")).unwrap_or(programs::shell());
                    program_icon::icon(ui, 16.0, program);
                }
                ui.add(Label::new(RichText::new(*label).size(14.0)).selectable(false));
            });
            if response.clicked() {
                actions.push(Action::OpenIn { app: (*app).to_string(), path: repo.path.clone() });
                ui.close();
            }
        }
        design::divider(ui);
        let (copy, ()) = row_sized(ui, false, 6.0, ROW_HEIGHT, |ui| {
            ui.add(Label::new(RichText::new("Copy path").size(14.0)).selectable(false));
        });
        if copy.clicked() {
            ui.ctx().copy_text(repo.path.display().to_string());
            ui.close();
        }
    });
}

/// `path` with the home folder as `~`.
fn short_path(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    match home.and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// A box with an arrow leaving its top right corner.
fn button(ui: &mut Ui) -> Response {
    let (rect, response, color) = icon_button_frame(ui, false);
    let stroke = Stroke::new(1.5, color);
    let c = rect.center();
    let painter = ui.painter();
    let (left, right, top, bottom) = (c.x - 6.0, c.x + 5.0, c.y - 5.0, c.y + 6.0);
    // The box, open where the arrow leaves it.
    let corners = [(c.x - 1.0, top), (left, top), (left, bottom), (right, bottom), (right, c.y + 1.0)];
    for pair in corners.windows(2) {
        painter.line_segment([pair[0].into(), pair[1].into()], stroke);
    }
    let tip = c + vec2(6.5, -6.5);
    painter.line_segment([c, tip], stroke);
    painter.line_segment([tip, tip + vec2(-4.5, 0.0)], stroke);
    painter.line_segment([tip, tip + vec2(0.0, 4.5)], stroke);
    response
}
