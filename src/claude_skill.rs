//! Offers Claude Code a `repodeck` skill in `~/.claude/skills`, so an agent in a pane knows it
//! can drive the app. The skill is a stub pointing at `repodeck --skill`, which prints the guide
//! for the installed version. A copy Repodeck wrote is refreshed when the stub changes; one the
//! user deleted stays deleted, and one the user wrote (no stub marker) is left alone.

use std::fs;
use std::io;
use std::path::Path;

const SKILL: &str = include_str!("claude-skill.md");
/// Front-matter key that marks the file as Repodeck's own.
const STUB_KEY: &str = "repodeck-stub:";
/// Data-dir file recording that the skill was installed once, so a deletion is respected.
const INSTALLED_MARKER: &str = "claude-skill-installed";

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Installed,
    Updated,
    Unchanged,
    /// Claude Code isn't set up, the user removed the skill, or the folder is the user's own.
    Skipped,
}

/// Installs or refreshes the skill for the current user; failures are only logged.
pub fn install() {
    let (Some(home), Some(data)) = (std::env::var_os("HOME"), crate::store::data_dir()) else { return };
    match install_into(Path::new(&home), &data) {
        Ok(outcome) => log::debug!("claude skill: {outcome:?}"),
        Err(err) => log::warn!("claude skill not installed: {err}"),
    }
}

fn install_into(home: &Path, data_dir: &Path) -> io::Result<Outcome> {
    let claude = home.join(".claude");
    if !claude.is_dir() {
        return Ok(Outcome::Skipped);
    }
    let folder = claude.join("skills").join("repodeck");
    let file = folder.join("SKILL.md");
    let marker = data_dir.join(INSTALLED_MARKER);
    if folder.exists() {
        // A disabled skill (SKILL.md renamed by the Library) or the user's own file stays as is.
        let Ok(current) = fs::read_to_string(&file) else { return Ok(Outcome::Skipped) };
        if !current.contains(STUB_KEY) {
            return Ok(Outcome::Skipped);
        }
        // Ours, maybe from another data folder: remember it, so deleting it later is respected.
        mark_installed(&marker)?;
        if current == SKILL {
            return Ok(Outcome::Unchanged);
        }
        fs::write(&file, SKILL)?;
        return Ok(Outcome::Updated);
    }
    if marker.exists() {
        return Ok(Outcome::Skipped);
    }
    fs::create_dir_all(&folder)?;
    fs::write(&file, SKILL)?;
    mark_installed(&marker)?;
    Ok(Outcome::Installed)
}

fn mark_installed(marker: &Path) -> io::Result<()> {
    if marker.exists() {
        return Ok(());
    }
    if let Some(dir) = marker.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(marker, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("repodeck-skill-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn installs_once_and_respects_deletion() {
        let root = temp_root("lifecycle");
        let (home, data) = (root.join("home"), root.join("data"));
        assert_eq!(install_into(&home, &data).unwrap(), Outcome::Skipped, "no ~/.claude: no install");

        fs::create_dir_all(home.join(".claude")).unwrap();
        assert_eq!(install_into(&home, &data).unwrap(), Outcome::Installed);
        let file = home.join(".claude/skills/repodeck/SKILL.md");
        assert_eq!(fs::read_to_string(&file).unwrap(), SKILL);
        assert_eq!(install_into(&home, &data).unwrap(), Outcome::Unchanged);
        let other = root.join("other-data");
        assert_eq!(install_into(&home, &other).unwrap(), Outcome::Unchanged);
        assert!(other.join(INSTALLED_MARKER).exists(), "an existing copy is remembered too");

        fs::write(&file, "---\nname: repodeck\nmetadata:\n  repodeck-stub: \"0\"\n---\nold\n").unwrap();
        assert_eq!(install_into(&home, &data).unwrap(), Outcome::Updated);
        assert_eq!(fs::read_to_string(&file).unwrap(), SKILL);

        fs::remove_dir_all(home.join(".claude/skills/repodeck")).unwrap();
        assert_eq!(install_into(&home, &data).unwrap(), Outcome::Skipped, "deleted by the user");
        assert!(!file.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn leaves_user_and_disabled_skills_alone() {
        let root = temp_root("foreign");
        let (home, data) = (root.join("home"), root.join("data"));
        let folder = home.join(".claude/skills/repodeck");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("SKILL.md"), "---\nname: repodeck\n---\nmine\n").unwrap();
        assert_eq!(install_into(&home, &data).unwrap(), Outcome::Skipped);
        assert_eq!(fs::read_to_string(folder.join("SKILL.md")).unwrap(), "---\nname: repodeck\n---\nmine\n");

        fs::rename(folder.join("SKILL.md"), folder.join("SKILL.md.disabled")).unwrap();
        assert_eq!(install_into(&home, &data).unwrap(), Outcome::Skipped);
        assert!(!folder.join("SKILL.md").exists(), "a disabled skill isn't re-enabled");
        let _ = fs::remove_dir_all(root);
    }
}
