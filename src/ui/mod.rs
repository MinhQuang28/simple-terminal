mod activity_bar;
pub mod agent_status;
mod agents_list;
mod branch_menu;
mod changes_panel;
pub mod chrome;
pub(crate) mod claude_icon;
pub mod code_sidebar;
mod code_tabs;
mod code_view;
pub mod commit_box;
pub mod definition_peek;
pub mod design;
pub(crate) mod dialog_kit;
pub mod dialogs;
pub(crate) mod diff_comments;
pub mod diff_view;
mod file_explorer;
mod file_icon;
mod history_menu;
mod inbox_panel;
mod library_panel;
pub mod palette;
mod palette_items;
mod pane_chrome;
pub mod peek;
mod program_icon;
mod repo_drag;
mod repo_indicators;
mod search_panel;
pub mod settings;
pub mod sidebar;
mod sidebar_rows;
mod sidebar_tree_rows;
pub(crate) mod spinner;
mod tab_bar;
pub mod toast;
mod usage_panel;
pub(crate) mod widgets;
pub mod workspace;

use std::path::PathBuf;

use uuid::Uuid;

use crate::model::{PaneKind, SidePanel, SidebarView, SplitDir};
use crate::pane_layout::Nav;

/// User intents emitted by the UI and applied by the app after the frame's layout.
#[derive(Clone, Debug)]
pub enum Action {
    OpenNewProject,
    OpenRenameProject(Uuid),
    OpenSettings,
    RemoveProject(Uuid),
    ToggleProject(Uuid),
    /// Checks out another local branch of the repo, or a new one at the current commit.
    SwitchBranch {
        repo: Uuid,
        branch: String,
        create: bool,
    },
    /// Stages and commits every change in the repo.
    CommitAll {
        repo: Uuid,
        message: String,
    },
    /// Swaps the active pane with the one beside it.
    SwapNeighbor(Nav),
    /// Shows a pane live over the current tab without switching to it.
    Peek(Uuid),
    ClosePeek,
    /// Switches to whatever repo and tab hold the pane.
    GoToPane(Uuid),
    SetSidebarView(SidebarView),
    TogglePinTab {
        repo: Uuid,
        tab: Uuid,
    },
    ToggleSettleTab {
        repo: Uuid,
        tab: Uuid,
    },
    ToggleRepoTabs(Uuid),
    AddRepos(Uuid),
    RemoveRepo(Uuid),
    RevealRepo(Uuid),
    /// Shows a file or folder in the Finder.
    RevealPath(PathBuf),
    /// Moves a file or folder of the repo to the Trash, closing tabs open on it.
    TrashPath {
        repo: Uuid,
        path: PathBuf,
    },
    SelectRepo(Uuid),
    SelectTab {
        repo: Uuid,
        tab: Uuid,
    },
    NewTab {
        repo: Uuid,
        kind: PaneKind,
    },
    /// Opens a tab running `Settings::presets[preset]`.
    RunPreset {
        repo: Uuid,
        preset: usize,
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
    /// A repo dragged in the sidebar to `index` among `project`'s other repos, which may be its own.
    MoveRepo {
        repo: Uuid,
        project: Uuid,
        index: usize,
    },
    /// Opens a shell tab at a folder dropped from Finder.
    OpenFolderTab(PathBuf),
    ToggleSidePanel,
    /// Shows the selected repo's Code view (files and diffs), or its terminals with `false`.
    ShowCode(bool),
    /// Shows the Code view with the Search panel, its query box focused.
    OpenSearch,
    /// Looks for where `word` is defined in the repo: one place opens, several (or `peek`) list.
    GoToDefinition {
        repo: Uuid,
        word: String,
        peek: bool,
    },
    ClosePeekDefinition,
    /// Runs the file's formatter (rustfmt, prettier, gofmt, ruff…) over the active file.
    FormatActiveFile,
    /// Shows the active file a second time beside itself, or closes that second editor.
    ToggleSplitEditor,
    /// Opens the find bar in the active terminal pane.
    OpenFind,
    /// Opens the palette listing the selected repo's files.
    OpenQuickOpen,
    /// Opens the go-to-line bar in the active file editor.
    OpenGoToLine,
    SetSidePanel(SidePanel),
    /// Shows or hides gitignored files in the Files panel.
    ToggleIgnoredFiles,
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
    /// Moves a tab to the end of another repo's tabs and shows it there.
    MoveTabToRepo {
        from: Uuid,
        tab: Uuid,
        to: Uuid,
    },
    /// Selects the tab `offset` places from the active one, wrapping around.
    SelectAdjacentTab(i32),
    OpenNewWorktree(Uuid),
    /// Makes a worktree on a temporary branch and opens Claude there; the branch is named later.
    NewWorktreeAgent(Uuid),
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
