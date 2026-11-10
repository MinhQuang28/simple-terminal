//! The Code view's app-level commands: go to / peek a definition, format the file, split the
//! editor.

use uuid::Uuid;

use crate::app::RepodeckApp;
use crate::definitions::{self, Peek};

impl RepodeckApp {
    /// Searches the repo for `word`'s declaration off the UI thread: one opens there, several (or
    /// a peek) are listed in the peek window.
    pub(crate) fn go_to_definition(&mut self, repo_id: Uuid, word: String, peek: bool) {
        let Some(repo) = self.state.repo(repo_id) else { return };
        let root = repo.path.clone();
        let current = self.active_file();
        self.spawn_job("definitions", move || {
            let found = definitions::find(&root, &word, current.as_deref());
            Box::new(move |app: &mut RepodeckApp| match (found.len(), peek) {
                (0, _) => crate::ui::toast::post(&app.ctx, format!("No definition of {word} found")),
                (1, false) => {
                    let place = &found[0];
                    app.open_file(repo_id, place.path.clone());
                    app.pending_goto = Some((place.line, place.col));
                }
                _ => app.peek_definition = Some(Peek { repo: repo_id, root, word, found }),
            })
        });
    }

    /// Formats the active file's text, saved or not, with the formatter its language uses.
    pub(crate) fn format_active_file(&mut self) {
        let (Some(path), Some(repo)) =
            (self.active_file(), self.state.selected_repo.and_then(|id| self.state.repo(id)))
        else {
            return;
        };
        let root = repo.path.clone();
        let Some(text) = self.buffers.text(&path) else { return };
        let shell = crate::shell_env::program(&self.state.settings.shell);
        let path_var = crate::shell_env::get(&shell).and_then(|env| env.get("PATH")).cloned();
        self.spawn_job("format", move || {
            let result = crate::code_format::format(&path, &root, &text, path_var.as_deref());
            Box::new(move |app: &mut RepodeckApp| match result {
                Ok(formatted) if formatted == text => {}
                Ok(formatted) => {
                    if !app.buffers.replace_text(&path, &text, formatted) {
                        crate::ui::toast::post(&app.ctx, "The file changed while formatting; format again");
                    }
                }
                Err(err) => crate::ui::toast::post(&app.ctx, err),
            })
        });
    }

    /// ⌘\: shows the active file again in a second editor beside it, or closes that editor.
    pub(crate) fn toggle_split_editor(&mut self) {
        let Some(repo) = self.selected_repo_mut().filter(|r| r.code_view) else { return };
        repo.split_tab = match repo.split_tab {
            Some(_) => None,
            None => repo.active_tab,
        };
    }
}
