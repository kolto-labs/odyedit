//! The `[InstallList]` section.
//!
//! This one nests. Each key in `[InstallList]` names a section of its own, and
//! that key's *value* is where the files in that section go:
//!
//! ```ini
//! [InstallList]
//! install_folder0=override
//!
//! [install_folder0]
//! File0=my_item.uti
//! Replace1=existing.uti
//! ```
//!
//! Inside a folder section, a `Replace<n>` key overwrites a file already in
//! place; a `File<n>` key leaves an existing file alone.

use crate::ini::IniFile;
use crate::model::{list_entries, INSTALL_LIST};

/// One file to copy in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallFile {
    /// Filename inside the mod's data folder.
    pub filename: String,
    /// Overwrite a file already there rather than leaving it alone.
    pub replace: bool,
}

/// One destination and the files going into it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallFolder {
    /// Section name holding this folder's files, such as `install_folder0`.
    pub section: String,
    /// Where the files go, relative to the game folder.
    pub destination: String,
    /// The files to copy.
    pub files: Vec<InstallFile>,
}

/// Read every destination and its files.
pub fn read(ini: &IniFile) -> Vec<InstallFolder> {
    list_entries(ini, INSTALL_LIST)
        .into_iter()
        .map(|(section, destination)| InstallFolder {
            files: read_files(ini, &section),
            section,
            destination,
        })
        .collect()
}

/// Read the files in one folder section.
fn read_files(ini: &IniFile, section: &str) -> Vec<InstallFile> {
    list_entries(ini, section)
        .into_iter()
        .map(|(key, filename)| InstallFile {
            filename,
            replace: key.to_ascii_lowercase().starts_with("replace"),
        })
        .collect()
}

/// Write every destination and its files back.
///
/// The folder sections are rewritten from scratch, so a file removed in the
/// panel does not linger under a key the renumbering skipped over.
pub fn write(ini: &mut IniFile, folders: &[InstallFolder]) {
    let existing: Vec<String> = list_entries(ini, INSTALL_LIST)
        .into_iter()
        .map(|(section, _)| section)
        .collect();

    let pairs: Vec<(String, String)> = folders
        .iter()
        .map(|folder| (folder.section.clone(), folder.destination.clone()))
        .collect();
    ini.replace_section_values(INSTALL_LIST, &pairs);

    // A folder dropped from the list leaves its section behind otherwise.
    for section in existing {
        if !folders.iter().any(|f| f.section.eq_ignore_ascii_case(&section)) {
            ini.delete_section(&section);
        }
    }

    for folder in folders {
        write_files(ini, &folder.section, &folder.files);
    }
}

/// Write one folder's files, renumbering the keys.
pub fn write_files(ini: &mut IniFile, section: &str, files: &[InstallFile]) {
    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(section)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();

    for (index, file) in files.iter().enumerate() {
        let prefix = if file.replace { "Replace" } else { "File" };
        pairs.push((format!("{prefix}{index}"), file.filename.clone()));
    }
    ini.replace_section_values(section, &pairs);
}

/// A folder section name nothing is using yet.
///
/// The original tool numbered these `install_folder0` upward and so does this
/// one, because the name shows up in error messages mod authors already read.
pub fn unused_folder_section(ini: &IniFile) -> String {
    for index in 0..10_000 {
        let candidate = format!("install_folder{index}");
        if !ini.section_present(&candidate) && !ini.has_key(INSTALL_LIST, &candidate) {
            return candidate;
        }
    }
    "install_folder".to_string()
}

/// Add a destination, returning the section name it was given.
pub fn add_folder(ini: &mut IniFile, folders: &mut Vec<InstallFolder>, destination: &str) -> String {
    let section = unused_folder_section(ini);
    folders.push(InstallFolder {
        section: section.clone(),
        destination: destination.to_string(),
        files: Vec::new(),
    });
    write(ini, folders);
    section
}

/// Remove a destination and everything listed under it.
pub fn remove_folder(ini: &mut IniFile, folders: &mut Vec<InstallFolder>, index: usize) {
    if index >= folders.len() {
        return;
    }
    let gone = folders.remove(index);
    ini.delete_section(&gone.section);
    write(ini, folders);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"[InstallList]\ninstall_folder0=override\ninstall_folder1=modules\n\n[install_folder0]\nFile0=a.uti\nReplace1=b.uti\n\n[install_folder1]\nFile0=c.mod\n";

    #[test]
    fn folders_read_with_their_files() {
        let folders = read(&IniFile::parse(SAMPLE));
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].section, "install_folder0");
        assert_eq!(folders[0].destination, "override");
        assert_eq!(folders[0].files.len(), 2);
        assert_eq!(folders[0].files[0].filename, "a.uti");
        assert!(!folders[0].files[0].replace);
        assert!(folders[0].files[1].replace);
        assert_eq!(folders[1].destination, "modules");
        assert_eq!(folders[1].files[0].filename, "c.mod");
    }

    #[test]
    fn folders_survive_a_write_and_read() {
        let mut ini = IniFile::parse(SAMPLE);
        let folders = read(&ini);
        write(&mut ini, &folders);
        assert_eq!(read(&ini), folders);
    }

    #[test]
    fn removing_a_file_renumbers_and_leaves_no_tail() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut folders = read(&ini);
        folders[0].files.remove(0);
        write(&mut ini, &folders);
        assert_eq!(ini.read_section("install_folder0"), vec!["Replace0"]);
        assert_eq!(read(&ini)[0].files.len(), 1);
        assert_eq!(read(&ini)[0].files[0].filename, "b.uti");
    }

    #[test]
    fn removing_a_folder_takes_its_section() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut folders = read(&ini);
        remove_folder(&mut ini, &mut folders, 0);
        assert!(!ini.section_present("install_folder0"));
        assert!(ini.section_present("install_folder1"));
        assert_eq!(read(&ini).len(), 1);
        assert_eq!(read(&ini)[0].destination, "modules");
    }

    #[test]
    fn added_folders_get_an_unused_section_name() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut folders = read(&ini);
        let section = add_folder(&mut ini, &mut folders, "lips");
        assert_eq!(section, "install_folder2");
        assert_eq!(ini.read_string(INSTALL_LIST, "install_folder2", ""), "lips");
    }

    #[test]
    fn an_empty_file_reads_as_no_folders() {
        assert!(read(&IniFile::new()).is_empty());
        assert_eq!(unused_folder_section(&IniFile::new()), "install_folder0");
    }
}
