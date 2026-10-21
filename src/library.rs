use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A reusable prompt the user pastes into a pane.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    pub text: String,
}

/// An agent skill folder, from the user's `~/.claude/skills` or a repo's `.claude/skills`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// The `SKILL.md` (or its disabled twin) inside the skill folder.
    pub file: PathBuf,
    pub enabled: bool,
    pub project: bool,
}

/// An MCP server from a repo's `.mcp.json` or the user's `~/.claude.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpServer {
    pub name: String,
    /// The command line or URL it is reached through.
    pub target: String,
    pub config: PathBuf,
    pub project: bool,
}

/// Disabling a skill renames its file so Claude Code no longer finds it; nothing is deleted.
const DISABLED_SUFFIX: &str = ".disabled";

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Skills of the user plus the repo, project ones first, each list sorted by name.
pub fn skills(repo: &Path) -> Vec<Skill> {
    let mut out = Vec::new();
    out.extend(scan_skills(&repo.join(".claude/skills"), true));
    if let Some(home) = home() {
        out.extend(scan_skills(&home.join(".claude/skills"), false));
    }
    out
}

fn scan_skills(dir: &Path, project: bool) -> Vec<Skill> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut skills: Vec<Skill> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let folder = e.path();
            let (file, enabled) = [("SKILL.md", true), (&format!("SKILL.md{DISABLED_SUFFIX}"), false)]
                .into_iter()
                .map(|(name, enabled)| (folder.join(name), enabled))
                .find(|(path, _)| path.is_file())?;
            let (name, description) = frontmatter(&file);
            let name = name.unwrap_or_else(|| e.file_name().to_string_lossy().into_owned());
            Some(Skill { name, description: description.unwrap_or_default(), file, enabled, project })
        })
        .collect();
    skills.sort_by_key(|s| s.name.to_lowercase());
    skills
}

/// `name:` and `description:` from a Markdown file's YAML front matter.
fn frontmatter(file: &Path) -> (Option<String>, Option<String>) {
    let Ok(text) = fs::read_to_string(file) else { return (None, None) };
    let Some(rest) = text.strip_prefix("---") else { return (None, None) };
    let Some(end) = rest.find("\n---") else { return (None, None) };
    let field = |key: &str| {
        rest[..end].lines().find_map(|line| {
            let value = line.strip_prefix(key)?.strip_prefix(':')?.trim();
            Some(value.trim_matches('"').trim_matches('\'').to_string()).filter(|v| !v.is_empty())
        })
    };
    (field("name"), field("description"))
}

/// Toggles a skill by renaming its `SKILL.md`; returns the new file path.
pub fn set_skill_enabled(file: &Path, enabled: bool) -> std::io::Result<PathBuf> {
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let target = if enabled {
        name.strip_suffix(DISABLED_SUFFIX).unwrap_or(name).to_string()
    } else if name.ends_with(DISABLED_SUFFIX) {
        name.to_string()
    } else {
        format!("{name}{DISABLED_SUFFIX}")
    };
    let target = file.with_file_name(target);
    if target != file {
        fs::rename(file, &target)?;
    }
    Ok(target)
}

/// MCP servers configured for the repo and for the user.
pub fn mcp_servers(repo: &Path) -> Vec<McpServer> {
    let mut out = read_mcp(&repo.join(".mcp.json"), true);
    if let Some(home) = home() {
        out.extend(read_mcp(&home.join(".claude.json"), false));
    }
    out
}

fn read_mcp(config: &Path, project: bool) -> Vec<McpServer> {
    let Ok(text) = fs::read_to_string(config) else { return Vec::new() };
    let Ok(value) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
    let Some(servers) = value["mcpServers"].as_object() else { return Vec::new() };
    let mut out: Vec<McpServer> = servers
        .iter()
        .map(|(name, server)| {
            let target = server["url"].as_str().map(str::to_string).unwrap_or_else(|| {
                let args: Vec<&str> =
                    server["args"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                format!("{} {}", server["command"].as_str().unwrap_or_default(), args.join(" "))
                    .trim()
                    .to_string()
            });
            McpServer { name: name.clone(), target, config: config.to_path_buf(), project }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("repodeck-lib-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn scans_skills_with_front_matter_and_toggles_them() {
        let root = temp_dir();
        let skill = root.join(".claude/skills/deploy");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "---\nname: ck:deploy\ndescription: \"Ship it\"\n---\n# Deploy\n")
            .unwrap();
        fs::create_dir_all(root.join(".claude/skills/bare")).unwrap();
        fs::write(root.join(".claude/skills/bare/SKILL.md"), "# No front matter\n").unwrap();

        let found: Vec<_> = scan_skills(&root.join(".claude/skills"), true);
        let names: Vec<_> =
            found.iter().map(|s| (s.name.as_str(), s.description.as_str(), s.enabled)).collect();
        assert_eq!(names, [("bare", "", true), ("ck:deploy", "Ship it", true)]);

        let disabled = set_skill_enabled(&found[1].file, false).unwrap();
        assert!(disabled.ends_with("SKILL.md.disabled"));
        assert!(!scan_skills(&root.join(".claude/skills"), true)[1].enabled);
        let enabled = set_skill_enabled(&disabled, true).unwrap();
        assert!(enabled.ends_with("SKILL.md"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reads_mcp_servers_from_project_config() {
        let root = temp_dir();
        let json = r#"{"mcpServers":{"figma":{"url":"http://127.0.0.1:3845/mcp"},"aws":{"command":"uvx","args":["aws-mcp"]}}}"#;
        fs::write(root.join(".mcp.json"), json).unwrap();
        let servers = read_mcp(&root.join(".mcp.json"), true);
        let view: Vec<_> = servers.iter().map(|s| (s.name.as_str(), s.target.as_str())).collect();
        assert_eq!(view, [("aws", "uvx aws-mcp"), ("figma", "http://127.0.0.1:3845/mcp")]);
        fs::remove_dir_all(root).unwrap();
    }
}
