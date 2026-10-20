//! Biblioteka projektów (zapisywanie/otwieranie) i własne ogniwa.

use crate::app::{cell_editor, PakieciarkaApp, Project};
use crate::platform::{self, OpenKind};
use crate::store::{self, SavedProject};
use egui::{Color32, Key, KeyboardShortcut, Modifiers, RichText, Ui};
use pakiet_core::cells::Cell;

/// Akcja odkładana do potwierdzenia, gdy są niezapisane zmiany.
#[derive(Clone, Debug)]
pub enum Pending {
    New,
    Open(String),
    Import(Box<Project>),
}

pub struct CellEdit {
    /// Indeks własnego ogniwa; `None` = nowe
    pub target: Option<usize>,
    pub cell: Cell,
    pub error: String,
}

const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);

fn summary(p: &Project) -> String {
    format!(
        "{}S{}P · {} · {:.0} Wh",
        p.s,
        p.p,
        p.cell.name,
        p.s as f64 * p.cell.nominal_v * p.p as f64 * p.cell.capacity_mah / 1000.0
    )
}

impl PakieciarkaApp {
    // ---- stan projektu -------------------------------------------------

    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.project)
    }

    pub fn refresh_library(&mut self) {
        self.library = store::list_projects();
    }

    fn set_status(&mut self, r: Result<String, String>) {
        self.status = match r {
            Ok(m) => format!("✔ {m}"),
            Err(e) => format!("✖ {e}"),
        };
    }

    pub fn save_current(&mut self) {
        let id = self.current_id.clone().unwrap_or_else(store::new_id);
        let sp = SavedProject { id: id.clone(), modified_ms: store::now_ms(), project: self.project.clone() };
        match store::save_project(&sp) {
            Ok(()) => {
                self.current_id = Some(id);
                self.saved = Some(self.project.clone());
                self.refresh_library();
                self.status = format!("✔ zapisano projekt „{}”", self.project.name);
            }
            Err(e) => self.status = format!("✖ nie udało się zapisać: {e}"),
        }
    }

    fn save_as_new(&mut self) {
        if self.library.iter().any(|s| s.project.name == self.project.name) {
            self.project.name = format!("{} (kopia)", self.project.name);
        }
        self.current_id = None;
        self.save_current();
    }

    /// Wykonuje akcję albo pyta o niezapisane zmiany.
    pub fn request(&mut self, action: Pending) {
        if self.dirty() {
            self.pending = Some(action);
        } else {
            self.perform(action);
        }
    }

    fn perform(&mut self, action: Pending) {
        match action {
            Pending::New => {
                self.project = Project::default();
                self.current_id = None;
                self.saved = Some(self.project.clone());
                self.status = "nowy projekt".into();
            }
            Pending::Open(id) => {
                self.refresh_library();
                if let Some(sp) = self.library.iter().find(|s| s.id == id).cloned() {
                    self.project = sp.project.clone();
                    self.saved = Some(sp.project);
                    self.current_id = Some(sp.id);
                    self.status = format!("✔ otwarto „{}”", self.project.name);
                    self.projects_window = false;
                } else {
                    self.status = "✖ projekt nie istnieje".into();
                }
            }
            Pending::Import(p) => {
                self.project = *p;
                self.current_id = None;
                self.saved = None;
                self.status = format!("✔ zaimportowano „{}” — zapisz, aby dodać do biblioteki", self.project.name);
            }
        }
    }

    pub fn export_project(&mut self, p: &Project) {
        let json = serde_json::to_string_pretty(p).unwrap_or_default();
        let name = format!("{}.json", platform::file_stem(&p.name));
        match platform::save_file(&name, &json, platform::JSON) {
            Ok(Some(m)) => self.status = format!("✔ {m}"),
            Ok(None) => {}
            Err(e) => self.status = format!("✖ {e}"),
        }
    }

    pub fn open_dialog(&mut self, kind: OpenKind, ctx: &egui::Context) {
        if let Err(e) = platform::open_file(kind, &self.inbox, ctx) {
            self.status = format!("✖ {e}");
        }
    }

    /// Obsługa wczytanych plików i skrótów klawiszowych — wołane co klatkę.
    pub fn handle_events(&mut self, ctx: &egui::Context) {
        let files: Vec<_> = self.inbox.borrow_mut().drain(..).collect();
        for f in files {
            match f.kind {
                OpenKind::Project => match store::parse_project(&f.content) {
                    Ok(p) => self.request(Pending::Import(Box::new(p))),
                    Err(e) => self.status = format!("✖ {}: to nie jest projekt Pakieciarki ({e})", f.name),
                },
                OpenKind::Cells => self.import_cells(&f.name, &f.content),
                OpenKind::Bms => self.import_bms(&f.name, &f.content),
            }
        }

        if ctx.input_mut(|i| i.consume_shortcut(&SAVE)) {
            self.save_current();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&OPEN)) {
            self.refresh_library();
            self.projects_window = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&NEW)) {
            self.request(Pending::New);
        }

        let title = format!("{}{} — Pakieciarka", if self.dirty() { "● " } else { "" }, self.project.name);
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    // ---- menu ----------------------------------------------------------

    pub fn project_menu(&mut self, ui: &mut Ui) {
        let label = "📁 Projekt";
        ui.menu_button(label, |ui| {
            if ui.add(egui::Button::new("🗋 Nowy").shortcut_text(ui.ctx().format_shortcut(&NEW))).clicked() {
                self.request(Pending::New);
                ui.close();
            }
            if ui.add(egui::Button::new("📂 Moje projekty…").shortcut_text(ui.ctx().format_shortcut(&OPEN))).clicked() {
                self.refresh_library();
                self.projects_window = true;
                ui.close();
            }
            ui.separator();
            if ui.add(egui::Button::new("💾 Zapisz").shortcut_text(ui.ctx().format_shortcut(&SAVE))).clicked() {
                self.save_current();
                ui.close();
            }
            if ui.button("💾 Zapisz jako nowy").clicked() {
                self.save_as_new();
                ui.close();
            }
            ui.separator();
            if ui.button("📤 Eksportuj do pliku…").clicked() {
                let p = self.project.clone();
                self.export_project(&p);
                ui.close();
            }
            if ui.button("📥 Importuj z pliku…").clicked() {
                self.open_dialog(OpenKind::Project, ui.ctx());
                ui.close();
            }
            if ui.button("{ } JSON (schowek)…").clicked() {
                self.json_buf = serde_json::to_string_pretty(&self.project).unwrap_or_default();
                self.json_window = true;
                ui.close();
            }
        });
    }

    // ---- okna ----------------------------------------------------------

    pub fn library_windows(&mut self, ctx: &egui::Context) {
        self.projects_window(ctx);
        self.unsaved_modal(ctx);
        self.cell_edit_window(ctx);
    }

    fn projects_window(&mut self, ctx: &egui::Context) {
        let mut open = self.projects_window;
        let mut action: Option<Pending> = None;
        let mut duplicate: Option<SavedProject> = None;
        let mut export: Option<Project> = None;
        let mut delete: Option<String> = None;

        egui::Window::new("📂 Moje projekty")
            .open(&mut open)
            .default_width(620.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("🗋 Nowy projekt").clicked() {
                        action = Some(Pending::New);
                    }
                    if ui.button("📥 Importuj z pliku…").clicked() {
                        self.open_dialog(OpenKind::Project, ui.ctx());
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.project_filter)
                            .hint_text("🔍 szukaj…")
                            .desired_width(180.0),
                    );
                });
                ui.label(RichText::new(format!("Zapisane w: {}", store::location())).small().weak());
                ui.separator();

                if self.library.is_empty() {
                    ui.label("Brak zapisanych projektów. Użyj „Projekt → Zapisz” (Ctrl+S).");
                    return;
                }
                let filter = self.project_filter.to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for sp in &self.library {
                        if !filter.is_empty()
                            && !sp.project.name.to_lowercase().contains(&filter)
                            && !sp.project.cell.name.to_lowercase().contains(&filter)
                        {
                            continue;
                        }
                        let current = self.current_id.as_deref() == Some(sp.id.as_str());
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    let mut name = RichText::new(&sp.project.name).strong();
                                    if current {
                                        name = name.color(ui.visuals().selection.stroke.color);
                                    }
                                    ui.label(name);
                                    ui.label(RichText::new(summary(&sp.project)).small());
                                    ui.label(
                                        RichText::new(format!("zmieniony {}", store::format_time(sp.modified_ms)))
                                            .small()
                                            .weak(),
                                    );
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let armed = self.delete_armed.as_deref() == Some(sp.id.as_str());
                                    if armed {
                                        if ui
                                            .button(RichText::new("Na pewno usunąć?").color(Color32::from_rgb(220, 60, 60)))
                                            .clicked()
                                        {
                                            delete = Some(sp.id.clone());
                                        }
                                    } else if ui.button("🗑").on_hover_text("Usuń").clicked() {
                                        self.delete_armed = Some(sp.id.clone());
                                    }
                                    if ui.button("📤").on_hover_text("Eksportuj do pliku").clicked() {
                                        export = Some(sp.project.clone());
                                    }
                                    if ui.button("📋").on_hover_text("Duplikuj").clicked() {
                                        duplicate = Some(sp.clone());
                                    }
                                    if ui.add_enabled(!current, egui::Button::new("Otwórz")).clicked() {
                                        action = Some(Pending::Open(sp.id.clone()));
                                    }
                                });
                            });
                        });
                    }
                });
            });

        self.projects_window = open && self.projects_window;
        if let Some(a) = action {
            self.request(a);
        }
        if let Some(mut sp) = duplicate {
            sp.id = store::new_id();
            sp.modified_ms = store::now_ms();
            sp.project.name = format!("{} (kopia)", sp.project.name);
            let r = store::save_project(&sp).map(|_| format!("utworzono „{}”", sp.project.name));
            self.set_status(r);
            self.refresh_library();
        }
        if let Some(p) = export {
            self.export_project(&p);
        }
        if let Some(id) = delete {
            let r = store::delete_project(&id).map(|_| "usunięto projekt".to_string());
            self.set_status(r);
            if self.current_id.as_deref() == Some(id.as_str()) {
                self.current_id = None;
                self.saved = None;
            }
            self.delete_armed = None;
            self.refresh_library();
        }
    }

    fn unsaved_modal(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending.clone() else { return };
        let mut choice = None;
        egui::Modal::new(egui::Id::unique("pakieciarka_unsaved")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Niezapisane zmiany");
            ui.add_space(4.0);
            ui.label(format!("Projekt „{}” ma niezapisane zmiany. Co zrobić?", self.project.name));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("💾 Zapisz i kontynuuj").clicked() {
                    choice = Some(1);
                }
                if ui.button("Odrzuć zmiany").clicked() {
                    choice = Some(2);
                }
                if ui.button("Anuluj").clicked() {
                    choice = Some(0);
                }
            });
        });
        match choice {
            Some(1) => {
                self.save_current();
                self.pending = None;
                if !self.dirty() {
                    self.perform(action);
                }
            }
            Some(2) => {
                self.pending = None;
                self.perform(action);
            }
            Some(_) => self.pending = None,
            None => {}
        }
    }

    // ---- własne ogniwa -------------------------------------------------

    pub fn edit_new_cell(&mut self, base: Cell) {
        let target = self.custom_cells.iter().position(|c| c.name == base.name);
        self.cell_edit = Some(CellEdit { target, cell: base, error: String::new() });
    }

    fn save_custom_cells(&mut self) {
        if let Err(e) = store::save_cells(&self.custom_cells) {
            self.status = format!("✖ nie udało się zapisać ogniw: {e}");
        }
    }

    fn validate(&self, e: &CellEdit) -> Result<(), String> {
        let c = &e.cell;
        if c.name.trim().is_empty() {
            return Err("Podaj nazwę ogniwa.".into());
        }
        if self.builtin.iter().any(|b| b.name == c.name.trim()) {
            return Err("Ta nazwa jest zajęta przez ogniwo wbudowane — zmień ją.".into());
        }
        if self.custom_cells.iter().enumerate().any(|(i, o)| o.name == c.name.trim() && Some(i) != e.target) {
            return Err("Masz już własne ogniwo o tej nazwie.".into());
        }
        if !(c.min_v < c.nominal_v && c.nominal_v < c.max_v) {
            return Err("Napięcia muszą spełniać: min < nominalne < maks.".into());
        }
        if c.capacity_mah <= 0.0 || c.max_cont_a <= 0.0 || c.diameter_mm <= 0.0 {
            return Err("Pojemność, prąd i średnica muszą być dodatnie.".into());
        }
        Ok(())
    }

    fn cell_edit_window(&mut self, ctx: &egui::Context) {
        let Some(mut edit) = self.cell_edit.take() else { return };
        let mut open = true;
        let mut result: Option<bool> = None; // Some(true) = zapisz i użyj, Some(false) = zapisz
        let mut cancel = false;
        let title = if edit.target.is_some() { "✏ Edycja własnego ogniwa" } else { "➕ Nowe własne ogniwo" };
        egui::Window::new(title).open(&mut open).collapsible(false).default_width(360.0).show(ctx, |ui| {
            cell_editor(ui, &mut edit.cell);
            if !edit.error.is_empty() {
                ui.colored_label(Color32::from_rgb(220, 60, 60), &edit.error);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("💾 Zapisz").clicked() {
                    result = Some(false);
                }
                if ui.button("💾 Zapisz i użyj").clicked() {
                    result = Some(true);
                }
                if ui.button("Anuluj").clicked() {
                    cancel = true;
                }
            });
        });
        if !open || cancel {
            return;
        }
        if let Some(use_it) = result {
            edit.cell.name = edit.cell.name.trim().to_string();
            match self.validate(&edit) {
                Ok(()) => {
                    match edit.target {
                        Some(i) => self.custom_cells[i] = edit.cell.clone(),
                        None => self.custom_cells.push(edit.cell.clone()),
                    }
                    self.save_custom_cells();
                    if use_it {
                        self.project.cell = edit.cell.clone();
                    }
                    self.status = format!("✔ zapisano ogniwo „{}”", edit.cell.name);
                    return;
                }
                Err(e) => edit.error = e,
            }
        }
        self.cell_edit = Some(edit);
    }

    fn import_cells(&mut self, file: &str, json: &str) {
        let cells = serde_json::from_str::<Vec<Cell>>(json)
            .or_else(|_| serde_json::from_str::<Cell>(json).map(|c| vec![c]));
        match cells {
            Ok(cells) => {
                let n = cells.len();
                for c in cells {
                    if self.builtin.iter().any(|b| b.name == c.name) {
                        continue;
                    }
                    match self.custom_cells.iter_mut().find(|o| o.name == c.name) {
                        Some(o) => *o = c,
                        None => self.custom_cells.push(c),
                    }
                }
                self.save_custom_cells();
                self.status = format!("✔ zaimportowano {n} ogniw z {file}");
            }
            Err(e) => self.status = format!("✖ {file}: niepoprawna lista ogniw ({e})"),
        }
    }

    // ---- zakładka bazy ogniw ------------------------------------------

    pub fn tab_cells(&mut self, ui: &mut Ui) {
        ui.heading("Baza ogniw");
        ui.horizontal_wrapped(|ui| {
            if ui.button("➕ Dodaj własne ogniwo").clicked() {
                let mut c = self.project.cell.clone();
                c.name = "Moje ogniwo".into();
                self.cell_edit = Some(CellEdit { target: None, cell: c, error: String::new() });
            }
            if ui.button("📥 Importuj ogniwa…").clicked() {
                self.open_dialog(OpenKind::Cells, ui.ctx());
            }
            if ui.add_enabled(!self.custom_cells.is_empty(), egui::Button::new("📤 Eksportuj własne…")).clicked() {
                let json = serde_json::to_string_pretty(&self.custom_cells).unwrap_or_default();
                match platform::save_file("ogniwa_pakieciarka.json", &json, platform::JSON) {
                    Ok(Some(m)) => self.status = format!("✔ {m}"),
                    Ok(None) => {}
                    Err(e) => self.status = format!("✖ {e}"),
                }
            }
        });
        ui.label(
            RichText::new(format!(
                "Własne ogniwa są zapisywane w: {}",
                store::cells_location()
            ))
            .small()
            .weak(),
        );
        ui.add_space(8.0);

        #[derive(Clone, Copy)]
        enum Act {
            Use(bool, usize),
            Edit(usize),
            Copy(bool, usize),
            Delete(usize),
        }
        let mut act: Option<Act> = None;

        let header = |ui: &mut Ui| {
            for h in ["", "Ogniwo", "Format", "Chemia", "Pojemność", "Prąd ciągły", "IR", "Masa", "Energia", "Wh/kg", ""] {
                ui.strong(h);
            }
            ui.end_row();
        };
        let row = |ui: &mut Ui, c: &Cell| {
            ui.label(&c.name);
            ui.label(&c.format);
            ui.label(c.chemistry.label());
            ui.label(format!("{:.0} mAh", c.capacity_mah));
            ui.label(format!("{:.1} A", c.max_cont_a));
            ui.label(format!("{:.0} mΩ", c.ir_mohm));
            ui.label(format!("{:.1} g", c.weight_g));
            ui.label(format!("{:.1} Wh", c.energy_wh()));
            ui.label(format!("{:.0}", c.energy_wh() / c.weight_g.max(0.1) * 1000.0));
        };

        ui.strong(format!("Własne ogniwa ({})", self.custom_cells.len()));
        if self.custom_cells.is_empty() {
            ui.label(RichText::new("Brak — dodaj własne albo skopiuj wbudowane przyciskiem 📋.").weak());
        } else {
            egui::Grid::new("custom_cells").striped(true).num_columns(11).spacing([14.0, 6.0]).show(ui, |ui| {
                header(ui);
                for (i, c) in self.custom_cells.iter().enumerate() {
                    let selected = self.project.cell == *c;
                    if ui.add_enabled(!selected, egui::Button::new(if selected { "✔" } else { "Wybierz" })).clicked() {
                        act = Some(Act::Use(true, i));
                    }
                    row(ui, c);
                    ui.horizontal(|ui| {
                        if ui.small_button("✏").on_hover_text("Edytuj").clicked() {
                            act = Some(Act::Edit(i));
                        }
                        if ui.small_button("📋").on_hover_text("Duplikuj").clicked() {
                            act = Some(Act::Copy(true, i));
                        }
                        let armed = self.cell_delete_armed == Some(i);
                        let del = if armed {
                            ui.small_button(RichText::new("Usunąć?").color(Color32::from_rgb(220, 60, 60)))
                        } else {
                            ui.small_button("🗑").on_hover_text("Usuń")
                        };
                        if del.clicked() {
                            if armed {
                                act = Some(Act::Delete(i));
                            } else {
                                self.cell_delete_armed = Some(i);
                            }
                        }
                    });
                    ui.end_row();
                }
            });
        }

        ui.add_space(12.0);
        ui.strong(format!("Wbudowane ({})", self.builtin.len()));
        egui::Grid::new("builtin_cells").striped(true).num_columns(11).spacing([14.0, 6.0]).show(ui, |ui| {
            header(ui);
            for (i, c) in self.builtin.iter().enumerate() {
                let selected = self.project.cell == *c;
                if ui.add_enabled(!selected, egui::Button::new(if selected { "✔" } else { "Wybierz" })).clicked() {
                    act = Some(Act::Use(false, i));
                }
                row(ui, c);
                if ui.small_button("📋").on_hover_text("Skopiuj do własnych i edytuj").clicked() {
                    act = Some(Act::Copy(false, i));
                }
                ui.end_row();
            }
        });

        match act {
            Some(Act::Use(custom, i)) => {
                self.project.cell = if custom { self.custom_cells[i].clone() } else { self.builtin[i].clone() };
            }
            Some(Act::Edit(i)) => {
                self.cell_edit = Some(CellEdit { target: Some(i), cell: self.custom_cells[i].clone(), error: String::new() });
            }
            Some(Act::Copy(custom, i)) => {
                let mut c = if custom { self.custom_cells[i].clone() } else { self.builtin[i].clone() };
                c.name = format!("{} (moje)", c.name);
                self.cell_edit = Some(CellEdit { target: None, cell: c, error: String::new() });
            }
            Some(Act::Delete(i)) => {
                let c = self.custom_cells.remove(i);
                self.cell_delete_armed = None;
                self.save_custom_cells();
                self.status = format!("usunięto ogniwo „{}”", c.name);
            }
            None => {}
        }
    }
}
