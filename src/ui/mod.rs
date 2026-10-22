pub mod agent_status;
pub mod chrome;
pub(crate) mod claude_icon;
pub(crate) mod dialog_kit;
pub mod dialogs;
pub(crate) mod diff_comments;
pub mod diff_view;
pub mod files_panel;
mod history_menu;
mod inbox_panel;
mod library_panel;
pub mod palette;
mod pane_chrome;
pub mod settings;
pub mod sidebar;
mod sidebar_rows;
mod tab_bar;
pub(crate) mod widgets;
pub mod workspace;

use std::path::PathBuf;

use uuid::Uuid;

use crate::model::{PaneKind, SidePanel, SplitDir};
use crate::pane_layout::Nav;

/// User intents emitted by the UI and applied by the app after the frame's layout.
#[derive(Clone, Debug)]
pub enum Action {
    OpenNewProject,
    OpenRenameProject(Uuid),
    OpenSettings,
    RemoveProject(Uuid),
    ToggleProject(Uuid),
    ToggleRepoTabs(Uuid),
    AddRepos(Uuid),
    RemoveRepo(Uuid),
    RevealRepo(Uuid),
    SelectRepo(Uuid),
    SelectTab {
        repo: Uuid,
        tab: Uuid,
    },
    NewTab {
        repo: Uuid,
        kind: PaneKind,
    },
    CloseTab {
        repo: Uuid,
        tab: Uuid,
    },
    /// Splits the active pane, starting the new one in the active pane's directory.
    Split {
        dir: SplitDir,
        kind: PaneKind,
    },
    /// Closes the active tab, whatever it holds.
    CloseActiveTab,
    /// Shows the next pane whose agent is waiting on the user.
    FocusNextAttention,
    FocusPane {
        tab: Uuid,
        pane: Uuid,
    },
    CloseActivePane,
    ClosePane {
        tab: Uuid,
        pane: Uuid,
    },
    RestartPane(Uuid),
    OpenFile {
        repo: Uuid,
        path: PathBuf,
    },
    /// Opens a file at a position, in the external editor when one is configured.
    OpenFileAt {
        repo: Uuid,
        path: PathBuf,
        line: Option<u32>,
        col: Option<u32>,
    },
    SaveActiveFile,
    /// Closes a tab even if its file has unsaved changes.
    DiscardAndCloseTab {
        repo: Uuid,
        tab: Uuid,
    },
    /// Opens a Claude tab that resumes a past conversation.
    ResumeSession {
        repo: Uuid,
        session: String,
        title: String,
    },
    /// Folders dropped from Finder onto the sidebar; `None` picks the current or a fallback project.
    AddDroppedRepos {
        project: Option<Uuid>,
        paths: Vec<PathBuf>,
    },
    /// Opens a shell tab at a folder dropped from Finder.
    OpenFolderTab(PathBuf),
    ToggleSidePanel,
    /// Opens the find bar in the active terminal pane.
    OpenFind,
    SetSidePanel(SidePanel),
    /// Shows the active pane alone, or restores the split.
    ToggleZoom,
    FocusNeighbor(Nav),
    /// Moves a divider by a fraction of its split's extent.
    ResizeSplit {
        tab: Uuid,
        path: Vec<usize>,
        index: usize,
        delta: f32,
    },
    ReopenClosedTab,
    OpenRenameTab {
        repo: Uuid,
        tab: Uuid,
    },
    MoveTab {
        repo: Uuid,
        tab: Uuid,
        to: usize,
    },
    /// Selects the tab `offset` places from the active one, wrapping around.
    SelectAdjacentTab(i32),
    OpenNewWorktree(Uuid),
    /// Asks before deleting a linked worktree and dropping it from its project.
    OpenRemoveWorktree(Uuid),
    /// Edits the command new worktrees of the repo run first.
    OpenRepoSetup(Uuid),
    /// Shows the pane a notice came from and marks it read.
    OpenNotice(u64),
    MarkNoticesRead,
    ClearNotices,
    OpenUrl(String),
    /// Opens `gh pr create` in the browser for the repo's branch.
    CreatePullRequest(Uuid),
    /// Sends the failing CI logs of the repo's pull request to its Claude pane.
    AutoFixCi(Uuid),
    /// Pastes a library prompt into the active pane.
    InsertPrompt(String),
    /// Edits the prompt at the index, or a new one.
    OpenEditPrompt(Option<usize>),
    DeletePrompt(usize),
    SetSkillEnabled {
        file: PathBuf,
        enabled: bool,
    },
    /// Answers an agent's permission prompt from the approvals list.
    AnswerApproval {
        pane: Uuid,
        allow: bool,
    },
    /// Shows the file's diff against HEAD in a tab.
    OpenDiff {
        repo: Uuid,
        path: PathBuf,
    },
    /// Pastes text into the repo's Claude pane, opening one if none exists.
    SendToAgent {
        repo: Uuid,
        text: String,
    },
}
