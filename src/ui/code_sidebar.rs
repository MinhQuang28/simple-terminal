//! The Code view's sidebar, which takes the project sidebar's place as VS Code's does: an
//! activity bar picking the folder tree, search across files, the repo's changes or the prompt
//! library, shown beside it.

use std::sync::Arc;

use eframe::egui::{Align, Layout, Rect, RichText, ScrollArea, Stroke, Ui, UiBuilder, vec2};

use super::Action;
use super::activity_bar;
use super::changes_panel::body;
use super::chrome::window_drag_area;
use super::file_explorer;
use super::search_panel;
use super::sidebar::TRAFFIC_LIGHTS_HEIGHT;
use super::widgets::muted;
use crate::file_tree::FileTree;
use crate::git_status::GitStatus;
use crate::library_cache::LibraryCache;
use crate::model::{AppState, Repo, SidePanel};
use crate::text_search_runner::TextSearch;

/// What the sidebar draws from besides the app state.
pub struct Sources<'a> {
    pub status: Option<Arc<GitStatus>>,
    pub tree: &'a mut FileTree,
    pub search: &'a mut TextSearch,
    pub library: &'a LibraryCache,
}

pub fn show(ui: &mut Ui, state: &AppState, repo: &Repo, sources: Sources<'_>, actions: &mut Vec<Action>) {
    // Room for the macOS traffic lights, which float over the hidden titlebar.
    let strip = ui.max_rect().with_max_y(ui.max_rect().top() + TRAFFIC_LIGHTS_HEIGHT);
    window_drag_area(ui, strip);
    ui.add_space(TRAFFIC_LIGHTS_HEIGHT);
    let Sources { status, tree, search, library } = sources;
    let panel = state.code_panel;
    // VS Code's activity bar down the left edge, the chosen view beside it.
    let area = ui.available_rect_before_wrap();
    // The bar and the view are child UIs, which don't grow this one: claim the width here, or the
    // panel snaps back to its minimum size and can't be resized.
    ui.expand_to_include_rect(area);
    let bar = Rect::from_min_size(area.min, vec2(activity_bar::WIDTH, area.height()));
    let content = area.with_min_x(bar.right() + 8.0);
    let mut bar_ui = ui.new_child(UiBuilder::new().max_rect(bar).layout(Layout::top_down(Align::Min)));
    bar_ui.spacing_mut().item_spacing.y = 2.0;
    let changes = status.as_ref().map_or(0, |s| s.files.len());
    activity_bar::show(&mut bar_ui, panel, changes, actions);
    ui.painter().vline(bar.right() + 3.0, area.y_range(), Stroke::new(1.0, state.settings.theme.border()));
    let mut ui = ui.new_child(UiBuilder::new().max_rect(content).layout(Layout::top_down(Align::Min)));
    let ui = &mut ui;
    let title = match panel {
        SidePanel::Files => "EXPLORER",
        SidePanel::Search => "SEARCH",
        SidePanel::Changes => "SOURCE CONTROL",
        SidePanel::Library => "LIBRARY",
    };
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new(title).size(11.0).strong().color(muted(ui)));
    });
    ui.add_space(8.0);
    match panel {
        SidePanel::Files => explorer(ui, state, repo, tree, actions),
        SidePanel::Search => search_panel::show(ui, search, repo, actions),
        SidePanel::Changes | SidePanel::Library => body(ui, state, repo, panel, status, library, actions),
    }
}

fn explorer(ui: &mut Ui, state: &AppState, repo: &Repo, tree: &mut FileTree, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new(repo.name.to_uppercase()).size(11.0).strong());
    });
    ui.add_space(2.0);
    let open_file = repo.active_tab.and_then(|id| repo.view_tabs().find(|t| t.id == id)?.file.clone());
    tree.set_show_ignored(state.show_ignored_files);
    ScrollArea::vertical().id_salt("code-explorer").auto_shrink(false).show(ui, |ui| {
        file_explorer::directory(ui, tree, repo, &repo.path, 0, open_file.as_deref(), actions);
    });
}
