use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::font_install;
use crate::library::Prompt;
use crate::pane_layout::PaneLayout;
use crate::theme::Theme;

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

/// How Claude Code draws its interface in an agent pane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRenderer {
    /// Claude takes over the screen and scrolls its own transcript, one line per wheel step.
    Fullscreen,
    /// Claude writes into the pane's scrollback, which Repodeck scrolls by the pixel.
    #[default]
    Classic,
}

impl AgentRenderer {
    /// Environment that makes Claude Code use this renderer.
    pub fn env(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Fullscreen => None,
            Self::Classic => Some(("CLAUDE_CODE_NO_FLICKER", "0")),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorShape {
    #[default]
    Block,
    Beam,
    Underline,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    /// Terminal and editor font; empty means the bundled monospace font.
    pub font_family: String,
    pub font_size: f32,
    /// Interface font; empty keeps egui's bundled one.
    pub ui_font_family: String,
    /// Interface text size in points; labels drawn at fixed sizes scale with it.
    pub ui_font_size: f32,
    /// Row height as a multiple of the font size.
    pub line_height: f32,
    /// Extra horizontal space between cells, in points.
    pub letter_spacing: f32,
    /// Space between the pane edge and the text grid, in points.
    pub padding: f32,
    pub cursor_shape: CursorShape,
    pub cursor_blink: bool,
    pub scrollback_lines: u32,
    /// Send Option+key as Escape-prefixed keys (Meta), as Emacs-style editing expects.
    pub option_as_meta: bool,
    pub copy_on_select: bool,
    /// Empty means the user's `$SHELL`.
    pub shell: String,
    pub agent_command: String,
    /// Run instead of `agent_command` when an agent pane is restored after restart.
    pub agent_resume_command: String,
    pub agent_renderer: AgentRenderer,
    /// Hide `ANTHROPIC_API_KEY` and `ANTHROPIC_AUTH_TOKEN` from Claude so it signs in with the
    /// user's subscription instead of billing an API key per token.
    pub agent_hide_api_keys: bool,
    /// Draw an agent pane's content against its bottom edge, so Claude's prompt box stays put
    /// instead of moving down as the conversation above it grows.
    pub agent_prompt_at_bottom: bool,
    /// Post system notifications for panes the user isn't watching.
    pub notifications: bool,
    /// Play the system alert sound with each notification.
    pub notification_sound: bool,
    /// Command for ⌘-clicked file paths, with `{path}`, `{line}` and `{col}` placeholders; empty
    /// opens the built-in editor.
    pub external_editor: String,
    /// Wrap long lines in the built-in editor instead of scrolling sideways.
    pub editor_word_wrap: bool,
    /// Reusable prompts shown in the Library panel.
    pub prompts: Vec<Prompt>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            font_family: String::new(),
            font_size: 13.0,
            ui_font_family: String::new(),
            ui_font_size: font_install::UI_BASE_FONT_SIZE,
            line_height: 1.25,
            letter_spacing: 0.0,
            padding: 8.0,
            cursor_shape: CursorShape::Block,
            cursor_blink: true,
            scrollback_lines: 10_000,
            option_as_meta: false,
            copy_on_select: false,
            shell: String::new(),
            agent_command: "claude".into(),
            agent_resume_command: "claude --continue || claude".into(),
            agent_renderer: AgentRenderer::default(),
            agent_hide_api_keys: true,
            agent_prompt_at_bottom: true,
            notifications: true,
            notification_sound: true,
            external_editor: String::new(),
            editor_word_wrap: false,
            prompts: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidePanel {
    #[default]
    Files,
    Changes,
    Library,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub projects: Vec<Project>,
    pub selected_repo: Option<Uuid>,
    pub settings: Settings,
    pub show_side_panel: bool,
    pub side_panel: SidePanel,
    /// Tabs closed most recently last, with the repo each came from, for "Reopen closed tab".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub closed_tabs: Vec<(Uuid, Tab)>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            selected_repo: None,
            settings: Settings::default(),
            show_side_panel: true,
            side_panel: SidePanel::default(),
            closed_tabs: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub repos: Vec<Repo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Repo {
    pub id: Uuid,
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub tabs: Vec<Tab>,
    #[serde(default)]
    pub active_tab: Option<Uuid>,
    /// Hides the repo's terminal rows in the sidebar.
    #[serde(default)]
    pub tabs_collapsed: bool,
    /// Shell command run in the first terminal of each new worktree, e.g. `npm install`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub setup_command: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDir {
    #[default]
    Right,
    Down,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tab {
    pub id: Uuid,
    pub title: String,
    pub panes: Vec<Pane>,
    #[serde(default)]
    pub split: SplitDir,
    #[serde(default)]
    pub active_pane: Option<Uuid>,
    #[serde(default)]
    pub last_active: u64,
    /// Set for file editor tabs, which have no terminal panes.
    #[serde(default)]
    pub file: Option<PathBuf>,
    /// Split tree of `panes`; absent for tabs saved before nested splits existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<PaneLayout>,
    /// Pane shown alone while the split is zoomed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoomed: Option<Uuid>,
    /// Set for diff viewer tabs, which have no terminal panes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<PathBuf>,
    /// Title chosen by the user, which agents and shells can't overwrite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneKind {
    Shell,
    Agent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pane {
    pub id: Uuid,
    pub kind: PaneKind,
    /// Starting directory when it differs from the repo root, e.g. a subfolder opened from Finder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    /// Claude Code session an agent pane resumes instead of starting a new conversation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

impl Project {
    pub fn new(name: String) -> Self {
        Self { id: Uuid::new_v4(), name, collapsed: false, repos: Vec::new() }
    }
}

impl Repo {
    pub fn from_path(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        Self {
            id: Uuid::new_v4(),
            name,
            path,
            tabs: Vec::new(),
            active_tab: None,
            tabs_collapsed: false,
            setup_command: String::new(),
        }
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        let id = self.active_tab?;
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn last_active(&self) -> u64 {
        self.tabs.iter().map(|t| t.last_active).max().unwrap_or_default()
    }
}

impl Tab {
    pub fn new(kind: PaneKind) -> Self {
        let pane = Pane { id: Uuid::new_v4(), kind: kind.clone(), cwd: None, session: None };
        let title = match kind {
            PaneKind::Shell => "Terminal",
            PaneKind::Agent => "Claude",
        };
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            active_pane: Some(pane.id),
            panes: vec![pane],
            split: SplitDir::Right,
            last_active: now_secs(),
            file: None,
            diff: None,
            layout: None,
            zoomed: None,
            name: None,
        }
    }

    pub fn for_file(path: PathBuf) -> Self {
        let title = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Self {
            id: Uuid::new_v4(),
            title,
            panes: Vec::new(),
            split: SplitDir::Right,
            active_pane: None,
            last_active: now_secs(),
            file: Some(path),
            diff: None,
            layout: None,
            zoomed: None,
            name: None,
        }
    }

    pub fn for_diff(path: PathBuf) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut tab = Self::for_file(path.clone());
        tab.title = format!("Δ {name}");
        tab.file = None;
        tab.diff = Some(path);
        tab
    }

    /// Whether the tab shows a document rather than terminal panes.
    pub fn is_document(&self) -> bool {
        self.file.is_some() || self.diff.is_some()
    }

    pub fn layout(&self) -> PaneLayout {
        self.layout.clone().unwrap_or_else(|| {
            let ids: Vec<_> = self.panes.iter().map(|p| p.id).collect();
            PaneLayout::from_flat(&ids, self.split)
        })
    }

    pub fn kind(&self) -> PaneKind {
        self.panes.first().map(|p| p.kind.clone()).unwrap_or(PaneKind::Shell)
    }
}

impl AppState {
    pub fn repo(&self, id: Uuid) -> Option<&Repo> {
        self.projects.iter().flat_map(|p| &p.repos).find(|r| r.id == id)
    }

    pub fn repo_mut(&mut self, id: Uuid) -> Option<&mut Repo> {
        self.projects.iter_mut().flat_map(|p| &mut p.repos).find(|r| r.id == id)
    }

    pub fn all_pane_ids(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.projects.iter().flat_map(|p| &p.repos).flat_map(|r| &r.tabs).flat_map(|t| &t.panes).map(|p| p.id)
    }
}
