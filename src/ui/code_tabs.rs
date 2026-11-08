//! VS Code-style tabs of the Code view: the file's vscode-icons icon and name (Δ before a diff's), a dot while unsaved
//! that turns into × on hover, and a top accent on the active one. Drag to reorder.

use std::path::Path;

use eframe::egui::{
    Align2, FontId, Rect, Response, RichText, ScrollArea, Sense, Ui, pos2, scroll_area::ScrollBarVisibility,
    vec2,
};

use super::Action;
use super::file_icon;
use super::widgets::{accent, muted};
use crate::editor::Buffers;
use crate::file_icons;
use crate::model::{Repo, Tab};

pub const HEIGHT: f32 = 36.0;
const CLOSE_SIZE: f32 = 16.0;
const ICON_SIZE: f32 = 16.0;

pub fn show(ui: &mut Ui, repo: &Repo, buffers: &Buffers, actions: &mut Vec<Action>) {
    ScrollArea::horizontal()
        .id_salt("code-tabs")
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(HEIGHT);
                ui.spacing_mut().item_spacing.x = 0.0;
                let mut centers = Vec::new();
                let mut dragging = None;
                for (index, tab) in repo.view_tabs().enumerate() {
                    let active = repo.active_tab == Some(tab.id);
                    let dirty = tab.file.as_deref().is_some_and(|p| buffers.is_dirty(p));
                    let (response, close) = tab_item(ui, tab, active, dirty);
                    centers.push(response.rect.center().x);
                    if response.dragged()
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        dragging = Some((index, tab.id, pos.x));
                    }
                    if close || response.middle_clicked() {
                        actions.push(Action::CloseTab { repo: repo.id, tab: tab.id });
                    } else if response.clicked() {
                        actions.push(Action::SelectTab { repo: repo.id, tab: tab.id });
                    }
                    response.context_menu(|ui| tab_menu(ui, repo, tab, actions));
                }
                // Dragging a tab past a neighbour's centre moves it there.
                if let Some((index, tab, x)) = dragging {
                    let to = centers.iter().enumerate().filter(|(i, c)| *i != index && **c < x).count();
                    if to != index {
                        actions.push(Action::MoveTab { repo: repo.id, tab, to });
                    }
                }
                ui.add_space(6.0);
                let open = ui
                    .add(
                        eframe::egui::Button::new(RichText::new("+").size(17.0).color(muted(ui)))
                            .frame(false),
                    )
                    .on_hover_text(crate::keymap_hint::with_hint("Open file", "palette.files"));
                if open.clicked() {
                    actions.push(Action::OpenQuickOpen);
                }
            });
        });
}

/// One tab; returns its response and whether its close button was clicked.
fn tab_item(ui: &mut Ui, tab: &Tab, active: bool, dirty: bool) -> (Response, bool) {
    let path = tab.file.as_deref().or(tab.diff.as_deref());
    let file_name =
        path.and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let icon = file_icons::for_file(&file_name);
    let name = if tab.diff.is_some() { format!("Δ {file_name}") } else { file_name };
    let text_color = if active { ui.visuals().strong_text_color() } else { muted(ui) };
    let name_galley = ui.painter().layout_no_wrap(name, FontId::proportional(13.0), text_color);
    let icon_width = ICON_SIZE + 6.0;
    let width = 12.0 + icon_width + name_galley.size().x + 10.0 + CLOSE_SIZE + 10.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::click_and_drag());

    let painter = ui.painter();
    let visuals = ui.visuals();
    if active {
        painter.rect_filled(rect, 0.0, visuals.extreme_bg_color.gamma_multiply(0.35));
        painter.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)), 0.0, accent(ui));
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, visuals.widgets.hovered.weak_bg_fill.gamma_multiply(0.6));
    }
    painter.vline(rect.right(), rect.y_range(), visuals.widgets.noninteractive.bg_stroke);
    let y = rect.center().y;
    let mut x = rect.left() + 12.0;
    let icon_rect = Rect::from_min_size(pos2(x, y - ICON_SIZE / 2.0), vec2(ICON_SIZE, ICON_SIZE));
    file_icon::paint(painter, icon_rect, icon);
    x += icon_width;
    painter.galley(pos2(x, y - name_galley.size().y / 2.0), name_galley, text_color);

    let close_rect =
        Rect::from_center_size(pos2(rect.right() - 10.0 - CLOSE_SIZE / 2.0, y), vec2(CLOSE_SIZE, CLOSE_SIZE));
    let over_close = response.hover_pos().is_some_and(|p| close_rect.contains(p));
    if dirty && !response.hovered() {
        painter.circle_filled(close_rect.center(), 4.0, text_color);
    } else if active || response.hovered() {
        if over_close {
            painter.rect_filled(close_rect, 4.0, visuals.widgets.active.weak_bg_fill);
        }
        painter.text(close_rect.center(), Align2::CENTER_CENTER, "×", FontId::proportional(14.0), muted(ui));
    }
    let close = response.clicked() && over_close;
    let response = match path {
        Some(path) => response.on_hover_text(path.display().to_string()),
        None => response,
    };
    (response, close)
}

fn tab_menu(ui: &mut Ui, repo: &Repo, tab: &Tab, actions: &mut Vec<Action>) {
    if ui.button("Close").clicked() {
        actions.push(Action::CloseTab { repo: repo.id, tab: tab.id });
    }
    if let Some(path) = tab.file.as_ref() {
        if ui.button("Show diff").clicked() {
            actions.push(Action::OpenDiff { repo: repo.id, path: path.clone() });
        }
        if ui.button("Reveal in Finder").clicked() {
            actions.push(Action::RevealPath(path.clone()));
        }
    }
}
