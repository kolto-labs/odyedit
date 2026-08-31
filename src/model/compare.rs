//! Work out the instructions that turn one file into another.
//!
//! An author who has already made their edit by hand has the answer sitting on
//! disk: the file they started from and the file they produced. Comparing the
//! two says what the instructions have to be, which is quicker and more reliable
//! than typing them out from memory.
//!
//! The comparison itself is `kotor-diff`'s work, shared with the query tool that
//! asks the same question from the other end. What belongs here is the part
//! specific to being an editor: folding the result into a document that is
//! already open, without disturbing what is in it.
//!
//! # Why the sections are merged and not pasted
//!
//! `kotor-diff` can render a finished instruction file, but pasting that into an
//! open document would collide with whatever sections were already there,
//! renumber nothing, and rewrite the file as text. The generator also hands over
//! its sections separately, so they go in through the same writer every panel
//! uses and the file keeps its comments and its spacing.
//!
//! # What the generator leaves to the caller
//!
//! The sections it hands over are the ones describing changes to a file. The
//! entry that *lists* that file — `Table0=spells.2da` — is only produced when it
//! renders a whole document, so it is added here instead.
//!
//! Comparing two dialog tables is different again: there is no section to write,
//! only lines to append to the mod's own `append.tlk` and tokens to point at
//! them. Those come back as entries for the caller to add, because the editor
//! already knows how to number tokens and this should not learn separately.

use std::collections::HashMap;

use kotor_diff::changes::ChangesIni;
use kotor_formats::gff::GffFile;
use kotor_formats::ssf::SsfFile;
use kotor_formats::tlk::{TlkEntry, TlkFile};
use kotor_formats::twoda::TwoDaFile;

use crate::ini::IniFile;
use crate::model::{
    unused_section_name, COMPILE_LIST, GFF_LIST, HACK_LIST, INSTALL_LIST, SSF_LIST, TLK_LIST,
    TWODA_LIST,
};

/// Sections that list files rather than describing changes to one.
const LIST_SECTIONS: [&str; 7] = [
    TWODA_LIST,
    GFF_LIST,
    SSF_LIST,
    TLK_LIST,
    COMPILE_LIST,
    INSTALL_LIST,
    HACK_LIST,
];

/// True when a section lists files rather than describing changes to one.
fn is_list_section(name: &str) -> bool {
    LIST_SECTIONS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(name))
}

/// The key a list uses for its entries.
fn list_key(list: &str) -> &'static str {
    if list.eq_ignore_ascii_case(TWODA_LIST) {
        "Table"
    } else {
        "File"
    }
}

/// What a comparison produced.
#[derive(Debug)]
pub struct Generated {
    /// The sections describing the change.
    pub changes: ChangesIni,
    /// The list the compared file belongs in, and the name to list it under.
    ///
    /// `None` for a dialog table, which is not listed by name.
    pub listing: Option<(&'static str, String)>,
    /// Dialog lines the newer table added, for the caller to append.
    pub appended: Vec<TlkEntry>,
}

/// What merging the result into a document did.
#[derive(Debug, Clone, Default)]
pub struct MergeReport {
    /// Sections added to the document.
    pub added_sections: Vec<String>,
    /// Sections that already existed and gained keys.
    pub merged_sections: Vec<String>,
    /// Sections renamed because the name was taken, as `(wanted, given)`.
    pub renamed: Vec<(String, String)>,
    /// Differences the instruction format cannot express.
    pub warnings: Vec<String>,
    /// How many dialog lines are waiting to be appended.
    pub dialog_lines: usize,
}

impl MergeReport {
    /// True when the comparison found nothing to write.
    pub fn is_empty(&self) -> bool {
        self.added_sections.is_empty() && self.merged_sections.is_empty() && self.dialog_lines == 0
    }

    /// One line saying what happened, for the status bar.
    pub fn summary(&self) -> String {
        if self.is_empty() {
            return "The two files are the same. Nothing to add.".to_string();
        }

        let mut text = if self.dialog_lines > 0 {
            format!("Found {} new dialog line(s).", self.dialog_lines)
        } else {
            let sections = self.added_sections.len() + self.merged_sections.len();
            format!("Added instructions across {sections} section(s).")
        };

        if !self.warnings.is_empty() {
            text.push_str(&format!(" {} could not be expressed.", self.warnings.len()));
        }
        text
    }
}

/// Compare two files of the same kind and describe the difference.
///
/// The kind is taken from the filename, since that is what the instruction file
/// will name too. Anything unrecognised is compared as a structured game file,
/// which is what most game data is.
pub fn generate(base_path: &str, modified_path: &str) -> Result<Generated, String> {
    let name = kotor_formats::text::extract_file_name(modified_path);
    let extension = kotor_formats::text::extract_file_ext(modified_path).to_ascii_lowercase();
    let base_extension = kotor_formats::text::extract_file_ext(base_path).to_ascii_lowercase();

    // Two files of different kinds are not two versions of one file. Most such
    // pairs fail to parse and say so, but two different structured formats both
    // parse happily and produce instructions that patch one file using the
    // differences of another.
    if extension != base_extension {
        return Err(format!(
            "Those are different kinds of file: .{base_extension} and .{extension}. \
             Pick the same file before and after an edit."
        ));
    }

    let mut changes = ChangesIni::new();

    match extension.as_str() {
        "2da" => {
            let base = load(base_path, TwoDaFile::load)?;
            let modified = load(modified_path, TwoDaFile::load)?;
            changes.add_twoda(&name, &base, &modified);
            Ok(Generated {
                changes,
                listing: Some((TWODA_LIST, name)),
                appended: Vec::new(),
            })
        }
        "ssf" => {
            let base = load(base_path, SsfFile::load)?;
            let modified = load(modified_path, SsfFile::load)?;
            changes.add_ssf(&name, &base, &modified);
            Ok(Generated {
                changes,
                listing: Some((SSF_LIST, name)),
                appended: Vec::new(),
            })
        }
        "tlk" => {
            let base = load(base_path, TlkFile::load)?;
            let modified = load(modified_path, TlkFile::load)?;
            let appended = changes.add_tlk(&base, &modified);
            Ok(Generated {
                changes,
                listing: None,
                appended,
            })
        }
        _ => {
            let base = load(base_path, GffFile::load)?;
            let modified = load(modified_path, GffFile::load)?;
            changes.add_gff(&name, &base, &modified);
            Ok(Generated {
                changes,
                listing: Some((GFF_LIST, name)),
                appended: Vec::new(),
            })
        }
    }
}

/// Read a file, naming it in the error so the operator knows which one failed.
fn load<T, F>(path: &str, read: F) -> Result<T, String>
where
    F: Fn(&str) -> kotor_formats::error::Result<T>,
{
    read(path).map_err(|error| format!("{path}: {error}"))
}

/// Fold generated instructions into a document that is already open.
///
/// Sections naming a file are merged with whatever the document already says
/// about that file. Sections describing one change are given a fresh name if
/// theirs is taken, and every reference to them is updated to match.
pub fn merge_into(ini: &mut IniFile, generated: &Generated) -> MergeReport {
    let mut report = MergeReport {
        warnings: generated.changes.warnings().to_vec(),
        dialog_lines: generated.appended.len(),
        ..MergeReport::default()
    };

    // The file's own section keeps its name: it is named after the file, and
    // renaming it would point the instructions at something that does not exist.
    let named_after_file = generated
        .listing
        .as_ref()
        .map(|(_, name)| name.clone())
        .unwrap_or_default();

    // Work every name out before writing anything, so a rename is reflected in
    // the section referring to it however the two are ordered.
    let mut names: HashMap<String, String> = HashMap::new();
    for section in generated.changes.sections() {
        let wanted = &section.name;
        let keeps_name = is_list_section(wanted) || wanted.eq_ignore_ascii_case(&named_after_file);

        let given = if keeps_name || !ini.section_present(wanted) {
            wanted.clone()
        } else {
            let fresh = unused_section_name(ini, wanted);
            report.renamed.push((wanted.clone(), fresh.clone()));
            fresh
        };

        names.insert(wanted.clone(), given);
    }

    for section in generated.changes.sections() {
        let target = names
            .get(&section.name)
            .cloned()
            .unwrap_or_else(|| section.name.clone());
        let existed = ini.section_present(&target);

        for (key, value) in &section.entries {
            // A value naming another generated section follows it to its new name.
            let value = names.get(value).cloned().unwrap_or_else(|| value.clone());
            let key = next_free_key(ini, &target, key);
            ini.write_string(&target, &key, &value);
        }

        if existed {
            report.merged_sections.push(target);
        } else {
            report.added_sections.push(target);
        }
    }

    // The entry that lists the file is only produced when the generator renders
    // a whole document, so it is added here — and only when the document does
    // not already name the file, or a patcher would open and rewrite it twice.
    if let Some((list, filename)) = &generated.listing {
        if !report.is_empty() && !lists_file(ini, list, filename) {
            let key = next_free_key(ini, list, &format!("{}0", list_key(list)));
            ini.write_string(list, &key, filename);
            if !report.added_sections.iter().any(|s| s == list) {
                report.merged_sections.push((*list).to_string());
            }
        }
    }

    report
}

/// True when a list already names this file.
fn lists_file(ini: &IniFile, list: &str, filename: &str) -> bool {
    ini.read_section_values(list)
        .into_iter()
        .any(|(key, value)| !key.starts_with('!') && value.eq_ignore_ascii_case(filename))
}

/// A key that does not collide with one the section already has.
///
/// Numbered keys are the only ones that can repeat: `AddRow0` from a comparison
/// means "another added row", not "replace the one already there". A key with no
/// number keeps its name and overwrites, which is what `!Destination` and a
/// column name both want.
fn next_free_key(ini: &IniFile, section: &str, key: &str) -> String {
    let Some((stem, _)) = split_numbered(key) else {
        return key.to_string();
    };

    let mut index = 0;
    loop {
        let candidate = format!("{stem}{index}");
        if !ini.has_key(section, &candidate) {
            return candidate;
        }
        index += 1;
    }
}

/// Split a key such as `AddRow3` into its name and its number.
fn split_numbered(key: &str) -> Option<(&str, u32)> {
    let digits = key.len() - key.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let split = key.len() - digits;
    let stem = &key[..split];
    if stem.is_empty() {
        return None;
    }
    key[split..].parse().ok().map(|number| (stem, number))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kotor_formats::twoda::build;

    fn generated_for(base: &TwoDaFile, modified: &TwoDaFile) -> Generated {
        let mut changes = ChangesIni::new();
        changes.add_twoda("spells.2da", base, modified);
        Generated {
            changes,
            listing: Some((TWODA_LIST, "spells.2da".to_string())),
            appended: Vec::new(),
        }
    }

    fn pair() -> (TwoDaFile, TwoDaFile) {
        (
            build(&["A"], &[("0", vec!["a0"])]),
            build(&["A"], &[("0", vec!["a0"]), ("1", vec!["a1"])]),
        )
    }

    #[test]
    fn numbered_keys_are_told_from_plain_ones() {
        assert_eq!(split_numbered("AddRow3"), Some(("AddRow", 3)));
        assert_eq!(split_numbered("Table0"), Some(("Table", 0)));
        assert_eq!(split_numbered("!Destination"), None);
        assert_eq!(split_numbered("label"), None);
        // A key that is only digits has no name to keep.
        assert_eq!(split_numbered("123"), None);
    }

    #[test]
    fn an_identical_pair_produces_nothing() {
        let table = build(&["A"], &[("0", vec!["a"])]);
        let generated = generated_for(&table, &table);

        let mut ini = IniFile::new();
        let report = merge_into(&mut ini, &generated);
        assert!(report.is_empty());
        assert!(report.summary().contains("the same"));
        assert!(ini.section_names().is_empty());
    }

    #[test]
    fn a_new_row_becomes_instructions_in_an_empty_document() {
        let (base, modified) = pair();
        let generated = generated_for(&base, &modified);

        let mut ini = IniFile::new();
        let report = merge_into(&mut ini, &generated);

        assert!(!report.is_empty());
        assert_eq!(ini.read_string(TWODA_LIST, "Table0", ""), "spells.2da");
        assert!(ini.section_exists("spells.2da"));

        // The file's section names a modifier, and that modifier exists.
        let modifier = ini.read_string("spells.2da", "AddRow0", "");
        assert!(!modifier.is_empty());
        assert_eq!(ini.read_string(&modifier, "A", ""), "a1");
    }

    #[test]
    fn merging_into_a_document_that_already_patches_the_file_appends() {
        let (base, modified) = pair();
        let generated = generated_for(&base, &modified);

        let mut ini = IniFile::parse(
            b"[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=existing\n\n[existing]\nlabel=X\n",
        );
        let report = merge_into(&mut ini, &generated);

        // The existing instruction is untouched and the new one sits after it.
        assert_eq!(ini.read_string("spells.2da", "AddRow0", ""), "existing");
        let added = ini.read_string("spells.2da", "AddRow1", "");
        assert!(!added.is_empty());
        assert!(ini.section_exists(&added));
        assert!(report.merged_sections.iter().any(|s| s == "spells.2da"));
    }

    #[test]
    fn the_file_is_listed_once_however_often_it_is_compared() {
        let (base, modified) = pair();

        let mut ini = IniFile::parse(b"[2DAList]\nTable0=spells.2da\n");
        merge_into(&mut ini, &generated_for(&base, &modified));
        merge_into(&mut ini, &generated_for(&base, &modified));

        // A second Table key would have the patcher open and rewrite the file
        // twice, applying the same additions again.
        let listed = ini
            .read_section_values(TWODA_LIST)
            .into_iter()
            .filter(|(_, value)| value == "spells.2da")
            .count();
        assert_eq!(listed, 1, "the file was listed more than once");
    }

    #[test]
    fn the_file_is_listed_when_the_document_did_not_know_about_it() {
        let (base, modified) = pair();
        let mut ini = IniFile::parse(b"[2DAList]\nTable0=other.2da\n");
        merge_into(&mut ini, &generated_for(&base, &modified));

        assert_eq!(ini.read_string(TWODA_LIST, "Table0", ""), "other.2da");
        assert_eq!(ini.read_string(TWODA_LIST, "Table1", ""), "spells.2da");
    }

    #[test]
    fn a_clashing_modifier_name_is_renamed_and_its_reference_follows() {
        let (base, modified) = pair();
        let generated = generated_for(&base, &modified);

        // Find the name the generator wants, then occupy it.
        let wanted = generated
            .changes
            .sections()
            .iter()
            .map(|s| s.name.clone())
            .find(|name| name != "spells.2da")
            .expect("a modifier section");

        let mut ini = IniFile::parse(format!("[{wanted}]\nAlreadyHere=1\n").as_bytes());
        let report = merge_into(&mut ini, &generated);

        assert_eq!(report.renamed.len(), 1);
        let (from, to) = &report.renamed[0];
        assert_eq!(from, &wanted);
        assert_ne!(to, &wanted);

        // The section referring to it points at the new name, not the old one.
        assert_eq!(ini.read_string("spells.2da", "AddRow0", ""), *to);
        // And the section that was already there kept its contents.
        assert_eq!(ini.read_string(&wanted, "AlreadyHere", ""), "1");
    }

    #[test]
    fn merging_leaves_the_rest_of_the_document_alone() {
        let (base, modified) = pair();
        let mut ini = IniFile::parse(
            b"; a comment\r\n[Settings]\r\nWindowCaption=Mine\r\n\r\n[CompileList]\r\nFile0=a.nss\r\n",
        );
        merge_into(&mut ini, &generated_for(&base, &modified));

        let out = String::from_utf8(ini.to_bytes()).unwrap();
        assert!(out.contains("; a comment\r\n"));
        assert!(out.contains("WindowCaption=Mine\r\n"));
        assert!(out.contains("File0=a.nss\r\n"));
        assert!(out.contains("[2DAList]"));
        // The generator's own Settings block is not pasted over the operator's.
        assert_eq!(ini.read_string("Settings", "WindowCaption", ""), "Mine");
        assert!(!ini.has_key("Settings", "FileExists"));
    }

    #[test]
    fn unrepresentable_differences_are_carried_through_as_warnings() {
        // A deleted row cannot be expressed, and the generator says so.
        let base = build(&["A"], &[("0", vec!["a"]), ("1", vec!["b"])]);
        let modified = build(&["A"], &[("0", vec!["a"])]);
        let generated = generated_for(&base, &modified);

        let mut ini = IniFile::new();
        let report = merge_into(&mut ini, &generated);
        assert_eq!(report.warnings, generated.changes.warnings().to_vec());
    }

    #[test]
    fn dialog_lines_are_handed_back_rather_than_written_as_sections() {
        // A dialog table has no section to merge: the lines go into the mod's
        // own append file, and the tokens are the editor's to number.
        let mut base = TlkFile::new_file();
        base.add_entry(TlkEntry {
            flags: 1,
            text: "one".into(),
            ..TlkEntry::default()
        })
        .unwrap();
        let mut modified = base.clone();
        modified
            .add_entry(TlkEntry {
                flags: 1,
                text: "two".into(),
                ..TlkEntry::default()
            })
            .unwrap();

        let mut changes = ChangesIni::new();
        let appended = changes.add_tlk(&base, &modified);
        let generated = Generated {
            changes,
            listing: None,
            appended,
        };

        let mut ini = IniFile::new();
        let report = merge_into(&mut ini, &generated);

        assert_eq!(report.dialog_lines, 1);
        assert!(!report.is_empty());
        assert!(report.summary().contains("dialog line"));
        assert_eq!(generated.appended[0].text, "two");
    }

    #[test]
    fn two_different_kinds_of_file_are_refused() {
        // Both parse as GFF, so nothing downstream would notice: the result
        // would be instructions patching one item with another item's changes.
        let error = generate("/nonexistent/a.uti", "/nonexistent/b.utc").unwrap_err();
        assert!(error.contains("different kinds"));
    }

    #[test]
    fn an_unreadable_file_is_reported_rather_than_panicking() {
        let error = generate("/nonexistent/a.2da", "/nonexistent/b.2da").unwrap_err();
        assert!(error.contains("/nonexistent/"));
    }
}
