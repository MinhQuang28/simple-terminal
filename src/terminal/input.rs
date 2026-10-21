use alacritty_terminal::term::TermMode;
use eframe::egui::{Key, Modifiers};

/// Encodes a key press as the bytes a terminal program expects, or `None` when the key
/// produces plain text (delivered separately via text events) or is an app shortcut.
pub fn key_to_bytes(key: Key, mods: Modifiers, mode: TermMode) -> Option<Vec<u8>> {
    if mods.mac_cmd || mods.command && !mods.ctrl {
        return None;
    }
    if mods.ctrl
        && !mods.alt
        && let Some(byte) = ctrl_byte(key)
    {
        return Some(vec![byte]);
    }

    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let modifier_param = xterm_modifier(mods);
    let csi_or_ss3 = |letter: char| -> Vec<u8> {
        match modifier_param {
            Some(m) => format!("\x1b[1;{m}{letter}").into_bytes(),
            None if app_cursor => format!("\x1bO{letter}").into_bytes(),
            None => format!("\x1b[{letter}").into_bytes(),
        }
    };
    let tilde = |code: u8| -> Vec<u8> {
        match modifier_param {
            Some(m) => format!("\x1b[{code};{m}~").into_bytes(),
            None => format!("\x1b[{code}~").into_bytes(),
        }
    };

    let bytes = match key {
        // Shift/Option+Enter insert a newline in agent CLIs such as Claude Code.
        Key::Enter if mods.shift || mods.alt => b"\x1b\r".to_vec(),
        Key::Enter => b"\r".to_vec(),
        Key::Backspace if mods.alt => b"\x1b\x7f".to_vec(),
        Key::Backspace => b"\x7f".to_vec(),
        Key::Tab if mods.shift => b"\x1b[Z".to_vec(),
        Key::Tab => b"\t".to_vec(),
        Key::Escape => b"\x1b".to_vec(),
        Key::ArrowUp => csi_or_ss3('A'),
        Key::ArrowDown => csi_or_ss3('B'),
        // Option+Left/Right jump words, matching macOS terminal conventions.
        Key::ArrowRight if mods.alt => b"\x1bf".to_vec(),
        Key::ArrowLeft if mods.alt => b"\x1bb".to_vec(),
        Key::ArrowRight => csi_or_ss3('C'),
        Key::ArrowLeft => csi_or_ss3('D'),
        Key::Home => csi_or_ss3('H'),
        Key::End => csi_or_ss3('F'),
        Key::Insert => tilde(2),
        Key::Delete => tilde(3),
        Key::PageUp => tilde(5),
        Key::PageDown => tilde(6),
        Key::F1 => b"\x1bOP".to_vec(),
        Key::F2 => b"\x1bOQ".to_vec(),
        Key::F3 => b"\x1bOR".to_vec(),
        Key::F4 => b"\x1bOS".to_vec(),
        Key::F5 => tilde(15),
        Key::F6 => tilde(17),
        Key::F7 => tilde(18),
        Key::F8 => tilde(19),
        Key::F9 => tilde(20),
        Key::F10 => tilde(21),
        Key::F11 => tilde(23),
        Key::F12 => tilde(24),
        _ => return None,
    };
    Some(bytes)
}

fn xterm_modifier(mods: Modifiers) -> Option<u8> {
    let bits = u8::from(mods.shift) | u8::from(mods.alt) << 1 | u8::from(mods.ctrl) << 2;
    (bits != 0).then_some(bits + 1)
}

fn ctrl_byte(key: Key) -> Option<u8> {
    let name = key.name();
    if name.len() == 1 {
        let c = name.as_bytes()[0].to_ascii_lowercase();
        if c.is_ascii_lowercase() {
            return Some(c & 0x1f);
        }
    }
    match key {
        Key::OpenBracket => Some(0x1b),
        Key::Backslash => Some(0x1c),
        Key::CloseBracket => Some(0x1d),
        Key::Space | Key::Num2 => Some(0x00),
        Key::Slash | Key::Minus => Some(0x1f),
        _ => None,
    }
}

/// Wraps pasted text in bracketed-paste markers when the program enabled them.
pub fn paste_bytes(text: &str, mode: TermMode) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if mode.contains(TermMode::BRACKETED_PASTE) {
        // Strip embedded end markers so pasted content cannot escape the bracket.
        let text = text.replace("\x1b[201~", "");
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_is_etx() {
        assert_eq!(key_to_bytes(Key::C, Modifiers::CTRL, TermMode::empty()), Some(vec![3]));
    }

    #[test]
    fn arrows_respect_app_cursor_mode() {
        let none = Modifiers::NONE;
        assert_eq!(key_to_bytes(Key::ArrowUp, none, TermMode::empty()).unwrap(), b"\x1b[A");
        assert_eq!(key_to_bytes(Key::ArrowUp, none, TermMode::APP_CURSOR).unwrap(), b"\x1bOA");
        assert_eq!(key_to_bytes(Key::ArrowUp, Modifiers::SHIFT, TermMode::empty()).unwrap(), b"\x1b[1;2A");
    }

    #[test]
    fn command_shortcuts_are_not_sent() {
        assert_eq!(key_to_bytes(Key::C, Modifiers::MAC_CMD, TermMode::empty()), None);
    }

    #[test]
    fn bracketed_paste_strips_end_marker() {
        let out = paste_bytes("a\x1b[201~b\n", TermMode::BRACKETED_PASTE);
        assert_eq!(out, b"\x1b[200~ab\r\x1b[201~");
    }
}
