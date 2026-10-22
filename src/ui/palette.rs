use eframe::egui::{
    Align, Align2, Area, Context, CornerRadius, FontId, Frame, Id, Key, Label, Layout, Modifiers, RichText,
    ScrollArea, Sense, Stroke, TextEdit, Ui, UiBuilder, vec2,
};

use super::Action;
use super::design::{self, RADIUS_ROW, ROW_HEIGHT, TEXT_BODY, TEXT_CAPTION, kbd};
use super::palette_items::{Item, Kind, items};
use super::widgets::muted;
use crate::model::AppState;
use crate::session_pool::SessionPool;

const WIDTH: f32 = 560.0;
const MAX_RESULTS: usize = 12;
const TOP_OFFSET: f32 = 110.0;

/// Command palette (⌘K): fuzzy-filters commands and tab switches, runs the chosen one.
#[derive(Default)]
pub struct Palette {
    query: String,
    selected: usize,
}

impl Palette {
    /// A palette already filtered to `query`, e.g. "Prompt: ".
    pub fn with_query(query: &str) -> Self {
        Self { query: query.to_string(), selected: 0 }
    }

    /// Shows the palette; returns true when it should close.
    pub fn show(
        &mut self,
        ctx: &Context,
        state: &AppState,
        pool: &SessionPool,
        actions: &mut Vec<Action>,
    ) -> bool {
        let items = items(state, pool);
        let mut ranked: Vec<(i64, &Item)> =
            items.iter().filter_map(|i| score(&self.query, &i.label).map(|s| (s, i))).collect();
        ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        ranked.truncate(MAX_RESULTS);
        self.selected = self.selected.min(ranked.len().saturating_sub(1));

        let mut close = false;
        let (up, down, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.key_pressed(Key::Enter),
            )
        });
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if down && self.selected + 1 < ranked.len() {
            self.selected += 1;
        }
        let area = Area::new(Id::unique("palette")).anchor(Align2::CENTER_TOP, vec2(0.0, TOP_OFFSET));
        let frame = design::surface_frame(&ctx.global_style().visuals, 0);
        let modal =
            eframe::egui::Modal::new(Id::unique("palette-modal")).area(area).frame(frame).show(ctx, |ui| {
                ui.set_width(WIDTH);
                ui.spacing_mut().item_spacing.y = 0.0;
                if self.search_field(ui) {
                    self.selected = 0;
                }
                design::divider(ui);
                ui.add_space(2.0);
                let list_height = ROW_HEIGHT * ranked.len().max(1) as f32;
                ScrollArea::vertical().max_height(list_height).min_scrolled_height(list_height).show(
                    ui,
                    |ui| {
                        for (index, (_, item)) in ranked.iter().enumerate() {
                            if row(ui, item, index == self.selected) {
                                actions.push(item.action.clone());
                                close = true;
                            }
                        }
                        if ranked.is_empty() {
                            design::empty_state(ui, "No matches");
                        }
                    },
                );
                hints(ui);
            });
        if enter && let Some((_, item)) = ranked.get(self.selected) {
            actions.push(item.action.clone());
            close = true;
        }
        close || modal.should_close()
    }

    /// Flat search field with a magnifier at its left; returns true when the text changed.
    fn search_field(&mut self, ui: &mut Ui) -> bool {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
        let painter = ui.painter();
        let stroke = Stroke::new(1.5, muted(ui));
        let center = rect.left_center() + vec2(22.0, -1.5);
        painter.circle_stroke(center, 5.5, stroke);
        painter.line_segment([center + vec2(4.0, 4.0), center + vec2(8.0, 8.0)], stroke);
        let field = rect.with_min_x(rect.left() + 42.0).with_max_x(rect.right() - 14.0);
        let mut child =
            ui.new_child(UiBuilder::new().max_rect(field).layout(Layout::left_to_right(Align::Center)));
        let edit = child.add(
            TextEdit::singleline(&mut self.query)
                .hint_text("Search commands, tabs, repos…")
                .desired_width(f32::INFINITY)
                .frame(Frame::NONE)
                .margin(vec2(0.0, 0.0))
                .font(FontId::proportional(15.0)),
        );
        edit.request_focus();
        edit.changed()
    }
}

fn row(ui: &mut Ui, item: &Item, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    let fill = if selected {
        Some(design::selection_fill(ui))
    } else if response.hovered() {
        Some(ui.visuals().widgets.hovered.weak_bg_fill)
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect.shrink2(vec2(6.0, 0.0)), CornerRadius::same(RADIUS_ROW), fill);
    }
    let inner = rect.shrink2(vec2(16.0, 0.0));
    let mut child =
        ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)));
    child.spacing_mut().item_spacing.x = 8.0;
    if !item.hint.is_empty() {
        kbd(&mut child, item.hint);
    }
    let kind = match item.kind {
        Kind::Command => None,
        Kind::Tab => Some("Tab"),
        Kind::Repo => Some("Repository"),
        Kind::Prompt => Some("Prompt"),
    };
    if let Some(kind) = kind {
        child.label(RichText::new(kind).size(TEXT_CAPTION).color(muted(&child)));
    }
    child.with_layout(Layout::left_to_right(Align::Center), |ui| {
        let color = if selected { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
        ui.add(
            Label::new(RichText::new(&item.label).size(TEXT_BODY).color(color)).truncate().selectable(false),
        );
    });
    response.clicked()
}

/// Key legend along the bottom edge.
fn hints(ui: &mut Ui) {
    design::divider(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    let mut child = ui.new_child(
        UiBuilder::new().max_rect(rect.shrink2(vec2(16.0, 0.0))).layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 6.0;
    for (keys, what) in [("↑↓", "navigate"), ("↩", "run"), ("esc", "close")] {
        kbd(&mut child, keys);
        child.label(RichText::new(what).size(TEXT_CAPTION).color(muted(&child)));
        child.add_space(8.0);
    }
}

/// Subsequence match score: higher for matches at word starts and consecutive characters;
/// `None` when `query` is not a subsequence of `text`. Empty queries match everything.
fn score(query: &str, text: &str) -> Option<i64> {
    let text: Vec<char> = text.to_lowercase().chars().collect();
    let mut score = 0;
    let mut last = None;
    for q in query.to_lowercase().chars().filter(|c| !c.is_whitespace()) {
        let from = last.map_or(0, |i| i + 1);
        let index = (from..text.len()).find(|i| text[*i] == q)?;
        score += 1;
        if index > 0 && last == Some(index - 1) {
            score += 3;
        }
        if index == 0 || !text[index - 1].is_alphanumeric() {
            score += 2;
        }
        last = Some(index);
    }
    Some(score - last.unwrap_or(0) as i64 / 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_subsequences_and_rejects_others() {
        assert!(score("nt", "New terminal") > score("nt", "Settings for tnt"));
        assert_eq!(score("xyz", "New terminal"), None);
        assert!(score("", "anything").is_some());
    }
}
