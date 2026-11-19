//! Creates a project from the "Add project" dialog: around a folder on disk, a new repository,
//! or a clone, which runs in the background.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app::RepodeckApp;
use crate::model::{PaneKind, Project};
use crate::ui::add_project_dialog::{ProjectSource, clone_name};
use crate::ui::toast;

impl RepodeckApp {
    pub(crate) fn add_project(&mut self, name: String, source: ProjectSource) {
        match source {
            ProjectSource::Folder(path) => self.open_project(name, &path),
            ProjectSource::Create { parent, name: repo } => {
                let path = parent.join(&repo);
                match create_repository(&path) {
                    Ok(()) => self.open_project(name, &path),
                    Err(err) => self.project_failed(&format!("Couldn't create {repo}"), err),
                }
            }
            ProjectSource::Clone { url, parent } => {
                let Some(repo) = clone_name(&url).map(str::to_string) else { return };
                let path = parent.join(&repo);
                toast::post(&self.ctx, format!("Cloning {repo}…"));
                self.spawn_job("git-clone", move || {
                    let result = git(&parent, &["clone", "--", &url, &repo]);
                    Box::new(move |app: &mut RepodeckApp| match result {
                        Ok(()) => {
                            app.open_project(name, &path);
                            toast::post(&app.ctx, format!("Cloned {repo}"));
                        }
                        Err(err) => app.project_failed(&format!("Couldn't clone {repo}"), err),
                    })
                });
            }
        }
    }

    /// Adds a project holding the repository at `path`, named after the folder when `name` is
    /// empty, and shows it with a terminal open.
    fn open_project(&mut self, name: String, path: &Path) {
        let folder = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let name = if name.is_empty() { folder.unwrap_or_else(|| path.display().to_string()) } else { name };
        let project = Project::new(name);
        let project_id = project.id;
        self.state.projects.push(project);
        let repo = self.add_repo_to(Some(project_id), path);
        self.state.selected_repo = Some(repo);
        if self.state.repo(repo).is_some_and(|r| r.tabs.is_empty()) {
            self.new_tab(repo, PaneKind::Shell, None);
        }
        self.focus_active_pane();
        self.dirty = true;
    }

    /// Answers where the click was made; the inbox keeps git's full message.
    fn project_failed(&mut self, title: &str, detail: String) {
        let why = detail.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or_default();
        toast::post(&self.ctx, format!("{title}: {why}"));
        self.report_error(title, detail);
    }
}

fn create_repository(path: &PathBuf) -> Result<(), String> {
    std::fs::create_dir(path).map_err(|err| format!("{}: {err}", path.display()))?;
    git(path, &["init"])
}

/// Runs git in `dir`; its error output on failure.
fn git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git").arg("-C").arg(dir).args(args).output().map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
