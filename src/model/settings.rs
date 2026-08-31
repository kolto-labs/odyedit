//! The `[Settings]` section.
//!
//! These decide what the installer looks like and what it checks before it
//! touches anything. Every value has a default, so a file that omits the whole
//! section still installs.

use crate::ini::IniFile;
use crate::model::SETTINGS;

/// Which game a folder lookup should target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Game {
    /// Knights of the Old Republic.
    Kotor1,
    /// Knights of the Old Republic II: The Sith Lords.
    Kotor2,
}

impl Game {
    /// Read the numeric setting, defaulting to the second game.
    pub fn from_setting(value: i32) -> Self {
        if value == 1 {
            Game::Kotor1
        } else {
            Game::Kotor2
        }
    }

    /// The number this game is written as.
    pub fn setting(self) -> i32 {
        match self {
            Game::Kotor1 => 1,
            Game::Kotor2 => 2,
        }
    }

    /// Name shown in the chooser.
    pub fn title(self) -> &'static str {
        match self {
            Game::Kotor1 => "KotOR",
            Game::Kotor2 => "KotOR II",
        }
    }
}

/// One `Required` / `RequiredN` group and the message shown if it is missing.
///
/// Every named file has to be in `override` before the install will start. The
/// usual reason is a mod that only makes sense on top of another one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequiredCheck {
    /// Files that must all be present.
    pub files: Vec<String>,
    /// What to say when any of them is missing.
    pub message: String,
}

/// Values in the `[Settings]` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Install a mod rather than patch files the operator picks by hand.
    pub installer_mode: bool,
    /// Copy files before changing them.
    pub backup_files: bool,
    /// How much detail reaches the log, 0 through 4.
    pub log_level: i32,
    /// Write the install log as plain text instead of styled text.
    pub plaintext_log: bool,
    /// Prompt shown before starting. `N/A` starts without asking.
    pub confirm_message: String,
    /// Window title.
    pub window_caption: String,
    /// Legacy TSLPatcher sentinel, kept so old files round-trip.
    pub file_exists: bool,
    /// Find the game folder from the registry instead of asking.
    pub lookup_game_folder: bool,
    /// Which game to look up.
    pub lookup_game: Game,
    /// Files that must already be in `override`.
    pub required: Vec<RequiredCheck>,
    /// Extra arguments passed to the script compiler.
    pub script_compiler_flags: String,
    /// Keep processed script sources for inspection.
    pub save_processed_scripts: bool,
    /// Compare installed filenames without their extensions.
    pub ignore_extensions: bool,
}

/// Title an installer gets when the file does not name one.
pub const DEFAULT_CAPTION: &str = "Game Data Patcher for KotOR/TSL";

impl Default for Settings {
    fn default() -> Self {
        Self {
            installer_mode: true,
            backup_files: true,
            log_level: 3,
            plaintext_log: false,
            confirm_message: String::new(),
            window_caption: DEFAULT_CAPTION.to_string(),
            file_exists: false,
            lookup_game_folder: false,
            lookup_game: Game::Kotor2,
            required: Vec::new(),
            script_compiler_flags: String::new(),
            save_processed_scripts: false,
            ignore_extensions: false,
        }
    }
}

impl Settings {
    /// Read the settings out of a file.
    pub fn read(ini: &IniFile) -> Self {
        let defaults = Settings::default();
        let caption = ini.read_string(SETTINGS, "WindowCaption", "");
        let window_caption = if caption.trim().is_empty() {
            defaults.window_caption.clone()
        } else {
            caption
        };

        Self {
            installer_mode: ini.read_bool(SETTINGS, "InstallerMode", true),
            backup_files: ini.read_bool(SETTINGS, "BackupFiles", true),
            log_level: ini.read_integer(SETTINGS, "LogLevel", 3).clamp(0, 4),
            plaintext_log: ini.read_bool(SETTINGS, "PlaintextLog", false),
            confirm_message: ini.read_string(SETTINGS, "ConfirmMessage", ""),
            window_caption,
            file_exists: ini.read_bool(SETTINGS, "FileExists", false),
            lookup_game_folder: ini.read_bool(SETTINGS, "LookupGameFolder", false),
            lookup_game: Game::from_setting(ini.read_integer(SETTINGS, "LookupGameNumber", 2)),
            required: read_required(ini),
            script_compiler_flags: ini.read_string(SETTINGS, "ScriptCompilerFlags", ""),
            save_processed_scripts: ini.read_bool(SETTINGS, "SaveProcessedScripts", false),
            ignore_extensions: ini.read_bool(SETTINGS, "IgnoreExtensions", false),
        }
    }

    /// Write the settings back.
    ///
    /// A value left at its default is removed rather than written out. Files
    /// stay as short as their authors wrote them, and a key that is absent means
    /// the same thing as a key holding the default.
    pub fn write(&self, ini: &mut IniFile) {
        let defaults = Settings::default();

        set_bool(
            ini,
            "InstallerMode",
            self.installer_mode,
            defaults.installer_mode,
        );
        set_bool(ini, "BackupFiles", self.backup_files, defaults.backup_files);
        set_int(
            ini,
            "LogLevel",
            self.log_level.clamp(0, 4),
            defaults.log_level,
        );
        set_bool(
            ini,
            "PlaintextLog",
            self.plaintext_log,
            defaults.plaintext_log,
        );
        set_text(ini, "ConfirmMessage", &self.confirm_message, "");
        set_text(
            ini,
            "WindowCaption",
            &self.window_caption,
            &defaults.window_caption,
        );
        set_bool(ini, "FileExists", self.file_exists, defaults.file_exists);
        set_bool(
            ini,
            "LookupGameFolder",
            self.lookup_game_folder,
            defaults.lookup_game_folder,
        );
        set_int(
            ini,
            "LookupGameNumber",
            self.lookup_game.setting(),
            defaults.lookup_game.setting(),
        );
        set_text(ini, "ScriptCompilerFlags", &self.script_compiler_flags, "");
        set_bool(
            ini,
            "SaveProcessedScripts",
            self.save_processed_scripts,
            defaults.save_processed_scripts,
        );
        set_bool(
            ini,
            "IgnoreExtensions",
            self.ignore_extensions,
            defaults.ignore_extensions,
        );

        write_required(ini, &self.required);
    }

    /// True when the operator should not be asked before starting.
    pub fn starts_without_asking(&self) -> bool {
        self.confirm_message == "N/A"
    }
}

/// Write a flag, or drop the key when it matches the default.
fn set_bool(ini: &mut IniFile, key: &str, value: bool, default: bool) {
    if value == default && !ini.has_key(SETTINGS, key) {
        return;
    }
    if value == default {
        ini.delete_key(SETTINGS, key);
        return;
    }
    ini.write_bool(SETTINGS, key, value);
}

/// Write a number, or drop the key when it matches the default.
fn set_int(ini: &mut IniFile, key: &str, value: i32, default: i32) {
    if value == default && !ini.has_key(SETTINGS, key) {
        return;
    }
    if value == default {
        ini.delete_key(SETTINGS, key);
        return;
    }
    ini.write_integer(SETTINGS, key, value);
}

/// Write text, or drop the key when it matches the default.
fn set_text(ini: &mut IniFile, key: &str, value: &str, default: &str) {
    if value == default {
        ini.delete_key(SETTINGS, key);
        return;
    }
    ini.write_string(SETTINGS, key, value);
}

/// The number at the end of `Required` / `Required3`, or `None` for another key.
fn required_suffix(key: &str, stem: &str) -> Option<String> {
    let lower = key.to_ascii_lowercase();
    if lower == stem {
        return Some(String::new());
    }
    let rest = lower.strip_prefix(stem)?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(rest.to_string())
}

/// Read the `Required` groups, pairing each with its message.
fn read_required(ini: &IniFile) -> Vec<RequiredCheck> {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    let mut messages: Vec<(String, String)> = Vec::new();

    for (key, value) in ini.read_section_values(SETTINGS) {
        // `RequiredMsg` also starts with `Required`, so it is checked first.
        if let Some(suffix) = required_suffix(&key, "requiredmsg") {
            // A bare `RequiredMsg=0` means "no message".
            if suffix.is_empty() && (value.trim() == "0" || value.trim().is_empty()) {
                continue;
            }
            messages.push((suffix, value));
            continue;
        }
        if let Some(suffix) = required_suffix(&key, "required") {
            let files: Vec<String> = value
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect();
            if !files.is_empty() {
                groups.push((suffix, files));
            }
        }
    }

    groups
        .into_iter()
        .map(|(suffix, files)| RequiredCheck {
            files,
            message: messages
                .iter()
                .find(|(found, _)| found == &suffix)
                .map(|(_, text)| text.clone())
                .unwrap_or_default(),
        })
        .collect()
}

/// Write the `Required` groups, replacing whatever was there.
///
/// The first group is written unnumbered, as the original tool wrote it, so a
/// file with one check looks the way twenty years of files look.
fn write_required(ini: &mut IniFile, checks: &[RequiredCheck]) {
    for key in ini.read_section(SETTINGS) {
        if required_suffix(&key, "requiredmsg").is_some()
            || required_suffix(&key, "required").is_some()
        {
            ini.delete_key(SETTINGS, &key);
        }
    }

    for (index, check) in checks.iter().enumerate() {
        let files: Vec<&str> = check
            .files
            .iter()
            .map(|f| f.trim())
            .filter(|f| !f.is_empty())
            .collect();
        if files.is_empty() {
            continue;
        }
        let suffix = if index == 0 {
            String::new()
        } else {
            index.to_string()
        };
        ini.write_string(SETTINGS, &format!("Required{suffix}"), &files.join(","));
        if !check.message.trim().is_empty() {
            ini.write_string(SETTINGS, &format!("RequiredMsg{suffix}"), &check.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_reads_as_the_defaults() {
        let settings = Settings::read(&IniFile::new());
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.window_caption, DEFAULT_CAPTION);
    }

    #[test]
    fn values_survive_a_write_and_read() {
        let mut ini = IniFile::new();
        let settings = Settings {
            installer_mode: false,
            backup_files: false,
            log_level: 4,
            plaintext_log: true,
            confirm_message: "Ready?\nProceed.".to_string(),
            window_caption: "My Mod".to_string(),
            lookup_game_folder: true,
            lookup_game: Game::Kotor1,
            script_compiler_flags: "-g".to_string(),
            ..Settings::default()
        };
        settings.write(&mut ini);
        assert_eq!(Settings::read(&ini), settings);
    }

    #[test]
    fn line_breaks_in_the_prompt_survive() {
        let mut ini = IniFile::new();
        Settings {
            confirm_message: "one\ntwo".to_string(),
            ..Settings::default()
        }
        .write(&mut ini);
        let text = String::from_utf8(ini.to_bytes()).unwrap();
        assert!(text.contains("ConfirmMessage=one<#LF#>two"));
        assert_eq!(Settings::read(&ini).confirm_message, "one\ntwo");
    }

    #[test]
    fn defaults_are_not_written_out() {
        let mut ini = IniFile::new();
        Settings::default().write(&mut ini);
        assert!(!ini.has_key(SETTINGS, "InstallerMode"));
        assert!(!ini.has_key(SETTINGS, "WindowCaption"));
        assert!(!ini.has_key(SETTINGS, "LogLevel"));
    }

    #[test]
    fn a_value_returning_to_its_default_removes_the_key() {
        let mut ini = IniFile::parse(b"[Settings]\nLogLevel=4\n");
        assert_eq!(Settings::read(&ini).log_level, 4);
        Settings::default().write(&mut ini);
        assert!(!ini.has_key(SETTINGS, "LogLevel"));
        assert_eq!(Settings::read(&ini).log_level, 3);
    }

    #[test]
    fn required_groups_read_with_their_messages() {
        let ini = IniFile::parse(
            b"[Settings]\nRequired=a.2da, b.2da\nRequiredMsg=Install the base mod.\nRequired2=c.2da\nRequiredMsg2=And that one.\n",
        );
        let checks = Settings::read(&ini).required;
        assert_eq!(checks.len(), 2);
        assert_eq!(checks[0].files, vec!["a.2da", "b.2da"]);
        assert_eq!(checks[0].message, "Install the base mod.");
        assert_eq!(checks[1].files, vec!["c.2da"]);
        assert_eq!(checks[1].message, "And that one.");
    }

    #[test]
    fn the_first_required_group_is_written_unnumbered() {
        let mut ini = IniFile::new();
        Settings {
            required: vec![
                RequiredCheck {
                    files: vec!["a.2da".to_string()],
                    message: "First".to_string(),
                },
                RequiredCheck {
                    files: vec!["b.2da".to_string()],
                    message: String::new(),
                },
            ],
            ..Settings::default()
        }
        .write(&mut ini);

        assert_eq!(ini.read_string(SETTINGS, "Required", ""), "a.2da");
        assert_eq!(ini.read_string(SETTINGS, "RequiredMsg", ""), "First");
        assert_eq!(ini.read_string(SETTINGS, "Required1", ""), "b.2da");
        // A group with no message writes no message key.
        assert!(!ini.has_key(SETTINGS, "RequiredMsg1"));
    }

    #[test]
    fn a_bare_required_msg_of_zero_means_none() {
        let ini = IniFile::parse(b"[Settings]\nRequired=a.2da\nRequiredMsg=0\n");
        let checks = Settings::read(&ini).required;
        assert_eq!(checks.len(), 1);
        assert!(checks[0].message.is_empty());
    }

    #[test]
    fn dropping_a_required_group_removes_its_keys() {
        let mut ini = IniFile::parse(
            b"[Settings]\nRequired=a.2da\nRequiredMsg=x\nRequired2=b.2da\nRequiredMsg2=y\n",
        );
        Settings {
            required: vec![RequiredCheck {
                files: vec!["a.2da".to_string()],
                message: "x".to_string(),
            }],
            ..Settings::default()
        }
        .write(&mut ini);
        assert!(!ini.has_key(SETTINGS, "Required2"));
        assert!(!ini.has_key(SETTINGS, "RequiredMsg2"));
        assert_eq!(Settings::read(&ini).required.len(), 1);
    }
}
