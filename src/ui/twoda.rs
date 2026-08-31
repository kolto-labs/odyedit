//! The Tables panel.
//!
//! Three levels: the tables on the left, the changes to the chosen table in the
//! middle, and the chosen change's own form below.
//!
//! When the table file itself can be found beside the instruction file, its
//! column names are read and offered as a dropdown. Typing a column name that
//! the table does not have is the easiest mistake to make here, and it fails
//! silently at install time.

use eframe::egui;

use super::widgets::{self, ListAction};
use super::App;
use crate::model::twoda::{self, Modifier, ModifierKind, TableEntry, EMPTY_CELL, HIGHEST};
use crate::model::Pane;

impl App {
    /// Draw the Tables panel.
    pub(super) fn show_twoda(&mut self, ui: &mut egui::Ui) {
        widgets::header(ui, Pane::TwoDa.title(), Pane::TwoDa.summary());

        let mut changed = false;

        ui.columns(2, |columns| {
            changed |= self.show_tables(&mut columns[0]);
            changed |= self.show_modifiers(&mut columns[1]);
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        changed |= self.show_modifier_editor(ui);

        if changed {
            let entries = self.tables.clone();
            twoda::write(&mut self.file.ini, &entries);
            self.touch();
        }
    }

    /// The tables on the left.
    fn show_tables(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Tables");
        ui.add_space(4.0);

        if let Some(action) = widgets::list_controls(
            ui,
            "Add a table",
            Some(self.selection.table),
            self.tables.len(),
            true,
        ) {
            match action {
                ListAction::Add => {
                    self.tables.push(TableEntry::default());
                    self.selection.table = self.tables.len() - 1;
                    self.selection.modifier = None;
                    changed = true;
                }
                ListAction::Remove => {
                    let index = self.selection.table;
                    let mut entries = std::mem::take(&mut self.tables);
                    twoda::remove_table(&mut self.file.ini, &mut entries, index);
                    self.tables = entries;
                    self.selection.table = index.min(self.tables.len().saturating_sub(1));
                    self.selection.modifier = None;
                    self.touch();
                }
                other => {
                    self.selection.table =
                        widgets::reorder(&mut self.tables, self.selection.table, other);
                    changed = true;
                }
            }
        }
        ui.add_space(4.0);

        let rows: Vec<String> = self
            .tables
            .iter()
            .map(|entry| {
                let name = if entry.filename.trim().is_empty() {
                    "(unnamed)"
                } else {
                    entry.filename.as_str()
                };
                format!("{name}  ({} changes)", entry.modifiers.len())
            })
            .collect();

        let picked = widgets::pick_list(ui, "tables", &rows, self.selection.table, 150.0);
        if picked.changed {
            self.selection.table = picked.index;
            self.selection.modifier = None;
            self.rename_buffer.clear();
        }

        ui.add_space(6.0);

        let folder = self
            .file
            .path
            .as_deref()
            .map(kotor_formats::text::extract_file_path);

        if let Some(entry) = self.tables.get_mut(self.selection.table) {
            changed |= widgets::file_field(ui, "File", &mut entry.filename, || {
                super::pick_any_file("Choose a table", folder)
            });
            changed |= widgets::field(ui, "Destination", &mut entry.destination);
            changed |= ui
                .checkbox(&mut entry.replace, "Overwrite the file already there")
                .changed();
        }

        changed
    }

    /// The changes to the chosen table.
    fn show_modifiers(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Changes");
        ui.add_space(4.0);

        let Some(entry) = self.tables.get(self.selection.table) else {
            ui.label(egui::RichText::new("Pick a table first.").weak());
            return false;
        };
        let modifier_count = entry.modifiers.len();

        let mut add_kind: Option<ModifierKind> = None;
        ui.horizontal(|ui| {
            ui.menu_button("Add a change", |ui| {
                for kind in ModifierKind::ALL {
                    if ui.button(kind.title()).clicked() {
                        add_kind = Some(kind);
                        ui.close();
                    }
                }
            });

            let chosen = self.selection.modifier.filter(|&i| i < modifier_count);
            if ui
                .add_enabled(chosen.is_some(), egui::Button::new("Remove"))
                .clicked()
            {
                if let Some(index) = chosen {
                    let mut table = self.tables[self.selection.table].clone();
                    twoda::remove_modifier(&mut self.file.ini, &mut table, index);
                    self.tables[self.selection.table] = table;
                    self.selection.modifier = None;
                    self.touch();
                }
            }
        });
        ui.add_space(4.0);

        if let Some(kind) = add_kind {
            let base = default_section_name(kind);
            let section = crate::model::unused_section_name(&self.file.ini, &base);
            let mut modifier = Modifier::new(&section, kind);
            // A row change with no row named does nothing, so the key is put
            // there from the start for the operator to fill in.
            if kind.needs_a_row() {
                modifier.set_directive("RowIndex", "0");
            }
            if kind == ModifierKind::AddColumn {
                modifier.set_directive("DefaultValue", EMPTY_CELL);
            }
            self.tables[self.selection.table].modifiers.push(modifier);
            self.selection.modifier = Some(self.tables[self.selection.table].modifiers.len() - 1);
            changed = true;
        }

        let rows: Vec<String> = self.tables[self.selection.table]
            .modifiers
            .iter()
            .map(|modifier| format!("{}  —  {}", modifier.kind.title(), modifier.section))
            .collect();

        let selected = self.selection.modifier.unwrap_or(usize::MAX);
        egui::ScrollArea::vertical()
            .id_salt("modifiers")
            .max_height(180.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if rows.is_empty() {
                    ui.label(egui::RichText::new("Nothing here yet.").weak());
                }
                for (row, label) in rows.iter().enumerate() {
                    if ui.selectable_label(row == selected, label).clicked() {
                        self.selection.modifier = Some(row);
                        // A half-typed name does not follow the operator to the
                        // next change they look at.
                        self.rename_buffer.clear();
                    }
                }
            });

        changed
    }

    /// The form for the chosen change.
    fn show_modifier_editor(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(table_index) = Some(self.selection.table).filter(|&i| i < self.tables.len())
        else {
            return false;
        };
        let Some(index) = self
            .selection
            .modifier
            .filter(|&i| i < self.tables[table_index].modifiers.len())
        else {
            ui.label(egui::RichText::new("Pick a change to edit it.").weak());
            return false;
        };

        // The column names come from the table file when it can be found, and
        // the rename box needs the document, so both are handled before the
        // modifier itself is borrowed for editing.
        let columns = self.columns_for(table_index);
        let kind = self.tables[table_index].modifiers[index].kind;
        let old = self.tables[table_index].modifiers[index].section.clone();
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.strong(kind.title());
            ui.label(egui::RichText::new(format!("written under [{old}]")).weak());
        });
        ui.add_space(4.0);

        let mut section = if self.rename_buffer.is_empty() {
            old.clone()
        } else {
            self.rename_buffer.clone()
        };

        if widgets::field(ui, "Name", &mut section) {
            self.rename_buffer = section;
        }

        // A rename is only applied once the operator is done typing. Renaming on
        // every keystroke would move the section through every half-typed name
        // along the way, and each of those could land on one already in use.
        let typed = self.rename_buffer.trim().to_string();
        let renaming = !typed.is_empty() && typed != old;
        let clashes = renaming && self.file.ini.section_present(&typed);

        if clashes {
            ui.label(
                egui::RichText::new(format!("[{typed}] is already used. Pick another name."))
                    .weak(),
            );
        }

        if ui
            .add_enabled(renaming && !clashes, egui::Button::new("Rename"))
            .clicked()
        {
            // Renaming in place keeps the section where it sits in the file. The
            // writer would otherwise delete it and append the new one at the
            // bottom, turning a one-field edit into a whole-file diff.
            self.file.ini.rename_section(&old, &typed);
            self.tables[table_index].modifiers[index].section = typed;
            self.rename_buffer.clear();
            changed = true;
        }

        let modifier = &mut self.tables[table_index].modifiers[index];

        changed |= show_directives(ui, modifier, &columns);

        ui.add_space(6.0);
        ui.strong(if kind == ModifierKind::AddColumn {
            "Rows to fill"
        } else {
            "Cells to set"
        });
        ui.label(
            egui::RichText::new(format!(
                "{EMPTY_CELL} leaves a cell empty. {HIGHEST} takes one more than the highest number in the column."
            ))
            .weak(),
        );
        ui.add_space(4.0);

        changed |= show_cells(ui, modifier, &columns, kind);
        ui.add_space(6.0);
        changed |= show_captures(ui, modifier);

        changed
    }

    /// Column names read from the table file, when it can be found.
    fn columns_for(&self, index: usize) -> Vec<String> {
        let Some(entry) = self.tables.get(index) else {
            return Vec::new();
        };
        let Some(folder) = self.file.folder() else {
            return Vec::new();
        };
        if entry.filename.trim().is_empty() {
            return Vec::new();
        }

        let path = kotor_formats::text::join_path(&folder, &entry.filename);
        let Ok(table) = kotor_formats::twoda::TwoDaFile::load(&path) else {
            return Vec::new();
        };

        (0..table.column_count())
            .filter_map(|column| table.column_label(column).ok())
            .map(str::to_string)
            .collect()
    }
}

/// The keys that direct this kind of change.
fn show_directives(ui: &mut egui::Ui, modifier: &mut Modifier, columns: &[String]) -> bool {
    let mut changed = false;
    let kind = modifier.kind;

    if kind.needs_a_row() {
        let (current_key, current_value) = modifier
            .row_target()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .unwrap_or_else(|| ("RowIndex".to_string(), String::new()));

        let mut key = current_key.clone();
        let mut value = current_value.clone();

        ui.horizontal(|ui| {
            ui.label("Find the row by");
            for option in ["RowIndex", "RowLabel", "LabelIndex"] {
                if ui
                    .selectable_label(key == option, friendly(option))
                    .clicked()
                {
                    key = option.to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(friendly(&key));
            ui.text_edit_singleline(&mut value);
        });

        if key != current_key || value != current_value {
            for option in ["RowIndex", "RowLabel", "LabelIndex"] {
                modifier.set_directive(option, "");
            }
            modifier.set_directive(&key, &value);
            changed = true;
        }
    }

    if matches!(kind, ModifierKind::AddRow | ModifierKind::CopyRow) {
        let mut exclusive = modifier
            .directive("ExclusiveColumn")
            .unwrap_or("")
            .to_string();
        let before = exclusive.clone();
        if columns.is_empty() {
            widgets::field(ui, "Skip if this column already matches", &mut exclusive);
        } else {
            let names: Vec<&str> = columns.iter().map(String::as_str).collect();
            widgets::choice(
                ui,
                "exclusive-column",
                "Skip if this column already matches",
                &mut exclusive,
                &names,
            );
        }
        if exclusive != before {
            modifier.set_directive("ExclusiveColumn", &exclusive);
            changed = true;
        }
    }

    if kind == ModifierKind::AddRow {
        let mut label = modifier.directive("RowLabel").unwrap_or("").to_string();
        if widgets::field(ui, "New row label", &mut label) {
            modifier.set_directive("RowLabel", &label);
            changed = true;
        }
    }

    if kind == ModifierKind::CopyRow {
        let mut label = modifier.directive("NewRowLabel").unwrap_or("").to_string();
        if widgets::field(ui, "New row label", &mut label) {
            modifier.set_directive("NewRowLabel", &label);
            changed = true;
        }
    }

    if kind == ModifierKind::AddColumn {
        let mut label = modifier.directive("ColumnLabel").unwrap_or("").to_string();
        if widgets::field(ui, "Column name", &mut label) {
            modifier.set_directive("ColumnLabel", &label);
            changed = true;
        }
        let mut default = modifier.directive("DefaultValue").unwrap_or("").to_string();
        if widgets::field(ui, "Value for every other row", &mut default) {
            modifier.set_directive("DefaultValue", &default);
            changed = true;
        }
    }

    changed
}

/// The cells this change sets.
fn show_cells(
    ui: &mut egui::Ui,
    modifier: &mut Modifier,
    columns: &[String],
    kind: ModifierKind,
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        if kind == ModifierKind::AddColumn {
            // A new column's individual rows are named `i<row>` by number or
            // `l<label>` by row label, not by column.
            if ui.button("Set a row by number").clicked() {
                let next = modifier.cells().len();
                modifier.set_cell(&format!("i{next}"), EMPTY_CELL);
                changed = true;
            }
            if ui.button("Set a row by label").clicked() {
                modifier.set_cell("lLABEL", EMPTY_CELL);
                changed = true;
            }
        } else if columns.is_empty() {
            if ui.button("Add a cell").clicked() {
                modifier.set_cell("", "");
                changed = true;
            }
        } else {
            ui.menu_button("Add a cell", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for column in columns {
                            let taken = modifier
                                .cells()
                                .iter()
                                .any(|(key, _)| key.eq_ignore_ascii_case(column));
                            if ui.add_enabled(!taken, egui::Button::new(column)).clicked() {
                                modifier.set_cell(column, "");
                                changed = true;
                                ui.close();
                            }
                        }
                    });
            });
        }
    });
    ui.add_space(4.0);

    let mut pairs: Vec<(String, String)> = modifier
        .cells()
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();

    let header = if kind == ModifierKind::AddColumn {
        "Row"
    } else {
        "Column"
    };
    let result = widgets::pair_grid(ui, "cells", &mut pairs, header, "Value", true);

    if result.edited {
        replace_cells(modifier, &pairs);
        changed = true;
    }
    if let Some(row) = result.removed {
        let mut kept = pairs;
        kept.remove(row);
        replace_cells(modifier, &kept);
        changed = true;
    }

    changed
}

/// Swap this modifier's cells for a new set, leaving everything else in place.
fn replace_cells(modifier: &mut Modifier, pairs: &[(String, String)]) {
    let directives: Vec<(String, String)> = modifier
        .kind
        .directing_keys()
        .iter()
        .filter_map(|key| {
            modifier
                .directive(key)
                .map(|value| ((*key).to_string(), value.to_string()))
        })
        .collect();
    let captures: Vec<(String, String)> = modifier
        .captures()
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();

    modifier.entries = directives;
    modifier.entries.extend(
        pairs
            .iter()
            .filter(|(key, _)| !key.trim().is_empty())
            .cloned(),
    );
    modifier.entries.extend(captures);
}

/// The values this change hands on to later changes.
fn show_captures(ui: &mut egui::Ui, modifier: &mut Modifier) -> bool {
    let mut changed = false;

    ui.strong("Values to remember");
    ui.label(
        egui::RichText::new(
            "A later change can use these by name, such as 2DAMEMORY0 in another cell.",
        )
        .weak(),
    );
    ui.add_space(4.0);

    let sources = match modifier.kind {
        ModifierKind::AddColumn => vec!["I", "L"],
        _ => vec!["RowIndex", "RowLabel"],
    };

    ui.horizontal(|ui| {
        if ui.button("Remember a value").clicked() {
            let number = modifier.unused_capture();
            let source = sources.first().copied().unwrap_or("RowIndex").to_string();
            modifier
                .entries
                .push((format!("2DAMEMORY{number}"), source));
            changed = true;
        }
    });
    ui.add_space(4.0);

    let mut pairs: Vec<(String, String)> = modifier
        .captures()
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();

    let result = widgets::pair_grid(ui, "captures", &mut pairs, "Name", "Remembers", true);

    if result.edited {
        replace_captures(modifier, &pairs);
        changed = true;
    }
    if let Some(row) = result.removed {
        let mut kept = pairs;
        kept.remove(row);
        replace_captures(modifier, &kept);
        changed = true;
    }

    changed
}

/// Swap this modifier's captures for a new set, leaving everything else alone.
fn replace_captures(modifier: &mut Modifier, pairs: &[(String, String)]) {
    modifier.entries.retain(|(key, _)| !twoda::is_capture(key));
    modifier.entries.extend(
        pairs
            .iter()
            .filter(|(key, _)| !key.trim().is_empty())
            .cloned(),
    );
}

/// A directing key's name in words rather than in the file's spelling.
fn friendly(key: &str) -> &str {
    match key {
        "RowIndex" => "Row number",
        "RowLabel" => "Row label",
        "LabelIndex" => "Label column",
        other => other,
    }
}

/// The name a new change gets before the operator renames it.
fn default_section_name(kind: ModifierKind) -> String {
    match kind {
        ModifierKind::AddRow => "add_row".to_string(),
        ModifierKind::ChangeRow => "change_row".to_string(),
        ModifierKind::CopyRow => "copy_row".to_string(),
        ModifierKind::AddColumn => "add_column".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_cells_keeps_directives_and_captures_in_order() {
        let mut modifier = Modifier::new("m", ModifierKind::ChangeRow);
        modifier.set_directive("RowIndex", "3");
        modifier.set_cell("old", "1");
        modifier
            .entries
            .push(("2DAMEMORY0".to_string(), "RowIndex".to_string()));

        replace_cells(&mut modifier, &[("new".to_string(), "2".to_string())]);

        assert_eq!(modifier.row_target(), Some(("RowIndex", "3")));
        assert_eq!(modifier.cells(), vec![("new", "2")]);
        assert_eq!(modifier.captures(), vec![("2DAMEMORY0", "RowIndex")]);
    }

    #[test]
    fn replacing_cells_drops_unnamed_rows() {
        let mut modifier = Modifier::new("m", ModifierKind::AddRow);
        replace_cells(
            &mut modifier,
            &[
                ("".to_string(), "orphan".to_string()),
                ("label".to_string(), "X".to_string()),
            ],
        );
        assert_eq!(modifier.cells(), vec![("label", "X")]);
    }

    #[test]
    fn replacing_captures_leaves_the_cells_alone() {
        let mut modifier = Modifier::new("m", ModifierKind::AddRow);
        modifier.set_cell("label", "X");
        modifier
            .entries
            .push(("2DAMEMORY0".to_string(), "RowIndex".to_string()));

        replace_captures(
            &mut modifier,
            &[("2DAMEMORY5".to_string(), "RowLabel".to_string())],
        );

        assert_eq!(modifier.cells(), vec![("label", "X")]);
        assert_eq!(modifier.captures(), vec![("2DAMEMORY5", "RowLabel")]);
    }

    #[test]
    fn directing_keys_read_as_words() {
        assert_eq!(friendly("RowIndex"), "Row number");
        assert_eq!(friendly("Whatever"), "Whatever");
    }
}
