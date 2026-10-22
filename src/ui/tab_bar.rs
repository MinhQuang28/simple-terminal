use eframe::egui::{Align2, Button, Color32, FontId, Rect, Response, RichText, Sense, Ui, pos2, vec2};

use super::Action;
use super::agent_icon;
use super::agent_status;
use super::claude_icon::ICON_SIZE;
use super::history_menu;
use super::sidebar::{tab_agent, tab_busy, tab_menu, tab_status, tab_title};
use super::widgets::{accent, muted, right_aligned, spinner_at};
use crate::agent_events::AgentStatus;
use crate::agent_launch::Agent;
use crate::claude_history::ClaudeHistory;
use crate::editor::Buffers;
use crate::model::{AppState, PaneKind, Repo, Tab};
use crate::session_pool::SessionPool;

pub const TAB_HEIGHT: f32 = 38.0;
const MAX_TITLE_CHARS: usize = 28;

/// Per-tab state that changes how a tab is drawn.
#[derive(Clone, Copy)]
struct TabState {
    active: bool,
    dirty: bool,
    busy: bool,
    attention: Option<AgentStatus>,
    agent: Agent,
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    state: &AppState,
    repo: &Repo,
    branch: Option<&str>,
    pool: &SessionPool,
    buffers: &Buffers,
    history: &ClaudeHistory,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        ui.set_height(TAB_HEIGHT);
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_space(10.0);
        let mut centers = Vec::with_capacity(repo.tabs.len());
        let mut dragging = None;
        for (index, tab) in repo.tabs.iter().enumerate() {
            let fallback = branch.unwrap_or(&repo.name);
            let title = truncate(&tab_title(tab, pool, fallback), MAX_TITLE_CHARS);
            let tab_state = TabState {
                active: repo.active_tab == Some(tab.id),
                dirty: tab.file.as_deref().is_some_and(|p| buffers.is_dirty(p)),
                busy: tab_busy(tab, pool),
                attention: tab_status(tab, pool).filter(|s| s.needs_attention()),
                agent: tab_agent(tab, pool),
            };
            let (response, close) = tab_item(ui, index + 1, tab, &title, &tab_state);
            centers.push(response.rect.center().x);
            if response.dragged()
                && let Some(pos) = response.interact_pointer_pos()
            {
                dragging = Some((index, tab.id, pos.x));
            }
            if close || response.middle_clicked() {
                actions.push(Action::CloseTab { repo: repo.id, tab: tab.id });
            } else if response.clicked() {
                actions.push(Action::SelectTab { repo: repo.id, tab: tab.id });
            }
            response.context_menu(|ui| tab_menu(ui, state, repo.id, tab.id, actions));
        }
        // Dragging a tab past a neighbour's centre moves it there.
        if let Some((index, tab, x)) = dragging {
            let to = centers.iter().enumerate().filter(|(i, c)| *i != index && **c < x).count();
            if to != index {
                actions.push(Action::MoveTab { repo: repo.id, tab, to });
            }
        }
        ui.add_space(6.0);
        ui.menu_button(RichText::new("+").size(19.0), |ui| {
            ui.set_min_width(180.0);
            if ui.add(Button::new("New terminal").shortcut_text("⌘T")).clicked() {
                actions.push(Action::NewTab { repo: repo.id, kind: PaneKind::Shell });
            }
            if ui.add(Button::new("New Claude session").shortcut_text("⇧⌘T")).clicked() {
                actions.push(Action::NewTab { repo: repo.id, kind: PaneKind::Agent });
            }
        });
        right_aligned(ui, |ui| {
            ui.add_space(8.0);
            history_menu::show(ui, repo, history, actions);
        });
    });
}

/// A numbered tab with an accent underline when active; returns (response, close clicked).
fn tab_item(ui: &mut Ui, number: usize, tab: &Tab, title: &str, state: &TabState) -> (Response, bool) {
    let TabState { active, dirty, busy, attention, agent } = *state;
    let attention_color = attention.map(|s| agent_status::color(ui, s));
    let text_color = if active { ui.visuals().text_color() } else { muted(ui) };
    let icon_width = match (&tab.file, tab.kind()) {
        (None, PaneKind::Agent) => agent_icon::width(ICON_SIZE, agent),
        (Some(_), _) | (None, PaneKind::Shell) => 0.0,
    };
    let font = FontId::proportional(13.5);
    let layout = |text: String, color: Color32| ui.painter().layout_no_wrap(text, font.clone(), color);
    let number_galley = layout(number.to_string(), muted(ui));
    let title_galley = layout(title.to_string(), text_color);
    let icon_gap = if icon_width > 0.0 { 6.0 } else { 0.0 };

    let close_size = 16.0;
    let width = 12.0
        + number_galley.size().x
        + 8.0
        + icon_width
        + icon_gap
        + title_galley.size().x
        + 8.0
        + close_size
        + 8.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, TAB_HEIGHT), Sense::click_and_drag());
    let painter = ui.painter();
    if response.hovered() && !active {
        painter.rect_filled(rect.shrink2(vec2(0.0, 5.0)), 6.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }

    let mut x = rect.left() + 12.0;
    let y = rect.center().y;
    let number_width = number_galley.size().x;
    painter.galley(pos2(x, y - number_galley.size().y / 2.0), number_galley, text_color);
    x += number_width + 8.0;
    if icon_width > 0.0 {
        let icon = Rect::from_min_size(pos2(x, y - ICON_SIZE / 2.0), vec2(icon_width, ICON_SIZE));
        agent_icon::paint(painter, icon, agent, text_color);
        x += icon_width + icon_gap;
    }
    painter.galley(pos2(x, y - title_galley.size().y / 2.0), title_galley, text_color);

    let close_rect =
        Rect::from_center_size(pos2(rect.right() - 8.0 - close_size / 2.0, y), vec2(close_size, close_size));
    let pointer = response.hover_pos();
    let over_close = pointer.is_some_and(|p| close_rect.contains(p));
    if dirty && !over_close {
        painter.circle_filled(close_rect.center(), 3.5, text_color);
    } else if let Some(color) = attention_color.filter(|_| !over_close) {
        painter.circle_filled(close_rect.center(), 3.5, color);
    } else if busy && !response.hovered() {
        spinner_at(ui, close_rect.shrink(2.0), muted(ui));
    } else if response.hovered() || active {
        if over_close {
            painter.rect_filled(close_rect, 4.0, ui.visuals().widgets.active.weak_bg_fill);
        }
        painter.text(close_rect.center(), Align2::CENTER_CENTER, "×", FontId::proportional(14.0), muted(ui));
    }
    if active {
        let underline = Rect::from_min_max(
            pos2(rect.left() + 6.0, rect.bottom() - 2.0),
            pos2(rect.right() - 6.0, rect.bottom()),
        );
        painter.rect_filled(underline, 1.0, accent(ui));
    }
    let close = response.clicked() && over_close;
    let response = match (tab.file.as_ref().or(tab.diff.as_ref()), attention) {
        (Some(path), _) => response.on_hover_text(path.display().to_string()),
        (None, Some(status)) => response.on_hover_text(status.label()),
        (None, None) => response,
    };
    (response, close)
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}
