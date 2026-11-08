//! Go to Definition without a language server: the lines across the repo that declare a name the
//! way common languages do (`fn x`, `struct X`, `class X`, `function x`, `def x`, `const x =`, …),
//! the file being edited first.

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::text_search;

/// Lines of context shown around a definition in the peek window.
const CONTEXT: usize = 8;

/// One place `word` is declared.
#[derive(Clone, Debug)]
pub struct Definition {
    pub path: PathBuf,
    /// One-based.
    pub line: usize,
    pub col: usize,
    /// The declaration line and the ones after it, from `line`.
    pub excerpt: Vec<String>,
}

/// The peek window's contents.
pub struct Peek {
    pub repo: uuid::Uuid,
    pub root: PathBuf,
    pub word: String,
    pub found: Vec<Definition>,
}

/// A line declaring `word`: keywords of Rust, TypeScript/JavaScript, Python, Go, Swift, Kotlin,
/// Java/C#, Ruby and shell, with the modifiers that may come first.
pub fn pattern(word: &str) -> Option<Regex> {
    if word.is_empty() || !word.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
        return None;
    }
    let name = regex::escape(word);
    let modifiers = r"(?:(?:pub(?:\([^)]*\))?|export|default|declare|async|static|public|private|protected|internal|open|abstract|final|override|const|unsafe|extern(?:\s+\x22[^\x22]*\x22)?|readonly|data|sealed|inline)\s+)*";
    let keywords = r"(?:fn|struct|enum|trait|type|union|mod|const|static|let|var|val|class|interface|function\*?|def|func|fun|protocol|extension|object|namespace|module|macro_rules!|typealias)";
    let go_method = format!(r"func\s+\([^)]*\)\s*{name}\b");
    let rust_like = format!(r"{keywords}\s+(?:mut\s+)?{name}\b");
    let arrow = format!(r"(?:const|let|var)\s+{name}\s*=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*=>");
    Regex::new(&format!(
        r"^\s*{modifiers}(?:{rust_like}|{go_method}|{arrow})|^\s*(?:alias\s+)?{name}\s*\(\)\s*\{{"
    ))
    .ok()
}

/// Every declaration of `word` under `root`, those in `current` first.
pub fn find(root: &Path, word: &str, show_ignored: bool, current: Option<&Path>) -> Vec<Definition> {
    let Some(regex) = pattern(word) else { return Vec::new() };
    let results = text_search::search(root, &regex, show_ignored, &|| false);
    let mut found: Vec<Definition> = results
        .files
        .into_iter()
        .flat_map(|file| {
            let lines: Vec<String> = fs::read_to_string(&file.path)
                .map(|t| t.lines().map(str::to_string).collect())
                .unwrap_or_default();
            file.hits.into_iter().map(move |hit| {
                let excerpt = lines.iter().skip(hit.line - 1).take(CONTEXT).cloned().collect();
                // The match starts at the line's start; the cursor goes to the name.
                let line = lines.get(hit.line - 1).map_or("", String::as_str);
                let col = line.find(word).map_or(hit.col, |byte| line[..byte].chars().count() + 1);
                Definition { path: file.path.clone(), line: hit.line, col, excerpt }
            })
        })
        .collect();
    found.sort_by_key(|d| Some(d.path.as_path()) != current);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declares(line: &str, word: &str) -> bool {
        pattern(word).unwrap().is_match(line)
    }

    #[test]
    fn matches_declarations_across_languages() {
        assert!(declares("pub(crate) fn open_file(&mut self) {", "open_file"));
        assert!(declares("    pub async fn run() {", "run"));
        assert!(declares("struct Repo {", "Repo"));
        assert!(!declares("impl Repo {", "Repo"));
        assert!(declares("export default function App() {", "App"));
        assert!(declares("export const useThing = (a) => a", "useThing"));
        assert!(declares("const total = 3;", "total"));
        assert!(declares("class Cart extends Base {", "Cart"));
        assert!(declares("def handle(event):", "handle"));
        assert!(declares("func (s *Server) Serve() error {", "Serve"));
        assert!(declares("interface Props {", "Props"));
        assert!(declares("build() {", "build"));
        assert!(!declares("    open_file(path);", "open_file"), "a call is not a declaration");
        assert!(!declares("fn open_files() {", "open_file"), "whole names only");
        assert!(pattern("a b").is_none());
    }

    #[test]
    fn finds_definitions_current_file_first() {
        let root = std::env::temp_dir().join(format!("repodeck-defs-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.rs"), "fn helper() {}\n").unwrap();
        fs::write(root.join("b.rs"), "use x;\n\npub fn helper() -> u8 {\n    1\n}\n").unwrap();
        let found = find(&root, "helper", false, Some(&root.join("b.rs")));
        let at: Vec<_> =
            found.iter().map(|d| (d.path.file_name().unwrap().to_owned(), d.line, d.col)).collect();
        assert_eq!(at, [("b.rs".into(), 3, 8), ("a.rs".into(), 1, 4)]);
        assert_eq!(found[0].excerpt[0], "pub fn helper() -> u8 {");
        fs::remove_dir_all(root).unwrap();
    }
}
