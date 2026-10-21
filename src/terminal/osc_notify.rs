use std::borrow::Cow;
use std::fs::File;
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::mpsc::Sender;

use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
use polling::{Event, PollMode, Poller};

/// Longest notification sequence kept while waiting for its terminator.
const MAX_OSC: usize = 4096;
const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

/// Picks `OSC 9` (iTerm2/ConEmu) and `OSC 777;notify` (rxvt-unicode) notifications out of PTY
/// output, which the terminal parser itself discards.
#[derive(Default)]
pub struct OscScanner {
    /// Start of a sequence whose terminator hasn't arrived yet.
    pending: Vec<u8>,
}

impl OscScanner {
    /// Feeds a chunk of output; returns the messages of the notifications completed by it.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        let buf: Cow<[u8]> = if self.pending.is_empty() {
            Cow::Borrowed(chunk)
        } else {
            let mut joined = std::mem::take(&mut self.pending);
            joined.extend_from_slice(chunk);
            Cow::Owned(joined)
        };
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(esc) = buf[i..].iter().position(|b| *b == ESC) {
            let start = i + esc;
            match buf.get(start + 1) {
                None => {
                    self.pending = vec![ESC];
                    break;
                }
                Some(b']') => {}
                Some(_) => {
                    i = start + 1;
                    continue;
                }
            }
            let payload = &buf[start + 2..];
            match terminator(payload) {
                End::Terminated { len, consumed } => {
                    out.extend(parse(&payload[..len]));
                    i = start + 2 + consumed;
                }
                End::Aborted(at) => i = start + 2 + at,
                End::Incomplete => {
                    let tail = &buf[start..];
                    if tail.len() <= MAX_OSC {
                        self.pending = tail.to_vec();
                    }
                    break;
                }
            }
        }
        out
    }
}

enum End {
    /// Payload length before a BEL or ST, and the bytes consumed including the terminator.
    Terminated {
        len: usize,
        consumed: usize,
    },
    /// An escape that isn't ST cut the sequence short; scanning resumes at that offset.
    Aborted(usize),
    Incomplete,
}

fn terminator(payload: &[u8]) -> End {
    for (i, byte) in payload.iter().enumerate() {
        match *byte {
            BEL => return End::Terminated { len: i, consumed: i + 1 },
            ESC => {
                return match payload.get(i + 1) {
                    Some(b'\\') => End::Terminated { len: i, consumed: i + 2 },
                    Some(_) => End::Aborted(i),
                    None => End::Incomplete,
                };
            }
            _ => {}
        }
    }
    End::Incomplete
}

fn parse(payload: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(payload).ok()?;
    if let Some(message) = text.strip_prefix("9;") {
        // `9;4;…` is ConEmu's progress bar, not a message.
        if message.starts_with("4;") {
            return None;
        }
        return Some(message.trim().to_string()).filter(|m| !m.is_empty());
    }
    let rest = text.strip_prefix("777;notify;")?;
    let (title, body) = rest.split_once(';').unwrap_or((rest, ""));
    let message = match (title.trim(), body.trim()) {
        ("", "") => return None,
        (title, "") => title.to_string(),
        ("", body) => body.to_string(),
        (title, body) => format!("{title}: {body}"),
    };
    Some(message)
}

/// The PTY's read side, forwarding notifications found in the output as it passes through.
pub struct NotifyReader {
    file: File,
    scanner: OscScanner,
    tx: Sender<String>,
}

impl Read for NotifyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.file.read(buf)?;
        for message in self.scanner.feed(&buf[..n]) {
            let _ = self.tx.send(message);
        }
        Ok(n)
    }
}

/// A PTY whose reads go through a [`NotifyReader`] sharing the master descriptor.
pub struct NotifyingPty {
    inner: Pty,
    reader: NotifyReader,
}

impl NotifyingPty {
    pub fn new(inner: Pty, tx: Sender<String>) -> io::Result<Self> {
        let file = inner.file().try_clone()?;
        Ok(Self { inner, reader: NotifyReader { file, scanner: OscScanner::default(), tx } })
    }
}

impl EventedReadWrite for NotifyingPty {
    type Reader = NotifyReader;
    type Writer = File;

    unsafe fn register(&mut self, poll: &Arc<Poller>, interest: Event, mode: PollMode) -> io::Result<()> {
        unsafe { self.inner.register(poll, interest, mode) }
    }

    fn reregister(&mut self, poll: &Arc<Poller>, interest: Event, mode: PollMode) -> io::Result<()> {
        self.inner.reregister(poll, interest, mode)
    }

    fn deregister(&mut self, poll: &Arc<Poller>) -> io::Result<()> {
        self.inner.deregister(poll)
    }

    fn reader(&mut self) -> &mut NotifyReader {
        &mut self.reader
    }

    fn writer(&mut self) -> &mut File {
        self.inner.writer()
    }
}

impl EventedPty for NotifyingPty {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
        self.inner.next_child_event()
    }
}

impl OnResize for NotifyingPty {
    fn on_resize(&mut self, window_size: WindowSize) {
        self.inner.on_resize(window_size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_osc_9_and_777_messages() {
        let mut scanner = OscScanner::default();
        let out = scanner.feed(b"ls\n\x1b]9;Build done\x07more\x1b]777;notify;Tests;3 failed\x1b\\");
        assert_eq!(out, ["Build done", "Tests: 3 failed"]);
    }

    #[test]
    fn ignores_other_sequences_and_empty_messages() {
        let mut scanner = OscScanner::default();
        assert!(scanner.feed(b"\x1b[31mred\x1b[0m \x1b]0;title\x07 \x1b]9;\x07").is_empty());
    }

    #[test]
    fn an_aborted_sequence_does_not_hide_later_ones() {
        let mut scanner = OscScanner::default();
        assert_eq!(scanner.feed(b"\x1b]0;title\x1b[0m\x1b]9;hi\x07"), ["hi"]);
        assert!(scanner.pending.is_empty());
        assert!(scanner.feed(b"\x1b]9;4;3;50\x07").is_empty());
    }

    #[test]
    fn joins_sequences_split_across_reads() {
        let mut scanner = OscScanner::default();
        assert!(scanner.feed(b"abc\x1b").is_empty());
        assert!(scanner.feed(b"]9;hel").is_empty());
        assert_eq!(scanner.feed(b"lo\x07rest"), ["hello"]);
        assert!(scanner.feed(b"\x1b]9;tail\x1b").is_empty());
        assert_eq!(scanner.feed(b"\\"), ["tail"]);
    }

    #[test]
    fn drops_oversized_unterminated_sequences() {
        let mut scanner = OscScanner::default();
        let huge = [b"\x1b]9;".as_slice(), &vec![b'x'; MAX_OSC + 1]].concat();
        assert!(scanner.feed(&huge).is_empty());
        assert!(scanner.pending.is_empty());
    }
}
