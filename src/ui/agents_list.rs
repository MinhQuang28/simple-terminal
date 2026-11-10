//! The sidebar's Agents view: every agent tab across projects in one list, the ones waiting on
//! the user first, so a dozen agents in different repos can be watched without opening each.
//! An agent started by another through the `repodeck` CLI is listed under it.

use std::cmp::Reverse;

use eframe::egui::{Align, Label, Layout, RichText, Ui};
use uuid::Uuid;

use super::Action;
use super::agent_status;
use super::claude_icon::ICON_SIZE;
use super::program_icon;
use super::repo_indicators::busy_mark;
use super::sidebar::{tab_agent, tab_busy, tab_menu, tab_program, tab_status, tab_title};
use super::widgets::{muted, right_aligned, row_sized};
use crate::agent_events::AgentStatus;
use crate::model::{AppState, PaneKind, Repo, Tab};
use crate::session_pool::SessionPool;

const ROW_HEIGHT: f32 = 40.0;
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
            Some(status) if status.needs_attention() => 3 + status as u8,
            _ if self.busy => 2,
            _ => 1,
        }
    }
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
    if entries.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new("No agents running.").color(muted(ui)));
        return;
    }
    let roots: Vec<Option<Uuid>> = entries.iter().map(|e| root_of(e.tab, &entries)).collect();
    let order = |e: &&Entry| (Reverse(e.urgency()), Reverse(e.tab.last_active));
    // Each agent with the sub-agents it started under it; it takes the place in the list its
    // most pressing one would have.
    let mut groups: Vec<(&Entry, Vec<&Entry>)> = entries
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
            (entry, children)
        })
        .collect();
    groups.sort_by_key(|(entry, children)| {
        let urgency = children.iter().map(|c| c.urgency()).fold(entry.urgency(), u8::max);
        (Reverse(entry.tab.pinned), Reverse(urgency), Reverse(entry.tab.last_active))
    });
    for (entry, children) in groups {
        // Rows re-sort as statuses change; ids follow the tab, not the slot, so a click or an
        // open menu stays with the tab it began on.
        ui.push_id(entry.tab.id, |ui| entry_row(ui, state, pool, entry, 0.0, actions));
        for child in children {
            ui.push_id(child.tab.id, |ui| entry_row(ui, state, pool, child, CHILD_INDENT, actions));
        }
    }
}

fn entry_row(
    ui: &mut Ui,
    state: &AppState,
    pool: &SessionPool,
    entry: &Entry<'_>,
    indent: f32,
    actions: &mut Vec<Action>,
) {
    let Entry { repo, tab, status, busy } = *entry;
    let selected = state.selected_repo == Some(repo.id) && repo.active_tab == Some(tab.id);
    let agent = tab_agent(tab, pool);
    let (response, ()) = row_sized(ui, selected, 6.0 + indent, ROW_HEIGHT, |ui| {
        right_aligned(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add_space(6.0);
            if busy {
                busy_mark(ui);
            }
            if let Some(status) = status.filter(|s| s.needs_attention()) {
                agent_status::pill(ui, status);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                program_icon::icon(ui, ICON_SIZE, tab_program(tab, pool));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let pin = if tab.pinned { "📌 " } else { "" };
                    let title = format!("{pin}{}", tab_title(tab, pool, agent.label()));
                    let color = if selected {
                        ui.visuals().text_color()
                    } else {
                        ui.visuals().text_color().gamma_multiply(0.85)
                    };
                    ui.add(
                        Label::new(RichText::new(title).size(12.5).color(color)).truncate().selectable(false),
                    );
                    let caption = RichText::new(&repo.name).size(11.0).color(muted(ui));
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
