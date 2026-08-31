//! The Settings panel.
//!
//! Plain controls, no list. Anything the operator changes is written straight
//! back to the document, and a value returned to its default drops out of the
//! file rather than being written as a redundant line.

use eframe::egui;

use super::widgets;
use super::App;
use crate::model::settings::{Game, RequiredCheck};
use crate::model::Pane;

impl App {
    /// Draw the Settings panel.
    pub(super) fn show_settings(&mut self, ui: &mut egui::Ui) {
        widgets::header(ui, Pane::Settings.title(), Pane::Settings.summary());

        let mut changed = false;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                changed |= self.show_appearance(ui);
                ui.add_space(10.0);
                changed |= self.show_behaviour(ui);
                ui.add_space(10.0);
                changed |= self.show_required(ui);
            });

        if changed {
            let settings = self.settings.clone();
            settings.write(&mut self.file.ini);
            self.touch();
        }
    }

    /// What the installer looks like when it runs.
    fn show_appearance(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Appearance");
        ui.add_space(4.0);

        changed |= widgets::field(ui, "Window title", &mut self.settings.window_caption);

        ui.label("Prompt shown before starting");
        changed |= ui
            .add(
                egui::TextEdit::multiline(&mut self.settings.confirm_message)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            )
            .changed();

        let mut skip = self.settings.starts_without_asking();
        if ui
            .checkbox(&mut skip, "Start without asking")
            .on_hover_text("Writes N/A as the prompt, which the installer reads as \"do not ask\".")
            .changed()
        {
            self.settings.confirm_message = if skip {
                "N/A".to_string()
            } else {
                String::new()
            };
            changed = true;
        }

        changed
    }

    /// What the installer does while it runs.
    fn show_behaviour(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Behaviour");
        ui.add_space(4.0);

        changed |= ui
            .checkbox(&mut self.settings.installer_mode, "Install a mod")
            .on_hover_text("Turn this off to patch files the operator picks by hand.")
            .changed();
        changed |= ui
            .checkbox(&mut self.settings.backup_files, "Back up files before changing them")
            .changed();
        changed |= ui
            .checkbox(&mut self.settings.plaintext_log, "Write the log as plain text")
            .changed();
        changed |= ui
            .checkbox(
                &mut self.settings.save_processed_scripts,
                "Keep processed scripts",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut self.settings.ignore_extensions,
                "Compare filenames without extensions",
            )
            .changed();

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Log detail");
            changed |= ui
                .add(egui::Slider::new(&mut self.settings.log_level, 0..=4))
                .changed();
        });

        ui.add_space(6.0);
        changed |= ui
            .checkbox(
                &mut self.settings.lookup_game_folder,
                "Find the game folder automatically",
            )
            .on_hover_text("Reads the install path from the Windows registry.")
            .changed();

        if self.settings.lookup_game_folder {
            ui.horizontal(|ui| {
                ui.label("Game");
                for game in [Game::Kotor1, Game::Kotor2] {
                    if ui
                        .selectable_label(self.settings.lookup_game == game, game.title())
                        .clicked()
                    {
                        self.settings.lookup_game = game;
                        changed = true;
                    }
                }
            });
        }

        ui.add_space(6.0);
        changed |= widgets::field(
            ui,
            "Script compiler flags",
            &mut self.settings.script_compiler_flags,
        );

        changed
    }

    /// Files that have to be in place before the install will start.
    fn show_required(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Files that must already be installed");
        ui.label(
            egui::RichText::new(
                "The install stops if any of these is missing from the override folder.",
            )
            .weak(),
        );
        ui.add_space(4.0);

        let mut remove: Option<usize> = None;

        for (index, check) in self.settings.required.iter_mut().enumerate() {
            ui.group(|ui| {
                let mut files = check.files.join(", ");
                ui.horizontal(|ui| {
                    ui.label("Files");
                    if ui.text_edit_singleline(&mut files).changed() {
                        check.files = files
                            .split(',')
                            .map(str::trim)
                            .filter(|name| !name.is_empty())
                            .map(str::to_string)
                            .collect();
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Message");
                    changed |= ui.text_edit_singleline(&mut check.message).changed();
                });
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
            });
        }

        if let Some(index) = remove {
            self.settings.required.remove(index);
            changed = true;
        }

        if ui.button("Add a check").clicked() {
            self.settings.required.push(RequiredCheck::default());
            changed = true;
        }

        changed
    }
}
