use alacritty_terminal::term::TermMode;
use eframe::egui::{Modifiers, PointerButton, Pos2, Response, Ui};

use super::render::Grid;
use super::session::TerminalSession;

const BUTTONS: [PointerButton; 3] = [PointerButton::Primary, PointerButton::Middle, PointerButton::Secondary];
const RELEASE: u8 = 3;
const MOTION: u8 = 32;

#[derive(Clone, Copy, Default)]
pub struct MouseReportState {
    held: Option<u8>,
    last_cell: Option<(usize, usize)>,
}

/// Forwards presses, releases and motion to a program that enabled mouse reporting; Shift keeps
/// the pointer local so text can still be selected. Returns whether the program owns the pointer.
pub fn report(
    ui: &Ui,
    response: &Response,
    session: &TerminalSession,
    mode: TermMode,
    grid: Grid,
    size: (u16, u16),
    state: &mut MouseReportState,
) -> bool {
    if !mode.intersects(TermMode::MOUSE_MODE) {
        *state = MouseReportState::default();
        return false;
    }
    let (mods, pressed, released, pos) = ui.input(|i| {
        let pressed = BUTTONS.map(|b| i.pointer.button_pressed(b));
        let released = BUTTONS.map(|b| i.pointer.button_released(b));
        (i.modifiers, pressed, released, i.pointer.latest_pos())
    });
    if mods.shift {
        return false;
    }
    let Some(pos) = pos.filter(|_| response.hovered() || state.held.is_some()) else {
        return true;
    };
    let (col, row) = cell(grid, pos, size);
    let modifier_bits = modifier_bits(mods);
    for (index, button) in (0u8..).zip(BUTTONS) {
        let _ = button;
        if pressed[index as usize] && response.hovered() {
            state.held = Some(index);
            state.last_cell = Some((col, row));
            session.write(encode(mode, index | modifier_bits, col, row, true));
        } else if released[index as usize] && state.held == Some(index) {
            state.held = None;
            session.write(encode(mode, index | modifier_bits, col, row, false));
        }
    }
    let reports_motion = match state.held {
        Some(_) => mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION),
        None => mode.contains(TermMode::MOUSE_MOTION),
    };
    if reports_motion && state.last_cell != Some((col, row)) {
        state.last_cell = Some((col, row));
        let button = state.held.unwrap_or(RELEASE) | MOTION | modifier_bits;
        session.write(encode(mode, button, col, row, true));
    }
    true
}

fn modifier_bits(mods: Modifiers) -> u8 {
    u8::from(mods.shift) << 2 | u8::from(mods.alt) << 3 | u8::from(mods.ctrl) << 4
}

/// One-based cell under `pos`, clamped to the grid.
fn cell(grid: Grid, pos: Pos2, (cols, rows): (u16, u16)) -> (usize, usize) {
    let rel = pos - grid.origin;
    let col = ((rel.x / grid.cell.x).floor() as i64).clamp(0, cols as i64 - 1) as usize;
    let row = ((rel.y / grid.cell.y).floor() as i64).clamp(0, rows as i64 - 1) as usize;
    (col + 1, row + 1)
}

fn encode(mode: TermMode, button: u8, col: usize, row: usize, press: bool) -> Vec<u8> {
    if mode.contains(TermMode::SGR_MOUSE) {
        let suffix = if press { 'M' } else { 'm' };
        format!("\x1b[<{button};{col};{row}{suffix}").into_bytes()
    } else {
        // Legacy X10 encoding has no release button, only a "released" code.
        let button = if press { button } else { RELEASE | (button & !0b11) };
        let byte = |v: usize| (v.min(223) + 32) as u8;
        vec![0x1b, b'[', b'M', button + 32, byte(col), byte(row)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_sgr_and_x10() {
        assert_eq!(encode(TermMode::SGR_MOUSE, 0, 5, 7, true), b"\x1b[<0;5;7M");
        assert_eq!(encode(TermMode::SGR_MOUSE, 2 | MOTION, 5, 7, false), b"\x1b[<34;5;7m");
        assert_eq!(encode(TermMode::empty(), 0, 1, 1, true), b"\x1b[M\x20\x21\x21");
        assert_eq!(encode(TermMode::empty(), 0, 1, 1, false), b"\x1b[M\x23\x21\x21");
    }
}
