use eframe::egui::{
    CentralPanel, Context, Event, Frame, Margin, Panel, PointerButton, RawInput, Rect, ScrollArea, Sense,
    pos2, vec2,
};

use crate::file_tree::FileTree;
use crate::library_cache::LibraryCache;
use crate::model::{AppState, Repo};
use crate::text_search_runner::TextSearch;
use crate::ui::code_sidebar;

#[test]
fn the_code_sidebar_resizes_from_either_side_of_its_edge() {
    let ctx = Context::default();
    egui_extras::install_image_loaders(&ctx);
    let state = AppState::default();
    let repo: Repo = serde_json::from_value(serde_json::json!({
        "id": uuid::Uuid::nil(), "name": "repo", "path": env!("CARGO_MANIFEST_DIR"), "code_view": true
    }))
    .unwrap();
    let mut tree = FileTree::new(ctx.clone());
    let mut search = TextSearch::new(ctx.clone());
    let library = LibraryCache::new(ctx.clone());
    let mut frame = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 900.0))),
            events,
            ..Default::default()
        };
        let mut right = 0.0;
        let mut output = ctx.run_ui(input, |ui| {
            let mut actions = Vec::new();
            let panel = Panel::left("code-sidebar")
                .resizable(true)
                .show_separator_line(false)
                .default_size(330.0)
                .size_range(270.0..=900.0)
                .frame(Frame::new().inner_margin(Margin { left: 10, right: 6, top: 0, bottom: 8 }))
                .show(ui, |ui| {
                    let sources = code_sidebar::Sources {
                        status: None,
                        tree: &mut tree,
                        search: &mut search,
                        library: &library,
                    };
                    code_sidebar::show(ui, &state, &repo, sources, &mut actions)
                });
            right = panel.response.rect.right();
            CentralPanel::default().frame(Frame::new()).show(ui, |ui| {
                ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                    ui.allocate_response(vec2(3000.0, 3000.0), Sense::click_and_drag());
                });
            });
            super::resize_handle(ui, panel.response.rect, "code-sidebar", egui::Color32::BLACK);
        });
        output.textures_delta.clear();
        right
    };
    let mut r = 0.0;
    for _ in 0..3 {
        r = frame(vec![]);
    }
    assert_eq!(r, 330.0);
    for dx in [-3.0f32, 0.0, 3.0] {
        let start = pos2(r + dx, 400.0);
        frame(vec![Event::PointerMoved(start)]);
        frame(vec![Event::PointerButton {
            pos: start,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        let mut after = 0.0;
        for step in 1..=10 {
            after = frame(vec![Event::PointerMoved(pos2(start.x + step as f32 * 10.0, 400.0))]);
        }
        let end = pos2(start.x + 100.0, 400.0);
        frame(vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        let settled = frame(vec![]);
        assert_eq!(after, start.x + 100.0, "the edge follows a drag begun {dx} px off it");
        assert_eq!(settled, after);
        r = settled;
    }
}
