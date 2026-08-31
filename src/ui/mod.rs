//! The window.
//!
//! A list of sections down the left, the chosen section's editor on the right,
//! a status line along the bottom. That is the shape the original tool had, and
//! it is still the right one: the eight sections have nothing to say to each
//! other, so there is no reason to show two at once.
//!
//! Every panel edits a typed buffer held here and writes it straight back to the
//! document. Nothing is queued up for later, so what the file would contain and
//! what the panels show never drift apart.

mod gff;
mod install;
mod namespace;
mod script;
mod settings;
mod ssf;
mod tlk;
mod twoda;
mod widgets;

use eframe::egui;

use crate::model::gff::GffEntry;
use crate::model::install::InstallFolder;
use crate::model::namespace::NamespaceFile;
use crate::model::script::ScriptEntry;
use crate::model::settings::Settings;
use crate::model::ssf::SsfEntry;
use crate::model::tlk::{AppendFile, TokenMapping, DEFAULT_APPEND_FILE};
use crate::model::twoda::TableEntry;
use crate::model::{ChangesFile, Pane};

/// Name shown in the title bar and the About box.
pub const APP_NAME: &str = "OdyEdit";
/// Version shown beside the name.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Something the operator has to answer before anything else happens.
enum Dialog {
    /// Explains what the program is.
    About,
    /// Warns that something has changes, and asks what to do.
    ///
    /// The action waiting on the answer is carried along so it can run if the
    /// operator decides to go ahead. The second field says why the question is
    /// being asked again, when it is.
    Unsaved(PendingAction, Option<String>),
    /// Reports something that went wrong.
    Problem(String),
}

/// What to do once an unsaved-changes warning is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingAction {
    /// Start a new file.
    New,
    /// Open a different file.
    Open,
    /// Close the window.
    Quit,
}

/// The editor.
pub struct App {
    /// The open instruction file.
    file: ChangesFile,
    /// Which section is showing.
    pane: Pane,

    /// The `[Settings]` values being edited.
    settings: Settings,
    /// Dialog string tokens.
    tokens: Vec<TokenMapping>,
    /// The append file the tokens point into, once it has been found.
    append: Option<AppendFile>,
    /// Tables and their changes.
    tables: Vec<TableEntry>,
    /// Game files and their changes.
    gff: Vec<GffEntry>,
    /// Scripts to compile.
    scripts: Vec<ScriptEntry>,
    /// Destinations and the files going into them.
    installs: Vec<InstallFolder>,
    /// Soundsets and their changes.
    ssfs: Vec<SsfEntry>,
    /// Install variants, which live in their own file.
    namespaces: NamespaceFile,

    /// Which row each list panel has selected.
    selection: Selection,
    /// What the status line is showing.
    status: String,
    /// A question waiting on an answer.
    dialog: Option<Dialog>,
    /// An action held up behind a problem the operator has to read first.
    pending: Option<PendingAction>,
    /// A section name being typed, before the rename is applied.
    rename_buffer: String,
}

/// Which row each list panel has selected.
#[derive(Debug, Default, Clone, Copy)]
struct Selection {
    /// Chosen table, and the modifier inside it.
    table: usize,
    /// Chosen modifier within the chosen table.
    modifier: Option<usize>,
    /// Chosen game file, and the field inside it.
    gff: usize,
    /// Chosen script.
    script: usize,
    /// Chosen destination folder.
    install: usize,
    /// Chosen soundset.
    ssf: usize,
    /// Chosen dialog string token.
    token: Option<usize>,
    /// Chosen install variant.
    namespace: usize,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// An editor with an empty file open.
    pub fn new() -> Self {
        let mut app = Self {
            file: ChangesFile::new(),
            pane: Pane::Settings,
            settings: Settings::default(),
            tokens: Vec::new(),
            append: None,
            tables: Vec::new(),
            gff: Vec::new(),
            scripts: Vec::new(),
            installs: Vec::new(),
            ssfs: Vec::new(),
            namespaces: NamespaceFile::new(),
            selection: Selection::default(),
            status: "Ready.".to_string(),
            dialog: None,
            pending: None,
            rename_buffer: String::new(),
        };
        app.reload_from_file();
        app
    }

    /// An editor with a file already open, for starting from the command line.
    pub fn with_file(path: &str) -> Self {
        let mut app = Self::new();
        app.open_path(path);
        app
    }

    /// Read every panel's buffer back out of the document.
    ///
    /// Called after opening a file, and after anything that rewrites the
    /// document wholesale.
    fn reload_from_file(&mut self) {
        self.settings = Settings::read(&self.file.ini);
        self.tokens = crate::model::tlk::read(&self.file.ini);
        self.tables = crate::model::twoda::read(&self.file.ini);
        self.gff = crate::model::gff::read(&self.file.ini);
        self.scripts = crate::model::script::read(&self.file.ini);
        self.installs = crate::model::install::read(&self.file.ini);
        self.ssfs = crate::model::ssf::read(&self.file.ini);
        self.selection = Selection::default();
        self.rename_buffer.clear();

        // The append file and the variants live beside the instruction file, so
        // they can only be found once it has a home.
        let folder = self.file.folder();
        self.append = folder
            .as_deref()
            .and_then(|folder| AppendFile::beside(folder, DEFAULT_APPEND_FILE));
        self.namespaces = folder
            .as_deref()
            .and_then(NamespaceFile::beside)
            .unwrap_or_default();
    }

    /// Whether the open instruction file has changes not yet on disk.
    pub fn is_dirty(&self) -> bool {
        self.file.is_dirty()
    }

    /// Whether anything at all is unsaved.
    ///
    /// A session edits up to three files — the instructions, the append table
    /// they point into, and the variants list — and each is saved separately.
    /// Asking only about the instruction file would let the other two be thrown
    /// away without a word.
    fn anything_unsaved(&self) -> bool {
        self.file.is_dirty()
            || self.append.as_ref().is_some_and(AppendFile::is_dirty)
            || self.namespaces.is_dirty()
    }

    /// The names of whatever is unsaved, for the status line to show.
    fn unsaved_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if self.file.is_dirty() {
            names.push(self.file.display_name());
        }
        if self.append.as_ref().is_some_and(AppendFile::is_dirty) {
            names.push(DEFAULT_APPEND_FILE.to_string());
        }
        if self.namespaces.is_dirty() {
            names.push(crate::model::namespace::FILENAME.to_string());
        }
        names
    }

    /// Write every file that has changes. Reports what could not be written.
    ///
    /// Only files that were actually edited are written. Asking for a name for
    /// an untitled instruction file nobody touched — because the append table
    /// beside it was the thing that changed — would be a dialog out of nowhere.
    fn save_everything(&mut self) {
        if self.file.is_dirty() {
            self.save_file();
        }
        if self.append.as_ref().is_some_and(AppendFile::is_dirty) {
            self.save_append();
        }
        if self.namespaces.is_dirty() {
            self.save_namespaces();
        }
    }

    /// Note that the document changed.
    fn touch(&mut self) {
        self.file.touch();
    }

    /// Say something on the status line.
    fn say(&mut self, message: impl Into<String>) {
        self.status = message.into();
    }

    /// Report a problem the operator has to see.
    fn complain(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status = message.clone();
        self.dialog = Some(Dialog::Problem(message));
    }

    /// Title bar text, marked when there are unsaved changes.
    fn title(&self) -> String {
        let mark = if self.file.is_dirty() { " *" } else { "" };
        format!("{APP_NAME} — {}{mark}", self.file.display_name())
    }

    // -- file actions ----------------------------------------------------

    /// Start a new file, asking first if the open one has changes.
    fn new_file(&mut self) {
        if self.guard(PendingAction::New) {
            return;
        }
        self.file = ChangesFile::new();
        self.reload_from_file();
        self.say("Started a new file.");
    }

    /// Open a file, asking first if the open one has changes.
    fn open_file(&mut self) {
        if self.guard(PendingAction::Open) {
            return;
        }
        self.choose_and_open();
    }

    /// Ask for a file and open it, without warning about unsaved changes.
    ///
    /// Used once the warning has already been answered. A dismissed dialog
    /// leaves everything exactly as it was.
    fn choose_and_open(&mut self) {
        let Some(path) = pick_file("Open instruction file", &["ini"], self.file.folder()) else {
            self.say("No file chosen. Nothing was opened.");
            return;
        };
        self.open_path(&path);
    }

    /// Open a named file.
    fn open_path(&mut self, path: &str) {
        match ChangesFile::load(path) {
            Ok(file) => {
                self.file = file;
                self.reload_from_file();
                self.say(format!("Opened {}.", self.file.display_name()));
            }
            Err(error) => self.complain(format!("Could not open {path}: {error}")),
        }
    }

    /// Save the file, asking for a name if it does not have one.
    fn save_file(&mut self) {
        if self.file.path.is_none() {
            self.save_file_as();
            return;
        }
        match self.file.save() {
            Ok(()) => self.say(format!("Saved {}.", self.file.display_name())),
            Err(error) => self.complain(format!("Could not save: {error}")),
        }
    }

    /// Save the file under a chosen name.
    fn save_file_as(&mut self) {
        let Some(path) = save_file_dialog(
            "Save instruction file",
            &["ini"],
            self.file.folder(),
            "changes.ini",
        ) else {
            return;
        };
        match self.file.save_as(&path) {
            Ok(()) => {
                // The append file and the variants are found relative to the
                // instruction file, so a new home means looking again.
                self.reload_side_files();
                self.say(format!("Saved {}.", self.file.display_name()));
            }
            Err(error) => self.complain(format!("Could not save: {error}")),
        }
    }

    /// Look again for the files that sit beside the instruction file.
    fn reload_side_files(&mut self) {
        let Some(folder) = self.file.folder() else {
            return;
        };
        if self.append.is_none() {
            self.append = AppendFile::beside(&folder, DEFAULT_APPEND_FILE);
        }
        if self.namespaces.path.is_none() {
            if let Some(found) = NamespaceFile::beside(&folder) {
                self.namespaces = found;
            }
        }
    }

    /// Warn about unsaved changes. True when the caller should stop and wait.
    fn guard(&mut self, action: PendingAction) -> bool {
        if !self.anything_unsaved() {
            return false;
        }
        self.dialog = Some(Dialog::Unsaved(action, None));
        true
    }

    /// Carry out what an unsaved-changes warning was holding up.
    fn resume(&mut self, action: PendingAction, ctx: &egui::Context) {
        match action {
            PendingAction::New => {
                // A new file replaces all three straight away, which is what was
                // asked for.
                self.file = ChangesFile::new();
                self.append = None;
                self.namespaces = NamespaceFile::new();
                self.reload_from_file();
                self.say("Started a new file.");
            }
            PendingAction::Open => {
                // Nothing is thrown away until there is a replacement to put in
                // its place. Clearing first and then having the file dialog
                // dismissed would leave the window holding nothing, with the
                // document that was open gone from memory.
                self.choose_and_open();
            }
            PendingAction::Quit => {
                // The close request is checked again on the way out, so the
                // flags have to come down or the warning fires forever.
                self.discard_changes();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Give up on every unsaved change without touching what is on screen.
    fn discard_changes(&mut self) {
        self.file = ChangesFile::new();
        self.append = None;
        self.namespaces = NamespaceFile::new();
    }

    // -- drawing ---------------------------------------------------------

    /// The menu bar along the top.
    fn show_menu(&mut self, ctx: &egui::Context) {
        let mut action: Option<MenuAction> = None;

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        action = Some(MenuAction::New);
                        ui.close();
                    }
                    if ui.button("Open").clicked() {
                        action = Some(MenuAction::Open);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Save").clicked() {
                        action = Some(MenuAction::Save);
                        ui.close();
                    }
                    if ui.button("Save as").clicked() {
                        action = Some(MenuAction::SaveAs);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        action = Some(MenuAction::Quit);
                        ui.close();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button(format!("About {APP_NAME}")).clicked() {
                        action = Some(MenuAction::About);
                        ui.close();
                    }
                });
            });
        });

        match action {
            Some(MenuAction::New) => self.new_file(),
            Some(MenuAction::Open) => self.open_file(),
            Some(MenuAction::Save) => self.save_file(),
            Some(MenuAction::SaveAs) => self.save_file_as(),
            Some(MenuAction::About) => self.dialog = Some(Dialog::About),
            Some(MenuAction::Quit) if !self.guard(PendingAction::Quit) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(MenuAction::Quit) => {}
            None => {}
        }
    }

    /// The section list down the left.
    fn show_sections(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sections")
            .resizable(true)
            .default_width(190.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("Sections");
                ui.add_space(4.0);

                for pane in Pane::ALL {
                    let count = self.count_for(pane);
                    let label = match count {
                        Some(n) => format!("{}  ({n})", pane.title()),
                        None => pane.title().to_string(),
                    };
                    if ui
                        .selectable_label(self.pane == pane, label)
                        .on_hover_text(pane.summary())
                        .clicked()
                    {
                        self.pane = pane;
                    }
                }
            });
    }

    /// How many entries a section holds, for the count beside its name.
    fn count_for(&self, pane: Pane) -> Option<usize> {
        match pane {
            Pane::Settings => None,
            Pane::Tlk => Some(self.tokens.len()),
            Pane::TwoDa => Some(self.tables.len()),
            Pane::Gff => Some(self.gff.len()),
            Pane::Script => Some(self.scripts.len()),
            Pane::Install => Some(self.installs.len()),
            Pane::Ssf => Some(self.ssfs.len()),
            Pane::Namespace => Some(self.namespaces.entries.len()),
        }
    }

    /// The status line along the bottom.
    fn show_status(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let unsaved = self.unsaved_names();
                    if !unsaved.is_empty() {
                        ui.label(format!("Unsaved: {}", unsaved.join(", ")));
                    }
                });
            });
        });
    }

    /// Whichever question is waiting on an answer.
    fn show_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let mut keep = true;

        match dialog {
            Dialog::About => {
                egui::Window::new(format!("About {APP_NAME}"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("{APP_NAME} {VERSION}"));
                        ui.add_space(6.0);
                        ui.label("Edits the changes.ini files that mod installers read.");
                        ui.label("A rewrite of ChangeEdit, which stopped at Delphi 7.");
                        ui.add_space(8.0);
                        if ui.button("Close").clicked() {
                            keep = false;
                        }
                    });
                if keep {
                    self.dialog = Some(Dialog::About);
                }
            }

            Dialog::Problem(message) => {
                egui::Window::new("Something went wrong")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(&message);
                        ui.add_space(8.0);
                        if ui.button("Close").clicked() {
                            keep = false;
                        }
                    });
                if keep {
                    self.dialog = Some(Dialog::Problem(message));
                } else if let Some(action) = self.pending.take() {
                    // Something was waiting on a save that did not happen.
                    self.dialog = Some(Dialog::Unsaved(action, None));
                }
            }

            Dialog::Unsaved(action, note) => {
                let unsaved = self.unsaved_names().join(", ");
                let mut choice: Option<UnsavedChoice> = None;
                egui::Window::new("Save your changes?")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        if let Some(note) = &note {
                            ui.label(note);
                            ui.add_space(4.0);
                        }
                        ui.label(format!("{unsaved} is not on disk yet."));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("Save").clicked() {
                                choice = Some(UnsavedChoice::Save);
                            }
                            if ui.button("Discard").clicked() {
                                choice = Some(UnsavedChoice::Discard);
                            }
                            if ui.button("Cancel").clicked() {
                                choice = Some(UnsavedChoice::Cancel);
                            }
                        });
                    });

                match choice {
                    Some(UnsavedChoice::Save) => {
                        self.save_everything();
                        if self.anything_unsaved() {
                            // Going ahead now would lose exactly what was asked
                            // to be kept, so the warning stays. It must say why,
                            // or the same question comes back with no visible
                            // cause — which is what happens when the operator
                            // dismisses the file dialog rather than choosing a
                            // name, since that raises no error of its own.
                            match self.dialog.take() {
                                Some(problem) => {
                                    self.pending = Some(action);
                                    self.dialog = Some(problem);
                                }
                                None => {
                                    self.dialog = Some(Dialog::Unsaved(
                                        action,
                                        Some(
                                            "Nothing was written — no filename was chosen."
                                                .to_string(),
                                        ),
                                    ))
                                }
                            }
                        } else {
                            self.resume(action, ctx);
                        }
                    }
                    Some(UnsavedChoice::Discard) => self.resume(action, ctx),
                    Some(UnsavedChoice::Cancel) => {}
                    None => self.dialog = Some(Dialog::Unsaved(action, note)),
                }
            }
        }
    }
}

/// What the operator chose in the unsaved-changes warning.
#[derive(Debug, Clone, Copy)]
enum UnsavedChoice {
    /// Write the file, then go ahead.
    Save,
    /// Go ahead and lose the changes.
    Discard,
    /// Stay where we are.
    Cancel,
}

/// Something picked from the menu bar.
#[derive(Debug, Clone, Copy)]
enum MenuAction {
    /// Start a new file.
    New,
    /// Open a file.
    Open,
    /// Save the file.
    Save,
    /// Save the file under a new name.
    SaveAs,
    /// Show what the program is.
    About,
    /// Close the window.
    Quit,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));

        self.show_menu(ctx);
        self.show_sections(ctx);
        self.show_status(ctx);

        egui::CentralPanel::default().show(ctx, |ui| match self.pane {
            Pane::Settings => self.show_settings(ui),
            Pane::Tlk => self.show_tlk(ui),
            Pane::TwoDa => self.show_twoda(ui),
            Pane::Gff => self.show_gff(ui),
            Pane::Script => self.show_script(ui),
            Pane::Install => self.show_install(ui),
            Pane::Ssf => self.show_ssf(ui),
            Pane::Namespace => self.show_namespace(ui),
        });

        self.show_dialog(ctx);

        // The window manager's close button goes through the same warning the
        // Quit menu item does.
        if ctx.input(|i| i.viewport().close_requested()) && self.file.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.guard(PendingAction::Quit);
        }
    }
}

/// Ask for a file to open.
fn pick_file(title: &str, extensions: &[&str], folder: Option<String>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if !extensions.is_empty() {
        dialog = dialog.add_filter("Instruction files", extensions);
    }
    if let Some(folder) = folder {
        dialog = dialog.set_directory(folder);
    }
    dialog
        .pick_file()
        .map(|path| path.to_string_lossy().into_owned())
}

/// Ask where to write a file.
fn save_file_dialog(
    title: &str,
    extensions: &[&str],
    folder: Option<String>,
    suggested: &str,
) -> Option<String> {
    let mut dialog = rfd::FileDialog::new()
        .set_title(title)
        .set_file_name(suggested);
    if !extensions.is_empty() {
        dialog = dialog.add_filter("Instruction files", extensions);
    }
    if let Some(folder) = folder {
        dialog = dialog.set_directory(folder);
    }
    dialog
        .save_file()
        .map(|path| path.to_string_lossy().into_owned())
}

/// Ask for a file of any kind, used where the mod names its own data files.
fn pick_any_file(title: &str, folder: Option<String>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(folder) = folder {
        dialog = dialog.set_directory(folder);
    }
    dialog
        .pick_file()
        .map(|path| path.to_string_lossy().into_owned())
}
