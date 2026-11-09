//! App shortcuts by command name, with the user's overrides from `keys.json` in the data folder:
//! `{"palette.toggle": "cmd+shift+k", "pane.zoom": ["cmd+shift+enter", "ctrl+z"], "tab.reopen": null}`.
//! A string or a list of strings rebinds the command; `null` unbinds it. Read at startup.

use std::collections::HashMap;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use serde_json::Value;

use crate::store;

/// Every bindable command and its default keys, in the order shortcuts are checked: a binding
/// with Shift has to come before the same key without it, since a binding without Shift also
/// matches with Shift held.
pub const DEFAULTS: &[(&str, &[&str])] = &[
    // The Code view's editor keys, as VS Code binds them; they act only while it shows.
    ("editor.copy_line_up", &["alt+shift+up"]),
    ("editor.copy_line_down", &["alt+shift+down"]),
    ("editor.cursor_above", &["cmd+alt+up"]),
    ("editor.cursor_below", &["cmd+alt+down"]),
    ("editor.select_all_matches", &["cmd+shift+l"]),
    ("editor.delete_line", &["cmd+shift+k"]),
    ("editor.format", &["alt+shift+f"]),
    ("editor.peek_definition", &["alt+f12"]),
    ("editor.move_line_up", &["alt+up"]),
    ("editor.move_line_down", &["alt+down"]),
    ("editor.select_next", &["cmd+d"]),
    ("editor.undo_cursor", &["cmd+u"]),
    ("editor.toggle_comment", &["cmd+/"]),
    ("editor.go_to_line", &["ctrl+g"]),
    ("editor.split", &["cmd+\\"]),
    ("editor.definition", &["f12"]),
    ("editor.command_palette", &["cmd+shift+p", "f1"]),
    ("palette.toggle", &["cmd+shift+k"]),
    ("tab.move_left", &["cmd+alt+shift+left"]),
    ("tab.move_right", &["cmd+alt+shift+right"]),
    ("tab.new_agent_worktree", &["cmd+alt+shift+t"]),
    ("pane.swap_left", &["ctrl+cmd+shift+left"]),
    ("pane.swap_right", &["ctrl+cmd+shift+right"]),
    ("pane.swap_up", &["ctrl+cmd+shift+up"]),
    ("pane.swap_down", &["ctrl+cmd+shift+down"]),
    ("pane.focus_left", &["cmd+alt+left"]),
    ("pane.focus_right", &["cmd+alt+right"]),
    ("pane.focus_up", &["cmd+alt+up"]),
    ("pane.focus_down", &["cmd+alt+down"]),
    ("tab.reopen", &["cmd+alt+t"]),
    ("pane.zoom", &["cmd+shift+enter"]),
    ("tab.previous", &["cmd+shift+["]),
    ("tab.next", &["cmd+shift+]"]),
    ("tab.new_agent", &["cmd+shift+t"]),
    ("pane.split_down", &["cmd+shift+d"]),
    ("palette.prompts", &["cmd+shift+p"]),
    ("tab.close", &["cmd+shift+w"]),
    ("attention.next", &["cmd+ctrl+n"]),
    ("tab.new", &["cmd+t"]),
    ("palette.files", &["cmd+p"]),
    ("pane.split_right", &["cmd+d"]),
    ("pane.close", &["cmd+w"]),
    ("file.save", &["cmd+s"]),
    ("panel.toggle", &["cmd+b"]),
    ("search.files", &["cmd+shift+f"]),
    ("view.terminals", &["ctrl+1"]),
    ("view.code", &["ctrl+2"]),
    ("settings.open", &["cmd+,"]),
    ("font.bigger", &["cmd+=", "cmd++"]),
    ("font.smaller", &["cmd+-"]),
];

pub struct Keymap {
    bindings: HashMap<&'static str, Vec<KeyboardShortcut>>,
}

impl Default for Keymap {
    fn default() -> Self {
        let bindings = DEFAULTS
            .iter()
            .map(|(command, keys)| (*command, keys.iter().filter_map(|k| parse(k)).collect()))
            .collect();
        Self { bindings }
    }
}

impl Keymap {
    /// The defaults with `keys.json` applied; a broken file is reported and ignored.
    pub fn load() -> Self {
        let mut keymap = Self::default();
        let Some(path) = store::data_dir().map(|d| d.join("keys.json")) else { return keymap };
        let Ok(text) = std::fs::read_to_string(&path) else { return keymap };
        match serde_json::from_str::<HashMap<String, Value>>(&text) {
            Ok(overrides) => keymap.apply(&overrides),
            Err(err) => log::warn!("Ignoring {}: {err}", path.display()),
        }
        keymap
    }

    fn apply(&mut self, overrides: &HashMap<String, Value>) {
        for (command, value) in overrides {
            let Some((name, _)) = DEFAULTS.iter().find(|(c, _)| c == command) else {
                log::warn!("keys.json: unknown command {command}");
                continue;
            };
            let keys: Vec<&str> = match value {
                Value::Null => Vec::new(),
                Value::String(key) => vec![key],
                Value::Array(keys) => keys.iter().filter_map(Value::as_str).collect(),
                _ => {
                    log::warn!("keys.json: {command} needs a string, a list or null");
                    continue;
                }
            };
            let parsed: Vec<_> = keys.iter().filter_map(|k| parse(k)).collect();
            if parsed.len() < keys.len() {
                log::warn!("keys.json: {command} has a key that can't be read: {keys:?}");
            }
            self.bindings.insert(name, parsed);
        }
    }

    /// The first key bound to `command`, the one menus show.
    pub fn first(&self, command: &str) -> Option<&KeyboardShortcut> {
        self.bindings.get(command)?.first()
    }

    /// Whether a key bound to `command` was pressed this frame; consumes it.
    pub fn pressed(&self, ctx: &egui::Context, command: &str) -> bool {
        let Some(shortcuts) = self.bindings.get(command) else { return false };
        shortcuts.iter().any(|s| ctx.input_mut(|i| i.consume_shortcut(s)))
    }

    /// Every command (other than those in `skip`) whose key was pressed this frame; consumes
    /// them. Bindings with more modifiers are matched first: egui ignores an extra Shift or Alt
    /// when matching, so ⌘⇧D must reach a ⌘⇧D binding before a ⌘D one can take it, whatever
    /// the user rebound.
    pub fn fired(&self, ctx: &egui::Context, skip: &[&str]) -> Vec<&'static str> {
        let mut bindings: Vec<(&'static str, &KeyboardShortcut)> = self
            .bindings
            .iter()
            .filter(|(command, _)| !skip.contains(command))
            .flat_map(|(command, shortcuts)| shortcuts.iter().map(move |s| (*command, s)))
            .collect();
        let weight = |s: &KeyboardShortcut| {
            let m = s.modifiers;
            [m.command || m.mac_cmd, m.ctrl, m.alt, m.shift].into_iter().filter(|on| *on).count()
        };
        // Ties keep the defaults' order, so equal bindings resolve as they always have.
        let order = |command: &str| DEFAULTS.iter().position(|(c, _)| *c == command);
        bindings.sort_by_key(|(command, s)| (std::cmp::Reverse(weight(s)), order(command)));
        let mut fired = Vec::new();
        for (command, shortcut) in bindings {
            if ctx.input_mut(|i| i.consume_shortcut(shortcut)) && !fired.contains(&command) {
                fired.push(command);
            }
        }
        fired
    }
}

/// Reads `cmd+shift+k`-style text: modifiers (cmd, shift, alt/option, ctrl) and one key name.
pub fn parse(text: &str) -> Option<KeyboardShortcut> {
    let text = text.trim().to_lowercase();
    // A trailing "+" is the plus key itself, as in "cmd++".
    let (mods, key) = match text.strip_suffix("++") {
        Some(mods) => (mods, "+"),
        None => text.rsplit_once('+').unwrap_or(("", text.as_str())),
    };
    let mut modifiers = Modifiers::NONE;
    for m in mods.split('+').filter(|m| !m.is_empty()) {
        modifiers |= match m {
            "cmd" | "command" | "super" => Modifiers::COMMAND,
            "shift" => Modifiers::SHIFT,
            "alt" | "option" | "opt" => Modifiers::ALT,
            "ctrl" | "control" => Modifiers::CTRL,
            _ => return None,
        };
    }
    let key = match key {
        "left" | "right" | "up" | "down" => {
            Key::from_name(&format!("Arrow{}{}", key[..1].to_uppercase(), &key[1..]))
        }
        "esc" => Some(Key::Escape),
        other => Key::from_name(other)
            .or_else(|| Key::from_name(&capitalized(other)))
            .or_else(|| Key::ALL.iter().copied().find(|k| k.name().eq_ignore_ascii_case(other))),
    }?;
    // A bare key would take that key away from typing; function keys type nothing.
    let function_key = key.name().strip_prefix('F').is_some_and(|n| n.parse::<u8>().is_ok());
    (!modifiers.is_none() || function_key).then(|| KeyboardShortcut::new(modifiers, key))
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_modifiers_and_key_names() {
        let k = |m, key| Some(KeyboardShortcut::new(m, key));
        assert_eq!(parse("cmd+shift+k"), k(Modifiers::COMMAND | Modifiers::SHIFT, Key::K));
        assert_eq!(parse("Cmd+Alt+Left"), k(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowLeft));
        assert_eq!(parse("cmd++"), k(Modifiers::COMMAND, Key::Plus));
        assert_eq!(parse("cmd+,"), k(Modifiers::COMMAND, Key::Comma));
        assert_eq!(
            parse("ctrl+cmd+shift+enter"),
            k(Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter)
        );
        assert_eq!(parse("cmd+pageup"), k(Modifiers::COMMAND, Key::PageUp));
        assert_eq!(parse("k"), None, "a bare key would eat typing");
        assert_eq!(parse("hyper+k"), None);
        assert_eq!(parse("cmd+nope"), None);
    }

    #[test]
    fn every_default_parses() {
        for (command, keys) in DEFAULTS {
            for key in *keys {
                assert!(parse(key).is_some(), "{command}: {key}");
            }
        }
    }

    #[test]
    fn more_specific_bindings_win_whatever_the_check_order() {
        let mut keymap = Keymap::default();
        let overrides: HashMap<String, Value> = serde_json::from_str(r#"{"pane.zoom": "cmd+d"}"#).unwrap();
        keymap.apply(&overrides);
        let ctx = egui::Context::default();
        let press = egui::Event::Key {
            key: Key::D,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND | Modifiers::SHIFT,
        };
        ctx.begin_pass(egui::RawInput { events: vec![press], ..Default::default() });
        assert_eq!(keymap.fired(&ctx, &[]), vec!["pane.split_down"]);
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
    }

    #[test]
    fn overrides_rebind_and_unbind() {
        let mut keymap = Keymap::default();
        let overrides: HashMap<String, Value> =
            serde_json::from_str(r#"{"tab.new": "ctrl+t", "tab.reopen": null, "bogus": "cmd+x"}"#).unwrap();
        keymap.apply(&overrides);
        assert_eq!(keymap.bindings["tab.new"], vec![KeyboardShortcut::new(Modifiers::CTRL, Key::T)]);
        assert!(keymap.bindings["tab.reopen"].is_empty());
    }
}
