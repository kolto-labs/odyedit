//! The Game files panel.
//!
//! Files on the left; on the right, the two kinds of change a file can carry.
//! Setting a field already there is one line, so those are a plain grid. Adding
//! a field needs a type and a place in the tree, and can nest, so those are a
//! tree with a form under it.

use eframe::egui;

use super::widgets::{self, ListAction};
use super::App;
use crate::model::gff::{self, AddField, FieldChange, GffEntry, CONTAINER_TYPES, FIELD_TYPES};
use crate::model::Pane;

/// Which field-creating section the operator has open, as a path of positions
/// from the file down through the nesting.
type FieldPath = Vec<usize>;

impl App {
    /// Draw the Game files panel.
    pub(super) fn show_gff(&mut self, ui: &mut egui::Ui) {
        widgets::header(ui, Pane::Gff.title(), Pane::Gff.summary());

        let mut changed = false;

        ui.columns(2, |columns| {
            changed |= self.show_gff_files(&mut columns[0]);
            changed |= self.show_gff_changes(&mut columns[1]);
        });

        if changed {
            let entries = self.gff.clone();
            gff::write(&mut self.file.ini, &entries);
            self.touch();
        }
    }

    /// The game files on the left.
    fn show_gff_files(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.strong("Files");
        ui.add_space(4.0);

        if let Some(action) = widgets::list_controls(
            ui,
            "Add a file",
            Some(self.selection.gff),
            self.gff.len(),
            true,
        ) {
            match action {
                ListAction::Add => {
                    self.gff.push(GffEntry::default());
                    self.selection.gff = self.gff.len() - 1;
                    changed = true;
                }
                ListAction::Remove => {
                    let index = self.selection.gff;
                    let mut entries = std::mem::take(&mut self.gff);
                    gff::remove_entry(&mut self.file.ini, &mut entries, index);
                    self.gff = entries;
                    self.selection.gff = index.min(self.gff.len().saturating_sub(1));
                    self.touch();
                }
                other => {
                    self.selection.gff = widgets::reorder(&mut self.gff, self.selection.gff, other);
                    changed = true;
                }
            }
        }
        ui.add_space(4.0);

        let rows: Vec<String> = self
            .gff
            .iter()
            .map(|entry| {
                let name = if entry.filename.trim().is_empty() {
                    "(unnamed)"
                } else {
                    entry.filename.as_str()
                };
                format!(
                    "{name}  ({} changes, {} new)",
                    entry.changes.len(),
                    entry.additions.len()
                )
            })
            .collect();

        let picked = widgets::pick_list(ui, "gff-files", &rows, self.selection.gff, 180.0);
        if picked.changed {
            self.selection.gff = picked.index;
        }

        ui.add_space(6.0);

        let folder = self
            .file
            .path
            .as_deref()
            .map(kotor_formats::text::extract_file_path);

        if let Some(entry) = self.gff.get_mut(self.selection.gff) {
            changed |= widgets::file_field(ui, "File", &mut entry.filename, || {
                super::pick_any_file("Choose a game file", folder)
            });
            changed |= widgets::field(ui, "Destination", &mut entry.destination);
            changed |= ui
                .checkbox(&mut entry.replace, "Overwrite the file already there")
                .changed();
        }

        changed
    }

    /// The changes to the chosen file.
    fn show_gff_changes(&mut self, ui: &mut egui::Ui) -> bool {
        let index = self.selection.gff;
        if index >= self.gff.len() {
            ui.label(egui::RichText::new("Pick a file first.").weak());
            return false;
        }

        let mut changed = false;
        let ini = &self.file.ini;
        let entry = &mut self.gff[index];

        egui::ScrollArea::vertical()
            .id_salt("gff-changes")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                changed |= show_existing_fields(ui, entry);
                ui.add_space(10.0);
                changed |= show_new_fields(ui, entry, ini);
            });

        changed
    }
}

/// The grid of `path = value` changes to fields already in the file.
fn show_existing_fields(ui: &mut egui::Ui, entry: &mut GffEntry) -> bool {
    let mut changed = false;

    ui.strong("Fields to change");
    ui.label(
        egui::RichText::new(
            "The path names a field already in the file, such as PropertiesList\\0\\CostValue.",
        )
        .weak(),
    );
    ui.add_space(4.0);

    if ui.button("Add a field").clicked() {
        entry.changes.push(FieldChange::default());
        changed = true;
    }
    ui.add_space(4.0);

    let mut pairs: Vec<(String, String)> = entry
        .changes
        .iter()
        .map(|change| (change.path.clone(), change.value.clone()))
        .collect();

    let result = widgets::pair_grid(ui, "gff-existing", &mut pairs, "Path", "Value", true);

    if result.edited {
        entry.changes = pairs
            .iter()
            .map(|(path, value)| FieldChange {
                path: path.clone(),
                value: value.clone(),
            })
            .collect();
        changed = true;
    }
    if let Some(row) = result.removed {
        entry.changes.remove(row);
        changed = true;
    }

    changed
}

/// The tree of fields to create, and the form for whichever one is open.
fn show_new_fields(ui: &mut egui::Ui, entry: &mut GffEntry, ini: &crate::ini::IniFile) -> bool {
    let mut changed = false;

    ui.strong("Fields to add");
    ui.label(egui::RichText::new("Each one gets a section of its own.").weak());
    ui.add_space(4.0);

    if ui.button("Add a field").clicked() {
        let section = crate::model::unused_section_name(ini, "add_field");
        entry.additions.push(AddField {
            section,
            field_type: "Byte".to_string(),
            ..AddField::default()
        });
        changed = true;
    }
    ui.add_space(4.0);

    if entry.additions.is_empty() {
        ui.label(egui::RichText::new("Nothing here yet.").weak());
        return changed;
    }

    let mut remove: Option<FieldPath> = None;
    for (index, field) in entry.additions.iter_mut().enumerate() {
        changed |= show_field(ui, field, &mut vec![index], &mut remove, ini);
    }

    if let Some(path) = remove {
        remove_at(&mut entry.additions, &path);
        changed = true;
    }

    changed
}

/// One field-creating section, and anything nested inside it.
fn show_field(
    ui: &mut egui::Ui,
    field: &mut AddField,
    path: &mut FieldPath,
    remove: &mut Option<FieldPath>,
    ini: &crate::ini::IniFile,
) -> bool {
    let mut changed = false;

    // A reference that loops back up the tree is shown but not opened: the
    // section it names is edited where it really belongs, higher up.
    if field.loops_back {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("↩ points back at [{}]", field.section)).weak());
            if ui.small_button("Remove").clicked() {
                *remove = Some(path.clone());
            }
        });
        return false;
    }

    let title = if field.label.trim().is_empty() {
        format!("{}  [{}]", field.field_type, field.section)
    } else {
        format!("{}  {}", field.field_type, field.label)
    };

    egui::CollapsingHeader::new(title)
        .id_salt(&field.section)
        .default_open(false)
        .show(ui, |ui| {
            changed |= widgets::choice(
                ui,
                &format!("type-{}", field.section),
                "Type",
                &mut field.field_type,
                &all_types(),
            );
            changed |= widgets::field(ui, "Name", &mut field.label);
            changed |= widgets::field(ui, "Path", &mut field.path);
            ui.label(
                egui::RichText::new("Left empty, the field goes at the top of the file.").weak(),
            );

            if gff::is_container(&field.field_type) {
                if field.field_type.eq_ignore_ascii_case("Struct") {
                    changed |= widgets::field(ui, "Struct type number", &mut field.type_id);
                }
            } else if field.field_type.eq_ignore_ascii_case("ExoLocString") {
                changed |= show_localised(ui, field);
            } else {
                changed |= widgets::field(ui, "Value", &mut field.value);
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Add a field inside").clicked() {
                    let section = crate::model::unused_section_name(ini, "add_field");
                    field.children.push(AddField {
                        section,
                        field_type: "Byte".to_string(),
                        ..AddField::default()
                    });
                    changed = true;
                }
                if ui.button("Remove this field").clicked() {
                    *remove = Some(path.clone());
                }
            });

            if !field.children.is_empty() {
                ui.add_space(4.0);
                ui.separator();
                for (index, child) in field.children.iter_mut().enumerate() {
                    path.push(index);
                    changed |= show_field(ui, child, path, remove, ini);
                    path.pop();
                }
            }
        });

    changed
}

/// The string reference and translations of a localised string field.
fn show_localised(ui: &mut egui::Ui, field: &mut AddField) -> bool {
    let mut changed = false;

    changed |= widgets::field(ui, "String number", &mut field.strref);
    ui.label(
        egui::RichText::new("-1 means the text below is used instead of a dialog table line.")
            .weak(),
    );
    ui.add_space(4.0);

    if ui.button("Add a translation").clicked() {
        let next = field
            .languages
            .iter()
            .filter_map(|(number, _)| number.parse::<u32>().ok())
            .max()
            .map(|highest| highest + 1)
            .unwrap_or(0);
        field.languages.push((next.to_string(), String::new()));
        changed = true;
    }
    ui.add_space(4.0);

    let mut pairs = field.languages.clone();
    let result = widgets::pair_grid(
        ui,
        &format!("langs-{}", field.section),
        &mut pairs,
        "Language",
        "Text",
        true,
    );

    if result.edited {
        field.languages = pairs;
        changed = true;
    }
    if let Some(row) = result.removed {
        field.languages.remove(row);
        changed = true;
    }

    changed
}

/// Every field type, values and containers together.
fn all_types() -> Vec<&'static str> {
    FIELD_TYPES
        .iter()
        .copied()
        .chain(CONTAINER_TYPES.iter().copied())
        .collect()
}

/// Remove the field a path points at.
fn remove_at(fields: &mut Vec<AddField>, path: &[usize]) {
    match path {
        [] => {}
        [index] => {
            if *index < fields.len() {
                fields.remove(*index);
            }
        }
        [index, rest @ ..] => {
            if let Some(field) = fields.get_mut(*index) {
                remove_at(&mut field.children, rest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Vec<AddField> {
        vec![AddField {
            section: "outer".to_string(),
            children: vec![
                AddField {
                    section: "a".to_string(),
                    ..AddField::default()
                },
                AddField {
                    section: "b".to_string(),
                    ..AddField::default()
                },
            ],
            ..AddField::default()
        }]
    }

    #[test]
    fn removing_a_top_level_field_takes_its_children() {
        let mut fields = tree();
        remove_at(&mut fields, &[0]);
        assert!(fields.is_empty());
    }

    #[test]
    fn removing_a_nested_field_leaves_its_siblings() {
        let mut fields = tree();
        remove_at(&mut fields, &[0, 0]);
        assert_eq!(fields[0].children.len(), 1);
        assert_eq!(fields[0].children[0].section, "b");
    }

    #[test]
    fn a_path_past_the_end_does_nothing() {
        let mut fields = tree();
        remove_at(&mut fields, &[5]);
        remove_at(&mut fields, &[0, 9]);
        assert_eq!(fields[0].children.len(), 2);
    }

    #[test]
    fn every_field_type_is_offered() {
        let types = all_types();
        assert!(types.contains(&"Byte"));
        assert!(types.contains(&"ExoLocString"));
        assert!(types.contains(&"Struct"));
        assert!(types.contains(&"List"));
    }
}
