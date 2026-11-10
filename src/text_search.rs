//! Search across a repo's files, as VS Code's Search view does: `.gitignore`-aware, skipping
//! binary and very large files. `text_search_runner` runs it off the UI thread.

use ignore::WalkBuilder;
use regex::{Regex, RegexBuilder};
use std::fs;
use std::path::{Path, PathBuf};

/// Files bigger than this are skipped, like the editor refuses to open them.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Matching stops here; the results say they were cut.
const MAX_HITS: usize = 5_000;
/// Long lines are shown as this many characters around their first match.
const PREVIEW_CHARS: usize = 160;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
}

impl Query {
    /// The pattern the options describe, or why the regex doesn't parse.
    pub fn compile(&self) -> Result<Regex, String> {
        let pattern = if self.regex { self.text.clone() } else { regex::escape(&self.text) };
        let pattern = if self.whole_word { format!(r"\b(?:{pattern})\b") } else { pattern };
        RegexBuilder::new(&pattern).case_insensitive(!self.match_case).build().map_err(|e| e.to_string())
    }
}

/// One matching line: where it is, a preview of it, and the matches' byte ranges in the preview.
#[derive(Clone, Debug)]
pub struct Hit {
    /// One-based, as editors count.
    pub line: usize,
    pub col: usize,
    pub preview: String,
    pub ranges: Vec<(usize, usize)>,
}

#[derive(Debug)]
pub struct FileHits {
    pub path: PathBuf,
    pub hits: Vec<Hit>,
}

#[derive(Debug, Default)]
pub struct Results {
    pub files: Vec<FileHits>,
    pub hits: usize,
    pub truncated: bool,
}

/// Every match of `regex` under `root`, files in path order; stops early once `cancelled`.
/// Gitignored files are always skipped, as in VS Code: build output such as `target/` would
/// make every search take seconds.
pub fn search(root: &Path, regex: &Regex, cancelled: &dyn Fn() -> bool) -> Results {
    let mut paths: Vec<PathBuf> = WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .map(ignore::DirEntry::into_path)
        .collect();
    paths.sort();
    let mut results = Results::default();
    for path in paths {
        if cancelled() {
            break;
        }
        let Some(text) = read_text(&path) else { continue };
        let hits = search_text(&text, regex, MAX_HITS - results.hits);
        if hits.is_empty() {
            continue;
        }
        results.hits += hits.len();
        results.files.push(FileHits { path, hits });
        if results.hits >= MAX_HITS {
            results.truncated = true;
            break;
        }
    }
    results
}

fn read_text(path: &Path) -> Option<String> {
    if fs::metadata(path).ok()?.len() > MAX_FILE_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// The lines of `text` that match, at most `limit`.
fn search_text(text: &str, regex: &Regex, limit: usize) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let ranges: Vec<(usize, usize)> =
            regex.find_iter(line).filter(|m| !m.is_empty()).map(|m| (m.start(), m.end())).collect();
        let Some(&(first, _)) = ranges.first() else { continue };
        let col = line[..first].chars().count() + 1;
        let (preview, ranges) = preview(line, &ranges);
        hits.push(Hit { line: index + 1, col, preview, ranges });
        if hits.len() >= limit {
            break;
        }
    }
    hits
}

/// The line without its indentation, cut to a window around the first match when long; the
/// ranges moved to match.
fn preview(line: &str, ranges: &[(usize, usize)]) -> (String, Vec<(usize, usize)>) {
    let indent = line.len() - line.trim_start().len();
    let mut start = indent.min(ranges[0].0);
    if line[start..].chars().count() > PREVIEW_CHARS {
        // Keep a little context before the match.
        let before: Vec<usize> = line[start..ranges[0].0].char_indices().map(|(i, _)| start + i).collect();
        start = before.len().checked_sub(30).map_or(start, |i| before[i]);
    }
    let end = line[start..].char_indices().nth(PREVIEW_CHARS).map_or(line.len(), |(i, _)| start + i);
    let ranges = ranges
        .iter()
        .filter(|(s, e)| *s >= start && *e <= end)
        .map(|(s, e)| (s - start, e - start))
        .collect();
    (line[start..end].to_string(), ranges)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(text: &str) -> Query {
        Query { text: text.into(), ..Query::default() }
    }

    #[test]
    fn options_shape_the_pattern() {
        let text = "let Foo = foo_bar;\nfoo";
        let lines = |q: Query| {
            search_text(text, &q.compile().unwrap(), 10).iter().map(|h| h.line).collect::<Vec<_>>()
        };
        assert_eq!(lines(query("foo")), [1, 2]);
        assert_eq!(lines(Query { match_case: true, ..query("Foo") }), [1]);
        assert_eq!(lines(Query { whole_word: true, ..query("foo") }), [1, 2], "Foo is a whole word");
        assert_eq!(lines(Query { whole_word: true, match_case: true, ..query("foo") }), [2]);
        assert_eq!(lines(Query { regex: true, ..query("f.o_b") }), [1]);
        assert!(Query { regex: true, ..query("(") }.compile().is_err());
    }

    #[test]
    fn hits_point_at_the_first_match_and_trim_the_preview() {
        let hits = search_text("    x = needle + needle", &query("needle").compile().unwrap(), 10);
        let hit = &hits[0];
        assert_eq!((hit.line, hit.col), (1, 9));
        assert_eq!(hit.preview, "x = needle + needle");
        assert_eq!(hit.ranges, [(4, 10), (13, 19)]);
    }

    #[test]
    fn long_lines_preview_around_the_match() {
        let line = format!("{}needle{}", "a".repeat(500), "b".repeat(500));
        let hit = &search_text(&line, &query("needle").compile().unwrap(), 10)[0];
        let (s, e) = hit.ranges[0];
        assert_eq!(&hit.preview[s..e], "needle");
        assert!(hit.preview.chars().count() <= PREVIEW_CHARS);
    }

    #[test]
    fn searches_a_tree_skipping_ignored_and_binary_files() {
        let root = std::env::temp_dir().join(format!("repodeck-search-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), "build.txt\n").unwrap();
        fs::write(root.join("src/a.rs"), "fn needle() {}\n").unwrap();
        fs::write(root.join("build.txt"), "needle\n").unwrap();
        fs::write(root.join("bin.dat"), b"needle\0").unwrap();
        let regex = query("needle").compile().unwrap();
        let found: Vec<_> = search(&root, &regex, &|| false).files.into_iter().map(|f| f.path).collect();
        assert_eq!(found, [root.join("src/a.rs")]);
        fs::remove_dir_all(root).unwrap();
    }
}
