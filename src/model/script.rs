//! The `[CompileList]` section.
//!
//! Each entry names a source script to compile and install. The compiled result
//! goes to `!DefaultDestination`, or to the script's own `!Destination` when it
//! names one.

use crate::ini::IniFile;
use crate::model::{list_entries, write_list, COMPILE_LIST};

/// Where compiled scripts go when nothing says otherwise.
pub const DEFAULT_DESTINATION: &str = "override";

/// One script to compile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptEntry {
    /// Source filename, such as `k_con_myscript.nss`.
    pub filename: String,
    /// Where the compiled script goes. Empty follows the list default.
    pub destination: String,
    /// Overwrite a file already there rather than stepping around it.
    pub replace: bool,
}

/// Read the scripts to compile.
pub fn read(ini: &IniFile) -> Vec<ScriptEntry> {
    list_entries(ini, COMPILE_LIST)
        .into_iter()
        .map(|(key, filename)| ScriptEntry {
            destination: ini.read_string(&filename, "!Destination", ""),
            replace: key.to_ascii_lowercase().starts_with("replace"),
            filename,
        })
        .collect()
}

/// The destination scripts use when they do not name one.
pub fn default_destination(ini: &IniFile) -> String {
    let value = ini.read_string(COMPILE_LIST, "!DefaultDestination", DEFAULT_DESTINATION);
    if value.trim().is_empty() {
        DEFAULT_DESTINATION.to_string()
    } else {
        value
    }
}

/// Set the destination scripts use when they do not name one.
pub fn set_default_destination(ini: &mut IniFile, destination: &str) {
    if destination.trim().is_empty() || destination == DEFAULT_DESTINATION {
        ini.delete_key(COMPILE_LIST, "!DefaultDestination");
    } else {
        ini.write_string(COMPILE_LIST, "!DefaultDestination", destination);
    }
}

/// Write the scripts back, renumbering the keys.
///
/// A script marked `replace` is written under a `Replace<n>` key, which is how
/// the list says "overwrite what is there". The two kinds share one numbering
/// run, because they share one list.
pub fn write(ini: &mut IniFile, entries: &[ScriptEntry]) {
    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(COMPILE_LIST)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();

    for (index, entry) in entries.iter().enumerate() {
        let prefix = if entry.replace { "Replace" } else { "File" };
        pairs.push((format!("{prefix}{index}"), entry.filename.clone()));
    }
    ini.replace_section_values(COMPILE_LIST, &pairs);

    for entry in entries {
        if entry.destination.trim().is_empty() {
            if ini.section_present(&entry.filename) {
                ini.delete_key(&entry.filename, "!Destination");
            }
        } else {
            ini.write_string(&entry.filename, "!Destination", &entry.destination);
        }
    }
}

/// Remove a script and the section holding its settings.
pub fn remove(ini: &mut IniFile, entries: &mut Vec<ScriptEntry>, index: usize) {
    if index >= entries.len() {
        return;
    }
    let gone = entries.remove(index);
    // Another entry may still point at the same file, so only drop the section
    // when nothing is left referring to it.
    if !entries.iter().any(|e| e.filename == gone.filename) {
        ini.delete_section(&gone.filename);
    }
    write(ini, entries);
}

/// Keys the numbered-list writer owns, exposed for the panels that reorder.
pub fn list_prefix(replace: bool) -> &'static str {
    if replace {
        "Replace"
    } else {
        "File"
    }
}

/// Write a plain list of filenames, used where nothing needs a destination.
pub fn write_names(ini: &mut IniFile, names: &[String]) {
    write_list(ini, COMPILE_LIST, "File", names);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_read_with_their_destinations() {
        let ini = IniFile::parse(
            b"[CompileList]\n!DefaultDestination=override\nFile0=a.nss\nReplace1=b.nss\n\n[b.nss]\n!Destination=modules\\danm13.mod\n",
        );
        let entries = read(&ini);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].filename, "a.nss");
        assert!(!entries[0].replace);
        assert!(entries[0].destination.is_empty());
        assert_eq!(entries[1].filename, "b.nss");
        assert!(entries[1].replace);
        assert_eq!(entries[1].destination, "modules\\danm13.mod");
    }

    #[test]
    fn entries_survive_a_write_and_read() {
        let mut ini = IniFile::new();
        let entries = vec![
            ScriptEntry {
                filename: "a.nss".to_string(),
                destination: String::new(),
                replace: false,
            },
            ScriptEntry {
                filename: "b.nss".to_string(),
                destination: "modules\\x.mod".to_string(),
                replace: true,
            },
        ];
        write(&mut ini, &entries);
        assert_eq!(read(&ini), entries);
    }

    #[test]
    fn the_replace_flag_picks_the_key_name() {
        let mut ini = IniFile::new();
        write(
            &mut ini,
            &[ScriptEntry {
                filename: "a.nss".to_string(),
                replace: true,
                ..ScriptEntry::default()
            }],
        );
        assert_eq!(ini.read_section(COMPILE_LIST), vec!["Replace0"]);
    }

    #[test]
    fn removing_a_script_takes_its_section_and_renumbers() {
        let mut ini = IniFile::parse(
            b"[CompileList]\nFile0=a.nss\nFile1=b.nss\nFile2=c.nss\n\n[b.nss]\n!Destination=modules\n",
        );
        let mut entries = read(&ini);
        remove(&mut ini, &mut entries, 1);
        assert!(!ini.section_present("b.nss"));
        assert_eq!(ini.read_section(COMPILE_LIST), vec!["File0", "File1"]);
        assert_eq!(ini.read_string(COMPILE_LIST, "File1", ""), "c.nss");
    }

    #[test]
    fn the_default_destination_is_dropped_when_it_is_the_default() {
        let mut ini = IniFile::parse(b"[CompileList]\n!DefaultDestination=modules\nFile0=a.nss\n");
        assert_eq!(default_destination(&ini), "modules");
        set_default_destination(&mut ini, "override");
        assert!(!ini.has_key(COMPILE_LIST, "!DefaultDestination"));
        assert_eq!(default_destination(&ini), "override");
    }

    #[test]
    fn clearing_a_destination_removes_the_key() {
        let mut ini =
            IniFile::parse(b"[CompileList]\nFile0=a.nss\n\n[a.nss]\n!Destination=modules\n");
        let mut entries = read(&ini);
        entries[0].destination.clear();
        write(&mut ini, &entries);
        assert!(!ini.has_key("a.nss", "!Destination"));
    }

    #[test]
    fn write_names_renumbers_a_plain_list() {
        let mut ini = IniFile::new();
        write_names(&mut ini, &["a.nss".to_string(), "b.nss".to_string()]);
        assert_eq!(ini.read_section(COMPILE_LIST), vec!["File0", "File1"]);
        assert_eq!(list_prefix(false), "File");
        assert_eq!(list_prefix(true), "Replace");
    }
}
