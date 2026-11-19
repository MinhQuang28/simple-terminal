//! The sidebar's Agents strip: every agent tab across projects in one list under the project
//! tree, in sections by what they are doing (the ones waiting on the user first) or by repo, so
//! a dozen agents in different repos can be watched without opening each. An agent started by another
//! through the `repodeck` CLI is listed under it.

use std::cmp::Reverse;

use eframe::egui::{Align, Color32, Label, Layout, RichText, Ui};
use uuid::Uuid;

use super::Action;
use super::agent_status;
use super::claude_icon::ICON_SIZE;
use super::design::{TEXT_CAPTION, TEXT_SECONDARY};
use super::program_icon;
use super::repo_indicators::busy_mark;
use super::sidebar::{tab_agent, tab_busy, tab_menu, tab_program, tab_status, tab_title};
use super::widgets::{ago, glyph_button, muted, right_aligned, row_hovered, row_sized};
use crate::agent_events::AgentStatus;
use crate::model::{AgentsSort, AppState, PaneKind, Repo, Tab, now_secs};
use crate::session_pool::SessionPool;

/// A row naming its repo under the title, and one under a repo heading that already does.
const ROW_HEIGHT: f32 = 40.0;
const SHORT_ROW_HEIGHT: f32 = 30.0;
/// How far a sub-agent's row sits in from the agent that started it.
const CHILD_INDENT: f32 = 16.0;

/// An agent tab with what decides its place in the list.
struct Entry<'a> {
    repo: &'a Repo,
    tab: &'a Tab,
    status: Option<AgentStatus>,
    busy: bool,
}

impl Entry<'_> {
    /// Waiting on the user (most pressing first), then working, then idle.
    fn urgency(&self) -> u8 {
        match self.status {
            // Errors, then approvals, questions, and finished turns.
            Some(status) if status.needs_attention() => NEEDS_YOU + status as u8,
            _ if self.busy => WORKING,
            _ => 1,
        }
    }
}

/// Urgency of a working agent, and the least urgency of one waiting on the user.
const WORKING: u8 = 2;
const NEEDS_YOU: u8 = 3;

/// The list's sections, most pressing first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    NeedsYou,
    Working,
    Idle,
}

impl Section {
    fn of(urgency: u8) -> Self {
        match urgency {
            NEEDS_YOU.. => Self::NeedsYou,
            WORKING => Self::Working,
            _ => Self::Idle,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::NeedsYou => "Needs you",
            Self::Working => "Working",
            Self::Idle => "Idle",
        }
    }

    fn color(self, ui: &Ui) -> Color32 {
        match self {
            Self::NeedsYou => ui.visuals().text_color(),
            Self::Working => ui.visuals().warn_fg_color,
            Self::Idle => muted(ui),
        }
    }
}

/// A section's name and how many agents it holds.
fn section_heading(ui: &mut Ui, label: &str, color: Color32, count: usize) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        let name = RichText::new(label).size(TEXT_CAPTION).color(color);
        ui.add(Label::new(name).truncate().selectable(false));
        let count = RichText::new(count.to_string()).size(TEXT_CAPTION).color(muted(ui));
        ui.add(Label::new(count).selectable(false));
    });
    ui.add_space(2.0);
}

/// Agent tabs in every project.
pub fn count(state: &AppState, pool: &SessionPool) -> usize {
    agent_tabs(state, pool).count()
}

/// Tabs started as agents, and terminal tabs where an agent reports its status (OSC 7501).
fn agent_tabs<'a>(state: &'a AppState, pool: &'a SessionPool) -> impl Iterator<Item = (&'a Repo, &'a Tab)> {
    state.projects.iter().flat_map(|p| &p.repos).flat_map(|r| r.tabs.iter().map(move |t| (r, t))).filter(
        move |(_, t)| {
            !t.settled
                && t.panes.iter().any(|p| p.kind == PaneKind::Agent || pool.agent_status(p.id).is_some())
        },
    )
}

/// The listed tab holding the agent that started the one in `tab`, followed up to the first
/// agent of the chain so a sub-agent's own sub-agents sit under the same row.
fn root_of(tab: &Tab, entries: &[Entry<'_>]) -> Option<Uuid> {
    let holder = |tab: &Tab| {
        let parents: Vec<Uuid> = tab.panes.iter().filter_map(|p| p.parent).collect();
        entries
            .iter()
            .map(|e| e.tab)
            .find(|t| t.id != tab.id && t.panes.iter().any(|p| parents.contains(&p.id)))
    };
    let mut root = holder(tab)?;
    for _ in 0..8 {
        match holder(root) {
            Some(above) if above.id != tab.id => root = above,
            _ => break,
        }
    }
    Some(root.id)
}

pub fn show(ui: &mut Ui, state: &AppState, pool: &SessionPool, actions: &mut Vec<Action>) {
    let entries: Vec<Entry> = agent_tabs(state, pool)
        .map(|(repo, tab)| Entry { repo, tab, status: tab_status(tab, pool), busy: tab_busy(tab, pool) })
        .collect();
    let roots: Vec<Option<Uuid>> = entries.iter().map(|e| root_of(e.tab, &entries)).collect();
    let order = |e: &&Entry| (Reverse(e.urgency()), Reverse(e.tab.last_active));
    // Each agent with the sub-agents it started under it; it takes the place in the list its
    // most pressing one would have.
    let mut groups: Vec<(u8, &Entry, Vec<&Entry>)> = entries
        .iter()
        .zip(&roots)
        .filter(|(_, root)| root.is_none())
        .map(|(entry, _)| {
            let mut children: Vec<&Entry> = entries
                .iter()
                .zip(&roots)
                .filter(|(_, root)| **root == Some(entry.tab.id))
                .map(|(child, _)| child)
                .collect();
            children.sort_by_key(order);
            let urgency = children.iter().map(|c| c.urgency()).fold(entry.urgency(), u8::max);
            (urgency, entry, children)
        })
        .collect();
    let by_repo = state.agents_sort == AgentsSort::ByRepo;
    // Which section a group falls in: its repo, in sidebar order, or its status.
    let repo_index = |repo: &Repo| {
        state.projects.iter().flat_map(|p| &p.repos).position(|r| r.id == repo.id).unwrap_or(usize::MAX)
    };
    let section_of = |urgency: u8, entry: &Entry| {
        if by_repo { repo_index(entry.repo) } else { Section::of(urgency) as usize }
    };
    groups.sort_by_key(|(urgency, entry, _)| {
        let within = (Reverse(entry.tab.pinned), Reverse(*urgency), Reverse(entry.tab.last_active));
        (section_of(*urgency, entry), within)
    });
    let now = now_secs();
    let mut current = None;
    for (urgency, entry, children) in &groups {
        let section = section_of(*urgency, entry);
        if current != Some(section) {
            current = Some(section);
            let held = groups.iter().filter(|(u, e, _)| section_of(*u, e) == section);
            let count = held.map(|(_, _, children)| 1 + children.len()).sum();
            if by_repo {
                section_heading(ui, &entry.repo.name, muted(ui), count);
            } else {
                let status = Section::of(*urgency);
                section_heading(ui, status.label(), status.color(ui), count);
            }
        }
        let row = |ui: &mut Ui, entry: &Entry, indent: f32, actions: &mut Vec<Action>| {
            entry_row(ui, state, pool, entry, indent, !by_repo, now, actions);
        };
        // Rows re-sort as statuses change; ids follow the tab, not the slot, so a click or an
        // open menu stays with the tab it began on.
        ui.push_id(entry.tab.id, |ui| row(ui, entry, 0.0, actions));
        for child in children {
            ui.push_id(child.tab.id, |ui| row(ui, child, CHILD_INDENT, actions));
        }
    }
}

/// One agent: its title, with its repo under it when `captioned`, then how long ago it was
/// last active and what it is doing.
#[allow(clippy::too_many_arguments)]
fn entry_row(
    ui: &mut Ui,
    state: &AppState,
    pool: &SessionPool,
    entry: &Entry<'_>,
    indent: f32,
    captioned: bool,
    now: u64,
    actions: &mut Vec<Action>,
) {
    let Entry { repo, tab, status, busy } = *entry;
    let selected = state.selected_repo == Some(repo.id) && repo.active_tab == Some(tab.id);
    let agent = tab_agent(tab, pool);
    let height = if captioned { ROW_HEIGHT } else { SHORT_ROW_HEIGHT };
    let (response, ()) = row_sized(ui, selected, 6.0 + indent, height, |ui| {
        let hovered = row_hovered(ui);
        right_aligned(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // On hover the time gives way to a quick close; a working agent still asks first.
            if hovered {
                if glyph_button(ui, "×").on_hover_text("Close tab").clicked() {
                    actions.push(Action::CloseTab { repo: repo.id, tab: tab.id });
                }
            } else if busy {
                ui.add_space(6.0);
                busy_mark(ui);
            } else {
                ui.add_space(6.0);
                ui.label(RichText::new(ago(tab.last_active, now)).color(muted(ui)).size(TEXT_CAPTION));
            }
            if let Some(status) = status.filter(|s| s.needs_attention()) {
                agent_status::pill(ui, status);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                program_icon::icon(ui, ICON_SIZE, tab_program(tab, pool));
                let pin = if tab.pinned { "📌 " } else { "" };
                let title = format!("{pin}{}", tab_title(tab, pool, agent.label()));
                let color = if selected {
                    ui.visuals().text_color()
                } else {
                    ui.visuals().text_color().gamma_multiply(0.85)
                };
                let title = Label::new(RichText::new(title).size(TEXT_SECONDARY).color(color))
                    .truncate()
                    .selectable(false);
                if !captioned {
                    ui.add(title);
                    return;
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.add(title);
                    let caption = RichText::new(&repo.name).size(TEXT_CAPTION).color(muted(ui));
                    ui.add(Label::new(caption).truncate().selectable(false));
                });
            });
        });
    });
    if response.clicked() {
        actions.push(Action::SelectTab { repo: repo.id, tab: tab.id });
    }
    response.context_menu(|ui| {
        let label = if tab.pinned { "Unpin" } else { "Pin to top" };
        if ui.button(label).clicked() {
            actions.push(Action::TogglePinTab { repo: repo.id, tab: tab.id });
        }
        ui.separator();
        tab_menu(ui, state, repo.id, tab.id, actions);
    });
}
