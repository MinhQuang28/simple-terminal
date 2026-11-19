use std::sync::LazyLock;

use regex::Regex;

/// The block Claude Code wraps pasted text in; an unclosed one runs to the end of the prompt.
static PASTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<pasted_content\b[^>]*>(.*?)(?:</pasted_content>|\z)").expect("valid pattern")
});

/// What stands in for an attachment: `[Image #1]`, `[Image: source: /tmp/a.png]`,
/// `[Pasted text #2 +40 lines]`.
static PLACEHOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(?:Image|Pasted text)[^\]\n]*\]").expect("valid pattern"));

/// The first line of `prompt` that says what the user asked for: what they typed, without
/// attachment placeholders, before anything they pasted. `None` when no line carries words.
pub fn task_line(prompt: &str) -> Option<String> {
    let typed = PASTED.replace_all(prompt, "\n");
    worded_line(&typed).or_else(|| PASTED.captures_iter(prompt).find_map(|block| worded_line(&block[1])))
}

/// Whether `line` names a task well enough to title a tab: not a slash command, and at least
/// `min_words` words.
pub fn names_a_task(line: &str, min_words: usize) -> bool {
    !line.starts_with('/') && line.split_whitespace().count() >= min_words
}

fn worded_line(text: &str) -> Option<String> {
    text.lines()
        .map(|line| PLACEHOLDER.replace_all(line, "").trim().to_string())
        .find(|line| line.chars().any(char::is_alphanumeric) && !is_lone_tag(line))
}

/// A line that is only markup, such as `<system-reminder>`.
fn is_lone_tag(line: &str) -> bool {
    line.starts_with('<') && line.ends_with('>') && line.matches('<').count() == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_typed_words_over_pasted_text() {
        let prompt =
            "<pasted_content id=\"5179\">\nerror: build failed\n</pasted_content>\nwhy does this fail?";
        assert_eq!(task_line(prompt).as_deref(), Some("why does this fail?"));
        assert_eq!(task_line("fix the\nlogin bug").as_deref(), Some("fix the"));
    }

    #[test]
    fn falls_back_to_the_pasted_text() {
        let closed = "<pasted_content id=\"1\">\n\nerror: build failed\nat main.rs\n</pasted_content>";
        assert_eq!(task_line(closed).as_deref(), Some("error: build failed"));
        assert_eq!(task_line("<pasted_content id=\"1\">\ncut off").as_deref(), Some("cut off"));
    }

    #[test]
    fn drops_attachment_placeholders_and_markup() {
        let prompt = "[Image #1] rename the tab [Image: source: /tmp/paste-1.png]";
        assert_eq!(task_line(prompt).as_deref(), Some("rename the tab"));
        assert_eq!(
            task_line("[Pasted text #2 +40 lines]\n<system-reminder>\nsummarise it").as_deref(),
            Some("summarise it")
        );
        assert_eq!(task_line("[Image #1]\n \n<pasted_content id=\"2\">"), None);
    }

    #[test]
    fn short_prompts_and_commands_name_nothing() {
        assert!(names_a_task("fix the login timeout", 3));
        assert!(names_a_task("fix login", 2));
        assert!(!names_a_task("ok do it", 4));
        assert!(!names_a_task("/review the open PR", 2));
    }
}
