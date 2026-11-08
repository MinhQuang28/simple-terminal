//! A repo shows one of two views: Terminals (shells and agents) or Code (files and diffs). Each
//! keeps its own active tab, so switching views never mixes an editor in with the terminals.
//! Invariant: `active_tab`, when set, is a tab of the view showing; `parked_tab` one of the other.

use uuid::Uuid;

use crate::model::{Repo, Tab};

impl Repo {
    /// Tabs of the view showing, in order.
    pub fn view_tabs(&self) -> impl Iterator<Item = &Tab> {
        self.tabs.iter().filter(move |t| t.is_document() == self.code_view)
    }

    /// Switches to the Code view, or back to the terminals, restoring that view's tab.
    pub fn show_code(&mut self, code: bool) {
        if self.code_view != code {
            std::mem::swap(&mut self.active_tab, &mut self.parked_tab);
            self.code_view = code;
        }
        // The remembered tab may have closed or moved to another repo meanwhile.
        if !self.active_tab.is_some_and(|id| self.view_tabs().any(|t| t.id == id)) {
            self.active_tab = self.view_tabs().last().map(|t| t.id);
        }
    }

    /// Shows `tab`, switching to the view it belongs to.
    pub fn activate(&mut self, tab: Uuid) {
        let Some(doc) = self.tabs.iter().find(|t| t.id == tab).map(Tab::is_document) else { return };
        self.show_code(doc);
        self.active_tab = Some(tab);
    }

    /// Removes the tab, activating its neighbour in the same view; returns it so it can be reopened.
    pub fn remove_tab(&mut self, tab: Uuid) -> Option<Tab> {
        let index = self.tabs.iter().position(|t| t.id == tab)?;
        let doc = self.tabs[index].is_document();
        let siblings: Vec<Uuid> = self.tabs.iter().filter(|t| t.is_document() == doc).map(|t| t.id).collect();
        let at = siblings.iter().position(|id| *id == tab)?;
        let next = siblings.get(at + 1).or_else(|| siblings.get(at.checked_sub(1)?)).copied();
        for slot in [&mut self.active_tab, &mut self.parked_tab] {
            if *slot == Some(tab) {
                *slot = next;
            }
        }
        if self.split_tab == Some(tab) {
            self.split_tab = None;
        }
        Some(self.tabs.remove(index))
    }

    /// The tab `offset` places from the active one within the view, wrapping around.
    pub fn adjacent_tab(&self, offset: i32) -> Option<Uuid> {
        let ids: Vec<Uuid> = self.view_tabs().map(|t| t.id).collect();
        let index = ids.iter().position(|id| Some(*id) == self.active_tab)?;
        Some(ids[(index as i32 + offset).rem_euclid(ids.len() as i32) as usize])
    }

    /// Moves `tab` to position `to` among the view's tabs; other views' tabs keep their places.
    pub fn move_in_view(&mut self, tab: Uuid, to: usize) {
        let ids: Vec<Uuid> = self.view_tabs().map(|t| t.id).collect();
        let (Some(from), Some(&target)) =
            (ids.iter().position(|id| *id == tab), ids.get(to.min(ids.len().saturating_sub(1))))
        else {
            return;
        };
        if target == tab {
            return;
        }
        let Some(index) = self.tabs.iter().position(|t| t.id == tab) else { return };
        let moved = self.tabs.remove(index);
        let Some(at) = self.tabs.iter().position(|t| t.id == target) else { return };
        // Moving right lands after the tab it passed, moving left before it.
        self.tabs.insert(if to > from { at + 1 } else { at }, moved);
    }

    /// Repairs state saved before the views existed, where a file tab could be active among
    /// terminals, and remembered tabs that no longer exist.
    pub fn normalize_view(&mut self) {
        let kind = |id: Option<Uuid>| self.tabs.iter().find(|t| Some(t.id) == id).map(Tab::is_document);
        if let Some(doc) = kind(self.active_tab) {
            self.code_view = doc;
        } else {
            self.active_tab = None;
        }
        if kind(self.parked_tab) != Some(!self.code_view) {
            self.parked_tab = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::model::PaneKind;

    fn repo() -> (Repo, [Uuid; 4]) {
        let mut repo = Repo::from_path(PathBuf::from("/tmp/r"));
        let tabs = [
            Tab::new(PaneKind::Shell),
            Tab::for_file(PathBuf::from("/tmp/r/a.rs")),
            Tab::new(PaneKind::Agent),
            Tab::for_file(PathBuf::from("/tmp/r/b.rs")),
        ];
        let ids = [tabs[0].id, tabs[1].id, tabs[2].id, tabs[3].id];
        repo.tabs = tabs.into();
        repo.active_tab = Some(ids[0]);
        (repo, ids)
    }

    #[test]
    fn each_view_keeps_its_own_tab() {
        let (mut repo, [shell, a, agent, b]) = repo();
        assert_eq!(repo.view_tabs().map(|t| t.id).collect::<Vec<_>>(), [shell, agent]);
        repo.activate(b);
        assert!(repo.code_view);
        assert_eq!(repo.view_tabs().map(|t| t.id).collect::<Vec<_>>(), [a, b]);
        repo.show_code(false);
        assert_eq!(repo.active_tab, Some(shell));
        repo.show_code(true);
        assert_eq!(repo.active_tab, Some(b));
        assert_eq!(repo.adjacent_tab(1), Some(a), "wraps within the view");
    }

    #[test]
    fn closing_a_file_activates_a_file() {
        let (mut repo, [shell, a, _, b]) = repo();
        repo.activate(a);
        repo.remove_tab(a);
        assert_eq!(repo.active_tab, Some(b));
        repo.remove_tab(b);
        assert_eq!(repo.active_tab, None, "no terminal takes the Code view's place");
        assert!(repo.code_view);
        repo.show_code(false);
        assert_eq!(repo.active_tab, Some(shell));
    }

    #[test]
    fn moves_among_the_view_only() {
        let (mut repo, [shell, a, agent, b]) = repo();
        repo.move_in_view(agent, 0);
        assert_eq!(repo.tabs.iter().map(|t| t.id).collect::<Vec<_>>(), [agent, shell, a, b]);
        repo.code_view = true;
        repo.move_in_view(a, 1);
        assert_eq!(repo.tabs.iter().map(|t| t.id).collect::<Vec<_>>(), [agent, shell, b, a]);
    }

    #[test]
    fn normalizes_a_file_tab_active_among_terminals() {
        let (mut repo, [_, a, ..]) = repo();
        repo.active_tab = Some(a);
        repo.parked_tab = Some(Uuid::new_v4());
        repo.normalize_view();
        assert!(repo.code_view);
        assert_eq!(repo.parked_tab, None);
    }
}
