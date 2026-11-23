//! The rows under a repo's branch in the sidebar: one per linked worktree, naming its branch
//! with the same activity marks as the repo's own branch row. Tabs only appear in the tab bar
//! and in the Agents strip.

use eframe::egui::{Align, Label, Layout, RichText, Stroke, Ui, pos2};

use super::Action;
use super::branch_menu::inline_glyph;
use super::design::TEXT_SECONDARY;
use super::repo_indicators::{RepoRow, indicators};
use super::sidebar_rows::select_and_open;
use super::widgets::{muted, right_aligned, row_sized};
use crate::model::{PaneKind, Repo};

/// The branch row and the tree guides line up with the repo name, past its avatar.
pub const TEXT_INDENT: f32 = 6.0 + 24.0 + 10.0;
const ROW_INDENT: f32 = TEXT_INDENT + 22.0;
const ROW_HEIGHT: f32 = 26.0;

/// A worktree's branch and activity; clicking shows the worktree. `last` ends the tree guide.
pub fn worktree_row(ui: &mut Ui, row: &RepoRow, selected: bool, last: bool, actions: &mut Vec<Action>) {
    let repo = row.repo;
    let branch = row.status.map_or(repo.name.as_str(), |s| s.branch.as_str());
    let (response, ()) = row_sized(ui, selected, ROW_INDENT, ROW_HEIGHT, |ui| {
        // Indicators go first so the branch name truncates to the space they leave.
        right_aligned(ui, |ui| {
            indicators(ui, row, actions);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                inline_glyph(ui, muted(ui));
                let color = if selected {
                    ui.visuals().text_color()
                } else {
                    ui.visuals().text_color().gamma_multiply(0.8)
                };
                ui.add(
                    Label::new(RichText::new(branch).size(TEXT_SECONDARY).color(color))
                        .truncate()
                        .selectable(false),
                );
            });
        });
    });
    tree_connector(ui, response.rect.left() + TEXT_INDENT + 6.0, response.rect, last);
    if response.clicked() {
        select_and_open(row, actions);
    }
    response.on_hover_text(repo.path.display().to_string()).context_menu(|ui| {
        if ui.button("New Claude session").clicked() {
            actions.push(Action::NewTab { repo: repo.id, kind: PaneKind::Agent });
        }
        if ui.button("New terminal").clicked() {
            actions.push(Action::NewTab { repo: repo.id, kind: PaneKind::Shell });
        }
        worktree_menu(ui, repo, actions);
    });
}

fn worktree_menu(ui: &mut Ui, repo: &Repo, actions: &mut Vec<Action>) {
    ui.separator();
    if ui.button("Reveal worktree in Finder").clicked() {
        actions.push(Action::RevealRepo(repo.id));
    }
    if ui.button("Remove worktree…").clicked() {
        actions.push(Action::OpenRemoveWorktree(repo.id));
    }
}

/// The `├`/`└` guide joining a worktree to its repo; `last` stops the vertical line at this row.
fn tree_connector(ui: &Ui, x: f32, row: eframe::egui::Rect, last: bool) {
    let stroke = Stroke::new(1.0, muted(ui).gamma_multiply(0.5));
    let bottom = if last { row.center().y } else { row.bottom() };
    ui.painter().line_segment([pos2(x, row.top()), pos2(x, bottom)], stroke);
    ui.painter().line_segment([pos2(x, row.center().y), pos2(x + 8.0, row.center().y)], stroke);
}
