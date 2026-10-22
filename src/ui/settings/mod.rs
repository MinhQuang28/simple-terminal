mod agent;
mod appearance;
mod controls;
mod nav;
mod terminal;
mod typography;

use eframe::egui::{self, CentralPanel, Frame, Key, Margin, Panel, RichText, ScrollArea, Ui};

use crate::font_catalog::FontCatalog;
use crate::model::Settings;
use crate::ui::claude_icon;
use crate::ui::widgets::muted;

const CONTENT_MAX_WIDTH: f32 = 760.0;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Appearance,
    Typography,
    Terminal,
    Agents,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Appearance => "Appearance",
            Page::Typography => "Typography",
            Page::Terminal => "Terminal",
            Page::Agents => "Agents",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Page::Appearance => "Theme, colors and the overall look of Repodeck.",
            Page::Typography => "Fonts and spacing for terminals, the editor and the app.",
            Page::Terminal => "Cursor and shell behaviour of terminal panes.",
            Page::Agents => "Commands Repodeck runs for Claude tabs.",
        }
    }
}

/// What the user changed this frame, so the app only redoes the expensive work needed.
#[derive(Default)]
pub struct Changes {
    pub theme: bool,
    /// Cursor shape or scrollback, which running terminals must be told about.
    pub terminal: bool,
    pub any: bool,
}

/// Shows settings as a full-window page; edits apply live to `settings`. Sets `open` to false when
/// the user goes back to the app.
pub fn show(
    ui: &mut Ui,
    open: &mut bool,
    page: &mut Page,
    settings: &mut Settings,
    fonts: &FontCatalog,
) -> Changes {
    let before = settings.clone();
    let window_bg = settings.theme.window_background();
    let content_bg = settings.theme.background.color();

    if ui.input(|i| i.key_pressed(Key::Escape)) && ui.memory(|m| m.focused().is_none()) {
        *open = false;
    }

    Panel::left("settings-nav")
        .resizable(false)
        .exact_size(250.0)
        .show_separator_line(false)
        .frame(Frame::new().fill(window_bg).inner_margin(Margin { left: 12, right: 12, top: 0, bottom: 12 }))
        .show(ui, |ui| nav::show(ui, page, open));

    CentralPanel::default().frame(Frame::new().fill(content_bg)).show(ui, |ui| {
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let width = ui.available_width().min(CONTENT_MAX_WIDTH);
            ui.horizontal(|ui| {
                ui.add_space(40.0);
                ui.vertical(|ui| {
                    ui.set_max_width(width - 80.0);
                    ui.add_space(40.0);
                    page_header(ui, *page);
                    match page {
                        Page::Appearance => appearance::show(ui, settings),
                        Page::Typography => typography::show(ui, settings, fonts),
                        Page::Terminal => terminal::show(ui, settings),
                        Page::Agents => agent::show(ui, settings),
                    }
                    ui.add_space(40.0);
                });
            });
        });
    });

    Changes {
        theme: settings.theme != before.theme,
        terminal: settings.cursor_shape != before.cursor_shape
            || settings.scrollback_lines != before.scrollback_lines,
        any: *settings != before,
    }
}

fn page_header(ui: &mut Ui, page: Page) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        page_icon(ui, page, 22.0);
        ui.label(RichText::new(page.title()).size(24.0).strong().color(ui.visuals().strong_text_color()));
    });
    ui.add_space(4.0);
    ui.label(RichText::new(page.subtitle()).size(13.5).color(muted(ui)));
}

/// Small glyph identifying a page in the nav and header.
fn page_icon(ui: &mut Ui, page: Page, size: f32) {
    let (glyph, font) = match page {
        Page::Appearance => ("◐", egui::FontId::proportional(size * 0.8)),
        Page::Typography => ("Aa", egui::FontId::proportional(size * 0.7)),
        Page::Terminal => (">_", egui::FontId::monospace(size * 0.65)),
        Page::Agents => return claude_icon::icon(ui, size * 0.6),
    };
    let icon = egui::Label::new(RichText::new(glyph).font(font).color(ui.visuals().text_color()));
    ui.add(icon.selectable(false));
}
