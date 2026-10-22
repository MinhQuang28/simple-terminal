use eframe::egui::{Color32, RichText, Ui};

use super::controls::{card, section_label, toggle};
use crate::attention;
use crate::model::Settings;
use crate::notification_permission::{self, NotificationPermission, Permission};
use crate::ui::dialog_kit::{Tone, button};
use crate::ui::widgets::muted;

const ALLOWED: Color32 = Color32::from_rgb(0x4f, 0xb8, 0x6e);

pub fn show(ui: &mut Ui, settings: &mut Settings, permission: &NotificationPermission) {
    section_label(ui, "System notifications");
    card(ui, |card| {
        card.row(
            "Notifications",
            "Post to Notification Center when a pane you aren't watching needs you: an agent \
             finishes or asks, a long command ends, or a script calls `repodeck notify`.",
            |ui| {
                toggle(ui, &mut settings.notifications);
            },
        );
        card.row("Sound", "Play the system alert sound with each notification.", |ui| {
            toggle(ui, &mut settings.notification_sound);
        });
        card.row("Test", "Posts a notification now, with the settings above.", |ui| {
            if button(ui, "Send test notification", Tone::Secondary).clicked() {
                attention::notify("Repodeck", "Notifications are working.", settings.notification_sound);
            }
        });
    });

    section_label(ui, "macOS permission");
    card(ui, |card| {
        let status = permission.status();
        card.row("Allowed by macOS", "Notifications only show when System Settings allows them.", |ui| {
            match status.map(|s| s.permission) {
                None if !NotificationPermission::available() => {
                    ui.label(RichText::new("Unavailable outside the app bundle").color(muted(ui)));
                }
                None => {
                    ui.label(RichText::new("Checking…").color(muted(ui)));
                }
                Some(Permission::Allowed) => {
                    if button(ui, "System Settings…", Tone::Secondary).clicked() {
                        notification_permission::open_system_settings();
                    }
                    ui.label(RichText::new("● Allowed").color(ALLOWED));
                }
                Some(Permission::Denied) => {
                    if button(ui, "Open System Settings…", Tone::Primary).clicked() {
                        notification_permission::open_system_settings();
                    }
                    ui.label(RichText::new("● Not allowed").color(ui.visuals().error_fg_color));
                }
                Some(Permission::NotAsked) => {
                    if button(ui, "Ask…", Tone::Primary).clicked() {
                        permission.request();
                    }
                    ui.label(RichText::new("● Not asked yet").color(muted(ui)));
                }
            }
        });
        if let Some(status) = status.filter(|s| s.permission == Permission::Allowed) {
            card.row("Sound allowed by macOS", "The sound switch for Repodeck in System Settings.", |ui| {
                let (text, color) = if status.sound_allowed {
                    ("● On", ALLOWED)
                } else {
                    ("● Off", ui.visuals().error_fg_color)
                };
                ui.label(RichText::new(text).color(color));
            });
        }
    });
}
