//! VS Code's activity bar for the Code sidebar: a column of icons picking what the sidebar shows,
//! the active one lit with an accent edge, and the change count as a badge on Source Control.

use std::sync::OnceLock;

use eframe::egui::load::TexturePoll;
use eframe::egui::{Color32, FontId, Image, Rect, Sense, Ui, pos2, vec2};

use super::Action;
use super::widgets::{accent, muted};
use crate::model::SidePanel;

pub const WIDTH: f32 = 40.0;
const BUTTON: f32 = 40.0;
const ICON: f32 = 22.0;

/// Lucide icons (assets/icons/activity, ISC), by the panel each opens.
const ITEMS: &[(SidePanel, &str, &str, &[u8])] = &[
    (SidePanel::Files, "Explorer", "files", include_bytes!("../../assets/icons/activity/files.svg")),
    (SidePanel::Search, "Search", "search", include_bytes!("../../assets/icons/activity/search.svg")),
    (
        SidePanel::Changes,
        "Source Control",
        "git-branch",
        include_bytes!("../../assets/icons/activity/git-branch.svg"),
    ),
    (SidePanel::Library, "Library", "library", include_bytes!("../../assets/icons/activity/library-big.svg")),
];

/// The column of buttons; `changes` badges Source Control when non-zero.
pub fn show(ui: &mut Ui, current: SidePanel, changes: usize, actions: &mut Vec<Action>) {
    for (index, (panel, label, name, _)) in ITEMS.iter().enumerate() {
        let (rect, response) = ui.allocate_exact_size(vec2(WIDTH, BUTTON), Sense::click());
        let active = *panel == current;
        let color = if active || response.hovered() { ui.visuals().strong_text_color() } else { muted(ui) };
        if active {
            let edge = Rect::from_min_size(rect.left_top() + vec2(0.0, 6.0), vec2(2.0, BUTTON - 12.0));
            ui.painter().rect_filled(edge, 1.0, accent(ui));
        }
        let icon = Rect::from_center_size(rect.center(), vec2(ICON, ICON));
        paint(ui, icon, index, name, color);
        if *panel == SidePanel::Changes && changes > 0 {
            badge(ui, icon.right_bottom() + vec2(-1.0, -2.0), changes);
        }
        let hint = match panel {
            SidePanel::Search => crate::keymap_hint::with_hint(label, "search.files"),
            _ => label.to_string(),
        };
        if response.on_hover_text(hint).clicked() {
            actions.push(Action::SetSidePanel(*panel));
        }
    }
}

/// Paints icon `index` white-tinted to `color`, stroked a little thinner than Lucide's default,
/// as VS Code's codicons are.
fn paint(ui: &Ui, rect: Rect, index: usize, name: &str, color: Color32) {
    static PREPARED: OnceLock<Vec<&'static [u8]>> = OnceLock::new();
    let svgs = PREPARED.get_or_init(|| {
        let prepare = |svg: &[u8]| -> &'static [u8] {
            let text = String::from_utf8_lossy(svg).replace("currentColor", "#ffffff");
            let text = text.replace("stroke-width=\"2\"", "stroke-width=\"1.6\"");
            Box::leak(text.into_bytes().into_boxed_slice())
        };
        ITEMS.iter().map(|(_, _, _, svg)| prepare(svg)).collect()
    });
    let image = Image::from_bytes(format!("bytes://activity/{name}.svg"), svgs[index]);
    let size = rect.size() * ui.pixels_per_point();
    let Ok(TexturePoll::Ready { texture }) = image.load_for_size(ui.ctx(), size) else { return };
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    ui.painter().image(texture.id, rect, uv, color);
}

/// A round count, as VS Code badges its Source Control icon; "99+" past two digits.
fn badge(ui: &Ui, center: eframe::egui::Pos2, count: usize) {
    let text = if count > 99 { "99+".to_string() } else { count.to_string() };
    let galley = ui.painter().layout_no_wrap(text, FontId::proportional(9.0), Color32::WHITE);
    let width = (galley.size().x + 6.0).max(14.0);
    let rect = Rect::from_center_size(center, vec2(width, 14.0));
    ui.painter().rect_filled(rect, 7.0, accent(ui));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Context, RawInput, Vec2};

    use super::*;

    #[test]
    fn every_icon_renders() {
        let ctx = Context::default();
        egui_extras::install_image_loaders(&ctx);
        ctx.begin_pass(RawInput::default());
        for (_, _, name, svg) in ITEMS {
            let image = Image::from_bytes(format!("bytes://activity-test/{name}.svg"), *svg);
            let ready = matches!(image.load_for_size(&ctx, Vec2::splat(22.0)), Ok(TexturePoll::Ready { .. }));
            assert!(ready, "{name}");
        }
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
    }
}
