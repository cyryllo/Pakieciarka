use crate::draw::{self, PackView};
use crate::library::{CellEdit, Pending};
use crate::platform::{self, Inbox};
use crate::store::{self, SavedProject};
use egui::{Color32, RichText, Ui};
use pakiet_core::calc::{
    auto_strip_width, pack_stats, presets, recommendations, PackStats, Recommendations, StripMaterial, StripPlan, StripSettings,
    STRIP_THICKNESSES, STRIP_WIDTHS,
};
use pakiet_core::cells::{builtin_cells, format_dims, Cell, Chemistry};
use pakiet_core::disclaimer;
use pakiet_core::gcode::{generate, Flip, GcodeOutput, GcodeSettings, Order, Trigger};
use pakiet_core::layout::{build_layout, weld_spots, Arrangement, Layout, LayoutParams, Side};
use pakiet_core::modules::ModulePlan;
use serde::{Deserialize, Serialize};

const VIEW_KEY: &str = "pakieciarka_view";

/// Wszystko, co opisuje pakiet — zapisywane i eksportowane jako JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub name: String,
    pub cell: Cell,
    pub s: usize,
    pub p: usize,
    /// Projektowy prąd obciążenia [A]
    pub load_a: f64,
    pub layout: LayoutParams,
    pub strip: StripMaterial,
    /// Największa grubość taśmy, którą zgrzewa zgrzewarka [mm]
    pub strip_max_t_mm: f64,
    /// Szerokość taśmy [mm]; 0 = automatycznie wg ogniwa
    pub strip_width_mm: f64,
    pub mod_series: usize,
    pub mod_parallel: usize,
    pub gcode: GcodeSettings,
}

impl Project {
    pub fn strip_settings(&self) -> StripSettings {
        StripSettings { material: self.strip, max_thickness_mm: self.strip_max_t_mm, width_mm: self.strip_width_mm }
    }
}

impl Default for Project {
    fn default() -> Self {
        Project {
            name: "Mój pakiet".into(),
            cell: Cell::default(),
            s: 10,
            p: 4,
            load_a: 20.0,
            layout: LayoutParams::default(),
            strip: StripMaterial::PureNickel,
            strip_max_t_mm: 0.2,
            strip_width_mm: 0.0,
            mod_series: 2,
            mod_parallel: 1,
            gcode: GcodeSettings::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Tab {
    Calc,
    Layout,
    Modules,
    Gcode,
    Cells,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Sides {
    Both,
    Top,
    Bottom,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct ViewOpts {
    tab: Tab,
    sides: Sides,
    show_spots: bool,
    labels: bool,
    numbers: bool,
    gcode_file: String,
    /// Id projektu w bibliotece, który jest otwarty
    current_id: Option<String>,
    /// Wersja zaakceptowanych zastrzeżeń (0 = nie zaakceptowano)
    disclaimer_ok: u32,
}

impl Default for ViewOpts {
    fn default() -> Self {
        ViewOpts {
            tab: Tab::Calc,
            sides: Sides::Both,
            show_spots: false,
            labels: true,
            numbers: true,
            gcode_file: "pakiet".into(),
            current_id: None,
            disclaimer_ok: 0,
        }
    }
}

#[derive(Default)]
struct Cache {
    layout_key: String,
    layout: Option<Layout>,
    gcode_key: String,
    gcode: GcodeOutput,
}

pub struct PakieciarkaApp {
    pub(crate) project: Project,
    view: ViewOpts,
    pub(crate) builtin: Vec<Cell>,
    pub(crate) custom_cells: Vec<Cell>,
    cache: Cache,
    hovered: Option<usize>,
    pub(crate) status: String,
    pub(crate) json_window: bool,
    pub(crate) json_buf: String,
    // biblioteka projektów
    pub(crate) current_id: Option<String>,
    /// Stan projektu przy ostatnim zapisie/otwarciu (do wykrywania zmian)
    pub(crate) saved: Option<Project>,
    pub(crate) library: Vec<SavedProject>,
    pub(crate) projects_window: bool,
    pub(crate) project_filter: String,
    pub(crate) delete_armed: Option<String>,
    pub(crate) pending: Option<Pending>,
    pub(crate) cell_edit: Option<CellEdit>,
    pub(crate) cell_delete_armed: Option<usize>,
    pub(crate) inbox: Inbox,
    pub(crate) last_title: String,
    about_window: bool,
    frame_no: u64,
}

impl PakieciarkaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let (mut project, view) = cc
            .storage
            .map(|s| {
                (
                    eframe::get_value::<Project>(s, eframe::APP_KEY).unwrap_or_default(),
                    eframe::get_value::<ViewOpts>(s, VIEW_KEY).unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut view: ViewOpts = view;
        let inbox = Inbox::default();
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut projects_window = false;
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut open_cell_editor = false;
        #[cfg_attr(not(target_arch = "wasm32"), allow(unused_assignments))]
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut open_id: Option<String> = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let args: Vec<String> = std::env::args().skip(1).collect();
            if let Some(t) = args.iter().skip_while(|a| *a != "--tab").nth(1) {
                view.tab = match t.as_str() {
                    "uklad" => Tab::Layout,
                    "moduly" => Tab::Modules,
                    "gcode" => Tab::Gcode,
                    "baza" => Tab::Cells,
                    _ => Tab::Calc,
                };
                projects_window = t == "projekty";
                open_cell_editor = t == "ogniwo";
            }
            // --projekt <id> — otwiera projekt z biblioteki
            open_id = args.iter().skip_while(|a| *a != "--projekt").nth(1).cloned();
            // --widok '{"sides":"Bottom","numbers":false}' — nadpisanie ustawień widoku
            if let Some(json) = args.iter().skip_while(|a| *a != "--widok").nth(1) {
                if let Ok(serde_json::Value::Object(over)) = serde_json::from_str(json) {
                    if let Ok(serde_json::Value::Object(mut cur)) = serde_json::to_value(&view) {
                        cur.extend(over);
                        if let Ok(v) = serde_json::from_value(serde_json::Value::Object(cur)) {
                            view = v;
                        }
                    }
                }
            }
            // pliki podane w wierszu poleceń (np. z menedżera plików) — import projektu
            for path in args.iter().filter(|a| a.ends_with(".json")) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    inbox.borrow_mut().push(platform::OpenedFile {
                        kind: platform::OpenKind::Project,
                        name: path.clone(),
                        content,
                    });
                }
            }
        }
        let library = store::list_projects();
        if let Some(sp) = open_id.and_then(|id| library.iter().find(|s| s.id == id)) {
            project = sp.project.clone();
            view.current_id = Some(sp.id.clone());
        }
        let current_id = view.current_id.clone().filter(|id| library.iter().any(|s| &s.id == id));
        let saved = match &current_id {
            Some(id) => library.iter().find(|s| &s.id == id).map(|s| s.project.clone()),
            None => Some(Project::default()),
        };
        let cell_edit = open_cell_editor.then(|| CellEdit {
            target: None,
            cell: Cell { name: "Moje ogniwo".into(), ..project.cell.clone() },
            error: String::new(),
        });
        PakieciarkaApp {
            project,
            view,
            builtin: builtin_cells(),
            custom_cells: store::load_cells(),
            cache: Cache::default(),
            hovered: None,
            status: String::new(),
            json_window: false,
            json_buf: String::new(),
            current_id,
            saved,
            library,
            projects_window,
            project_filter: String::new(),
            delete_armed: None,
            pending: None,
            cell_edit,
            cell_delete_armed: None,
            inbox,
            last_title: String::new(),
            about_window: false,
            frame_no: 0,
        }
    }

    fn layout(&mut self) -> Layout {
        let pr = &self.project;
        let key = format!("{}|{}|{}|{:?}", pr.s, pr.p, pr.cell.diameter_mm, pr.layout);
        if key != self.cache.layout_key || self.cache.layout.is_none() {
            self.cache.layout = Some(build_layout(pr.s, pr.p, pr.cell.diameter_mm, &pr.layout));
            self.cache.layout_key = key;
            self.cache.gcode_key.clear();
        }
        self.cache.layout.clone().unwrap()
    }

    fn gcode(&mut self, layout: &Layout) -> GcodeOutput {
        let key = serde_json::to_string(&self.project.gcode).unwrap_or_default();
        if key != self.cache.gcode_key {
            self.cache.gcode = generate(layout, &self.project.gcode);
            self.cache.gcode_key = key;
        }
        self.cache.gcode.clone()
    }

    /// `PAKIECIARKA_SCREENSHOT=plik.ppm` — zapisuje zrzut okna i kończy program (do testów).
    #[cfg(not(target_arch = "wasm32"))]
    fn debug_screenshot(&self, ctx: &egui::Context) {
        let Some(path) = std::env::var_os("PAKIECIARKA_SCREENSHOT") else { return };
        if self.frame_no == 20 {
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let mut data = format!("P6\n{w} {h}\n255\n").into_bytes();
                for p in &img.pixels {
                    data.extend_from_slice(&[p.r(), p.g(), p.b()]);
                }
                let _ = std::fs::write(&path, data);
                std::process::exit(0);
            });
        }
        if self.frame_no < 40 {
            ctx.request_repaint();
        }
    }

    /// Zastrzeżenia: akceptacja przy pierwszym uruchomieniu i okno „O programie”.
    fn disclaimer_windows(&mut self, ctx: &egui::Context) {
        let paragraphs = |ui: &mut Ui| {
            for p in disclaimer::FULL.split("\n\n") {
                ui.label(p);
                ui.add_space(4.0);
            }
        };
        if self.view.disclaimer_ok < disclaimer::VERSION {
            egui::Modal::new(egui::Id::unique("pakieciarka_disclaimer")).show(ctx, |ui| {
                ui.set_width(560.0);
                ui.heading(format!("⚠ {}", disclaimer::TITLE));
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(380.0).show(ui, paragraphs);
                ui.add_space(6.0);
                if ui.button(RichText::new("Rozumiem i akceptuję").strong()).clicked() {
                    self.view.disclaimer_ok = disclaimer::VERSION;
                }
                ui.label(RichText::new("Treść jest zawsze dostępna pod przyciskiem ℹ na górnym pasku.").small().weak());
            });
        }
        let mut open = self.about_window;
        egui::Window::new("ℹ O programie").open(&mut open).default_width(560.0).collapsible(false).show(ctx, |ui| {
            ui.heading(format!("Pakieciarka {}", env!("CARGO_PKG_VERSION")));
            ui.label("Konfigurator pakietów akumulatorów z rysunkami połączeń, drukiem PDF i generatorem G-code dla zgrzewarki.");
            ui.horizontal(|ui| {
                ui.label("Kod źródłowy:");
                ui.hyperlink("https://github.com/cyryllo/Pakieciarka");
            });
            ui.label("Copyright © 2026 Cyryl Sochacki · licencja GNU GPL v3.0 lub nowsza");
            ui.separator();
            ui.strong(disclaimer::TITLE);
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(360.0).show(ui, paragraphs);
        });
        self.about_window = open;
    }

    fn save_file(&mut self, name: &str, content: &str, filter: (&str, &[&str])) {
        match platform::save_file(name, content, filter) {
            Ok(Some(m)) => self.status = format!("✔ {m}"),
            Ok(None) => {}
            Err(e) => self.status = format!("✖ błąd zapisu: {e}"),
        }
    }
}

impl eframe::App for PakieciarkaApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.view.current_id = self.current_id.clone();
        eframe::set_value(storage, eframe::APP_KEY, &self.project);
        eframe::set_value(storage, VIEW_KEY, &self.view);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.handle_events(&ui.ctx().clone());
        self.frame_no += 1;
        #[cfg(not(target_arch = "wasm32"))]
        self.debug_screenshot(ui.ctx());
        let layout = self.layout();
        let stats = pack_stats(&self.project.cell, self.project.s, self.project.p, self.project.load_a);
        let rec = recommendations(&self.project.cell, &stats, &self.project.strip_settings(), layout.min_series_links, layout.pitch);

        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui, &stats));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("Pakieciarka {}", env!("CARGO_PKG_VERSION"))).small().weak());
                    ui.separator();
                    ui.label(RichText::new("Wartości orientacyjne — korzystasz na własną odpowiedzialność").small().weak())
                        .on_hover_text(disclaimer::SHORT);
                });
            });
        });
        egui::Panel::left("config").resizable(true).default_size(300.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.config_panel(ui, &stats));
        });
        egui::CentralPanel::default().show(ui, |ui| match self.view.tab {
            Tab::Calc => {
                egui::ScrollArea::vertical().show(ui, |ui| self.tab_calc(ui, &stats, &rec, &layout));
            }
            Tab::Layout => {
                egui::ScrollArea::vertical().show(ui, |ui| self.tab_layout(ui, &layout, &stats));
            }
            Tab::Modules => {
                egui::ScrollArea::vertical().show(ui, |ui| self.tab_modules(ui));
            }
            Tab::Gcode => self.tab_gcode(ui, &layout),
            Tab::Cells => {
                egui::ScrollArea::both().show(ui, |ui| self.tab_cells(ui));
            }
        });

        self.json_window(ui);
        self.library_windows(&ui.ctx().clone());
        self.disclaimer_windows(&ui.ctx().clone());
    }
}

// ---------------------------------------------------------------------------
// panele

impl PakieciarkaApp {
    fn top_bar(&mut self, ui: &mut Ui, st: &PackStats) {
        ui.horizontal(|ui| {
            ui.heading("🔋 Pakieciarka");
            ui.separator();
            ui.label(RichText::new(&self.project.name).strong());
            if self.dirty() {
                ui.label(RichText::new("• niezapisany").color(Color32::from_rgb(230, 160, 0)))
                    .on_hover_text("Niezapisane zmiany (Ctrl+S)");
            }
            ui.separator();
            let tabs = [
                (Tab::Calc, "📊 Kalkulator"),
                (Tab::Layout, "🔀 Układ i połączenia"),
                (Tab::Modules, "🔗 Moduły"),
                (Tab::Gcode, "⚙ G-code"),
                (Tab::Cells, "📚 Baza ogniw"),
            ];
            for (t, label) in tabs {
                ui.selectable_value(&mut self.view.tab, t, label);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                if ui.button("ℹ").on_hover_text("O programie i zastrzeżenia").clicked() {
                    self.about_window = true;
                }
                if self.dirty() && ui.button("💾 Zapisz").on_hover_text("Ctrl+S").clicked() {
                    self.save_current();
                }
                self.project_menu(ui);
                ui.label(
                    RichText::new(format!(
                        "{}S{}P · {:.1} V · {:.1} Ah · {:.0} Wh",
                        st.s, st.p, st.v_nom, st.capacity_ah, st.energy_wh
                    ))
                    .strong(),
                );
            });
        });
    }

    fn config_panel(&mut self, ui: &mut Ui, st: &PackStats) {
        let pr = &mut self.project;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Nazwa:");
            ui.text_edit_singleline(&mut pr.name);
        });

        ui.add_space(6.0);
        ui.strong("Ogniwo");
        egui::ComboBox::from_id_salt("cell")
            .width(ui.available_width())
            .selected_text(&pr.cell.name)
            .show_ui(ui, |ui| {
                for c in &self.builtin {
                    let txt = format!("{} — {} mAh, {} A", c.name, c.capacity_mah, c.max_cont_a);
                    if ui.selectable_label(pr.cell == *c, txt).clicked() {
                        pr.cell = c.clone();
                    }
                }
                if !self.custom_cells.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("Własne").small().strong());
                    for c in &self.custom_cells {
                        let txt = format!("★ {} — {} mAh, {} A", c.name, c.capacity_mah, c.max_cont_a);
                        if ui.selectable_label(pr.cell == *c, txt).clicked() {
                            pr.cell = c.clone();
                        }
                    }
                }
            });
        let mut save_cell = false;
        egui::CollapsingHeader::new("Parametry ogniwa (edycja)").show(ui, |ui| {
            cell_editor(ui, &mut pr.cell);
            save_cell = ui.button("★ Zapisz jako własne ogniwo").clicked();
        });

        ui.add_space(6.0);
        ui.strong("Konfiguracja SxP");
        let current = presets()
            .into_iter()
            .find(|ps| ps.s == pr.s && ps.p == pr.p)
            .map(|ps| ps.name.to_string())
            .unwrap_or_else(|| format!("Własna: {}S{}P", pr.s, pr.p));
        egui::ComboBox::from_id_salt("preset")
            .width(ui.available_width())
            .selected_text(current)
            .show_ui(ui, |ui| {
                for ps in presets() {
                    if ui.selectable_label(pr.s == ps.s && pr.p == ps.p, format!("{} · {}", ps.name, ps.note)).clicked() {
                        pr.s = ps.s;
                        pr.p = ps.p;
                    }
                }
            });
        egui::Grid::new("sp").num_columns(2).show(ui, |ui| {
            ui.label("Szeregowo S:");
            ui.add(egui::DragValue::new(&mut pr.s).range(1..=40).speed(0.1));
            ui.end_row();
            ui.label("Równolegle P:");
            ui.add(egui::DragValue::new(&mut pr.p).range(1..=40).speed(0.1));
            ui.end_row();
            ui.label("Prąd obciążenia:");
            ui.add(egui::DragValue::new(&mut pr.load_a).range(0.0..=1000.0).speed(0.5).suffix(" A"));
            ui.end_row();
        });
        ui.label(
            RichText::new(format!(
                "{} ogniw · {:.1}–{:.1} V · maks. {:.0} A",
                st.cells, st.v_min, st.v_max, st.max_cont_a
            ))
            .small(),
        );

        ui.add_space(6.0);
        ui.strong("Układ ogniw");
        ui.horizontal(|ui| {
            ui.radio_value(&mut pr.layout.arrangement, Arrangement::Grid, "Siatka");
            ui.radio_value(&mut pr.layout.arrangement, Arrangement::Honeycomb, "Plaster miodu");
        });
        egui::Grid::new("lay").num_columns(2).show(ui, |ui| {
            ui.label("Ogniw w kolumnie:");
            ui.add(
                egui::DragValue::new(&mut pr.layout.column_height)
                    .range(0..=200)
                    .speed(0.1)
                    .custom_formatter(|v, _| if v == 0.0 { "auto (=P)".into() } else { format!("{v}") }),
            )
            .on_hover_text("0 = każda grupa to jedna kolumna. Większa wartość = grupy zawijane wężykiem (pakiet krótszy, szerszy).");
            ui.end_row();
            ui.label("Odstęp ogniw:");
            ui.add(egui::DragValue::new(&mut pr.layout.gap_mm).range(0.0..=10.0).speed(0.05).suffix(" mm"));
            ui.end_row();
        });
        ui.checkbox(&mut pr.layout.horizontal, "Grupy w wierszach (obrót o 90°)");

        ui.add_space(6.0);
        ui.strong("Taśma do zgrzewania");
        egui::ComboBox::from_id_salt("strip")
            .width(ui.available_width())
            .selected_text(pr.strip.label())
            .show_ui(ui, |ui| {
                for m in StripMaterial::ALL {
                    ui.selectable_value(&mut pr.strip, m, m.label());
                }
            });
        egui::Grid::new("strip_opts").num_columns(2).show(ui, |ui| {
            ui.label("Zgrzewarka zgrzewa do:")
                .on_hover_text("Najgrubsza taśma, którą Twoja zgrzewarka pewnie zgrzewa. Gdy potrzeba więcej, program zaproponuje kilka warstw.");
            egui::ComboBox::from_id_salt("strip_max_t")
                .selected_text(format!("{:.2} mm", pr.strip_max_t_mm))
                .show_ui(ui, |ui| {
                    for t in STRIP_THICKNESSES {
                        ui.selectable_value(&mut pr.strip_max_t_mm, t, format!("{t:.2} mm"));
                    }
                });
            ui.end_row();
            ui.label("Szerokość taśmy:");
            let auto = auto_strip_width(pr.cell.diameter_mm);
            let txt = |w: f64| if w == 0.0 { format!("auto ({auto:.0} mm)") } else { format!("{w:.0} mm") };
            egui::ComboBox::from_id_salt("strip_w").selected_text(txt(pr.strip_width_mm)).show_ui(ui, |ui| {
                ui.selectable_value(&mut pr.strip_width_mm, 0.0, txt(0.0));
                for w in STRIP_WIDTHS {
                    ui.selectable_value(&mut pr.strip_width_mm, w, txt(w));
                }
            });
            ui.end_row();
        });

        ui.add_space(10.0);
        if ui.button("↺ Przywróć domyślne").clicked() {
            let name = std::mem::take(&mut pr.name);
            *pr = Project { name, ..Project::default() };
        }
        if save_cell {
            self.edit_new_cell(self.project.cell.clone());
        }
    }

    fn tab_calc(&mut self, ui: &mut Ui, st: &PackStats, rec: &Recommendations, layout: &Layout) {
        let cell = &self.project.cell;
        ui.heading(format!("{} — {}S{}P z {}", self.project.name, st.s, st.p, cell.name));
        ui.add_space(6.0);

        for w in &rec.warnings {
            warning(ui, w);
        }

        ui.columns(2, |cols| {
            card(&mut cols[0], "⚡ Parametry elektryczne", |ui| {
                kv_grid(ui, "el", &[
                    ("Napięcie nominalne", format!("{:.1} V", st.v_nom)),
                    ("Napięcie maks. (naładowany)", format!("{:.1} V", st.v_max)),
                    ("Napięcie min. (odcięcie)", format!("{:.1} V", st.v_min)),
                    ("Pojemność", format!("{:.2} Ah", st.capacity_ah)),
                    ("Energia", format!("{:.0} Wh", st.energy_wh)),
                    ("Prąd ciągły maks.", format!("{:.1} A", st.max_cont_a)),
                    ("Moc ciągła maks.", format!("{:.0} W", st.max_cont_w)),
                    ("Rezystancja wewn. (ogniwa)", format!("{:.1} mΩ", st.ir_mohm)),
                ]);
            });
            card(&mut cols[1], &format!("🔥 Pod obciążeniem {:.1} A", st.load_a), |ui| {
                kv_grid(ui, "load", &[
                    ("Prąd na ogniwo", format!("{:.2} A (limit {:.1} A)", st.load_per_cell_a, cell.max_cont_a)),
                    ("Obciążenie", format!("{:.2} C", st.load_c_rate)),
                    ("Moc", format!("{:.0} W", st.load_a * st.v_nom)),
                    ("Spadek napięcia", format!("{:.2} V", st.sag_v)),
                    ("Straty cieplne w ogniwach", format!("{:.1} W", st.heat_w)),
                    (
                        "Czas pracy (teoret.)",
                        if st.runtime_min.is_finite() { format!("{:.0} min", st.runtime_min) } else { "—".into() },
                    ),
                ]);
                ui.add(egui::ProgressBar::new((st.load_a / st.max_cont_a.max(0.001)).min(1.0) as f32).text(format!(
                    "{:.0}% prądu maks.",
                    st.load_a / st.max_cont_a.max(0.001) * 100.0
                )));
            });
        });

        ui.columns(2, |cols| {
            card(&mut cols[0], "📐 Mechanika", |ui| {
                kv_grid(ui, "mech", &[
                    ("Liczba ogniw", format!("{}", st.cells)),
                    ("Układ", format!("{} kolumn × {} ogniw", layout.columns, layout.column_height)),
                    (
                        "Wymiary bloku ogniw",
                        format!("{:.0} × {:.0} × {:.0} mm", layout.width, layout.height, cell.length_mm),
                    ),
                    ("Masa ogniw", format!("{:.2} kg", st.weight_cells_g / 1000.0)),
                    ("Masa szacunkowa (+15%)", format!("{:.2} kg", st.weight_est_g / 1000.0)),
                    ("Gęstość energii", format!("{:.0} Wh/kg", st.energy_wh / (st.weight_est_g / 1000.0))),
                ]);
            });
            card(&mut cols[1], "🔩 Dobór osprzętu", |ui| {
                let s = &rec.strip.series;
                let (awg, mm2, amp) = rec.main_wire;
                kv_grid(ui, "rec", &[
                    ("BMS", rec.bms.clone()),
                    ("Ładowarka", rec.charger.clone()),
                    ("Prąd ładowania (≈0,3C)", format!("{:.1} A", rec.charge_current_a)),
                    ("Bezpiecznik główny", format!("{:.0} A", rec.fuse_a)),
                    ("Nikiel szeregowo", format!("{} (szczegóły niżej)", s.label())),
                    ("Przewody główne", format!("{awg} AWG ({mm2} mm², ~{amp:.0} A)")),
                    ("Przewody balansera", rec.balance_wire.to_string()),
                ]);
            });
        });

        strip_card(ui, &rec.strip);

        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Dobór taśmy, przewodów, BMS i masy to reguły kciuka — wyniki są orientacyjne. Zawsze sprawdź karty katalogowe ogniw, BMS i taśmy. Korzystasz na własną odpowiedzialność (ℹ na górnym pasku).",
            )
            .small()
            .weak(),
        );
    }

    fn tab_layout(&mut self, ui: &mut Ui, layout: &Layout, st: &PackStats) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Pokaż:");
            ui.selectable_value(&mut self.view.sides, Sides::Both, "obie strony");
            ui.selectable_value(&mut self.view.sides, Sides::Top, "górę");
            ui.selectable_value(&mut self.view.sides, Sides::Bottom, "dół");
            ui.separator();
            ui.label("Dół po obróceniu pakietu:")
                .on_hover_text("Dół jest pokazany tak, jak go zobaczysz po obróceniu pakietu. Ta sama oś obrotu jest używana w G-code.");
            let f = &mut self.project.gcode.bottom_flip;
            egui::ComboBox::from_id_salt("flip_view").selected_text(f.label()).show_ui(ui, |ui| {
                for x in Flip::ALL {
                    ui.selectable_value(f, x, x.label());
                }
            });
            ui.checkbox(&mut self.view.labels, "etykiety");
            ui.checkbox(&mut self.view.numbers, "numery ogniw");
            ui.checkbox(&mut self.view.show_spots, "punkty zgrzewów");
            ui.separator();
            if ui
                .button("🖶 PDF do druku…")
                .on_hover_text("Góra i dół (1:1, jeśli się mieści), odczepy balansera i lista kontrolna blaszek — do ręcznego zgrzewania")
                .clicked()
            {
                let opts = crate::pdf::PdfOptions { numbers: self.view.numbers, labels: self.view.labels, spots: self.view.show_spots };
                let bytes = crate::pdf::layout_pdf(&self.project, layout, &opts);
                let name = format!("{}_uklad.pdf", platform::file_stem(&self.project.name));
                match platform::save_bytes(&name, &bytes, platform::PDF, true) {
                    Ok(Some(m)) => self.status = format!("✔ {m}"),
                    Ok(None) => {}
                    Err(e) => self.status = format!("✖ {e}"),
                }
            }
        });
        draw::draw_legend(ui);
        ui.add_space(4.0);

        let g = &self.project.gcode;
        let spots = weld_spots(layout, g.spots_per_cell, g.spot_offset_mm, g.spot_angle_deg);
        let spots = self.view.show_spots.then_some(spots.as_slice());
        let sides: &[Side] = match self.view.sides {
            Sides::Both => &[Side::Top, Side::Bottom],
            Sides::Top => &[Side::Top],
            Sides::Bottom => &[Side::Bottom],
        };
        let aspect = (layout.height / layout.width) as f32;
        let w_full = ui.available_width();
        let side_by_side = sides.len() == 2 && aspect > 0.6;
        let w = if side_by_side { (w_full - 8.0) / 2.0 } else { w_full };
        // pojedynczy widok (lub dwa obok siebie) mieści się w widocznej części okna
        let visible = (ui.clip_rect().bottom() - ui.cursor().top() - 8.0).max(240.0);
        let h_max = if sides.len() == 1 || side_by_side { visible } else { 720.0 };
        let h = (w * aspect + 56.0).clamp(180.0, h_max.max(180.0));
        let flip_view = Some(self.project.gcode.bottom_flip);
        let mut hovered = None;
        let mut draw_side = |ui: &mut Ui, side: Side| {
            let v = PackView { side, flip: flip_view, spots, labels: self.view.labels, numbers: self.view.numbers };
            if let Some(i) = draw::draw_pack(ui, layout, &v, egui::vec2(w, h)) {
                hovered = Some(i);
            }
        };
        if side_by_side {
            ui.horizontal(|ui| {
                for &s in sides {
                    draw_side(ui, s);
                }
            });
        } else {
            for &s in sides {
                draw_side(ui, s);
                ui.add_space(6.0);
            }
        }
        self.hovered = hovered;

        ui.add_space(4.0);
        if let Some(i) = self.hovered {
            let c = &layout.cells[i];
            let cell = &self.project.cell;
            ui.label(format!(
                "Ogniwo #{} · grupa G{} (z {}) · + na {} · potencjał grupy: {:.2}–{:.2} V względem B−",
                i + 1,
                c.group + 1,
                layout.s,
                if c.positive_top { "górze" } else { "dole" },
                c.group as f64 * cell.nominal_v,
                (c.group + 1) as f64 * cell.nominal_v,
            ));
        } else {
            ui.label(RichText::new("Najedź na ogniwo, aby zobaczyć szczegóły.").weak());
        }

        ui.add_space(8.0);
        let rec = recommendations(&self.project.cell, st, &self.project.strip_settings(), layout.min_series_links, layout.pitch);
        strip_card(ui, &rec.strip);
        ui.columns(2, |cols| {
            card(&mut cols[0], "🔌 Odczepy balansera", |ui| {
                let cell = &self.project.cell;
                egui::Grid::new("taps").striped(true).num_columns(4).show(ui, |ui| {
                    ui.strong("Odczep");
                    ui.strong("Strona");
                    ui.strong("Nominalnie");
                    ui.strong("Naładowany");
                    ui.end_row();
                    let mut plates: Vec<_> = layout.plates.iter().collect();
                    plates.sort_by_key(|p| p.tap);
                    for pl in plates {
                        let name = if pl.tap == 0 {
                            "B0 (B−)".to_string()
                        } else if pl.tap == st.s {
                            format!("B{} (B+)", pl.tap)
                        } else {
                            format!("B{}", pl.tap)
                        };
                        ui.label(name);
                        ui.label(pl.side.label());
                        ui.label(format!("{:.1} V", pl.tap as f64 * cell.nominal_v));
                        ui.label(format!("{:.2} V", pl.tap as f64 * cell.max_v));
                        ui.end_row();
                    }
                });
            });
            card(&mut cols[1], "🛠 Kolejność montażu", |ui| {
                let steps = [
                    "Posegreguj ogniwa (napięcie, IR, pojemność); w grupie równoległej ogniwa o zbliżonej pojemności.".to_string(),
                    "Ułóż ogniwa w koszykach zgodnie z widokiem GÓRA: grupy nieparzyste (G1, G3…) plusem do góry, parzyste minusem do góry.".to_string(),
                    format!(
                        "Zgrzej blaszki na górze ({} szt.), potem obróć pakiet wokół osi pionowej i zgrzej dół ({} szt.).",
                        layout.plates_on(Side::Top).count(),
                        layout.plates_on(Side::Bottom).count()
                    ),
                    "Pomarańczowe odcinki to połączenia szeregowe — przenoszą pełny prąd; nie oszczędzaj na nich niklu.".to_string(),
                    format!(
                        "Przylutuj/zgrzej przewody balansera B0…B{} do blaszek wg etykiet, a przed podłączeniem BMS zmierz napięcia narastająco.",
                        st.s
                    ),
                    "Przewody główne do B− i B+, izolacja (fishpaper na plusach, przekładki), na końcu termokurczka.".to_string(),
                ];
                for (i, s) in steps.iter().enumerate() {
                    ui.label(format!("{}. {}", i + 1, s));
                }
            });
        });
    }

    fn tab_modules(&mut self, ui: &mut Ui) {
        let pr = &mut self.project;
        ui.heading("Duże pakiety z modułów");
        ui.label(format!(
            "Moduł = bieżąca konfiguracja {}S{}P. Ustaw, ile modułów połączyć szeregowo (więcej napięcia) i równolegle (więcej pojemności i prądu).",
            pr.s, pr.p
        ));
        ui.horizontal(|ui| {
            ui.label("Moduły szeregowo:");
            ui.add(egui::DragValue::new(&mut pr.mod_series).range(1..=12).speed(0.05));
            ui.add_space(12.0);
            ui.label("Łańcuchy równolegle:");
            ui.add(egui::DragValue::new(&mut pr.mod_parallel).range(1..=8).speed(0.05));
        });
        let plan = ModulePlan { module_s: pr.s, module_p: pr.p, series: pr.mod_series, parallel: pr.mod_parallel };
        let total = pack_stats(&pr.cell, plan.total_s(), plan.total_p(), pr.load_a);
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "Razem: {} modułów ➡ {}S{}P · {} ogniw · {:.1} V ({:.1} V maks.) · {:.1} Ah · {:.2} kWh · {:.0} A ciągłe",
                plan.modules(),
                plan.total_s(),
                plan.total_p(),
                total.cells,
                total.v_nom,
                total.v_max,
                total.capacity_ah,
                total.energy_wh / 1000.0,
                total.max_cont_a
            ))
            .strong(),
        );
        ui.add_space(6.0);
        let w = ui.available_width();
        let h = (w * (plan.parallel as f32 * 130.0 + 70.0) / (plan.series as f32 * 240.0 + 140.0)).clamp(180.0, 600.0);
        draw::draw_modules(ui, &plan, egui::vec2(w, h));
        ui.add_space(8.0);
        if total.v_max > 60.0 {
            warning(ui, &format!("Napięcie maks. {:.1} V przekracza 60 V DC — praca pod niebezpiecznym napięciem.", total.v_max));
        }
        card(ui, "Jak połączyć", |ui| {
            for (i, s) in plan.steps(&total).iter().enumerate() {
                ui.label(format!("{}. {}", i + 1, s));
            }
        });
    }

    fn tab_gcode(&mut self, ui: &mut Ui, layout: &Layout) {
        let out = self.gcode(layout);
        egui::Panel::left("gcode_settings").resizable(true).default_size(330.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| gcode_settings(ui, &mut self.project.gcode));
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let (side, dry_run) = (self.project.gcode.side, self.project.gcode.dry_run);
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "Zgrzewów: {} · przejazdy: {:.2} m · szacowany czas: {}",
                    out.welds,
                    out.travel_mm / 1000.0,
                    fmt_duration(out.est_time_s)
                ));
                if dry_run {
                    ui.colored_label(Color32::from_rgb(230, 160, 0), "TRYB PRÓBNY");
                }
            });
            ui.horizontal(|ui| {
                ui.label("Plik:");
                ui.add(egui::TextEdit::singleline(&mut self.view.gcode_file).desired_width(160.0));
                let side_sfx = |s: Side| if s == Side::Top { "gora" } else { "dol" };
                if ui.button("💾 Zapisz tę stronę").clicked() {
                    let name = format!("{}_{}.gcode", self.view.gcode_file, side_sfx(side));
                    let text = out.text.clone();
                    self.save_file(&name, &text, platform::GCODE);
                }
                if ui.button("💾 Zapisz obie strony").clicked() {
                    let mut msgs = Vec::new();
                    for side in [Side::Top, Side::Bottom] {
                        let s = GcodeSettings { side, ..self.project.gcode.clone() };
                        let text = generate(layout, &s).text;
                        let name = format!("{}_{}.gcode", self.view.gcode_file, side_sfx(side));
                        self.status.clear();
                        self.save_file(&name, &text, platform::GCODE);
                        msgs.push(self.status.clone());
                    }
                    self.status = msgs.join("   ");
                }
                if ui.button("📋 Kopiuj").clicked() {
                    ui.ctx().copy_text(out.text.clone());
                    self.status = "✔ skopiowano G-code do schowka".into();
                }
            });
            ui.add_space(4.0);
            let w = ui.available_width();
            let h = (ui.available_height() * 0.55).max(220.0);
            draw::draw_gcode_preview(ui, layout, &self.project.gcode, &out, egui::vec2(w, h));
            ui.add_space(4.0);
            egui::ScrollArea::vertical().id_salt("gcode_text").show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut out.text.as_str())
                        .code_editor()
                        .desired_width(f32::INFINITY),
                );
            });
        });
    }

    fn json_window(&mut self, ui: &mut Ui) {
        let mut open = self.json_window;
        let mut load = false;
        let mut save = false;
        egui::Window::new("Projekt (JSON)").open(&mut open).default_width(520.0).show(ui.ctx(), |ui| {
            ui.label("Skopiuj, aby zachować projekt, albo wklej zapisany JSON i kliknij „Wczytaj”.");
            ui.horizontal(|ui| {
                if ui.button("📋 Kopiuj").clicked() {
                    ui.ctx().copy_text(self.json_buf.clone());
                }
                if ui.button("💾 Zapisz plik").clicked() {
                    save = true;
                }
                if ui.button("📂 Wczytaj z pola").clicked() {
                    load = true;
                }
            });
            egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.json_buf).code_editor().desired_width(f32::INFINITY));
            });
        });
        if save {
            let name = format!("{}.json", platform::file_stem(&self.project.name));
            let text = self.json_buf.clone();
            self.save_file(&name, &text, platform::JSON);
        }
        if load {
            match store::parse_project(&self.json_buf) {
                Ok(p) => {
                    self.request(Pending::Import(Box::new(p)));
                    open = false;
                }
                Err(e) => self.status = format!("✖ błędny JSON: {e}"),
            }
        }
        self.json_window = open;
    }
}

// ---------------------------------------------------------------------------
// pomocnicze

fn card(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.strong(title);
        ui.add_space(4.0);
        add(ui);
    });
    ui.add_space(6.0);
}

/// Karta „Łączniki niklowe” — grubość i liczba warstw taśmy dla każdego rodzaju połączenia.
pub(crate) fn strip_card(ui: &mut Ui, plan: &StripPlan) {
    card(ui, "🔗 Łączniki niklowe — jaka taśma", |ui| {
        let red = Color32::from_rgb(220, 70, 60);
        egui::Grid::new("strips").striped(true).num_columns(5).spacing([18.0, 5.0]).show(ui, |ui| {
            for h in ["Połączenie", "Prąd na pasek", "Taśma", "Dopuszczalny prąd", "Uwagi"] {
                ui.strong(h);
            }
            ui.end_row();
            let rows = [
                (
                    "Szeregowe (pomarańczowe)",
                    &plan.series,
                    plan.series_current_a,
                    format!("{} pask. na najsłabszym złączu — pełny prąd pakietu", plan.series_links),
                ),
                ("Równoległe (szare)", &plan.parallel, plan.parallel_current_a, "wyrównują prąd w grupie (≈ prąd 1 ogniwa)".to_string()),
                (
                    "Bieguny B+ / B−",
                    &plan.terminal,
                    plan.terminal_current_a,
                    "przewód główny w środku blaszki — połowa prądu w każdą stronę".to_string(),
                ),
            ];
            for (name, c, current, note) in rows {
                ui.label(name);
                ui.label(format!("{current:.1} A"));
                let t = RichText::new(c.label()).strong();
                ui.label(if c.ok { t } else { t.color(red) });
                ui.label(format!("{:.1} A", c.rating_a));
                ui.label(RichText::new(note).small());
                ui.end_row();
            }
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "{} · szerokość {:.0} mm · zgrzewarka do {:.2} mm · spadek na pasku złącza {:.0} mV · straty we wszystkich złączach szeregowych {:.1} W",
                plan.material.label(),
                plan.width_mm,
                plan.max_thickness_mm,
                plan.series_drop_mv,
                plan.series_loss_w
            ))
            .small(),
        );
        ui.label(
            RichText::new(
                "Warstwy taśmy układaj jedna na drugiej i zgrzewaj razem (albo kolejno). Dopuszczalny prąd: czysty nikiel 5 A/mm², stal niklowana 1,5 A/mm², miedź+nikiel 15 A/mm² — reguły kciuka. Stal niklowaną rozpoznasz magnesem (przyciąga mocno) i szlifowaniem (rdzewieje).",
            )
            .small()
            .weak(),
        );
    });
}

fn kv_grid(ui: &mut Ui, id: &str, rows: &[(&str, String)]) {
    egui::Grid::new(id).num_columns(2).striped(true).spacing([16.0, 4.0]).show(ui, |ui| {
        for (k, v) in rows {
            ui.label(*k);
            ui.label(RichText::new(v).strong());
            ui.end_row();
        }
    });
}

fn warning(ui: &mut Ui, text: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(230, 160, 0, 40))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(230, 160, 0)))
        .corner_radius(4.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(format!("⚠ {text}"));
        });
    ui.add_space(4.0);
}

fn fmt_duration(s: f64) -> String {
    let s = s.round() as u64;
    if s >= 3600 {
        format!("{} h {:02} min", s / 3600, s % 3600 / 60)
    } else {
        format!("{} min {:02} s", s / 60, s % 60)
    }
}

pub(crate) fn cell_editor(ui: &mut Ui, c: &mut Cell) {
    egui::Grid::new("cell_edit").num_columns(2).show(ui, |ui| {
        ui.label("Nazwa:");
        ui.text_edit_singleline(&mut c.name);
        ui.end_row();
        ui.label("Format:");
        let before = c.format.clone();
        egui::ComboBox::from_id_salt("fmt").selected_text(&c.format).show_ui(ui, |ui| {
            for f in ["14500", "18650", "21700", "26650", "32700"] {
                ui.selectable_value(&mut c.format, f.to_string(), f);
            }
        });
        if c.format != before {
            (c.diameter_mm, c.length_mm) = format_dims(&c.format);
        }
        ui.end_row();
        ui.label("Chemia:");
        ui.horizontal(|ui| {
            for ch in [Chemistry::LiIon, Chemistry::LiFePO4] {
                if ui.selectable_label(c.chemistry == ch, ch.label()).clicked() && c.chemistry != ch {
                    c.chemistry = ch;
                    (c.nominal_v, c.max_v, c.min_v) = match ch {
                        Chemistry::LiIon => (3.6, 4.2, 2.5),
                        Chemistry::LiFePO4 => (3.2, 3.65, 2.5),
                    };
                }
            }
        });
        ui.end_row();
        let row = |ui: &mut Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, sfx: &str| {
            ui.label(label);
            ui.add(egui::DragValue::new(v).range(range).speed(speed).suffix(sfx));
            ui.end_row();
        };
        row(ui, "Średnica:", &mut c.diameter_mm, 10.0..=50.0, 0.05, " mm");
        row(ui, "Długość:", &mut c.length_mm, 30.0..=150.0, 0.1, " mm");
        row(ui, "Pojemność:", &mut c.capacity_mah, 100.0..=50000.0, 10.0, " mAh");
        row(ui, "U nominalne:", &mut c.nominal_v, 1.0..=5.0, 0.01, " V");
        row(ui, "U maks.:", &mut c.max_v, 1.0..=5.0, 0.01, " V");
        row(ui, "U min.:", &mut c.min_v, 1.0..=5.0, 0.01, " V");
        row(ui, "Prąd ciągły:", &mut c.max_cont_a, 0.1..=200.0, 0.1, " A");
        row(ui, "Rezystancja:", &mut c.ir_mohm, 0.5..=500.0, 0.1, " mΩ");
        row(ui, "Masa:", &mut c.weight_g, 1.0..=1000.0, 0.1, " g");
    });
}

fn gcode_settings(ui: &mut Ui, g: &mut GcodeSettings) {
    ui.strong("Strona");
    ui.horizontal(|ui| {
        ui.radio_value(&mut g.side, Side::Top, "Góra");
        ui.radio_value(&mut g.side, Side::Bottom, "Dół");
    });
    ui.label("Obrót pakietu do zgrzewania dołu:");
    egui::ComboBox::from_id_salt("flip_g").width(ui.available_width()).selected_text(g.bottom_flip.label()).show_ui(ui, |ui| {
        for x in Flip::ALL {
            ui.selectable_value(&mut g.bottom_flip, x, x.label());
        }
    });
    ui.checkbox(&mut g.flip_y, "Góra rysunku = Y+ maszyny (od operatora)")
        .on_hover_text("Zaznaczone: pakiet kładziesz na stole tak, jak wygląda na rysunku, gdy patrzysz na ekran stojąc przed maszyną (Y+ rośnie od operatora). Odznacz, jeśli Y+ Twojej maszyny rośnie w stronę operatora.");

    let num = |ui: &mut Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, sfx: &str| {
        ui.label(label);
        ui.add(egui::DragValue::new(v).range(range).speed(speed).suffix(sfx));
        ui.end_row();
    };

    ui.add_space(6.0);
    ui.strong("Pozycja i ruch").on_hover_text(
        "Narożnik ogniw = lewy-przedni róg obrysu samych ogniw (nie koszyka), we współrzędnych roboczych (G54, względem ustawionego zera), nie maszynowych.\n\
         Środek ogniwa #1 leży o promień ogniwa dalej. Dla dołu: róg pakietu już po obróceniu.\n\
         Z bezpieczne / Z zgrzewania też liczone od zera roboczego Z.",
    );
    ui.label(RichText::new("Narożnik ogniw: lewy-przedni róg obrysu ogniw, we współrzędnych roboczych (G54).").small().weak());
    egui::Grid::new("g_pos").num_columns(2).show(ui, |ui| {
        num(ui, "Narożnik ogniw X:", &mut g.origin_x, -2000.0..=2000.0, 0.1, " mm");
        num(ui, "Narożnik ogniw Y:", &mut g.origin_y, -2000.0..=2000.0, 0.1, " mm");
        num(ui, "Posuw XY:", &mut g.feed_xy, 10.0..=20000.0, 10.0, " mm/min");
    });
    ui.checkbox(&mut g.use_z, "Oś Z opuszcza elektrody");
    ui.add_enabled_ui(g.use_z, |ui| {
        egui::Grid::new("g_z").num_columns(2).show(ui, |ui| {
            num(ui, "Z bezpieczne:", &mut g.z_safe, -100.0..=100.0, 0.1, " mm");
            num(ui, "Z zgrzewania:", &mut g.z_weld, -100.0..=100.0, 0.1, " mm");
            num(ui, "Posuw Z:", &mut g.feed_z, 10.0..=5000.0, 10.0, " mm/min");
        });
    });

    ui.add_space(6.0);
    ui.strong("Wyzwalanie zgrzewu");
    egui::ComboBox::from_id_salt("trigger").width(ui.available_width()).selected_text(g.trigger.label()).show_ui(ui, |ui| {
        for t in Trigger::ALL {
            ui.selectable_value(&mut g.trigger, t, t.label());
        }
    });
    if g.trigger == Trigger::Custom {
        ui.label(RichText::new("Zmienne: {pulse} [s], {n} nr zgrzewu, {cell} nr ogniwa").small());
        ui.add(egui::TextEdit::multiline(&mut g.custom_template).code_editor().desired_rows(3).desired_width(f32::INFINITY));
    } else {
        ui.label(RichText::new(g.trigger.template().replace('\n', " · ")).monospace().small());
    }
    egui::Grid::new("g_t").num_columns(2).show(ui, |ui| {
        num(ui, "Docisk przed impulsem:", &mut g.settle_ms, 0.0..=5000.0, 5.0, " ms");
        num(ui, "Sygnał wyzwolenia:", &mut g.pulse_ms, 1.0..=2000.0, 1.0, " ms");
        num(ui, "Odczekanie po:", &mut g.after_ms, 0.0..=5000.0, 5.0, " ms");
    });

    ui.add_space(6.0);
    ui.strong("Punkty zgrzewów");
    egui::Grid::new("g_s").num_columns(2).show(ui, |ui| {
        ui.label("Na biegun:");
        ui.add(egui::DragValue::new(&mut g.spots_per_cell).range(1..=8).speed(0.05));
        ui.end_row();
        num(ui, "Odległość od środka:", &mut g.spot_offset_mm, 0.0..=8.0, 0.05, " mm");
        num(ui, "Obrót wzoru:", &mut g.spot_angle_deg, -180.0..=180.0, 1.0, "°");
    });

    ui.add_space(6.0);
    ui.strong("Kolejność i opcje");
    ui.horizontal(|ui| {
        ui.radio_value(&mut g.order, Order::Nearest, "najbliższy sąsiad");
        ui.radio_value(&mut g.order, Order::Snake, "wężyk");
    });
    ui.checkbox(&mut g.home_first, "Bazowanie $H na początku");
    ui.checkbox(&mut g.dry_run, "Tryb próbny (bez wyzwalania)");
    ui.horizontal(|ui| {
        ui.label("Pauza M0 co");
        ui.add(egui::DragValue::new(&mut g.pause_every).range(0..=500).speed(0.1));
        ui.label("ogniw (0 = brak)");
    });

    ui.add_space(6.0);
    egui::CollapsingHeader::new("Własny nagłówek / stopka").show(ui, |ui| {
        ui.label("Nagłówek:");
        ui.add(egui::TextEdit::multiline(&mut g.header).code_editor().desired_rows(2).desired_width(f32::INFINITY));
        ui.label("Stopka:");
        ui.add(egui::TextEdit::multiline(&mut g.footer).code_editor().desired_rows(2).desired_width(f32::INFINITY));
    });
    ui.add_space(6.0);
    if ui.button("↺ Domyślne ustawienia G-code").clicked() {
        *g = GcodeSettings::default();
    }
}
