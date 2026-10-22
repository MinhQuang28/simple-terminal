use std::io::Write;

use super::*;

/// Self-deleting transcript file.
struct Transcript(PathBuf);

impl Transcript {
    fn new(lines: &[&str]) -> Self {
        let path = std::env::temp_dir().join(format!("repodeck-history-{}.jsonl", uuid::Uuid::new_v4()));
        let mut file = File::create(&path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
        Self(path)
    }
}

impl Drop for Transcript {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn prefers_generated_title_over_last_prompt() {
    let file = Transcript::new(&[
        r#"{"type":"last-prompt","lastPrompt":"build the app\nplease"}"#,
        r#"{"type":"user","message":{"content":"hi"}}"#,
        r#"{"type":"ai-title","aiTitle":"Build Test App"}"#,
        r#"{"type":"last-prompt","lastPrompt":"now add tests"}"#,
    ]);
    assert_eq!(read_title(&file.0).as_deref(), Some("Build Test App"));
}

#[test]
fn falls_back_to_first_line_of_latest_prompt() {
    let file = Transcript::new(&[r#"{"type":"last-prompt","lastPrompt":"fix login\nwith details"}"#]);
    assert_eq!(read_title(&file.0).as_deref(), Some("fix login"));
    let empty = Transcript::new(&[r#"{"type":"mode","mode":"normal"}"#]);
    assert_eq!(read_title(&empty.0), None);
}

#[test]
fn encodes_paths_like_claude_code() {
    let dir = transcripts_dir(Path::new("/Users/me/code/my-project.v2")).unwrap();
    assert!(dir.ends_with(".claude/projects/-Users-me-code-my-project-v2"));
}

#[test]
fn has_transcript_only_when_the_file_is_present_under_the_cwd() {
    // A unique cwd so its encoded transcript dir can't collide with a real one.
    let cwd = std::env::temp_dir().join(format!("repodeck-resume-{}", uuid::Uuid::new_v4()));
    let id = "79ed207a-4bd1-4b25-95cd-3e05df6968df";
    assert!(!has_transcript(&cwd, id));
    let dir = transcripts_dir(&cwd).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{id}.jsonl")), "{}\n").unwrap();
    assert!(has_transcript(&cwd, id));
    assert!(!has_transcript(&cwd, "00000000-0000-0000-0000-000000000000"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn accepts_only_uuid_like_ids() {
    assert!(is_session_id("f74a6790-b78d-4967-a62f-100cc8e5a231"));
    assert!(!is_session_id("abc; rm -rf ~"));
    assert!(!is_session_id(""));
}

#[test]
fn rereads_a_title_only_when_the_transcript_changes() {
    let file = Transcript::new(&[r#"{"type":"ai-title","aiTitle":"First"}"#]);
    let titles = Mutex::default();
    assert_eq!(cached_title(&titles, file.0.clone(), (1, 1)).as_deref(), Some("First"));
    std::fs::write(&file.0, "{\"type\":\"ai-title\",\"aiTitle\":\"Second\"}\n").unwrap();
    assert_eq!(cached_title(&titles, file.0.clone(), (1, 1)).as_deref(), Some("First"));
    assert_eq!(cached_title(&titles, file.0.clone(), (2, 1)).as_deref(), Some("Second"));
}
