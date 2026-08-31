//! The Scripts panel.
//!
//! A list of source scripts to compile, and where each one's result goes.

use eframe::egui;

use super::widgets::{self, ListAction};
use super::App;
use crate::model::script::{self, ScriptEntry};
use crate::model::Pane;

impl App {
    /// Draw the Scripts panel.
    pub(super) fn show_script(&mut self, ui: &mut egui::Ui) {
        widgets::header(ui, Pane::Script.title(), Pane::Script.summary());

        let mut changed = false;
        let mut default_destination = script::default_destination(&self.file.ini);
        if widgets::field(ui, "Default destination", &mut default_destination) {
            script::set_default_destination(&mut self.file.ini, &default_destination);
            self.touch();
        }
        ui.add_space(6.0);

        if let Some(action) = widgets::list_controls(
            ui,
            "Add a script",
            Some(self.selection.script),
            self.scripts.len(),
            true,
        ) {
            changed |= self.script_action(action);
        }
        ui.add_space(4.0);

        let rows: Vec<String> = self
            .scripts
            .iter()
            .map(|entry| {
                let mark = if entry.replace { "  (overwrites)" } else { "" };
                format!("{}{mark}", entry.filename)
            })
            .collect();

        let picked = widgets::pick_list(ui, "scripts", &rows, self.selection.script, 160.0);
        self.selection.script = picked.index;

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        if let Some(entry) = self.scripts.get_mut(self.selection.script) {
            let folder = self.file.path.as_deref().map(kotor_formats::text::extract_file_path);
            changed |= widgets::file_field(ui, "Source file", &mut entry.filename, || {
                super::pick_any_file("Choose a script", folder)
            });
            changed |= widgets::field(ui, "Destination", &mut entry.destination);
            changed |= ui
                .checkbox(&mut entry.replace, "Overwrite a file already there")
                .changed();
        } else {
            ui.label(egui::RichText::new("Pick a script to edit it.").weak());
        }

        if changed {
            let entries = self.scripts.clone();
            script::write(&mut self.file.ini, &entries);
            self.touch();
        }
    }

    /// Add, remove or reorder a script.
    fn script_action(&mut self, action: ListAction) -> bool {
        match action {
            ListAction::Add => {
                self.scripts.push(ScriptEntry::default());
                self.selection.script = self.scripts.len() - 1;
                true
            }
            ListAction::Remove => {
                let index = self.selection.script;
                let mut entries = std::mem::take(&mut self.scripts);
                script::remove(&mut self.file.ini, &mut entries, index);
                self.scripts = entries;
                self.selection.script = index.min(self.scripts.len().saturating_sub(1));
                self.touch();
                // The remove already wrote the document.
                false
            }
            ListAction::MoveUp | ListAction::MoveDown => {
                self.selection.script =
                    widgets::reorder(&mut self.scripts, self.selection.script, action);
                true
            }
        }
    }
}
