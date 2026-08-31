//! The `[2DAList]` section and the table modifiers it points at.
//!
//! Game tables are plain grids: named columns, numbered rows, a label on each.
//! Four kinds of change apply to one:
//!
//! ```ini
//! [2DAList]
//! Table0=spells.2da
//!
//! [spells.2da]
//! AddRow0=add_my_spell
//! ChangeRow1=fix_force_push
//!
//! [add_my_spell]
//! label=MY_SPELL
//! name=StrRef0
//! 2DAMEMORY0=RowIndex
//! ```
//!
//! A modifier section's keys are column names and its values are what to put
//! there, mixed with a handful of keys that direct the change rather than fill a
//! cell. Two of those directing keys have to come *first* — a `ChangeRow` that
//! names its row after the columns it changes has nothing to change — so this
//! module writes the keys of each modifier in a fixed order rather than in
//! whatever order the panel happened to produce.
//!
//! Values are not always literal. `high()` means one more than the largest
//! number already in the column, which is how a mod claims a free identifier
//! without knowing what else is installed. `2DAMEMORY<n>` on the left of an `=`
//! captures something for a later step to use; on the right it spends what an
//! earlier step captured.

use crate::ini::IniFile;
use crate::model::{list_entries, TWODA_LIST};

/// The cell value meaning "nothing here".
pub const EMPTY_CELL: &str = "****";
/// Value asking for one more than the largest number in the column.
pub const HIGHEST: &str = "high()";

/// The four kinds of change that apply to a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKind {
    /// Append a row.
    AddRow,
    /// Change cells in a row already there.
    ChangeRow,
    /// Append a copy of a row already there.
    CopyRow,
    /// Append a column.
    AddColumn,
}

impl ModifierKind {
    /// Every kind, in the order the panel offers them.
    pub const ALL: [ModifierKind; 4] = [
        ModifierKind::AddRow,
        ModifierKind::ChangeRow,
        ModifierKind::CopyRow,
        ModifierKind::AddColumn,
    ];

    /// The key prefix this kind is listed under.
    pub fn prefix(self) -> &'static str {
        match self {
            ModifierKind::AddRow => "AddRow",
            ModifierKind::ChangeRow => "ChangeRow",
            ModifierKind::CopyRow => "CopyRow",
            ModifierKind::AddColumn => "AddColumn",
        }
    }

    /// Name shown in the panel.
    pub fn title(self) -> &'static str {
        match self {
            ModifierKind::AddRow => "Add row",
            ModifierKind::ChangeRow => "Change row",
            ModifierKind::CopyRow => "Copy row",
            ModifierKind::AddColumn => "Add column",
        }
    }

    /// Work out the kind from a key in a table's section.
    pub fn from_key(key: &str) -> Option<Self> {
        let lower = key.to_ascii_lowercase();
        // `AddRow` and `AddColumn` share a prefix, so the longer name is tried first.
        for kind in [
            ModifierKind::AddColumn,
            ModifierKind::AddRow,
            ModifierKind::ChangeRow,
            ModifierKind::CopyRow,
        ] {
            if lower.starts_with(&kind.prefix().to_ascii_lowercase()) {
                return Some(kind);
            }
        }
        None
    }

    /// Keys that direct this kind of change, in the order they must be written.
    ///
    /// The first of these a modifier carries is the one that names its row, and
    /// a patcher reads the keys top to bottom, so it has to be written above the
    /// cells.
    pub fn directing_keys(self) -> &'static [&'static str] {
        match self {
            ModifierKind::AddRow => &["ExclusiveColumn", "RowLabel"],
            ModifierKind::ChangeRow => &["RowIndex", "RowLabel", "LabelIndex"],
            ModifierKind::CopyRow => &[
                "RowIndex",
                "RowLabel",
                "LabelIndex",
                "ExclusiveColumn",
                "NewRowLabel",
            ],
            ModifierKind::AddColumn => &["ColumnLabel", "DefaultValue"],
        }
    }

    /// Whether this kind must name a row before it can do anything.
    pub fn needs_a_row(self) -> bool {
        matches!(self, ModifierKind::ChangeRow | ModifierKind::CopyRow)
    }
}

/// One change to make to one table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modifier {
    /// Section holding this change, named by the author.
    pub section: String,
    /// Which kind of change it is.
    pub kind: ModifierKind,
    /// Every key in the section, in file order.
    ///
    /// Cells, directing keys and captures live together here rather than in
    /// separate fields. Keeping one ordered list means a key this editor does
    /// not recognise survives a load and save untouched.
    pub entries: Vec<(String, String)>,
}

impl Modifier {
    /// A new modifier with nothing in it.
    pub fn new(section: &str, kind: ModifierKind) -> Self {
        Self {
            section: section.to_string(),
            kind,
            entries: Vec::new(),
        }
    }

    /// The value of a directing key, if the modifier carries it.
    pub fn directive(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(found, _)| found.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.as_str())
    }

    /// Set a directing key, or remove it when the value is blank.
    pub fn set_directive(&mut self, key: &str, value: &str) {
        self.entries
            .retain(|(found, _)| !found.eq_ignore_ascii_case(key));
        if !value.trim().is_empty() {
            self.entries.push((key.to_string(), value.to_string()));
        }
    }

    /// Which directing key names this modifier's row, and what it says.
    ///
    /// Only meaningful for the kinds that change a row already in the table.
    pub fn row_target(&self) -> Option<(&str, &str)> {
        ["RowIndex", "RowLabel", "LabelIndex"]
            .iter()
            .find_map(|key| self.directive(key).map(|value| (*key, value)))
    }

    /// The cells this modifier sets: everything that is not a directing key and
    /// not a capture.
    pub fn cells(&self) -> Vec<(&str, &str)> {
        self.entries
            .iter()
            .filter(|(key, _)| !self.is_directing(key) && !is_capture(key))
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect()
    }

    /// The values this modifier captures for later steps to spend.
    pub fn captures(&self) -> Vec<(&str, &str)> {
        self.entries
            .iter()
            .filter(|(key, _)| is_capture(key))
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect()
    }

    /// Whether a key directs this kind of change rather than filling a cell.
    fn is_directing(&self, key: &str) -> bool {
        self.kind
            .directing_keys()
            .iter()
            .any(|known| known.eq_ignore_ascii_case(key))
    }

    /// Set a cell, adding it if the column is not mentioned yet.
    pub fn set_cell(&mut self, column: &str, value: &str) {
        if let Some(slot) = self
            .entries
            .iter_mut()
            .find(|(key, _)| key.eq_ignore_ascii_case(column))
        {
            slot.1 = value.to_string();
            return;
        }
        self.entries.push((column.to_string(), value.to_string()));
    }

    /// Stop setting a cell.
    pub fn remove_cell(&mut self, column: &str) {
        self.entries
            .retain(|(key, _)| !key.eq_ignore_ascii_case(column));
    }

    /// The lowest `2DAMEMORY` number this modifier is not already using.
    pub fn unused_capture(&self) -> u32 {
        let mut candidate = 0;
        while self
            .entries
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(&format!("2DAMEMORY{candidate}")))
        {
            candidate += 1;
        }
        candidate
    }

    /// The keys in the order they must be written.
    ///
    /// Directing keys first, in their fixed order, then the cells in the order
    /// the author put them, then the captures. A patcher applies the keys top to
    /// bottom, so this ordering is the difference between a modifier that works
    /// and one that silently does nothing.
    fn ordered(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();

        for key in self.kind.directing_keys() {
            if let Some(value) = self.directive(key) {
                out.push((key.to_string(), value.to_string()));
            }
        }
        for (key, value) in self.cells() {
            out.push((key.to_string(), value.to_string()));
        }
        for (key, value) in self.captures() {
            out.push((key.to_string(), value.to_string()));
        }

        out
    }
}

/// True when a key captures a value for a later step.
pub fn is_capture(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("2damemory") else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

/// One table and the changes to make to it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableEntry {
    /// Filename, which is also the name of its section.
    pub filename: String,
    /// Overwrite the file already in place rather than patching it.
    pub replace: bool,
    /// Where the patched file goes. Empty means the game's `override` folder.
    pub destination: String,
    /// The changes, in the order they are applied.
    pub modifiers: Vec<Modifier>,
}

/// Read the tables and their changes.
pub fn read(ini: &IniFile) -> Vec<TableEntry> {
    list_entries(ini, TWODA_LIST)
        .into_iter()
        .map(|(key, filename)| TableEntry {
            replace: key.to_ascii_lowercase().starts_with("replace"),
            destination: ini.read_string(&filename, "!Destination", ""),
            modifiers: read_modifiers(ini, &filename),
            filename,
        })
        .collect()
}

/// Read the modifiers listed in one table's section.
fn read_modifiers(ini: &IniFile, section: &str) -> Vec<Modifier> {
    ini.read_section_values(section)
        .into_iter()
        .filter(|(key, value)| !key.starts_with('!') && !value.is_empty())
        .filter_map(|(key, target)| {
            let kind = ModifierKind::from_key(&key)?;
            Some(Modifier {
                entries: ini
                    .read_section_values(&target)
                    .into_iter()
                    .filter(|(key, _)| !key.starts_with('!'))
                    .collect(),
                section: target,
                kind,
            })
        })
        .collect()
}

/// Write the tables and their changes back.
pub fn write(ini: &mut IniFile, entries: &[TableEntry]) {
    let previous = read(ini);

    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(TWODA_LIST)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();
    for (index, entry) in entries.iter().enumerate() {
        let prefix = if entry.replace { "Replace" } else { "Table" };
        pairs.push((format!("{prefix}{index}"), entry.filename.clone()));
    }
    ini.replace_section_values(TWODA_LIST, &pairs);

    // Sections belonging to tables and modifiers that are gone.
    for old in &previous {
        let still_here = entries
            .iter()
            .any(|e| e.filename.eq_ignore_ascii_case(&old.filename));
        if !still_here {
            for modifier in &old.modifiers {
                ini.delete_section(&modifier.section);
            }
            ini.delete_section(&old.filename);
            continue;
        }
        for modifier in &old.modifiers {
            let kept = entries.iter().any(|e| {
                e.modifiers
                    .iter()
                    .any(|m| m.section.eq_ignore_ascii_case(&modifier.section))
            });
            if !kept {
                ini.delete_section(&modifier.section);
            }
        }
    }

    for entry in entries {
        write_table(ini, entry);
    }
}

/// Write one table's section and every modifier it names.
pub fn write_table(ini: &mut IniFile, entry: &TableEntry) {
    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(&entry.filename)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();

    // Each kind numbers its own keys, which is how the files have always read.
    let mut counts = [0usize; 4];
    for modifier in &entry.modifiers {
        let slot = match modifier.kind {
            ModifierKind::AddRow => 0,
            ModifierKind::ChangeRow => 1,
            ModifierKind::CopyRow => 2,
            ModifierKind::AddColumn => 3,
        };
        pairs.push((
            format!("{}{}", modifier.kind.prefix(), counts[slot]),
            modifier.section.clone(),
        ));
        counts[slot] += 1;
    }
    ini.replace_section_values(&entry.filename, &pairs);

    if entry.destination.trim().is_empty() {
        ini.delete_key(&entry.filename, "!Destination");
    } else {
        ini.write_string(&entry.filename, "!Destination", &entry.destination);
    }

    for modifier in &entry.modifiers {
        write_modifier(ini, modifier);
    }
}

/// Write one modifier's section, with its keys in the order they must be read.
pub fn write_modifier(ini: &mut IniFile, modifier: &Modifier) {
    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(&modifier.section)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();
    pairs.extend(modifier.ordered());
    ini.replace_section_values(&modifier.section, &pairs);
}

/// Remove a table, its section, and every modifier section it named.
pub fn remove_table(ini: &mut IniFile, entries: &mut Vec<TableEntry>, index: usize) {
    if index >= entries.len() {
        return;
    }
    let gone = entries.remove(index);
    for modifier in &gone.modifiers {
        ini.delete_section(&modifier.section);
    }
    ini.delete_section(&gone.filename);
    write(ini, entries);
}

/// Remove one modifier and its section.
pub fn remove_modifier(ini: &mut IniFile, entry: &mut TableEntry, index: usize) {
    if index >= entry.modifiers.len() {
        return;
    }
    let gone = entry.modifiers.remove(index);
    ini.delete_section(&gone.section);
    write_table(ini, entry);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=add_my_spell\nChangeRow0=fix_push\n\n[add_my_spell]\nlabel=MY_SPELL\nname=StrRef0\n2DAMEMORY0=RowIndex\n\n[fix_push]\nRowIndex=12\nrange=5.0\n";

    #[test]
    fn tables_read_with_their_modifiers() {
        let entries = read(&IniFile::parse(SAMPLE));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].filename, "spells.2da");
        assert_eq!(entries[0].modifiers.len(), 2);
        assert_eq!(entries[0].modifiers[0].kind, ModifierKind::AddRow);
        assert_eq!(entries[0].modifiers[0].section, "add_my_spell");
        assert_eq!(entries[0].modifiers[1].kind, ModifierKind::ChangeRow);
    }

    #[test]
    fn cells_captures_and_directives_are_told_apart() {
        let entries = read(&IniFile::parse(SAMPLE));
        let add = &entries[0].modifiers[0];
        assert_eq!(add.cells(), vec![("label", "MY_SPELL"), ("name", "StrRef0")]);
        assert_eq!(add.captures(), vec![("2DAMEMORY0", "RowIndex")]);

        let change = &entries[0].modifiers[1];
        assert_eq!(change.row_target(), Some(("RowIndex", "12")));
        assert_eq!(change.cells(), vec![("range", "5.0")]);
    }

    #[test]
    fn everything_survives_a_write_and_read() {
        let mut ini = IniFile::parse(SAMPLE);
        let entries = read(&ini);
        write(&mut ini, &entries);
        assert_eq!(read(&ini), entries);
    }

    #[test]
    fn add_column_is_not_mistaken_for_add_row() {
        assert_eq!(
            ModifierKind::from_key("AddColumn0"),
            Some(ModifierKind::AddColumn)
        );
        assert_eq!(ModifierKind::from_key("AddRow3"), Some(ModifierKind::AddRow));
        assert_eq!(ModifierKind::from_key("Nonsense"), None);
    }

    #[test]
    fn the_row_identifier_is_written_above_the_cells() {
        // A ChangeRow whose RowIndex came after its columns would change nothing.
        let mut modifier = Modifier::new("fix", ModifierKind::ChangeRow);
        modifier.set_cell("range", "5.0");
        modifier.set_directive("RowIndex", "12");

        let mut ini = IniFile::new();
        write_modifier(&mut ini, &modifier);
        assert_eq!(ini.read_section("fix"), vec!["RowIndex", "range"]);
    }

    #[test]
    fn captures_are_written_last() {
        let mut modifier = Modifier::new("add", ModifierKind::AddRow);
        modifier.entries.push(("2DAMEMORY0".to_string(), "RowIndex".to_string()));
        modifier.set_cell("label", "X");
        modifier.set_directive("ExclusiveColumn", "label");

        let mut ini = IniFile::new();
        write_modifier(&mut ini, &modifier);
        assert_eq!(
            ini.read_section("add"),
            vec!["ExclusiveColumn", "label", "2DAMEMORY0"]
        );
    }

    #[test]
    fn add_column_directives_come_first_and_in_order() {
        let mut modifier = Modifier::new("newcol", ModifierKind::AddColumn);
        modifier.set_cell("i3", "yes");
        modifier.set_directive("DefaultValue", EMPTY_CELL);
        modifier.set_directive("ColumnLabel", "mycol");

        let mut ini = IniFile::new();
        write_modifier(&mut ini, &modifier);
        assert_eq!(
            ini.read_section("newcol"),
            vec!["ColumnLabel", "DefaultValue", "i3"]
        );
    }

    #[test]
    fn each_kind_numbers_its_own_keys() {
        let mut ini = IniFile::new();
        write(
            &mut ini,
            &[TableEntry {
                filename: "spells.2da".to_string(),
                modifiers: vec![
                    Modifier::new("a", ModifierKind::AddRow),
                    Modifier::new("b", ModifierKind::AddRow),
                    Modifier::new("c", ModifierKind::ChangeRow),
                ],
                ..TableEntry::default()
            }],
        );
        assert_eq!(
            ini.read_section("spells.2da"),
            vec!["AddRow0", "AddRow1", "ChangeRow0"]
        );
    }

    #[test]
    fn removing_a_modifier_takes_its_section() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut entries = read(&ini);
        let mut table = entries[0].clone();
        remove_modifier(&mut ini, &mut table, 0);
        assert!(!ini.section_present("add_my_spell"));
        assert!(ini.section_present("fix_push"));
        assert_eq!(ini.read_section("spells.2da"), vec!["ChangeRow0"]);
    }

    #[test]
    fn removing_a_table_takes_every_section_it_owned() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut entries = read(&ini);
        remove_table(&mut ini, &mut entries, 0);
        assert!(!ini.section_present("spells.2da"));
        assert!(!ini.section_present("add_my_spell"));
        assert!(!ini.section_present("fix_push"));
        assert!(read(&ini).is_empty());
    }

    #[test]
    fn unknown_keys_in_a_modifier_survive() {
        let ini = IniFile::parse(
            b"[2DAList]\nTable0=x.2da\n\n[x.2da]\nAddRow0=m\n\n[m]\nlabel=A\nSomethingNew=1\n",
        );
        let entries = read(&ini);
        let mut ini = ini;
        write(&mut ini, &entries);
        assert_eq!(ini.read_string("m", "SomethingNew", ""), "1");
    }

    #[test]
    fn new_capture_numbers_avoid_the_ones_in_use() {
        let mut modifier = Modifier::new("m", ModifierKind::AddRow);
        assert_eq!(modifier.unused_capture(), 0);
        modifier
            .entries
            .push(("2DAMEMORY0".to_string(), "RowIndex".to_string()));
        assert_eq!(modifier.unused_capture(), 1);
    }

    #[test]
    fn captures_are_recognised_by_shape() {
        assert!(is_capture("2DAMEMORY0"));
        assert!(is_capture("2damemory12"));
        assert!(!is_capture("2DAMEMORY"));
        assert!(!is_capture("label"));
    }

    #[test]
    fn setting_a_cell_twice_replaces_it() {
        let mut modifier = Modifier::new("m", ModifierKind::AddRow);
        modifier.set_cell("label", "A");
        modifier.set_cell("label", "B");
        assert_eq!(modifier.cells(), vec![("label", "B")]);
        modifier.remove_cell("label");
        assert!(modifier.cells().is_empty());
    }

    #[test]
    fn a_blank_directive_removes_it() {
        let mut modifier = Modifier::new("m", ModifierKind::ChangeRow);
        modifier.set_directive("RowIndex", "3");
        assert_eq!(modifier.row_target(), Some(("RowIndex", "3")));
        modifier.set_directive("RowIndex", "");
        assert_eq!(modifier.row_target(), None);
    }

    #[test]
    fn the_destination_key_is_kept_and_cleared() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut entries = read(&ini);
        entries[0].destination = "modules\\danm13.mod".to_string();
        write(&mut ini, &entries);
        assert_eq!(read(&ini)[0].destination, "modules\\danm13.mod");

        entries[0].destination.clear();
        write(&mut ini, &entries);
        assert!(!ini.has_key("spells.2da", "!Destination"));
    }
}
