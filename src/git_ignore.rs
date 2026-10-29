use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// The ignore rules of the repository at `root`: its top-level `.gitignore` and
/// `.git/info/exclude`. Nested `.gitignore` files are not read; the build and dependency
/// directories that churn are ignored from the top in practice.
pub fn rules(root: &Path) -> Option<Gitignore> {
    let mut builder = GitignoreBuilder::new(root);
    builder.add(root.join(".gitignore"));
    builder.add(root.join(".git/info/exclude"));
    match builder.build() {
        Ok(rules) => Some(rules),
        Err(err) => {
            log::warn!("Cannot read ignore rules of {}: {err}", root.display());
            None
        }
    }
}

/// Whether `path`, which must be inside the rules' root, is gitignored; a change to such a
/// path can't alter what `git status` reports.
pub fn is_ignored(rules: Option<&Gitignore>, path: &Path) -> bool {
    rules.is_some_and(|rules| rules.matched_path_or_any_parents(path, path.is_dir()).is_ignore())
}

/// Whether `path` is the file the rules are read from, so they need rebuilding.
pub fn is_rules_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == ".gitignore" || name == "exclude")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn ignores_paths_under_ignored_directories() {
        let root = std::env::temp_dir().join(format!("repodeck-ignore-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();
        let rules = rules(&root);
        assert!(is_ignored(rules.as_ref(), &root.join("target/debug/repodeck")));
        assert!(is_ignored(rules.as_ref(), &root.join("build.log")));
        assert!(!is_ignored(rules.as_ref(), &root.join("src/main.rs")));
        assert!(!is_ignored(None, &root.join("target/debug/repodeck")));
        assert!(is_rules_file(&root.join(".gitignore")));
        assert!(!is_rules_file(&root.join("src/main.rs")));
        fs::remove_dir_all(root).unwrap();
    }
}
