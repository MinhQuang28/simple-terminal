use eframe::egui::{
    Align, CornerRadius, FontId, Label, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    pos2, vec2,
};

use super::{Page, page_icon};
use crate::ui::chrome::window_drag_area;
use crate::ui::widgets::muted;

const TRAFFIC_LIGHTS_HEIGHT: f32 = 40.0;
const ITEM_HEIGHT: f32 = 34.0;

const GROUPS: &[(&str, &[Page])] = &[
    ("General", &[Page::Appearance, Page::Typography]),
    ("Terminal", &[Page::Terminal]),
    ("AI", &[Page::Agents]),
];

pub fn show(ui: &mut Ui, page: &mut Page, open: &mut bool) {
    let strip = ui.max_rect().with_max_y(ui.max_rect().top() + TRAFFIC_LIGHTS_HEIGHT);
    window_drag_area(ui, strip);
    ui.add_space(TRAFFIC_LIGHTS_HEIGHT);

    if back_row(ui) {
        *open = false;
    }
    for (group, pages) in GROUPS {
        ui.add_space(18.0);
        ui.label(RichText::new(group.to_uppercase()).size(11.0).strong().color(muted(ui)));
        ui.add_space(6.0);
        for &p in *pages {
            if item(ui, p, *page == p) {
                *page = p;
            }
        }
    }
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        let version = concat!("Repodeck ", env!("CARGO_PKG_VERSION"));
        ui.label(RichText::new(version).size(11.0).color(muted(ui)));
    });
}

/// "‹ Back to app" with an `esc` key hint; returns whether it was clicked.
fn back_row(ui: &mut Ui) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ITEM_HEIGHT), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let mut child = ui.new_child(
        UiBuilder::new().max_rect(rect.shrink2(vec2(10.0, 0.0))).layout(Layout::left_to_right(Align::Center)),
    );
    // Selectable labels would be interactive and steal the row's hover where the text is.
    child.add(Label::new(RichText::new("‹").size(16.0).color(muted(&child))).selectable(false));
    child.add(Label::new(RichText::new("Back to app").size(14.0)).selectable(false));
    let galley = ui.painter().layout_no_wrap("esc".into(), FontId::proportional(11.5), muted(ui));
    let badge = Rect::from_center_size(
        pos2(rect.right() - 10.0 - galley.size().x / 2.0 - 6.0, rect.center().y),
        galley.size() + vec2(12.0, 4.0),
    );
    let stroke = Stroke::new(1.0, muted(ui).gamma_multiply(0.6));
    ui.painter().rect_stroke(badge, CornerRadius::same(5), stroke, StrokeKind::Inside);
    ui.painter().galley(badge.center() - galley.size() / 2.0, galley, muted(ui));
    response.clicked()
}

fn item(ui: &mut Ui, page: Page, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ITEM_HEIGHT), Sense::click());
    let visuals = ui.visuals();
    if selected {
        ui.painter().rect(
            rect,
            CornerRadius::same(8),
            visuals.selection.bg_fill.gamma_multiply(0.55),
            Stroke::new(1.0, visuals.selection.bg_fill),
            StrokeKind::Inside,
        );
    } else if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), visuals.widgets.hovered.weak_bg_fill);
    }
    let mut child = ui.new_child(
        UiBuilder::new().max_rect(rect.shrink2(vec2(10.0, 0.0))).layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 10.0;
    let (icon_rect, _) = child.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
    child.scope_builder(
        UiBuilder::new()
            .max_rect(icon_rect)
            .layout(Layout::centered_and_justified(eframe::egui::Direction::LeftToRight)),
        |ui| page_icon(ui, page, 18.0),
    );
    child.add(Label::new(RichText::new(page.title()).size(14.0)).selectable(false));
    response.clicked()
}
