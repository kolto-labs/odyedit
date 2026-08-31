//! The Soundsets panel.
//!
//! A list of soundset files on the left, and the entries being repointed inside
//! the chosen one on the right.

use eframe::egui;

use super::widgets::{self, ListAction};
use super::App;
use crate::model::ssf::{self, SsfChange, SsfEntry};
use crate::model::Pane;

impl App {
    /// Draw the Soundsets panel.
    pub(super) fn show_ssf(&mut self, ui: &mut egui::Ui) {
        widgets::header(ui, Pane::Ssf.title(), Pane::Ssf.summary());

        let mut changed = false;

        ui.columns(2, |columns| {
            changed |= self.show_ssf_files(&mut columns[0]);
            changed |= self.show_ssf_changes(&mut columns[1]);
        });

        if changed {
            let entries = self.ssfs.clone();
            ssf::write(&mut self.file.ini, &entries);
            self.touch();
        }
    }

    /// The soundset files on the left.
    fn show_ssf_files(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Soundsets");
        ui.add_space(4.0);

        if let Some(action) = widgets::list_controls(
            ui,
            "Add a soundset",
            Some(self.selection.ssf),
            self.ssfs.len(),
            true,
        ) {
            match action {
                ListAction::Add => {
                    self.ssfs.push(SsfEntry::default());
                    self.selection.ssf = self.ssfs.len() - 1;
                    changed = true;
                }
                ListAction::Remove => {
                    let index = self.selection.ssf;
                    let mut entries = std::mem::take(&mut self.ssfs);
                    ssf::remove(&mut self.file.ini, &mut entries, index);
                    self.ssfs = entries;
                    self.selection.ssf = index.min(self.ssfs.len().saturating_sub(1));
                    self.touch();
                }
                other => {
                    self.selection.ssf =
                        widgets::reorder(&mut self.ssfs, self.selection.ssf, other);
                    changed = true;
                }
            }
        }
        ui.add_space(4.0);

        let rows: Vec<String> = self
            .ssfs
            .iter()
            .map(|entry| {
                let name = if entry.filename.trim().is_empty() {
                    "(unnamed)"
                } else {
                    entry.filename.as_str()
                };
                format!("{name}  ({} changes)", entry.changes.len())
            })
            .collect();

        let picked = widgets::pick_list(ui, "ssf-files", &rows, self.selection.ssf, 200.0);
        if picked.changed {
            self.selection.ssf = picked.index;
        }

        ui.add_space(6.0);

        let folder = self
            .file
            .path
            .as_deref()
            .map(kotor_formats::text::extract_file_path);

        if let Some(entry) = self.ssfs.get_mut(self.selection.ssf) {
            changed |= widgets::file_field(ui, "File", &mut entry.filename, || {
                super::pick_any_file("Choose a soundset", folder)
            });
            changed |= ui
                .checkbox(&mut entry.replace, "Overwrite the file already there")
                .changed();
        }

        changed
    }

    /// The entries being repointed inside the chosen soundset.
    fn show_ssf_changes(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Entries");
        ui.label(
            egui::RichText::new("A string number, or a StrRef / 2DAMEMORY token standing for one.")
                .weak(),
        );
        ui.add_space(4.0);

        let Some(entry) = self.ssfs.get_mut(self.selection.ssf) else {
            ui.label(egui::RichText::new("Pick a soundset first.").weak());
            return false;
        };

        let free = ssf::unused_labels(entry);
        ui.add_enabled_ui(!free.is_empty(), |ui| {
            ui.menu_button("Add an entry", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for label in &free {
                            if ui.button(*label).clicked() {
                                entry.changes.push(SsfChange {
                                    label: (*label).to_string(),
                                    value: "0".to_string(),
                                });
                                changed = true;
                                ui.close();
                            }
                        }
                    });
            });
        });
        ui.add_space(4.0);

        let mut pairs: Vec<(String, String)> = entry
            .changes
            .iter()
            .map(|change| (change.label.clone(), change.value.clone()))
            .collect();

        let result = widgets::pair_grid(ui, "ssf-changes", &mut pairs, "Entry", "String", false);

        if result.edited {
            for (change, pair) in entry.changes.iter_mut().zip(pairs) {
                change.value = pair.1;
            }
            changed = true;
        }
        if let Some(index) = result.removed {
            entry.changes.remove(index);
            changed = true;
        }

        changed
    }
}
