use crate::model::{PaneKind, Tab};

/// Titles shells and agents set when nothing task-specific is happening.
const GENERIC_TITLES: &[&str] = &[
    "claude",
    "claude code",
    "claude · resume",
    "terminal",
    "zsh",
    "-zsh",
    "bash",
    "-bash",
    "fish",
    "sh",
    "login",
];

/// Claude Code alternates these in its title while working and shows `✳` when idle.
const WORKING_GLYPHS: [char; 2] = ['◐', '◑'];

/// Whether the title says an agent is mid-task.
pub fn is_working(raw: &str) -> bool {
    raw.trim_start().starts_with(WORKING_GLYPHS)
}

/// Extracts a task summary from a terminal title such as `✳ Fix login redirect`, dropping leading
/// status glyphs (Claude's ✳, spinner frames). Returns `None` for generic shell/agent titles.
pub fn task_summary(raw: &str) -> Option<String> {
    let text = raw.trim_start_matches(|c: char| !c.is_alphanumeric() && !"~/.([".contains(c)).trim();
    let lower = text.to_lowercase();
    let generic = text.is_empty()
        || GENERIC_TITLES.contains(&lower.as_str())
        || text.starts_with('~')
        || text.starts_with('/')
        // `user@host:path`, the default title of most shell configs.
        || text.split_once(':').is_some_and(|(host, _)| host.contains('@') && !host.contains(' '));
    (!generic).then(|| text.to_string())
}

/// What a tab shows: the file name, the running task's summary, or `fallback` (the branch).
pub fn display(tab: &Tab, live_title: Option<&str>, fallback: &str) -> String {
    if let Some(name) = &tab.name {
        return name.clone();
    }
    if tab.is_document() {
        return tab.title.clone();
    }
    let persisted = || (tab.kind() == PaneKind::Agent).then(|| task_summary(&tab.title)).flatten();
    live_title.and_then(task_summary).or_else(persisted).unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_task_summaries() {
        assert_eq!(task_summary("✳ Build Test App").as_deref(), Some("Build Test App"));
        assert_eq!(task_summary("◐ Fix login: retry").as_deref(), Some("Fix login: retry"));
        assert!(is_working("◑ Fix login: retry"));
        assert!(!is_working("✳ Fix login: retry"));
    }

    #[test]
    fn drops_generic_titles() {
        for title in
            ["✳ Claude Code", "claude · resume", "me@mac:~/code/repodeck", "~/code", "zsh", "", "Terminal"]
        {
            assert_eq!(task_summary(title), None, "{title:?}");
        }
    }

    #[test]
    fn falls_back_to_repo_and_branch() {
        let mut tab = Tab::new(PaneKind::Agent);
        assert_eq!(display(&tab, Some("✳ Claude Code"), "repodeck - main"), "repodeck - main");
        tab.title = "Refactor parser".into();
        assert_eq!(display(&tab, None, "repodeck - main"), "Refactor parser");
        assert_eq!(display(&tab, Some("✳ Add tests"), "repodeck - main"), "Add tests");
    }
}
