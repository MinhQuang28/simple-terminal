//! Editor commands bound in the keymap (`editor.*`), queued by the app's shortcuts and run by the
//! editor showing the file, on its text and cursors.

use std::path::Path;

use super::line_ops::{self, Sel};
use super::multi_cursor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorCommand {
    SelectNext,
    SelectAll,
    CursorAbove,
    CursorBelow,
    UndoCursor,
    MoveLineUp,
    MoveLineDown,
    CopyLineUp,
    CopyLineDown,
    DeleteLine,
    ToggleComment,
    /// Asks the app for the definition of the word at the cursor.
    Definition {
        peek: bool,
    },
}

impl EditorCommand {
    /// The command a keymap name (`editor.select_next`) stands for.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "editor.select_next" => Self::SelectNext,
            "editor.select_all_matches" => Self::SelectAll,
            "editor.cursor_above" => Self::CursorAbove,
            "editor.cursor_below" => Self::CursorBelow,
            "editor.undo_cursor" => Self::UndoCursor,
            "editor.move_line_up" => Self::MoveLineUp,
            "editor.move_line_down" => Self::MoveLineDown,
            "editor.copy_line_up" => Self::CopyLineUp,
            "editor.copy_line_down" => Self::CopyLineDown,
            "editor.delete_line" => Self::DeleteLine,
            "editor.toggle_comment" => Self::ToggleComment,
            "editor.definition" => Self::Definition { peek: false },
            "editor.peek_definition" => Self::Definition { peek: true },
            _ => return None,
        })
    }
}

/// What a command asks of the app beyond the text.
pub enum Request {
    Definition { word: String, peek: bool },
}

/// Runs `command` on `text` with the primary selection and the extra cursors; line commands act on
/// the primary alone.
pub fn run(
    command: EditorCommand,
    text: &mut String,
    primary: &mut Sel,
    extra: &mut Vec<Sel>,
    path: &Path,
) -> Option<Request> {
    use EditorCommand::*;
    match command {
        SelectNext => *primary = multi_cursor::select_next(text, *primary, extra)?,
        SelectAll => *primary = multi_cursor::select_all(text, *primary, extra)?,
        CursorAbove | CursorBelow => {
            *primary = multi_cursor::add_line(text, *primary, extra, command == CursorAbove)?
        }
        UndoCursor => *primary = multi_cursor::undo_last(extra)?,
        MoveLineUp | MoveLineDown => {
            (*text, *primary) = line_ops::move_lines(text, *primary, command == MoveLineUp)?;
        }
        CopyLineUp | CopyLineDown => {
            (*text, *primary) = line_ops::copy_lines(text, *primary, command == CopyLineUp)
        }
        DeleteLine => (*text, *primary) = line_ops::delete_lines(text, *primary),
        ToggleComment => {
            let (prefix, suffix) = line_ops::comment_markers(path);
            (*text, *primary) = line_ops::toggle_comment(text, *primary, prefix, suffix);
        }
        Definition { peek } => {
            let (start, end) = match *primary {
                (a, b) if a != b => (a.min(b), a.max(b)),
                (_, at) => multi_cursor::word_at(text, at)?,
            };
            let word: String = text.chars().skip(start).take(end - start).collect();
            return Some(Request::Definition { word, peek });
        }
    }
    None
}
