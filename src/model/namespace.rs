//! `namespaces.ini` — the install variants a mod offers.
//!
//! A mod that ships more than one version of itself puts this file beside its
//! `tslpatchdata` folder. The patcher shows the list and installs the one the
//! player picks.
//!
//! ```ini
//! [Namespaces]
//! Namespace0=full
//! Namespace1=update
//!
//! [full]
//! Name=Full install
//! Description=Everything, for a new game.
//! IniName=changes.ini
//! InfoName=info.rtf
//! DataPath=full
//! ```
//!
//! This is a different file from `changes.ini`, so it is loaded and saved on
//! its own.

use crate::ini::IniFile;

/// The file's name, which the patcher looks for by exact spelling.
pub const FILENAME: &str = "namespaces.ini";
/// Section listing the variants.
pub const NAMESPACES: &str = "Namespaces";
/// Instruction filename a variant gets when it names none.
pub const DEFAULT_INI: &str = "changes.ini";
/// Information filename a variant gets when it names none.
pub const DEFAULT_INFO: &str = "info.rtf";

/// One install variant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Namespace {
    /// Section holding this variant's settings.
    pub section: String,
    /// Name shown in the chooser.
    pub name: String,
    /// Longer explanation shown beside the name.
    pub description: String,
    /// Instruction file to use.
    pub ini_name: String,
    /// Information file to show.
    pub info_name: String,
    /// Folder holding this variant's data, relative to `tslpatchdata`.
    pub data_path: String,
}

/// The variants a mod offers.
#[derive(Debug, Clone, Default)]
pub struct NamespaceFile {
    /// The document being edited.
    pub ini: IniFile,
    /// Where it lives.
    pub path: Option<String>,
    /// The variants, in the order the file lists them.
    pub entries: Vec<Namespace>,
}

impl NamespaceFile {
    /// A file with no variants in it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a namespaces file.
    pub fn load(path: &str) -> std::io::Result<Self> {
        let ini = IniFile::parse(&std::fs::read(path)?);
        let entries = read(&ini);
        Ok(Self {
            ini,
            path: Some(path.to_string()),
            entries,
        })
    }

    /// Open the namespaces file beside a mod, if the mod has one.
    pub fn beside(folder: &str) -> Option<Self> {
        let path = kotor_formats::text::join_path(folder, FILENAME);
        if !kotor_formats::fsutil::file_exists(&path) {
            return None;
        }
        Self::load(&path).ok()
    }

    /// Write the variants back and save.
    pub fn save_as(&mut self, path: &str) -> std::io::Result<()> {
        write(&mut self.ini, &self.entries);
        self.ini.save(path)?;
        self.path = Some(path.to_string());
        Ok(())
    }

    /// Write the variants back to where the file came from.
    pub fn save(&mut self) -> std::io::Result<()> {
        let path = self.path.clone().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "no filename chosen yet")
        })?;
        self.save_as(&path)
    }
}

/// Read the variants a file offers.
pub fn read(ini: &IniFile) -> Vec<Namespace> {
    ini.read_section_values(NAMESPACES)
        .into_iter()
        .filter(|(key, _)| !key.starts_with('!'))
        .map(|(_, configured)| {
            let section = if configured.trim().is_empty() {
                "default".to_string()
            } else {
                configured
            };
            Namespace {
                name: ini.read_string(&section, "Name", ""),
                description: ini.read_string(&section, "Description", ""),
                ini_name: ini.read_string(&section, "IniName", ""),
                info_name: ini.read_string(&section, "InfoName", ""),
                data_path: ini.read_string(&section, "DataPath", "").trim().to_string(),
                section,
            }
        })
        .collect()
}

/// Write the variants back, renumbering the list and rebuilding each section.
pub fn write(ini: &mut IniFile, entries: &[Namespace]) {
    let existing: Vec<String> = read(ini).into_iter().map(|e| e.section).collect();

    let pairs: Vec<(String, String)> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (format!("Namespace{index}"), entry.section.clone()))
        .collect();
    ini.replace_section_values(NAMESPACES, &pairs);

    for section in existing {
        if !entries.iter().any(|e| e.section.eq_ignore_ascii_case(&section)) {
            ini.delete_section(&section);
        }
    }

    for entry in entries {
        ini.write_string(&entry.section, "Name", &entry.name);
        ini.write_string(&entry.section, "Description", &entry.description);
        ini.write_string(
            &entry.section,
            "IniName",
            blank_as(&entry.ini_name, DEFAULT_INI),
        );
        ini.write_string(
            &entry.section,
            "InfoName",
            blank_as(&entry.info_name, DEFAULT_INFO),
        );
        // An empty data path means "the mod's own folder", which is written as
        // an empty value rather than left out — the patcher reads either the
        // same way, and an author scanning the file can see the field exists.
        ini.write_string(&entry.section, "DataPath", &entry.data_path);
    }
}

/// Fall back to a default when a field was left blank.
fn blank_as<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.trim().is_empty() {
        default
    } else {
        value
    }
}

/// A section name no variant is using yet.
pub fn unused_section(ini: &IniFile, entries: &[Namespace]) -> String {
    for index in 0..10_000 {
        let candidate = format!("namespace{index}");
        let taken = ini.section_present(&candidate)
            || entries
                .iter()
                .any(|e| e.section.eq_ignore_ascii_case(&candidate));
        if !taken {
            return candidate;
        }
    }
    "namespace".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"[Namespaces]\nNamespace0=full\nNamespace1=update\n\n[full]\nName=Full install\nDescription=Everything.\nIniName=changes.ini\nInfoName=info.rtf\nDataPath=full\n\n[update]\nName=Update\nDescription=Just the fixes.\nIniName=update.ini\nInfoName=update.rtf\nDataPath=update\n";

    #[test]
    fn variants_read_in_order() {
        let entries = read(&IniFile::parse(SAMPLE));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].section, "full");
        assert_eq!(entries[0].name, "Full install");
        assert_eq!(entries[0].data_path, "full");
        assert_eq!(entries[1].ini_name, "update.ini");
    }

    #[test]
    fn variants_survive_a_write_and_read() {
        let mut ini = IniFile::parse(SAMPLE);
        let entries = read(&ini);
        write(&mut ini, &entries);
        assert_eq!(read(&ini), entries);
    }

    #[test]
    fn a_blank_entry_value_means_the_default_section() {
        let entries = read(&IniFile::parse(b"[Namespaces]\nNamespace0=\n\n[default]\nName=Only\n"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].section, "default");
        assert_eq!(entries[0].name, "Only");
    }

    #[test]
    fn blank_filenames_are_written_as_the_defaults() {
        let mut ini = IniFile::new();
        write(
            &mut ini,
            &[Namespace {
                section: "only".to_string(),
                name: "Only".to_string(),
                ..Namespace::default()
            }],
        );
        assert_eq!(ini.read_string("only", "IniName", ""), DEFAULT_INI);
        assert_eq!(ini.read_string("only", "InfoName", ""), DEFAULT_INFO);
    }

    #[test]
    fn removing_a_variant_takes_its_section_and_renumbers() {
        let mut ini = IniFile::parse(SAMPLE);
        let mut entries = read(&ini);
        entries.remove(0);
        write(&mut ini, &entries);
        assert!(!ini.section_present("full"));
        assert_eq!(ini.read_section(NAMESPACES), vec!["Namespace0"]);
        assert_eq!(ini.read_string(NAMESPACES, "Namespace0", ""), "update");
    }

    #[test]
    fn new_sections_avoid_the_ones_in_use() {
        let ini = IniFile::parse(b"[namespace0]\nName=x\n");
        assert_eq!(unused_section(&ini, &[]), "namespace1");
    }

    #[test]
    fn files_survive_a_save_and_load() {
        let dir = std::env::temp_dir().join("odyedit-ns-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILENAME);
        let path = path.to_str().unwrap();

        let mut file = NamespaceFile::new();
        file.entries.push(Namespace {
            section: "full".to_string(),
            name: "Full".to_string(),
            description: "All of it.".to_string(),
            ..Namespace::default()
        });
        file.save_as(path).unwrap();

        let reopened = NamespaceFile::load(path).unwrap();
        assert_eq!(reopened.entries.len(), 1);
        assert_eq!(reopened.entries[0].name, "Full");
        std::fs::remove_dir_all(&dir).ok();
    }
}
