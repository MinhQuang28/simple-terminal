//! The keymap in effect, read once at startup, and its keys written the macOS way ("⇧⌘W") for
//! menus, tooltips and the command palette, so they show what the user bound in `keys.json`.

use std::sync::OnceLock;

use eframe::egui::{Key, KeyboardShortcut};

use crate::keymap::Keymap;

static ACTIVE: OnceLock<Keymap> = OnceLock::new();

/// Makes `keymap` the one shortcuts and hints use; only the first call counts.
pub fn install(keymap: Keymap) {
    let _ = ACTIVE.set(keymap);
}

/// The installed keymap, or the defaults before (and in tests without) one.
pub fn active() -> &'static Keymap {
    ACTIVE.get_or_init(Keymap::default)
}

/// `command`'s key, e.g. "⇧⌘W"; empty when it's unbound.
pub fn hint(command: &str) -> String {
    active().first(command).map(format).unwrap_or_default()
}

/// "Close pane (⌘W)", or just the label when the command is unbound.
pub fn with_hint(label: &str, command: &str) -> String {
    match hint(command) {
        keys if keys.is_empty() => label.to_string(),
        keys => format!("{label} ({keys})"),
    }
}

/// Modifiers in Apple's order (⌃⌥⇧⌘), then the key.
pub fn format(shortcut: &KeyboardShortcut) -> String {
    let m = shortcut.modifiers;
    let mut out = String::new();
    for (on, symbol) in [(m.ctrl, "⌃"), (m.alt, "⌥"), (m.shift, "⇧"), (m.command || m.mac_cmd, "⌘")] {
        if on {
            out.push_str(symbol);
        }
    }
    out.push_str(match shortcut.logical_key {
        Key::ArrowLeft => "←",
        Key::ArrowRight => "→",
        Key::ArrowUp => "↑",
        Key::ArrowDown => "↓",
        Key::Enter => "↩",
        Key::Escape => "⎋",
        Key::Tab => "⇥",
        Key::Backspace => "⌫",
        Key::Delete => "⌦",
        Key::Space => "Space",
        Key::Minus => "-",
        key => key.symbol_or_name(),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::parse;

    #[test]
    fn writes_keys_the_macos_way() {
        let f = |text: &str| format(&parse(text).unwrap());
        assert_eq!(f("cmd+shift+w"), "⇧⌘W");
        assert_eq!(f("cmd+alt+shift+left"), "⌥⇧⌘←");
        assert_eq!(f("ctrl+cmd+n"), "⌃⌘N");
        assert_eq!(f("cmd+shift+enter"), "⇧⌘↩");
        assert_eq!(f("cmd+,"), "⌘,");
        assert_eq!(f("cmd+-"), "⌘-");
        assert_eq!(f("cmd+shift+]"), "⇧⌘]");
        assert_eq!(hint("tab.close"), "⇧⌘W");
        assert_eq!(with_hint("Settings", "settings.open"), "Settings (⌘,)");
        assert_eq!(with_hint("Nothing", "no.such.command"), "Nothing");
    }
}
