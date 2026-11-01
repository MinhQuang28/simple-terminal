//! What runs in a pane, as people know it: a name and an icon on a coloured tile, from Herdr's
//! program table (assets/icons/programs.json). An agent is known by which agent it is; a
//! terminal by the command its shell is running (`vim`, `npm run dev`, `cargo test`).

use std::collections::HashMap;
use std::sync::OnceLock;

use eframe::egui::Color32;
use serde::Deserialize;

use crate::agent_launch::Agent;
use crate::program_icon_files::FILES;

const TABLE_JSON: &str = include_str!("../assets/icons/programs.json");

/// How one program shows.
pub struct Program {
    pub name: String,
    /// The SVG, ready to draw: a tinted glyph is white, for the tile's `color` to tint.
    pub icon: Option<&'static [u8]>,
    /// The icon's path, which also names it to egui's image cache.
    pub uri: String,
    pub tint: bool,
    pub tile: Color32,
    pub color: Color32,
}

#[derive(Deserialize)]
struct Table {
    #[serde(default)]
    interpreters: Vec<String>,
    #[serde(default)]
    shells: Vec<String>,
    programs: HashMap<String, Entry>,
}

#[derive(Deserialize)]
struct Entry {
    name: Option<String>,
    icon: Option<String>,
    tint: Option<bool>,
    tile: Option<String>,
    color: Option<String>,
    #[serde(default, rename = "match")]
    also: Vec<String>,
}

struct Programs {
    by_key: HashMap<String, Program>,
    /// Lower-case program and alias names to their key.
    names: HashMap<String, String>,
    interpreters: Vec<String>,
    shells: Vec<String>,
}

fn programs() -> &'static Programs {
    static PROGRAMS: OnceLock<Programs> = OnceLock::new();
    PROGRAMS.get_or_init(|| {
        let table: Table = serde_json::from_str(TABLE_JSON).expect("assets/icons/programs.json");
        let mut names = HashMap::new();
        let mut by_key = HashMap::new();
        for (key, entry) in table.programs {
            for alias in &entry.also {
                names.insert(alias.to_lowercase(), key.clone());
            }
            names.insert(key.to_lowercase(), key.clone());
            let tint = entry.tint.unwrap_or(true);
            let icon = entry.icon.as_deref().and_then(|path| FILES.iter().find(|(p, _)| *p == path));
            let program = Program {
                name: entry.name.unwrap_or_else(|| key.clone()),
                icon: icon.map(|(_, svg)| prepare(svg, tint)),
                uri: format!("bytes://{}", entry.icon.unwrap_or_default()),
                tint,
                tile: entry.tile.as_deref().and_then(hex).unwrap_or(Color32::from_rgb(0x1c, 0x1d, 0x21)),
                color: entry.color.as_deref().and_then(hex).unwrap_or(Color32::WHITE),
            };
            by_key.insert(key, program);
        }
        Programs { by_key, names, interpreters: table.interpreters, shells: table.shells }
    })
}

pub fn get(key: &str) -> Option<&'static Program> {
    programs().by_key.get(key)
}

pub fn agent(agent: Agent) -> &'static Program {
    let key = match agent {
        Agent::Claude => "claude",
        Agent::Codex => "codex",
        Agent::OpenCode => "opencode",
    };
    get(key).or_else(|| get("agent")).expect("programs.json lists agent")
}

/// The shell itself, when nothing else runs in it.
pub fn shell() -> &'static Program {
    get("shell").expect("programs.json lists shell")
}

/// What a command line runs: its first word (past `sudo`, `env`, variable assignments and the
/// like), or for an interpreter (`npx vite`, `node vite.js`, `uv run pytest`) the program it
/// runs when that one is known. `None` for a program the table doesn't know.
pub fn for_command(line: &str) -> Option<&'static Program> {
    let table = programs();
    let known = |word: &str| table.names.get(&word.to_lowercase()).and_then(|key| table.by_key.get(key));
    let words: Vec<&str> = line.split_whitespace().filter(|w| !is_assignment(w)).collect();
    let first = base(words.first()?);
    if table.interpreters.iter().any(|i| i == first) {
        for word in words[1..].iter().filter(|w| !w.starts_with('-')) {
            let name = base(word);
            let stem = [".js", ".mjs", ".cjs", ".ts", ".py", ".rb"].iter().find_map(|x| name.strip_suffix(x));
            let name = stem.unwrap_or(name);
            if !table.interpreters.iter().any(|i| i == name)
                && let Some(program) = known(name)
            {
                return Some(program);
            }
        }
    }
    if table.shells.iter().any(|s| s == first) {
        return Some(shell());
    }
    known(first)
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=')
        .is_some_and(|(name, _)| !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

fn base(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

fn hex(text: &str) -> Option<Color32> {
    let text = text.strip_prefix('#')?;
    let value = u32::from_str_radix(text, 16).ok().filter(|_| text.len() == 6)?;
    Some(Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

/// A tinted glyph drawn white, so egui's tint gives it the tile's colour: `currentColor`
/// strokes, and fills that default to black because nothing set them.
fn prepare(svg: &[u8], tint: bool) -> &'static [u8] {
    if !tint {
        return Box::leak(svg.to_vec().into_boxed_slice());
    }
    let mut text = String::from_utf8_lossy(svg).replace("currentColor", "#ffffff");
    // A glyph drawn in plain black means "the ink colour" too.
    for black in ["fill=\"#000\"", "fill=\"#000000\"", "fill=\"black\""] {
        text = text.replace(black, "fill=\"#ffffff\"");
    }
    if let Some(start) = text.find("<svg") {
        let end = text[start..].find('>').map_or(text.len(), |i| start + i);
        if !text[start..end].contains(" fill=") && !text[start..end].contains("\nfill=") {
            text.insert_str(start + 4, " fill=\"#ffffff\"");
        }
    }
    Box::leak(text.into_bytes().into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_show_the_program_they_run() {
        let key = |line: &str| for_command(line).map(|p| p.name.as_str());
        assert_eq!(key("nvim src/main.rs"), Some("Neovim"));
        assert_eq!(key("RUST_LOG=debug cargo test"), get("cargo").map(|p| p.name.as_str()));
        assert_eq!(key("/usr/bin/git status"), Some("Git"));
        assert_eq!(key("npx vite --port 3000"), get("vite").map(|p| p.name.as_str()));
        assert_eq!(key("frobnicate --now"), None);
        assert!(get("vite").is_some() && get("cargo").is_some());
    }

    #[test]
    fn every_listed_icon_is_bundled_and_tinted_ones_are_white() {
        let table: Table = serde_json::from_str(TABLE_JSON).unwrap();
        for (key, entry) in &table.programs {
            if let Some(path) = &entry.icon {
                assert!(FILES.iter().any(|(p, _)| p == path), "{key}: {path}");
            }
        }
        let shell = std::str::from_utf8(shell().icon.unwrap()).unwrap();
        assert!(!shell.contains("currentColor") && shell.contains("#ffffff"));
        let git = std::str::from_utf8(get("git").unwrap().icon.unwrap()).unwrap();
        assert!(git.starts_with("<svg fill=\"#ffffff\""), "{git}");
    }
}
