//! The `[TLKList]` section, and the append file it points into.
//!
//! A mod that adds text to the game cannot write into `dialog.tlk` directly —
//! every mod would fight over the same line numbers. Instead it ships an
//! `append.tlk` holding just its own lines, and `[TLKList]` says which of those
//! lines to append and what token to remember each one by:
//!
//! ```ini
//! [TLKList]
//! StrRef0=0
//! StrRef1=1
//! ```
//!
//! The key is the token: once installed, `StrRef0` elsewhere in the file stands
//! for whatever line number the first appended string actually landed on. The
//! value is the index of that string inside `append.tlk`.
//!
//! Newer patchers accept other spellings in this section — `AppendFile<n>`,
//! `Replace<n>`, `<n>\Text`. This editor reads a file containing them without
//! complaint and leaves them alone, but the entries it writes are the
//! `StrRef<token>=<index>` form, which every patcher understands.

use kotor_formats::tlk::{TlkEntry, TlkFile};

use crate::ini::IniFile;
use crate::model::TLK_LIST;

/// The append file's usual name.
pub const DEFAULT_APPEND_FILE: &str = "append.tlk";

/// One token mapped to a line in the append file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenMapping {
    /// The number in `StrRef<n>`, which is how the rest of the file refers to it.
    pub token: u32,
    /// Which string in the append file this token stands for.
    pub index: u32,
}

impl TokenMapping {
    /// The key this mapping is written under.
    pub fn key(&self) -> String {
        format!("StrRef{}", self.token)
    }
}

/// Read the token mappings, ignoring spellings this editor does not author.
pub fn read(ini: &IniFile) -> Vec<TokenMapping> {
    ini.read_section_values(TLK_LIST)
        .into_iter()
        .filter_map(|(key, value)| {
            let token = strref_token(&key)?;
            let index = value.trim().parse::<u32>().ok()?;
            Some(TokenMapping { token, index })
        })
        .collect()
}

/// Keys in the section this editor does not author, kept exactly as found.
///
/// A file written by a newer patcher may mix these in. Dropping them on save
/// would quietly break the mod, so they are carried through untouched.
pub fn foreign_keys(ini: &IniFile) -> Vec<(String, String)> {
    ini.read_section_values(TLK_LIST)
        .into_iter()
        .filter(|(key, _)| strref_token(key).is_none())
        .collect()
}

/// The number in a `StrRef<n>` key, or `None` for any other key.
fn strref_token(key: &str) -> Option<u32> {
    let rest = key.strip_prefix("StrRef").or_else(|| {
        key.to_ascii_lowercase()
            .starts_with("strref")
            .then(|| &key[6..])
    })?;
    if rest.is_empty() {
        return None;
    }
    rest.trim().parse::<u32>().ok()
}

/// Write the token mappings back, keeping any foreign keys ahead of them.
pub fn write(ini: &mut IniFile, mappings: &[TokenMapping]) {
    let mut pairs = foreign_keys(ini);
    for mapping in mappings {
        pairs.push((mapping.key(), mapping.index.to_string()));
    }
    ini.replace_section_values(TLK_LIST, &pairs);
}

/// The lowest token number nothing is using.
///
/// Tokens are referred to by number from other sections, so reusing one that is
/// already spoken for would silently repoint an existing reference.
pub fn unused_token(mappings: &[TokenMapping]) -> u32 {
    let mut candidate = 0;
    while mappings.iter().any(|m| m.token == candidate) {
        candidate += 1;
    }
    candidate
}

/// Add a mapping for a line in the append file, and return its token.
pub fn add_mapping(mappings: &mut Vec<TokenMapping>, index: u32) -> u32 {
    let token = unused_token(mappings);
    mappings.push(TokenMapping { token, index });
    token
}

/// Build an entry from text and an optional sound name.
///
/// The flags say which fields carry data, and a reader that trusts them will
/// skip text the flags do not announce. The recorded byte length is left alone:
/// `kotor-formats` recalculates it when the entry goes in.
fn entry_for(text: &str, sound: &str) -> TlkEntry {
    let sound = sound.trim();
    let mut flags = kotor_formats::tlk::FLAG_TEXT_PRESENT;
    if !sound.is_empty() {
        flags |= kotor_formats::tlk::FLAG_SOUND_PRESENT;
    }

    TlkEntry {
        flags,
        sound_resref: kotor_formats::latin1::encode_fixed::<16>(sound),
        text: text.to_string(),
        ..TlkEntry::default()
    }
}

/// The append file a mod ships, and the strings in it.
///
/// The parsing and writing are `kotor-formats`' work; this only adds what the
/// editor needs on top — where the file came from, and whether it has unsaved
/// changes.
pub struct AppendFile {
    /// The strings themselves.
    pub tlk: TlkFile,
    /// Where the file lives.
    pub path: Option<String>,
    /// Whether there are changes not yet on disk.
    dirty: bool,
}

impl Default for AppendFile {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendFile {
    /// An empty append file.
    pub fn new() -> Self {
        Self {
            tlk: TlkFile::new_file(),
            path: None,
            dirty: false,
        }
    }

    /// Open an append file.
    pub fn load(path: &str) -> kotor_formats::error::Result<Self> {
        Ok(Self {
            tlk: TlkFile::load(path)?,
            path: Some(path.to_string()),
            dirty: false,
        })
    }

    /// Open the append file sitting beside an instruction file, if there is one.
    pub fn beside(folder: &str, name: &str) -> Option<Self> {
        let path = kotor_formats::text::join_path(folder, name);
        if !kotor_formats::fsutil::file_exists(&path) {
            return None;
        }
        Self::load(&path).ok()
    }

    /// Whether there are changes not yet on disk.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The strings, in file order.
    pub fn entries(&self) -> &[TlkEntry] {
        self.tlk.entries()
    }

    /// The text of one string, or empty when the index is past the end.
    pub fn text(&self, index: u32) -> String {
        self.tlk
            .entry(index as usize)
            .map(|entry| entry.text.clone())
            .unwrap_or_default()
    }

    /// The sound name attached to one string, if it has one.
    pub fn sound(&self, index: u32) -> String {
        self.tlk
            .entry(index as usize)
            .map(TlkEntry::sound_name)
            .unwrap_or_default()
    }

    /// Append a string and return the index it landed on.
    pub fn append(&mut self, text: &str, sound: &str) -> kotor_formats::error::Result<u32> {
        let index = self.tlk.add_entry(entry_for(text, sound))?;
        self.dirty = true;
        Ok(index)
    }

    /// Change a string already in the file.
    pub fn replace(
        &mut self,
        index: u32,
        text: &str,
        sound: &str,
    ) -> kotor_formats::error::Result<()> {
        self.tlk
            .replace_entry(index as usize, entry_for(text, sound))?;
        self.dirty = true;
        Ok(())
    }

    /// Write the file back.
    pub fn save(&mut self) -> kotor_formats::error::Result<()> {
        let path = self.path.clone();
        self.tlk.save(path.as_deref())?;
        if let Some(path) = path {
            self.path = Some(path);
        }
        self.dirty = false;
        Ok(())
    }

    /// Write the file to a chosen place, which becomes its new home.
    pub fn save_as(&mut self, path: &str) -> kotor_formats::error::Result<()> {
        self.tlk.save(Some(path))?;
        self.path = Some(path.to_string());
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mappings_read_from_strref_keys() {
        let ini = IniFile::parse(b"[TLKList]\nStrRef0=0\nStrRef1=4\nStrRef12=7\n");
        let mappings = read(&ini);
        assert_eq!(mappings.len(), 3);
        assert_eq!(mappings[0], TokenMapping { token: 0, index: 0 });
        assert_eq!(
            mappings[2],
            TokenMapping {
                token: 12,
                index: 7
            }
        );
    }

    #[test]
    fn other_spellings_are_left_alone_but_kept() {
        let ini = IniFile::parse(b"[TLKList]\nAppendFile0=extra.tlk\nStrRef0=3\n");
        assert_eq!(read(&ini).len(), 1);
        let foreign = foreign_keys(&ini);
        assert_eq!(foreign.len(), 1);
        assert_eq!(foreign[0].0, "AppendFile0");

        let mut ini = ini;
        let mappings = read(&ini);
        write(&mut ini, &mappings);
        assert_eq!(ini.read_string(TLK_LIST, "AppendFile0", ""), "extra.tlk");
        assert_eq!(ini.read_string(TLK_LIST, "StrRef0", ""), "3");
    }

    #[test]
    fn mappings_survive_a_write_and_read() {
        let mut ini = IniFile::new();
        let mappings = vec![
            TokenMapping { token: 0, index: 0 },
            TokenMapping { token: 5, index: 2 },
        ];
        write(&mut ini, &mappings);
        assert_eq!(read(&ini), mappings);
        assert_eq!(ini.read_section(TLK_LIST), vec!["StrRef0", "StrRef5"]);
    }

    #[test]
    fn new_tokens_avoid_the_ones_in_use() {
        let mappings = vec![
            TokenMapping { token: 0, index: 0 },
            TokenMapping { token: 2, index: 1 },
        ];
        assert_eq!(unused_token(&mappings), 1);

        let mut mappings = mappings;
        assert_eq!(add_mapping(&mut mappings, 9), 1);
        assert_eq!(unused_token(&mappings), 3);
    }

    #[test]
    fn removing_a_mapping_leaves_no_key_behind() {
        let mut ini = IniFile::parse(b"[TLKList]\nStrRef0=0\nStrRef1=1\nStrRef2=2\n");
        let mut mappings = read(&ini);
        mappings.remove(1);
        write(&mut ini, &mappings);
        assert_eq!(ini.read_section(TLK_LIST), vec!["StrRef0", "StrRef2"]);
        assert!(!ini.has_key(TLK_LIST, "StrRef1"));
    }

    #[test]
    fn tokens_keep_their_numbers_rather_than_being_renumbered() {
        // The rest of the file refers to these by number, so a delete must not
        // shuffle the survivors the way a plain list would.
        let mut ini = IniFile::new();
        write(&mut ini, &[TokenMapping { token: 7, index: 0 }]);
        assert_eq!(ini.read_section(TLK_LIST), vec!["StrRef7"]);
    }

    #[test]
    fn appending_to_the_file_returns_the_new_index() {
        let mut file = AppendFile::new();
        assert_eq!(file.append("first", "").unwrap(), 0);
        assert_eq!(file.append("second", "snd").unwrap(), 1);
        assert_eq!(file.text(1), "second");
        assert!(file.is_dirty());
        assert_eq!(file.entries().len(), 2);
    }

    #[test]
    fn replacing_a_string_keeps_the_index() {
        let mut file = AppendFile::new();
        file.append("first", "").unwrap();
        file.replace(0, "changed", "").unwrap();
        assert_eq!(file.text(0), "changed");
        assert_eq!(file.entries().len(), 1);
    }

    #[test]
    fn an_index_past_the_end_reads_as_empty() {
        assert_eq!(AppendFile::new().text(3), "");
    }

    #[test]
    fn append_files_survive_a_save_and_load() {
        let dir = std::env::temp_dir().join("odyedit-tlk-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("append.tlk");
        let path = path.to_str().unwrap();

        let mut file = AppendFile::new();
        file.append("hello", "").unwrap();
        file.save_as(path).unwrap();
        assert!(!file.is_dirty());

        let reopened = AppendFile::load(path).unwrap();
        assert_eq!(reopened.text(0), "hello");
        std::fs::remove_dir_all(&dir).ok();
    }
}
