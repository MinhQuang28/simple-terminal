use eframe::egui::{TextEdit, Ui};

use super::controls::{card, number_field, section_label, segmented, toggle};
use crate::model::{CursorShape, Settings};

pub fn show(ui: &mut Ui, settings: &mut Settings) {
    section_label(ui, "Cursor");
    card(ui, |card| {
        card.row("Cursor shape", "Used unless the running program asks for another shape.", |ui| {
            segmented(
                ui,
                &mut settings.cursor_shape,
                &[
                    (CursorShape::Block, "Block"),
                    (CursorShape::Beam, "Bar"),
                    (CursorShape::Underline, "Underline"),
                ],
            );
        });
        card.row("Blinking cursor", "Blink the cursor while a terminal pane is focused.", |ui| {
            toggle(ui, &mut settings.cursor_blink);
        });
    });

    section_label(ui, "Behaviour");
    card(ui, |card| {
        card.row("Scrollback lines", "History kept per pane; applies to open panes too.", |ui| {
            number_field(ui, &mut settings.scrollback_lines, 500..=200_000, 1000.0, "");
        });
        card.row("Option as Meta", "Send ⌥+key as Esc-prefixed keys for Emacs-style line editing.", |ui| {
            toggle(ui, &mut settings.option_as_meta);
        });
        card.row("Copy on select", "Selected text goes to the clipboard as soon as you release.", |ui| {
            toggle(ui, &mut settings.copy_on_select);
        });
    });

    section_label(ui, "Shell");
    card(ui, |card| {
        card.row("Shell program", "Started as a login shell. Leave empty to use $SHELL.", |ui| {
            ui.add(TextEdit::singleline(&mut settings.shell).hint_text("$SHELL").desired_width(260.0));
        });
    });

    section_label(ui, "Links");
    card(ui, |card| {
        card.row(
            "Open ⌘-clicked files with",
            "A shell command using {path}, {line} and {col}. Leave empty for the built-in editor.",
            |ui| {
                ui.add(
                    TextEdit::singleline(&mut settings.external_editor)
                        .hint_text("code -g {path}:{line}:{col}")
                        .code_editor()
                        .desired_width(260.0),
                );
            },
        );
    });
}
