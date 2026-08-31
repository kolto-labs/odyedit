//! The Files to install panel.
//!
//! Two lists side by side: the destinations on the left, and the files going
//! into the chosen one on the right.

use eframe::egui;

use super::widgets::{self, ListAction};
use super::App;
use crate::model::install::{self, InstallFile};
use crate::model::Pane;

impl App {
    /// Draw the Files to install panel.
    pub(super) fn show_install(&mut self, ui: &mut egui::Ui) {
        widgets::header(ui, Pane::Install.title(), Pane::Install.summary());

        let mut changed = false;

        ui.columns(2, |columns| {
            changed |= self.show_destinations(&mut columns[0]);
            changed |= self.show_install_files(&mut columns[1]);
        });

        if changed {
            let folders = self.installs.clone();
            install::write(&mut self.file.ini, &folders);
            self.touch();
        }
    }

    /// The destinations on the left.
    fn show_destinations(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Destinations");
        ui.add_space(4.0);

        if let Some(action) = widgets::list_controls(
            ui,
            "Add a folder",
            Some(self.selection.install),
            self.installs.len(),
            true,
        ) {
            match action {
                ListAction::Add => {
                    let mut folders = std::mem::take(&mut self.installs);
                    install::add_folder(&mut self.file.ini, &mut folders, "override");
                    self.installs = folders;
                    self.selection.install = self.installs.len() - 1;
                    self.touch();
                }
                ListAction::Remove => {
                    let index = self.selection.install;
                    let mut folders = std::mem::take(&mut self.installs);
                    install::remove_folder(&mut self.file.ini, &mut folders, index);
                    self.installs = folders;
                    self.selection.install = index.min(self.installs.len().saturating_sub(1));
                    self.touch();
                }
                other => {
                    self.selection.install =
                        widgets::reorder(&mut self.installs, self.selection.install, other);
                    changed = true;
                }
            }
        }
        ui.add_space(4.0);

        let rows: Vec<String> = self
            .installs
            .iter()
            .map(|folder| {
                let name = if folder.destination.trim().is_empty() {
                    "(nowhere)"
                } else {
                    folder.destination.as_str()
                };
                format!("{name}  ({} files)", folder.files.len())
            })
            .collect();

        let picked = widgets::pick_list(ui, "install-folders", &rows, self.selection.install, 200.0);
        if picked.changed {
            self.selection.install = picked.index;
        }

        ui.add_space(6.0);

        if let Some(folder) = self.installs.get_mut(self.selection.install) {
            changed |= widgets::field(ui, "Folder", &mut folder.destination);
            ui.label(
                egui::RichText::new(
                    "Relative to the game folder. An archive name puts the files inside it.",
                )
                .weak(),
            );
        }

        changed
    }

    /// The files going into the chosen destination.
    fn show_install_files(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Files");
        ui.add_space(4.0);

        let folder_path = self
            .file
            .path
            .as_deref()
            .map(kotor_formats::text::extract_file_path);

        let Some(folder) = self.installs.get_mut(self.selection.install) else {
            ui.label(egui::RichText::new("Pick a destination first.").weak());
            return false;
        };

        if ui.button("Add files").clicked() {
            if let Some(paths) = pick_files("Choose files to install", folder_path.clone()) {
                for path in paths {
                    folder.files.push(InstallFile {
                        filename: kotor_formats::text::extract_file_name(&path),
                        replace: false,
                    });
                }
                changed = true;
            }
        }
        ui.add_space(4.0);

        let mut remove: Option<usize> = None;

        egui::ScrollArea::vertical()
            .id_salt("install-files")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if folder.files.is_empty() {
                    ui.label(egui::RichText::new("Nothing here yet.").weak());
                }
                for (index, file) in folder.files.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.text_edit_singleline(&mut file.filename).changed() {
                            changed = true;
                        }
                        if ui
                            .checkbox(&mut file.replace, "Overwrite")
                            .on_hover_text("Replace a file already in this folder.")
                            .changed()
                        {
                            changed = true;
                        }
                        if ui.small_button("Remove").clicked() {
                            remove = Some(index);
                        }
                    });
                }
            });

        if let Some(index) = remove {
            folder.files.remove(index);
            changed = true;
        }

        changed
    }
}

/// Ask for several files at once.
fn pick_files(title: &str, folder: Option<String>) -> Option<Vec<String>> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(folder) = folder {
        dialog = dialog.set_directory(folder);
    }
    dialog.pick_files().map(|paths| {
        paths
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    })
}
