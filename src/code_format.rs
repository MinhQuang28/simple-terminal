//! Format Document (⌥⇧F) through the formatter a project already uses: rustfmt, prettier (the
//! repo's own, else one on the PATH), gofmt, ruff or black. The text goes in on stdin and comes
//! back on stdout, so unsaved edits are formatted too.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The formatter for `path` and its arguments, found on `paths` (the login shell's PATH).
fn formatter(path: &Path, root: &Path, paths: &[PathBuf]) -> Result<(PathBuf, Vec<String>), String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let which = |name: &str| paths.iter().map(|dir| dir.join(name)).find(|p| p.is_file());
    let shown = path.display().to_string();
    match ext.as_str() {
        "rs" => {
            let program = which("rustfmt").ok_or("rustfmt is not installed")?;
            // Read from stdin, rustfmt can't see the crate's edition; Cargo.toml says it.
            let edition = cargo_edition(path, root).unwrap_or_else(|| "2021".into());
            Ok((program, vec!["--edition".into(), edition, "--emit".into(), "stdout".into()]))
        }
        "go" => Ok((which("gofmt").ok_or("gofmt is not installed")?, Vec::new())),
        "py" | "pyi" => match (which("ruff"), which("black")) {
            (Some(ruff), _) => {
                Ok((ruff, vec!["format".into(), "--stdin-filename".into(), shown, "-".into()]))
            }
            (None, Some(black)) => {
                Ok((black, vec!["-q".into(), "--stdin-filename".into(), shown, "-".into()]))
            }
            (None, None) => Err("Neither ruff nor black is installed".into()),
        },
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "json" | "jsonc" | "css" | "scss"
        | "less" | "html" | "vue" | "svelte" | "astro" | "md" | "markdown" | "mdx" | "yaml" | "yml"
        | "graphql" => {
            // The project's own prettier, with its plugins, wins over a global one.
            let local = path
                .ancestors()
                .take_while(|dir| dir.starts_with(root))
                .map(|dir| dir.join("node_modules/.bin/prettier"))
                .find(|p| p.is_file());
            let program = local
                .or_else(|| which("prettier"))
                .ok_or("prettier is not installed in the repo or on the PATH")?;
            Ok((program, vec!["--stdin-filepath".into(), shown]))
        }
        "" => Err("No formatter for files without an extension".into()),
        other => Err(format!("No formatter for .{other} files")),
    }
}

/// The `edition` of the nearest Cargo.toml above `path`, within `root`.
fn cargo_edition(path: &Path, root: &Path) -> Option<String> {
    let manifest = path
        .ancestors()
        .take_while(|d| d.starts_with(root))
        .map(|d| d.join("Cargo.toml"))
        .find(|p| p.is_file())?;
    let text = std::fs::read_to_string(manifest).ok()?;
    let line = text.lines().find(|l| l.trim_start().starts_with("edition"))?;
    Some(line.split('"').nth(1)?.to_string())
}

/// `text` as the formatter for `path` writes it, or why it couldn't. `path_var` is the PATH to
/// look for formatters on.
pub fn format(path: &Path, root: &Path, text: &str, path_var: Option<&str>) -> Result<String, String> {
    let path_var = path_var.map(str::to_string).or_else(|| std::env::var("PATH").ok()).unwrap_or_default();
    let paths: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
    let (program, args) = formatter(path, root, &paths)?;
    let name = program.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut child = Command::new(&program)
        .args(&args)
        .current_dir(path.parent().unwrap_or(root))
        .env("PATH", &path_var)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run {name}: {e}"))?;
    // Written from another thread, so a formatter that answers before reading it all can't
    // deadlock against a full pipe.
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let input = text.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output().map_err(|e| format!("{name} failed: {e}"))?;
    let _ = writer.join();
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let first = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("it exited with an error");
        return Err(format!("{name}: {first}"));
    }
    String::from_utf8(output.stdout).map_err(|_| format!("{name} wrote text that isn't UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_crate_edition_and_refuses_unknown_types() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(cargo_edition(&root.join("src/main.rs"), root).as_deref(), Some("2024"));
        let err = formatter(&root.join("notes.xyz"), root, &[]).unwrap_err();
        assert_eq!(err, "No formatter for .xyz files");
        assert!(formatter(&root.join("a.rs"), root, &[]).is_err(), "rustfmt not on an empty PATH");
    }

    #[test]
    fn formats_rust_through_rustfmt_when_installed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let Ok(formatted) = format(&root.join("src/x.rs"), root, "fn  main( ){ }", None) else { return };
        assert_eq!(formatted, "fn main() {}\n");
    }
}
