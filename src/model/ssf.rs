//! The `[SSFList]` section.
//!
//! A soundset file holds one dialog string reference per situation — what a
//! character says when wounded, when a lock is picked, and so on. A change here
//! points one of those entries at a different string.
//!
//! ```ini
//! [SSFList]
//! File0=p_bastila.ssf
//!
//! [p_bastila.ssf]
//! Battlecry 1=StrRef4
//! ```
//!
//! The value is a plain string reference, or a token standing for one that an
//! earlier step worked out: `StrRef<n>` for a line this mod added to the dialog
//! table, `2DAMEMORY<n>` for a value a table change captured.

use crate::ini::IniFile;
use crate::model::{list_entries, write_list, SSF_LIST};

/// The soundset entries, in the order the format stores them.
pub use kotor_formats::ssf::ENTRY_LABELS;

/// One change to one soundset entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SsfChange {
    /// Which entry, spelled as the file spells it.
    pub label: String,
    /// The new string reference, or a token standing for one.
    pub value: String,
}

/// One soundset file and the changes to make to it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SsfEntry {
    /// Filename, which is also the name of its modifier section.
    pub filename: String,
    /// Overwrite the file already in place rather than patching it.
    pub replace: bool,
    /// The entries to change.
    pub changes: Vec<SsfChange>,
}

/// Read the soundset files and their changes.
pub fn read(ini: &IniFile) -> Vec<SsfEntry> {
    list_entries(ini, SSF_LIST)
        .into_iter()
        .map(|(key, filename)| SsfEntry {
            replace: key.to_ascii_lowercase().starts_with("replace"),
            changes: read_changes(ini, &filename),
            filename,
        })
        .collect()
}

/// Read the changes in one soundset's section.
fn read_changes(ini: &IniFile, section: &str) -> Vec<SsfChange> {
    ini.read_section_values(section)
        .into_iter()
        .filter(|(key, _)| !key.starts_with('!'))
        .map(|(label, value)| SsfChange { label, value })
        .collect()
}

/// Write the soundset files and their changes back.
pub fn write(ini: &mut IniFile, entries: &[SsfEntry]) {
    let existing: Vec<String> = list_entries(ini, SSF_LIST)
        .into_iter()
        .map(|(_, filename)| filename)
        .collect();

    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(SSF_LIST)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();
    for (index, entry) in entries.iter().enumerate() {
        let prefix = if entry.replace { "Replace" } else { "File" };
        pairs.push((format!("{prefix}{index}"), entry.filename.clone()));
    }
    ini.replace_section_values(SSF_LIST, &pairs);

    for filename in existing {
        if !entries
            .iter()
            .any(|e| e.filename.eq_ignore_ascii_case(&filename))
        {
            ini.delete_section(&filename);
        }
    }

    for entry in entries {
        write_changes(ini, &entry.filename, &entry.changes);
    }
}

/// Write one soundset's changes, keeping any directive keys it carries.
pub fn write_changes(ini: &mut IniFile, section: &str, changes: &[SsfChange]) {
    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(section)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();
    for change in changes {
        if change.label.trim().is_empty() {
            continue;
        }
        pairs.push((change.label.clone(), change.value.clone()));
    }
    ini.replace_section_values(section, &pairs);
}

/// Remove a soundset and its section.
pub fn remove(ini: &mut IniFile, entries: &mut Vec<SsfEntry>, index: usize) {
    if index >= entries.len() {
        return;
    }
    let gone = entries.remove(index);
    if !entries.iter().any(|e| e.filename == gone.filename) {
        ini.delete_section(&gone.filename);
    }
    write(ini, entries);
}

/// Entry labels not yet changed in this file, for the chooser to offer.
pub fn unused_labels(entry: &SsfEntry) -> Vec<&'static str> {
    ENTRY_LABELS
        .iter()
        .copied()
        .filter(|label| {
            !entry
                .changes
                .iter()
                .any(|change| change.label.eq_ignore_ascii_case(label))
        })
        .collect()
}

/// Write a plain list of soundset filenames.
pub fn write_names(ini: &mut IniFile, names: &[String]) {
    write_list(ini, SSF_LIST, "File", names);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] =
        b"[SSFList]\nFile0=p_bastila.ssf\n\n[p_bastila.ssf]\nBattlecry 1=StrRef4\nSelected 1=123\n";

    #[test]
    fn soundsets_read_with_their_changes() {
        let entries = read(&IniFile::parse(SAMPLE));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].filename, "p_bastila.ssf");
        assert!(!entries[0].replace);
        assert_eq!(entries[0].changes.len(), 2);
        assert_eq!(entries[0].changes[0].label, "Battlecry 1");
        assert_eq!(entries[0].changes[0].value, "StrRef4");
        assert_eq!(entries[0].changes[1].value, "123");
    }

    #[test]
    fn changes_survive_a_write_and_read() {
        let mut ini = IniFile::parse(SAMPLE);
        let entries = read(&ini);
        write(&mut ini, &entries);
        assert_eq!(read(&ini), entries);
    }

    #[test]
    fn changes_keep_their_order() {
        let mut ini = IniFile::new();
        write(
            &mut ini,
            &[SsfEntry {
                filename: "x.ssf".to_string(),
                replace: false,
                changes: vec![
                    SsfChange {
                        label: "Selected 1".to_string(),
                        value: "1".to_string(),
                    },
                    SsfChange {
                        label: "Battlecry 1".to_string(),
                        value: "2".to_string(),
                    },
                ],
            }],
        );
        assert_eq!(ini.read_section("x.ssf"), vec!["Selected 1", "Battlecry 1"]);
    }

    #[test]
    fn removing_a_soundset_takes_its_section() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut entries = read(&ini);
        remove(&mut ini, &mut entries, 0);
        assert!(!ini.section_present("p_bastila.ssf"));
        assert!(read(&ini).is_empty());
    }

    #[test]
    fn unused_labels_leave_out_the_ones_already_changed() {
        let entry = &read(&IniFile::parse(SAMPLE))[0];
        let free = unused_labels(entry);
        assert!(!free.iter().any(|l| l.eq_ignore_ascii_case("Battlecry 1")));
        assert_eq!(free.len(), ENTRY_LABELS.len() - 2);
    }

    #[test]
    fn the_replace_flag_picks_the_key_name() {
        let mut ini = IniFile::new();
        write(
            &mut ini,
            &[SsfEntry {
                filename: "x.ssf".to_string(),
                replace: true,
                changes: Vec::new(),
            }],
        );
        assert_eq!(ini.read_section(SSF_LIST), vec!["Replace0"]);
        assert!(read(&ini)[0].replace);
    }

    #[test]
    fn write_names_renumbers_a_plain_list() {
        let mut ini = IniFile::new();
        write_names(&mut ini, &["a.ssf".to_string(), "b.ssf".to_string()]);
        assert_eq!(ini.read_section(SSF_LIST), vec!["File0", "File1"]);
    }
}
