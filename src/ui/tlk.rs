//! The Dialog strings panel.
//!
//! Two things are being edited here at once, and they live in different files.
//! The tokens on the left are lines in `changes.ini`. The strings they point at
//! are lines in `append.tlk`, a binary file the mod ships alongside. Editing one
//! without the other produces a mod that installs and shows blank text, so both
//! are shown together and saved together.

use eframe::egui;

use super::widgets::{self, ListAction};
use super::App;
use crate::model::tlk::{self, AppendFile, DEFAULT_APPEND_FILE};
use crate::model::Pane;

impl App {
    /// Draw the Dialog strings panel.
    pub(super) fn show_tlk(&mut self, ui: &mut egui::Ui) {
        widgets::header(ui, Pane::Tlk.title(), Pane::Tlk.summary());

        self.show_append_bar(ui);
        ui.add_space(6.0);

        if self.append.is_none() {
            ui.label(
                egui::RichText::new(
                    "Open or create an append file to see the text each token points at.",
                )
                .weak(),
            );
            ui.add_space(6.0);
        }

        let mut changed = false;

        if let Some(action) = widgets::list_controls(
            ui,
            "Add a string",
            self.selection.token,
            self.tokens.len(),
            false,
        ) {
            changed |= self.token_action(action);
        }
        ui.add_space(4.0);

        changed |= self.show_tokens(ui);

        if changed {
            let tokens = self.tokens.clone();
            tlk::write(&mut self.file.ini, &tokens);
            self.touch();
        }
    }

    /// The row naming the append file, with its own open and save controls.
    fn show_append_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Append file");

            let name = match &self.append {
                Some(file) => file
                    .path
                    .as_deref()
                    .map(kotor_formats::text::extract_file_name)
                    .unwrap_or_else(|| DEFAULT_APPEND_FILE.to_string()),
                None => "not found".to_string(),
            };
            let dirty = self.append.as_ref().is_some_and(AppendFile::is_dirty);
            ui.label(if dirty { format!("{name} *") } else { name });

            if ui.button("Open").clicked() {
                let folder = self
                    .file
                    .path
                    .as_deref()
                    .map(kotor_formats::text::extract_file_path);
                if let Some(path) = super::pick_any_file("Choose an append file", folder) {
                    match AppendFile::load(&path) {
                        Ok(file) => {
                            self.append = Some(file);
                            self.say("Opened the append file.");
                        }
                        Err(error) => self.complain(format!("Could not open {path}: {error}")),
                    }
                }
            }

            if ui.button("New").clicked() {
                self.append = Some(AppendFile::new());
                self.say("Started an empty append file.");
            }

            if ui
                .add_enabled(dirty, egui::Button::new("Save append file"))
                .clicked()
            {
                self.save_append();
            }
        });
    }

    /// Write the append file, asking for a name if it does not have one.
    fn save_append(&mut self) {
        let Some(append) = self.append.as_mut() else {
            return;
        };

        let path = match append.path.clone() {
            Some(path) => path,
            None => {
                let folder = self
                    .file
                    .path
                    .as_deref()
                    .map(kotor_formats::text::extract_file_path);
                let Some(chosen) = super::save_file_dialog(
                    "Save append file",
                    &[],
                    folder,
                    DEFAULT_APPEND_FILE,
                ) else {
                    return;
                };
                chosen
            }
        };

        match append.save_as(&path) {
            Ok(()) => self.say("Saved the append file."),
            Err(error) => self.complain(format!("Could not save the append file: {error}")),
        }
    }

    /// The token list, with the text of each one beside it.
    fn show_tokens(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let mut chosen = self.selection.token;

        egui::ScrollArea::vertical()
            .id_salt("tlk-tokens")
            .max_height(220.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.tokens.is_empty() {
                    ui.label(egui::RichText::new("Nothing here yet.").weak());
                }
                for (row, mapping) in self.tokens.iter().enumerate() {
                    let text = match &self.append {
                        Some(file) => file.text(mapping.index),
                        None => String::new(),
                    };
                    let preview = if text.is_empty() {
                        "(no text)".to_string()
                    } else {
                        first_line(&text, 60)
                    };
                    let label = format!(
                        "StrRef{}  →  line {}    {preview}",
                        mapping.token, mapping.index
                    );
                    if ui
                        .selectable_label(chosen == Some(row), label)
                        .clicked()
                    {
                        chosen = Some(row);
                    }
                }
            });

        self.selection.token = chosen;

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        changed |= self.show_token_editor(ui);
        changed
    }

    /// The form for the chosen token and the string it points at.
    fn show_token_editor(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(row) = self.selection.token.filter(|&row| row < self.tokens.len()) else {
            ui.label(egui::RichText::new("Pick a string to edit it.").weak());
            return false;
        };

        let mut changed = false;
        let mapping = self.tokens[row];

        ui.horizontal(|ui| {
            ui.label(format!("Token  StrRef{}", mapping.token));
            ui.label(
                egui::RichText::new("Use this name anywhere a string number is wanted.").weak(),
            );
        });

        let Some(append) = self.append.as_mut() else {
            ui.label(
                egui::RichText::new("The append file is not open, so its text cannot be edited.")
                    .weak(),
            );
            return false;
        };

        let mut text = append.text(mapping.index);
        let mut sound = append.sound(mapping.index);

        ui.label("Text");
        let text_changed = ui
            .add(
                egui::TextEdit::multiline(&mut text)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY),
            )
            .changed();

        let sound_changed = widgets::field(ui, "Sound", &mut sound);

        if text_changed || sound_changed {
            if let Err(error) = append.replace(mapping.index, &text, &sound) {
                self.status = format!("Could not change the string: {error}");
            }
            // The append file tracks its own unsaved changes; the instruction
            // file is untouched by an edit to the text.
            changed = false;
        }

        changed
    }

    /// Add or remove a token, keeping the append file in step.
    fn token_action(&mut self, action: ListAction) -> bool {
        match action {
            ListAction::Add => {
                let Some(append) = self.append.as_mut() else {
                    self.complain("Open or create an append file before adding a string.");
                    return false;
                };
                let index = match append.append("", "") {
                    Ok(index) => index,
                    Err(error) => {
                        self.complain(format!("Could not add the string: {error}"));
                        return false;
                    }
                };
                let token = tlk::add_mapping(&mut self.tokens, index);
                self.selection.token = Some(self.tokens.len() - 1);
                self.say(format!("Added StrRef{token}."));
                true
            }
            ListAction::Remove => {
                let Some(row) = self.selection.token.filter(|&row| row < self.tokens.len()) else {
                    return false;
                };
                // Only the mapping goes. The string stays in the append file:
                // the indices of everything after it would otherwise shift, and
                // every other token pointing past it would silently move.
                let gone = self.tokens.remove(row);
                self.selection.token = if self.tokens.is_empty() {
                    None
                } else {
                    Some(row.min(self.tokens.len() - 1))
                };
                self.say(format!(
                    "Removed StrRef{}. Line {} is still in the append file.",
                    gone.token, gone.index
                ));
                true
            }
            _ => false,
        }
    }
}

/// The first line of a string, shortened to fit a list row.
fn first_line(text: &str, width: usize) -> String {
    let line = text.lines().next().unwrap_or("");
    if line.chars().count() <= width {
        return line.to_string();
    }
    let cut: String = line.chars().take(width).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_stop_at_the_first_line() {
        assert_eq!(first_line("one\ntwo", 40), "one");
    }

    #[test]
    fn long_previews_are_cut_short() {
        assert_eq!(first_line("abcdefgh", 4), "abcd…");
    }

    #[test]
    fn short_previews_are_left_alone() {
        assert_eq!(first_line("abc", 40), "abc");
    }
}
