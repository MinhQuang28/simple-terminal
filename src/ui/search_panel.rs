//! The side panel's Search, as in VS Code: a query with match case / whole word / regex toggles,
//! and every matching line grouped by file. Clicking a line opens the file there.

use std::path::Path;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    Align, Color32, CornerRadius, FontId, Id, Key, Label, Layout, RichText, ScrollArea, Sense, TextEdit, Ui,
    vec2,
};

use super::Action;
use super::file_icon;
use super::widgets::{accent, muted, row_sized};
use crate::model::Repo;
use crate::text_search::{Hit, Results};
use crate::text_search_runner::TextSearch;

const ROW_HEIGHT: f32 = 22.0;

/// One line of the results list, so only the rows on screen are laid out.
enum Row {
    File(usize),
    Hit(usize, usize),
}

pub fn show(
    ui: &mut Ui,
    search: &mut TextSearch,
    repo: &Repo,
    show_ignored: bool,
    actions: &mut Vec<Action>,
) {
    query_row(ui, search);
    search.update(&repo.path, show_ignored);
    ui.add_space(6.0);
    let status = match (&search.error, &search.results) {
        (Some(err), _) => Some(RichText::new(err).color(Color32::LIGHT_RED)),
        _ if search.query.text.is_empty() => None,
        (None, None) => Some(RichText::new("Searching…").color(muted(ui))),
        (None, Some(results)) => Some(RichText::new(summary(results)).color(muted(ui))),
    };
    if let Some(status) = status {
        ui.add(Label::new(status.size(11.5)).wrap());
        ui.add_space(4.0);
    }
    let Some(results) = search.results.clone().filter(|_| search.error.is_none()) else { return };
    let collapsed = |path: &Path| Id::unique(("search-collapsed", path));
    let rows: Vec<Row> = results
        .files
        .iter()
        .enumerate()
        .flat_map(|(i, file)| {
            let hits = if ui.data(|d| d.get_temp(collapsed(&file.path)).unwrap_or(false)) {
                0
            } else {
                file.hits.len()
            };
            std::iter::once(Row::File(i)).chain((0..hits).map(move |j| Row::Hit(i, j)))
        })
        .collect();
    ScrollArea::vertical().id_salt("search-results").auto_shrink(false).show_rows(
        ui,
        ROW_HEIGHT,
        rows.len(),
        |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in &rows[range] {
                match *row {
                    Row::File(i) => {
                        let file = &results.files[i];
                        let id = collapsed(&file.path);
                        let folded = ui.data(|d| d.get_temp(id).unwrap_or(false));
                        if file_row(ui, repo, &file.path, file.hits.len(), folded).clicked() {
                            ui.data_mut(|d| d.insert_temp(id, !folded));
                        }
                    }
                    Row::Hit(i, j) => {
                        let file = &results.files[i];
                        let hit = &file.hits[j];
                        if hit_row(ui, hit).clicked() {
                            let (line, col) = (Some(hit.line as u32), Some(hit.col as u32));
                            actions.push(Action::OpenFileAt {
                                repo: repo.id,
                                path: file.path.clone(),
                                line,
                                col,
                            });
                        }
                    }
                }
            }
        },
    );
}

/// The query box and its option toggles. Enter searches again, e.g. after files changed.
fn query_row(ui: &mut Ui, search: &mut TextSearch) {
    let id = Id::unique("text-search-query");
    if std::mem::take(&mut search.focus) {
        ui.memory_mut(|m| m.request_focus(id));
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let toggles_width = 3.0 * 24.0 + 6.0;
        let edit = TextEdit::singleline(&mut search.query.text)
            .id(id)
            .hint_text("Search")
            .desired_width((ui.available_width() - toggles_width).max(60.0))
            .margin(vec2(8.0, 5.0));
        let response = ui.add(edit);
        if response.changed() {
            search.edited();
        }
        if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            search.refresh();
            ui.memory_mut(|m| m.request_focus(id));
        }
        ui.add_space(4.0);
        let query = &mut search.query;
        toggle(ui, "Aa", "Match case", &mut query.match_case);
        toggle(ui, "ab", "Match whole word", &mut query.whole_word);
        toggle(ui, ".*", "Use regular expression", &mut query.regex);
    });
}

fn toggle(ui: &mut Ui, label: &str, hint: &str, on: &mut bool) {
    let color = if *on { ui.visuals().strong_text_color() } else { muted(ui) };
    let (rect, response) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    if *on {
        ui.painter().rect_filled(rect, CornerRadius::same(5), accent(ui).gamma_multiply(0.3));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let galley = ui.painter().layout_no_wrap(label.into(), FontId::monospace(11.5), color);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    if response.on_hover_text(hint).clicked() {
        *on = !*on;
    }
}

fn summary(results: &Results) -> String {
    let files = results.files.len();
    match results.hits {
        0 => "No results".into(),
        hits => {
            let more = if results.truncated { "+" } else { "" };
            let (r, f) =
                (if hits == 1 { "result" } else { "results" }, if files == 1 { "file" } else { "files" });
            format!("{hits}{more} {r} in {files} {f}")
        }
    }
}

/// `⌄ [icon] name  folder   count`; clicking folds the file's lines.
fn file_row(ui: &mut Ui, repo: &Repo, path: &Path, hits: usize, folded: bool) -> eframe::egui::Response {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let folder = path.parent().and_then(|p| p.strip_prefix(&repo.path).ok()).map(|p| p.display().to_string());
    let (response, ()) = row_sized(ui, false, 0.0, ROW_HEIGHT, |ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.label(RichText::new(if folded { "›" } else { "⌄" }).color(muted(ui)).monospace());
        file_icon::show(ui, 15.0, &name, false, false);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let count = RichText::new(hits.to_string()).size(10.5).color(muted(ui));
            ui.label(count);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(Label::new(RichText::new(&name).size(12.5)).truncate().selectable(false));
                if let Some(folder) = folder.filter(|f| !f.is_empty()) {
                    let folder = RichText::new(folder).size(11.0).color(muted(ui));
                    ui.add(Label::new(folder).truncate().selectable(false));
                }
            });
        });
    });
    response
}

/// A matching line, its matches highlighted, indented under its file.
fn hit_row(ui: &mut Ui, hit: &Hit) -> eframe::egui::Response {
    let (response, ()) = row_sized(ui, false, 22.0, ROW_HEIGHT, |ui| {
        let font = FontId::proportional(12.5);
        let plain = TextFormat::simple(font.clone(), ui.visuals().text_color().gamma_multiply(0.8));
        let found = TextFormat {
            background: accent(ui).gamma_multiply(0.35),
            color: ui.visuals().strong_text_color(),
            ..TextFormat::simple(font, Color32::WHITE)
        };
        let mut job = LayoutJob::default();
        let mut at = 0;
        for &(start, end) in &hit.ranges {
            job.append(&hit.preview[at..start], 0.0, plain.clone());
            job.append(&hit.preview[start..end], 0.0, found.clone());
            at = end;
        }
        job.append(&hit.preview[at..], 0.0, plain);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.max_width = ui.available_width();
        ui.add(Label::new(job).selectable(false));
    });
    response.on_hover_text(format!("Line {}", hit.line))
}
