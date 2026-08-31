//! Pieces the panels share.
//!
//! Every panel is a list with a form beside it, so the list controls and the
//! grid of key/value pairs are written once here rather than eight times.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

/// A heading and the line under it that says what the section is for.
pub fn header(ui: &mut egui::Ui, title: &str, summary: &str) {
    ui.add_space(4.0);
    ui.heading(title);
    ui.label(egui::RichText::new(summary).weak());
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
}

/// What the operator asked a list to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListAction {
    /// Add an entry.
    Add,
    /// Remove the chosen entry.
    Remove,
    /// Move the chosen entry one place earlier.
    MoveUp,
    /// Move the chosen entry one place later.
    MoveDown,
}

/// Buttons for adding to, removing from and reordering a list.
///
/// `add_label` names what is being added, because "Add" on its own tells the
/// operator nothing about which of the two lists on screen it belongs to.
pub fn list_controls(
    ui: &mut egui::Ui,
    add_label: &str,
    selected: Option<usize>,
    len: usize,
    reorderable: bool,
) -> Option<ListAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        if ui.button(add_label).clicked() {
            action = Some(ListAction::Add);
        }

        let chosen = selected.filter(|&index| index < len);
        if ui
            .add_enabled(chosen.is_some(), egui::Button::new("Remove"))
            .clicked()
        {
            action = Some(ListAction::Remove);
        }

        if reorderable {
            let can_rise = chosen.is_some_and(|index| index > 0);
            let can_fall = chosen.is_some_and(|index| index + 1 < len);
            if ui
                .add_enabled(can_rise, egui::Button::new("Move up"))
                .clicked()
            {
                action = Some(ListAction::MoveUp);
            }
            if ui
                .add_enabled(can_fall, egui::Button::new("Move down"))
                .clicked()
            {
                action = Some(ListAction::MoveDown);
            }
        }
    });

    action
}

/// Carry out a reorder, and report where the chosen row ended up.
pub fn reorder<T>(items: &mut [T], index: usize, action: ListAction) -> usize {
    match action {
        ListAction::MoveUp if index > 0 => {
            items.swap(index, index - 1);
            index - 1
        }
        ListAction::MoveDown if index + 1 < items.len() => {
            items.swap(index, index + 1);
            index + 1
        }
        _ => index,
    }
}

/// A row the operator picked out of a list, and whether the pick just changed.
pub struct Picked {
    /// Which row is chosen now.
    pub index: usize,
    /// True when this frame is the one that changed it.
    pub changed: bool,
}

/// A single-column list of labels, one of which is selected.
pub fn pick_list(
    ui: &mut egui::Ui,
    id: &str,
    rows: &[String],
    selected: usize,
    height: f32,
) -> Picked {
    let mut index = selected;
    let mut changed = false;

    egui::ScrollArea::vertical()
        .id_salt(id)
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (row, label) in rows.iter().enumerate() {
                if ui.selectable_label(row == selected, label).clicked() {
                    index = row;
                    changed = true;
                }
            }
            if rows.is_empty() {
                ui.label(egui::RichText::new("Nothing here yet.").weak());
            }
        });

    Picked { index, changed }
}

/// What an editable grid of pairs reported back.
#[derive(Debug, Default)]
pub struct GridResult {
    /// True when a cell was edited.
    pub edited: bool,
    /// The row the operator asked to remove, if any.
    pub removed: Option<usize>,
}

/// A grid of editable name/value pairs with a Remove button on each row.
///
/// `name_header` and `value_header` name the columns, which differ by panel: a
/// table modifier's pairs are columns and cell values, a soundset's are entry
/// names and string references.
pub fn pair_grid(
    ui: &mut egui::Ui,
    id: &str,
    pairs: &mut [(String, String)],
    name_header: &str,
    value_header: &str,
    editable_names: bool,
) -> GridResult {
    let mut result = GridResult::default();

    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            TableBuilder::new(ui)
                .striped(true)
                .column(Column::remainder().at_least(120.0))
                .column(Column::remainder().at_least(120.0))
                .column(Column::auto())
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong(name_header);
                    });
                    header.col(|ui| {
                        ui.strong(value_header);
                    });
                    header.col(|ui| {
                        ui.strong("");
                    });
                })
                .body(|mut body| {
                    for (row, pair) in pairs.iter_mut().enumerate() {
                        body.row(22.0, |mut cells| {
                            cells.col(|ui| {
                                if editable_names {
                                    if ui.text_edit_singleline(&mut pair.0).changed() {
                                        result.edited = true;
                                    }
                                } else {
                                    ui.label(&pair.0);
                                }
                            });
                            cells.col(|ui| {
                                if ui.text_edit_singleline(&mut pair.1).changed() {
                                    result.edited = true;
                                }
                            });
                            cells.col(|ui| {
                                if ui.small_button("Remove").clicked() {
                                    result.removed = Some(row);
                                }
                            });
                        });
                    }
                });
        });

    result
}

/// A labelled single-line text box that reports whether it changed.
pub fn field(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        changed = ui.text_edit_singleline(value).changed();
    });
    changed
}

/// A labelled text box with a Browse button that fills it in.
///
/// The picked path is reduced to its filename: instruction files name files
/// inside the mod's own folder, and an absolute path from the operator's disk
/// would not mean anything on anyone else's.
pub fn file_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    browse: impl FnOnce() -> Option<String>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        changed = ui.text_edit_singleline(value).changed();
        if ui.button("Browse").clicked() {
            if let Some(path) = browse() {
                *value = kotor_formats::text::extract_file_name(&path);
                changed = true;
            }
        }
    });
    changed
}

/// A dropdown offering a fixed set of choices, plus whatever is already set.
///
/// A file may hold a spelling this editor does not know about, and silently
/// replacing it with the nearest known one would change what the mod does.
pub fn choice(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut String,
    options: &[&str],
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(id)
            .selected_text(if value.is_empty() { "—" } else { value.as_str() })
            .show_ui(ui, |ui| {
                let known = options.iter().any(|o| o.eq_ignore_ascii_case(value));
                if !known && !value.is_empty() {
                    let current = value.clone();
                    ui.selectable_value(value, current.clone(), format!("{current}  (from file)"));
                }
                for option in options {
                    if ui
                        .selectable_label(value.eq_ignore_ascii_case(option), *option)
                        .clicked()
                    {
                        *value = (*option).to_string();
                        changed = true;
                    }
                }
            });
    });

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_up_swaps_with_the_row_above() {
        let mut items = vec![1, 2, 3];
        assert_eq!(reorder(&mut items, 1, ListAction::MoveUp), 0);
        assert_eq!(items, vec![2, 1, 3]);
    }

    #[test]
    fn moving_down_swaps_with_the_row_below() {
        let mut items = vec![1, 2, 3];
        assert_eq!(reorder(&mut items, 1, ListAction::MoveDown), 2);
        assert_eq!(items, vec![1, 3, 2]);
    }

    #[test]
    fn moving_past_either_end_does_nothing() {
        let mut items = vec![1, 2, 3];
        assert_eq!(reorder(&mut items, 0, ListAction::MoveUp), 0);
        assert_eq!(reorder(&mut items, 2, ListAction::MoveDown), 2);
        assert_eq!(items, vec![1, 2, 3]);
    }
}
