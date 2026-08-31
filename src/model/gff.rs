//! The `[GFFList]` section and the field changes it points at.
//!
//! Items, creatures, dialogue and most other game files share one container
//! format: a tree of named fields. A change either sets a field already there,
//! or adds a new one.
//!
//! ```ini
//! [GFFList]
//! File0=my_sword.uti
//!
//! [my_sword.uti]
//! Cost=500
//! LocalizedName(strref)=StrRef0
//! AddField0=add_property
//!
//! [add_property]
//! FieldType=Struct
//! Path=PropertiesList
//! Label=
//! TypeId=0
//! AddField0=property_cost
//! ```
//!
//! Setting a field is a plain `path=value` line. Adding one needs more than a
//! line can carry — a type, a place in the tree, sometimes a whole subtree — so
//! it gets a section, and those sections nest through their own `AddField` keys.

use crate::ini::IniFile;
use crate::model::{list_entries, GFF_LIST};

/// Keys in a file's section that direct the change rather than name a field.
pub const DIRECTIVE_KEYS: [&str; 7] = [
    "!ReplaceFile",
    "!Destination",
    "!SourceFile",
    "!SaveAs",
    "!Filename",
    "!OverrideType",
    "!SourceFolder",
];

/// The field types a change can create.
///
/// These are the spellings the instruction file uses, which are not quite the
/// spellings the format itself uses.
pub const FIELD_TYPES: [&str; 15] = [
    "Byte",
    "Char",
    "Word",
    "Short",
    "DWORD",
    "Int",
    "Int64",
    "Float",
    "Double",
    "ExoString",
    "ResRef",
    "ExoLocString",
    "Position",
    "Orientation",
    "Binary",
];

/// Field types that hold other fields rather than a value.
pub const CONTAINER_TYPES: [&str; 2] = ["Struct", "List"];

/// True when a type holds other fields rather than a value of its own.
pub fn is_container(field_type: &str) -> bool {
    CONTAINER_TYPES
        .iter()
        .any(|known| known.eq_ignore_ascii_case(field_type))
}

/// One field to create, and anything nested inside it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AddField {
    /// Section holding this field, named by the author.
    pub section: String,
    /// Which type of field to make.
    pub field_type: String,
    /// Where in the tree it goes. Empty means the file's root.
    pub path: String,
    /// The field's name. A field inside a list has none.
    pub label: String,
    /// The value, for the types that hold one.
    pub value: String,
    /// The struct type number, for `Struct` fields.
    pub type_id: String,
    /// The string reference, for `ExoLocString` fields.
    pub strref: String,
    /// Translations, as `(language number, text)` pairs.
    pub languages: Vec<(String, String)>,
    /// Fields nested inside this one.
    pub children: Vec<AddField>,
    /// Keys this editor does not recognise, carried through untouched.
    pub extra: Vec<(String, String)>,
    /// True when this entry only points at a section already above it.
    ///
    /// Nothing stops `[a]` from adding `[b]` while `[b]` adds `[a]`. Such a file
    /// is broken — the patcher warns and skips the branch — but the editor must
    /// not quietly repair it by dropping the key. The reference is kept and
    /// written back; the section it names is left to the entry that really owns
    /// it, higher up the tree.
    pub loops_back: bool,
}

/// One plain `path=value` change to a field already in the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldChange {
    /// The field's path, such as `PropertiesList\0\CostValue`.
    pub path: String,
    /// The value to set.
    pub value: String,
}

/// One game file and the changes to make to it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GffEntry {
    /// Filename, which is also the name of its section.
    pub filename: String,
    /// Overwrite the file already in place rather than patching it.
    pub replace: bool,
    /// Where the patched file goes. Empty means the game's `override` folder.
    pub destination: String,
    /// Fields already in the file, changed in place.
    pub changes: Vec<FieldChange>,
    /// Fields to create.
    pub additions: Vec<AddField>,
}

/// Read the game files and their changes.
pub fn read(ini: &IniFile) -> Vec<GffEntry> {
    // A duplicate key in this list would otherwise re-apply the first filename
    // and skip the rest, so the last spelling of each key wins.
    let mut seen: Vec<(String, String)> = Vec::new();
    for (key, filename) in list_entries(ini, GFF_LIST) {
        if let Some(at) = seen.iter().position(|(k, _)| k.eq_ignore_ascii_case(&key)) {
            seen.remove(at);
        }
        seen.push((key, filename));
    }

    seen.into_iter()
        .map(|(key, filename)| GffEntry {
            replace: key.to_ascii_lowercase().starts_with("replace"),
            destination: ini.read_string(&filename, "!Destination", ""),
            changes: read_changes(ini, &filename),
            additions: read_additions(ini, &filename),
            filename,
        })
        .collect()
}

/// Read the plain `path=value` changes in a file's section.
fn read_changes(ini: &IniFile, section: &str) -> Vec<FieldChange> {
    ini.read_section_values(section)
        .into_iter()
        .filter(|(key, _)| !key.starts_with('!') && !is_add_field_key(key))
        .map(|(path, value)| FieldChange { path, value })
        .collect()
}

/// Read the fields a section asks to create, following the nesting.
fn read_additions(ini: &IniFile, section: &str) -> Vec<AddField> {
    read_additions_inner(ini, section, &mut Vec::new())
}

/// Read the nested fields, refusing to walk into a section already on the branch.
fn read_additions_inner(ini: &IniFile, section: &str, visiting: &mut Vec<String>) -> Vec<AddField> {
    visiting.push(section.to_string());

    let fields = ini
        .read_section_values(section)
        .into_iter()
        .filter(|(key, value)| is_add_field_key(key) && !value.is_empty())
        .map(|(_, target)| {
            if visiting
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&target))
            {
                AddField {
                    section: target,
                    loops_back: true,
                    ..AddField::default()
                }
            } else {
                read_add_field(ini, &target, visiting)
            }
        })
        .collect();

    visiting.pop();
    fields
}

/// Read one field-creating section.
fn read_add_field(ini: &IniFile, section: &str, visiting: &mut Vec<String>) -> AddField {
    let mut field = AddField {
        section: section.to_string(),
        field_type: ini.read_string(section, "FieldType", ""),
        path: ini.read_string(section, "Path", ""),
        label: ini.read_string(section, "Label", ""),
        value: ini.read_string(section, "Value", ""),
        type_id: ini.read_string(section, "TypeId", ""),
        strref: ini.read_string(section, "StrRef", ""),
        languages: Vec::new(),
        children: read_additions_inner(ini, section, visiting),
        extra: Vec::new(),
        loops_back: false,
    };

    for (key, value) in ini.read_section_values(section) {
        if let Some(number) = language_number(&key) {
            field.languages.push((number, value));
            continue;
        }
        if is_known_key(&key) || is_add_field_key(&key) || key.starts_with('!') {
            continue;
        }
        field.extra.push((key, value));
    }

    field
}

/// True when a key names another section describing a field to create.
fn is_add_field_key(key: &str) -> bool {
    key.to_ascii_lowercase().starts_with("addfield")
}

/// The number in a `lang<n>` key, or `None` for any other key.
///
/// Localised strings carry one line per translation, numbered by the game's own
/// language ids.
fn language_number(key: &str) -> Option<String> {
    let lower = key.to_ascii_lowercase();
    let rest = lower.strip_prefix("lang")?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(rest.to_string())
}

/// Keys this editor knows the meaning of.
fn is_known_key(key: &str) -> bool {
    ["FieldType", "Path", "Label", "Value", "TypeId", "StrRef"]
        .iter()
        .any(|known| known.eq_ignore_ascii_case(key))
}

/// Write the game files and their changes back.
pub fn write(ini: &mut IniFile, entries: &[GffEntry]) {
    let previous = read(ini);

    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(GFF_LIST)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();
    for (index, entry) in entries.iter().enumerate() {
        let prefix = if entry.replace { "Replace" } else { "File" };
        pairs.push((format!("{prefix}{index}"), entry.filename.clone()));
    }
    ini.replace_section_values(GFF_LIST, &pairs);

    // Sections belonging to files and fields that are gone.
    let kept: Vec<String> = entries
        .iter()
        .flat_map(|entry| section_names(&entry.additions))
        .collect();
    for old in &previous {
        let still_here = entries
            .iter()
            .any(|e| e.filename.eq_ignore_ascii_case(&old.filename));
        for section in section_names(&old.additions) {
            if !still_here || !kept.iter().any(|k| k.eq_ignore_ascii_case(&section)) {
                ini.delete_section(&section);
            }
        }
        if !still_here {
            ini.delete_section(&old.filename);
        }
    }

    for entry in entries {
        write_entry(ini, entry);
    }
}

/// Every section name a tree of field additions occupies.
fn section_names(fields: &[AddField]) -> Vec<String> {
    let mut out = Vec::new();
    for field in fields {
        out.push(field.section.clone());
        out.extend(section_names(&field.children));
    }
    out
}

/// Write one field-creating section, unless it only points back up the tree.
fn write_child(ini: &mut IniFile, field: &AddField) {
    if field.loops_back {
        return;
    }
    write_add_field(ini, field);
}

/// Write one file's section and every field-creating section it names.
pub fn write_entry(ini: &mut IniFile, entry: &GffEntry) {
    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(&entry.filename)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();

    for change in &entry.changes {
        if change.path.trim().is_empty() {
            continue;
        }
        pairs.push((change.path.clone(), change.value.clone()));
    }
    for (index, field) in entry.additions.iter().enumerate() {
        pairs.push((format!("AddField{index}"), field.section.clone()));
    }
    ini.replace_section_values(&entry.filename, &pairs);

    if entry.destination.trim().is_empty() {
        ini.delete_key(&entry.filename, "!Destination");
    } else {
        ini.write_string(&entry.filename, "!Destination", &entry.destination);
    }

    for field in &entry.additions {
        write_child(ini, field);
    }
}

/// Write one field-creating section, and everything nested inside it.
pub fn write_add_field(ini: &mut IniFile, field: &AddField) {
    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(&field.section)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();

    pairs.push(("FieldType".to_string(), field.field_type.clone()));
    pairs.push(("Path".to_string(), field.path.clone()));
    pairs.push(("Label".to_string(), field.label.clone()));

    if is_container(&field.field_type) {
        // Only a struct carries a type number, and only when one was set. A
        // patcher reads a missing TypeId as zero, so writing it out would add a
        // line the author never had without changing what the mod does.
        if field.field_type.eq_ignore_ascii_case("Struct") && !field.type_id.trim().is_empty() {
            pairs.push(("TypeId".to_string(), field.type_id.clone()));
        }
    } else if field.field_type.eq_ignore_ascii_case("ExoLocString") {
        if !field.strref.trim().is_empty() {
            pairs.push(("StrRef".to_string(), field.strref.clone()));
        }
        for (number, text) in &field.languages {
            pairs.push((format!("lang{number}"), text.clone()));
        }
    } else {
        pairs.push(("Value".to_string(), field.value.clone()));
    }

    pairs.extend(field.extra.iter().cloned());

    for (index, child) in field.children.iter().enumerate() {
        pairs.push((format!("AddField{index}"), child.section.clone()));
    }

    ini.replace_section_values(&field.section, &pairs);

    for child in &field.children {
        write_child(ini, child);
    }
}

/// Remove a file, its section, and every field section it named.
pub fn remove_entry(ini: &mut IniFile, entries: &mut Vec<GffEntry>, index: usize) {
    if index >= entries.len() {
        return;
    }
    let gone = entries.remove(index);
    for section in section_names(&gone.additions) {
        ini.delete_section(&section);
    }
    ini.delete_section(&gone.filename);
    write(ini, entries);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"[GFFList]\nFile0=my_sword.uti\n\n[my_sword.uti]\nCost=500\nLocalizedName(strref)=StrRef0\nAddField0=add_property\n\n[add_property]\nFieldType=Struct\nPath=PropertiesList\nLabel=\nTypeId=0\nAddField0=inner\n\n[inner]\nFieldType=Byte\nPath=\nLabel=CostValue\nValue=1\n";

    #[test]
    fn files_read_with_their_changes_and_additions() {
        let entries = read(&IniFile::parse(SAMPLE));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].filename, "my_sword.uti");
        assert_eq!(entries[0].changes.len(), 2);
        assert_eq!(entries[0].changes[0].path, "Cost");
        assert_eq!(entries[0].changes[0].value, "500");
        assert_eq!(entries[0].changes[1].path, "LocalizedName(strref)");
        assert_eq!(entries[0].additions.len(), 1);
    }

    #[test]
    fn additions_nest() {
        let entries = read(&IniFile::parse(SAMPLE));
        let outer = &entries[0].additions[0];
        assert_eq!(outer.field_type, "Struct");
        assert_eq!(outer.path, "PropertiesList");
        assert_eq!(outer.type_id, "0");
        assert_eq!(outer.children.len(), 1);

        let inner = &outer.children[0];
        assert_eq!(inner.section, "inner");
        assert_eq!(inner.field_type, "Byte");
        assert_eq!(inner.label, "CostValue");
        assert_eq!(inner.value, "1");
        assert!(inner.children.is_empty());
    }

    #[test]
    fn everything_survives_a_write_and_read() {
        let mut ini = IniFile::parse(SAMPLE);
        let entries = read(&ini);
        write(&mut ini, &entries);
        assert_eq!(read(&ini), entries);
    }

    #[test]
    fn a_localised_string_keeps_its_translations() {
        let ini = IniFile::parse(
            b"[GFFList]\nFile0=x.utc\n\n[x.utc]\nAddField0=n\n\n[n]\nFieldType=ExoLocString\nPath=\nLabel=FirstName\nStrRef=-1\nlang0=Hello\nlang3=Bonjour\n",
        );
        let entries = read(&ini);
        let field = &entries[0].additions[0];
        assert_eq!(field.strref, "-1");
        assert_eq!(
            field.languages,
            vec![
                ("0".to_string(), "Hello".to_string()),
                ("3".to_string(), "Bonjour".to_string()),
            ]
        );

        let mut ini = ini;
        write(&mut ini, &entries);
        assert_eq!(ini.read_string("n", "lang3", ""), "Bonjour");
        assert_eq!(read(&ini), entries);
    }

    #[test]
    fn a_struct_with_no_type_id_writes_no_key() {
        let mut ini = IniFile::new();
        write_add_field(
            &mut ini,
            &AddField {
                section: "s".to_string(),
                field_type: "Struct".to_string(),
                ..AddField::default()
            },
        );
        assert!(!ini.has_key("s", "TypeId"));
    }

    #[test]
    fn a_list_gets_no_type_id_and_no_value() {
        let mut ini = IniFile::new();
        write_add_field(
            &mut ini,
            &AddField {
                section: "l".to_string(),
                field_type: "List".to_string(),
                label: "PropertiesList".to_string(),
                ..AddField::default()
            },
        );
        assert!(!ini.has_key("l", "TypeId"));
        assert!(!ini.has_key("l", "Value"));
        assert_eq!(ini.read_string("l", "FieldType", ""), "List");
    }

    #[test]
    fn removing_a_file_takes_every_nested_section() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut entries = read(&ini);
        remove_entry(&mut ini, &mut entries, 0);
        assert!(!ini.section_present("my_sword.uti"));
        assert!(!ini.section_present("add_property"));
        assert!(!ini.section_present("inner"));
        assert!(read(&ini).is_empty());
    }

    #[test]
    fn dropping_a_nested_field_takes_its_section() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut entries = read(&ini);
        entries[0].additions[0].children.clear();
        write(&mut ini, &entries);
        assert!(!ini.section_present("inner"));
        assert!(ini.section_present("add_property"));
        assert!(!ini.has_key("add_property", "AddField0"));
    }

    #[test]
    fn unknown_keys_survive() {
        let ini = IniFile::parse(
            b"[GFFList]\nFile0=x.uti\n\n[x.uti]\nAddField0=n\n\n[n]\nFieldType=Byte\nLabel=A\nValue=1\nSomethingNew=yes\n",
        );
        let entries = read(&ini);
        assert_eq!(
            entries[0].additions[0].extra,
            vec![("SomethingNew".to_string(), "yes".to_string())]
        );
        let mut ini = ini;
        write(&mut ini, &entries);
        assert_eq!(ini.read_string("n", "SomethingNew", ""), "yes");
    }

    #[test]
    fn container_types_are_recognised() {
        assert!(is_container("Struct"));
        assert!(is_container("list"));
        assert!(!is_container("Byte"));
    }

    #[test]
    fn a_duplicate_list_key_keeps_the_last_spelling() {
        let ini = IniFile::parse(b"[GFFList]\nFile0=a.uti\nFile0=b.uti\n");
        let entries = read(&ini);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].filename, "b.uti");
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
        assert!(!ini.has_key("my_sword.uti", "!Destination"));
    }
}
