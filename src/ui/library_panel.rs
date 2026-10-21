use std::path::Path;

use eframe::egui::{Label, RichText, Ui};

use super::Action;
use super::widgets::{glyph_button, muted, right_aligned, row, spinner, tag};
use crate::library::{McpServer, Skill};
use crate::library_cache::LibraryCache;
use crate::model::{Repo, Settings};

/// Prompts, skills and MCP servers available to agents in the selected repo.
pub fn show(
    ui: &mut Ui,
    repo: &Repo,
    settings: &Settings,
    library: &LibraryCache,
    actions: &mut Vec<Action>,
) {
    section(ui, "Prompts", Some(Action::OpenEditPrompt(None)), actions);
    if settings.prompts.is_empty() {
        hint(ui, "Reusable snippets to paste into a pane. ⇧⌘P picks one from the keyboard.");
    }
    for (index, prompt) in settings.prompts.iter().enumerate() {
        let (response, ()) = row(ui, false, 0.0, |ui| {
            let first = prompt.text.lines().next().unwrap_or_default();
            ui.add(Label::new(RichText::new(&prompt.name).size(13.0)).truncate().selectable(false));
            ui.add(Label::new(RichText::new(first).size(11.5).color(muted(ui))).truncate().selectable(false));
        });
        if response.clicked() {
            actions.push(Action::InsertPrompt(prompt.text.clone()));
        }
        response.on_hover_text(&prompt.text).context_menu(|ui| {
            if ui.button("Edit…").clicked() {
                actions.push(Action::OpenEditPrompt(Some(index)));
            }
            if ui.button("Delete").clicked() {
                actions.push(Action::DeletePrompt(index));
            }
        });
    }

    // Scanned in the background: reading every SKILL.md and ~/.claude.json takes milliseconds.
    let Some(library) = library.get(&repo.path) else {
        section(ui, "Skills", None, actions);
        ui.horizontal(|ui| {
            spinner(ui, 12.0, muted(ui));
            ui.label(RichText::new("Loading…").color(muted(ui)));
        });
        return;
    };
    section(ui, "Skills", None, actions);
    if library.skills.is_empty() {
        hint(ui, "No skills in ~/.claude/skills or this repo's .claude/skills.");
    }
    for skill in &library.skills {
        skill_row(ui, repo, skill, actions);
    }

    section(ui, "MCP servers", None, actions);
    if library.servers.is_empty() {
        hint(ui, "No servers in .mcp.json or ~/.claude.json.");
    }
    for server in &library.servers {
        server_row(ui, repo, server, actions);
    }
}

fn section(ui: &mut Ui, title: &str, add: Option<Action>, actions: &mut Vec<Action>) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title.to_uppercase()).size(11.0).strong().color(muted(ui)));
        if let Some(action) = add {
            right_aligned(ui, |ui| {
                ui.add_space(6.0);
                if glyph_button(ui, "+").on_hover_text("New prompt").clicked() {
                    actions.push(action);
                }
            });
        }
    });
    ui.add_space(2.0);
}

fn hint(ui: &mut Ui, text: &str) {
    ui.add(Label::new(RichText::new(text).size(12.0).color(muted(ui))).wrap());
}

fn skill_row(ui: &mut Ui, repo: &Repo, skill: &Skill, actions: &mut Vec<Action>) {
    let (response, ()) = row(ui, false, 0.0, |ui| {
        let mut enabled = skill.enabled;
        if ui.checkbox(&mut enabled, "").on_hover_text("Enabled for Claude Code").changed() {
            actions.push(Action::SetSkillEnabled { file: skill.file.clone(), enabled });
        }
        let color = if skill.enabled { ui.visuals().text_color() } else { muted(ui) };
        ui.add(Label::new(RichText::new(&skill.name).size(13.0).color(color)).truncate().selectable(false));
        if skill.project {
            tag(ui, "repo", muted(ui));
        }
    });
    let hover = if skill.description.is_empty() {
        skill.file.display().to_string()
    } else {
        skill.description.clone()
    };
    if response.on_hover_text(hover).clicked() {
        actions.push(Action::OpenFile { repo: repo.id, path: skill.file.clone() });
    }
}

fn server_row(ui: &mut Ui, repo: &Repo, server: &McpServer, actions: &mut Vec<Action>) {
    let (response, ()) = row(ui, false, 0.0, |ui| {
        ui.add(Label::new(RichText::new(&server.name).size(13.0)).truncate().selectable(false));
        ui.add(
            Label::new(RichText::new(&server.target).size(11.5).color(muted(ui)))
                .truncate()
                .selectable(false),
        );
        if server.project {
            tag(ui, "repo", muted(ui));
        }
    });
    let config: &Path = &server.config;
    if response.on_hover_text(format!("Open {}", config.display())).clicked() {
        actions.push(Action::OpenFile { repo: repo.id, path: config.to_path_buf() });
    }
}
