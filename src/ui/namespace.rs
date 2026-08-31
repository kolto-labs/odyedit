//! The Install variants panel.
//!
//! These live in `namespaces.ini`, not in the instruction file, so this panel
//! has its own open and save controls.

use eframe::egui;

use super::widgets::{self, ListAction};
use super::App;
use crate::model::namespace::{self, Namespace, NamespaceFile};
use crate::model::Pane;

impl App {
    /// Draw the Install variants panel.
    pub(super) fn show_namespace(&mut self, ui: &mut egui::Ui) {
        widgets::header(ui, Pane::Namespace.title(), Pane::Namespace.summary());

        self.show_namespace_bar(ui);
        ui.add_space(6.0);

        if let Some(action) = widgets::list_controls(
            ui,
            "Add a variant",
            Some(self.selection.namespace),
            self.namespaces.entries.len(),
            true,
        ) {
            self.namespace_action(action);
        }
        ui.add_space(4.0);

        let rows: Vec<String> = self
            .namespaces
            .entries
            .iter()
            .map(|entry| {
                let name = if entry.name.trim().is_empty() {
                    entry.section.as_str()
                } else {
                    entry.name.as_str()
                };
                name.to_string()
            })
            .collect();

        let picked = widgets::pick_list(ui, "namespaces", &rows, self.selection.namespace, 150.0);
        self.selection.namespace = picked.index;

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        self.show_namespace_editor(ui);
    }

    /// The row naming the variants file, with its own open and save controls.
    fn show_namespace_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Variants file");
            let name = self
                .namespaces
                .path
                .as_deref()
                .map(kotor_formats::text::extract_file_name)
                .unwrap_or_else(|| "not found".to_string());
            ui.label(name);

            if ui.button("Open").clicked() {
                let folder = self
                    .file
                    .path
                    .as_deref()
                    .map(kotor_formats::text::extract_file_path);
                if let Some(path) = super::pick_file("Choose a variants file", &["ini"], folder) {
                    match NamespaceFile::load(&path) {
                        Ok(file) => {
                            self.namespaces = file;
                            self.selection.namespace = 0;
                            self.say("Opened the variants file.");
                        }
                        Err(error) => self.complain(format!("Could not open {path}: {error}")),
                    }
                }
            }

            if ui.button("Save variants").clicked() {
                self.save_namespaces();
            }
        });
    }

    /// Write the variants file, asking for a name if it does not have one.
    fn save_namespaces(&mut self) {
        let path = match self.namespaces.path.clone() {
            Some(path) => path,
            None => {
                let folder = self
                    .file
                    .path
                    .as_deref()
                    .map(kotor_formats::text::extract_file_path);
                let Some(chosen) = super::save_file_dialog(
                    "Save variants file",
                    &["ini"],
                    folder,
                    namespace::FILENAME,
                ) else {
                    return;
                };
                chosen
            }
        };

        match self.namespaces.save_as(&path) {
            Ok(()) => self.say("Saved the variants file."),
            Err(error) => self.complain(format!("Could not save the variants file: {error}")),
        }
    }

    /// The form for the chosen variant.
    fn show_namespace_editor(&mut self, ui: &mut egui::Ui) {
        let Some(entry) = self.namespaces.entries.get_mut(self.selection.namespace) else {
            ui.label(egui::RichText::new("Pick a variant to edit it.").weak());
            return;
        };

        widgets::field(ui, "Name", &mut entry.name);
        ui.label("Description");
        ui.add(
            egui::TextEdit::multiline(&mut entry.description)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        widgets::field(ui, "Instruction file", &mut entry.ini_name);
        widgets::field(ui, "Information file", &mut entry.info_name);
        widgets::field(ui, "Data folder", &mut entry.data_path);
        ui.label(
            egui::RichText::new("Left empty, the variant uses the mod's own tslpatchdata folder.")
                .weak(),
        );

        ui.add_space(6.0);
        ui.label(egui::RichText::new(format!("Written under [{}]", entry.section)).weak());
    }

    /// Add, remove or reorder a variant.
    fn namespace_action(&mut self, action: ListAction) {
        match action {
            ListAction::Add => {
                let section =
                    namespace::unused_section(&self.namespaces.ini, &self.namespaces.entries);
                self.namespaces.entries.push(Namespace {
                    section,
                    ini_name: namespace::DEFAULT_INI.to_string(),
                    info_name: namespace::DEFAULT_INFO.to_string(),
                    ..Namespace::default()
                });
                self.selection.namespace = self.namespaces.entries.len() - 1;
            }
            ListAction::Remove => {
                let index = self.selection.namespace;
                if index < self.namespaces.entries.len() {
                    self.namespaces.entries.remove(index);
                    self.selection.namespace =
                        index.min(self.namespaces.entries.len().saturating_sub(1));
                }
            }
            other => {
                self.selection.namespace = widgets::reorder(
                    &mut self.namespaces.entries,
                    self.selection.namespace,
                    other,
                );
            }
        }
    }
}
