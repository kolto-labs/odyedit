//! The `changes.ini` vocabulary, given names and types.
//!
//! The document itself stays an [`IniFile`](crate::ini::IniFile). Everything
//! here is a typed view over that one live object rather than a parallel copy
//! of it, which is how the original ChangeEdit worked too. The reason is the
//! round trip: a model that owned its own parsed copy would have to rebuild the
//! whole file on save and would lose every comment and every hand-made bit of
//! spacing along the way. Reading through to the INI means an edit touches only
//! the lines it actually changes.
//!
//! A file is a `[Settings]` section plus up to seven instruction lists. Each
//! list names files, and most of them give every named file a section of its own
//! holding the changes to make to it.

pub mod compare;
pub mod gff;
pub mod install;
pub mod namespace;
pub mod script;
pub mod settings;
pub mod ssf;
pub mod tlk;
pub mod twoda;

use crate::ini::IniFile;

/// Section holding the run's settings.
pub const SETTINGS: &str = "Settings";
/// Section listing dialog string changes.
pub const TLK_LIST: &str = "TLKList";
/// Section listing tables to change.
pub const TWODA_LIST: &str = "2DAList";
/// Section listing structured game files to change.
pub const GFF_LIST: &str = "GFFList";
/// Section listing scripts to compile.
pub const COMPILE_LIST: &str = "CompileList";
/// Section listing folders to copy files into.
pub const INSTALL_LIST: &str = "InstallList";
/// Section listing soundsets to change.
pub const SSF_LIST: &str = "SSFList";
/// Section listing compiled scripts to patch in place.
pub const HACK_LIST: &str = "HACKList";

/// The eight places the editor can take you.
///
/// These are the top-level entries in the window's tree, in the order they are
/// shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pane {
    /// Run settings.
    Settings,
    /// Dialog string changes.
    Tlk,
    /// Table changes.
    TwoDa,
    /// Structured game file changes.
    Gff,
    /// Scripts to compile.
    Script,
    /// Files to copy in.
    Install,
    /// Soundset changes.
    Ssf,
    /// Install variants.
    Namespace,
}

impl Pane {
    /// Every pane, in the order the tree shows them.
    pub const ALL: [Pane; 8] = [
        Pane::Settings,
        Pane::Tlk,
        Pane::TwoDa,
        Pane::Gff,
        Pane::Script,
        Pane::Install,
        Pane::Ssf,
        Pane::Namespace,
    ];

    /// Name shown in the tree.
    pub fn title(self) -> &'static str {
        match self {
            Pane::Settings => "Settings",
            Pane::Tlk => "Dialog strings",
            Pane::TwoDa => "Tables",
            Pane::Gff => "Game files",
            Pane::Script => "Scripts",
            Pane::Install => "Files to install",
            Pane::Ssf => "Soundsets",
            Pane::Namespace => "Install variants",
        }
    }

    /// One line saying what the pane is for.
    pub fn summary(self) -> &'static str {
        match self {
            Pane::Settings => "Name the installer, and decide what it checks before it runs.",
            Pane::Tlk => "Add lines to the game's dialog table and give them tokens.",
            Pane::TwoDa => "Add rows and columns to game tables, or change cells in them.",
            Pane::Gff => "Change fields inside items, creatures, dialogue and other game files.",
            Pane::Script => "Compile source scripts and install the result.",
            Pane::Install => "Copy files into the game's folders.",
            Pane::Ssf => "Point soundset entries at different dialog strings.",
            Pane::Namespace => "Offer more than one version of the mod to install.",
        }
    }
}

/// An open instruction file.
pub struct ChangesFile {
    /// The document being edited.
    pub ini: IniFile,
    /// Where it was loaded from, if it came from disk.
    pub path: Option<String>,
    /// Whether there are changes not yet on disk.
    dirty: bool,
}

impl Default for ChangesFile {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangesFile {
    /// A new file with nothing in it.
    pub fn new() -> Self {
        Self {
            ini: IniFile::new(),
            path: None,
            dirty: false,
        }
    }

    /// Open an instruction file.
    pub fn load(path: &str) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        Ok(Self {
            ini: IniFile::parse(&bytes),
            path: Some(path.to_string()),
            dirty: false,
        })
    }

    /// Write the file back where it came from.
    pub fn save(&mut self) -> std::io::Result<()> {
        let path = self.path.clone().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "no filename chosen yet")
        })?;
        self.save_as(&path)
    }

    /// Write the file to a chosen place, which becomes its new home.
    pub fn save_as(&mut self, path: &str) -> std::io::Result<()> {
        self.ini.save(path)?;
        self.path = Some(path.to_string());
        self.dirty = false;
        Ok(())
    }

    /// Whether there are changes not yet on disk.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Note that something changed. Every editing call goes through this.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// Name to show in the title bar.
    pub fn display_name(&self) -> String {
        match &self.path {
            Some(path) => kotor_formats::text::extract_file_name(path),
            None => "untitled".to_string(),
        }
    }

    /// The folder the file sits in, which is where its data files live too.
    pub fn folder(&self) -> Option<String> {
        self.path
            .as_deref()
            .map(kotor_formats::text::extract_file_path)
    }
}

/// Read a numbered list section into `(key, value)` pairs, skipping directives.
///
/// Instruction lists name their entries `File0`, `Table1`, `script2` and so on.
/// The numbering carries no meaning beyond order, and keys beginning with `!`
/// are settings for the list rather than entries in it.
pub fn list_entries(ini: &IniFile, section: &str) -> Vec<(String, String)> {
    ini.read_section_values(section)
        .into_iter()
        .filter(|(key, value)| !key.starts_with('!') && !value.is_empty())
        .collect()
}

/// Write a numbered list back, renumbering from zero.
///
/// A delete has to close the gap it leaves: a patcher walks these keys in file
/// order, and `File0`, `File2` with no `File1` reads as a mistake even though
/// nothing depends on the numbers being contiguous. Directive keys keep their
/// own spelling and stay at the front.
pub fn write_list(ini: &mut IniFile, section: &str, prefix: &str, values: &[String]) {
    let mut pairs: Vec<(String, String)> = ini
        .read_section_values(section)
        .into_iter()
        .filter(|(key, _)| key.starts_with('!'))
        .collect();

    for (index, value) in values.iter().enumerate() {
        pairs.push((format!("{prefix}{index}"), value.clone()));
    }

    ini.replace_section_values(section, &pairs);
}

/// Find a name no section is using yet, by adding a number to `base`.
///
/// Modifier sections are named by the author, and two rows called `add_feat`
/// would silently merge into one. The original tool let that happen; this one
/// offers `add_feat2` instead.
pub fn unused_section_name(ini: &IniFile, base: &str) -> String {
    if !ini.section_present(base) {
        return base.to_string();
    }
    for suffix in 2..10_000 {
        let candidate = format!("{base}{suffix}");
        if !ini.section_present(&candidate) {
            return candidate;
        }
    }
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_entries_skip_directives_and_blanks() {
        let ini = IniFile::parse(b"[L]\n!Destination=override\nFile0=a\nFile1=\nFile2=c\n");
        let entries = list_entries(&ini, "L");
        assert_eq!(
            entries,
            vec![
                ("File0".to_string(), "a".to_string()),
                ("File2".to_string(), "c".to_string()),
            ]
        );
    }

    #[test]
    fn writing_a_list_renumbers_from_zero_and_keeps_directives() {
        let mut ini = IniFile::parse(b"[L]\n!Destination=override\nFile0=a\nFile1=b\nFile2=c\n");
        write_list(&mut ini, "L", "File", &["a".to_string(), "c".to_string()]);
        assert_eq!(
            ini.read_section("L"),
            vec!["!Destination", "File0", "File1"]
        );
        assert_eq!(ini.read_string("L", "File1", ""), "c");
        assert_eq!(ini.read_string("L", "!Destination", ""), "override");
    }

    #[test]
    fn unused_names_step_around_what_is_taken() {
        let ini = IniFile::parse(b"[add_feat]\nA=1\n\n[add_feat2]\nB=2\n");
        assert_eq!(unused_section_name(&ini, "add_feat"), "add_feat3");
        assert_eq!(unused_section_name(&ini, "fresh"), "fresh");
    }

    #[test]
    fn saving_clears_the_dirty_flag() {
        let dir = std::env::temp_dir().join("odyedit-model-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("changes.ini");
        let path = path.to_str().unwrap();

        let mut file = ChangesFile::new();
        file.ini.write_string(SETTINGS, "WindowCaption", "Test");
        file.touch();
        assert!(file.is_dirty());

        file.save_as(path).unwrap();
        assert!(!file.is_dirty());
        assert_eq!(file.display_name(), "changes.ini");

        let reopened = ChangesFile::load(path).unwrap();
        assert_eq!(
            reopened.ini.read_string(SETTINGS, "WindowCaption", ""),
            "Test"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
