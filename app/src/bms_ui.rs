//! Zakładka BMS: dopasowane modele, własne BMS-y, schemat podłączenia i wtyczka balansera.

use crate::app::{card, PakieciarkaApp};
use crate::draw::{NEG, POS};
use crate::platform::{self, OpenKind};
use crate::store;
use egui::{vec2, Align2, Color32, FontId, Rect, RichText, Sense, Shape, Stroke, Ui};
use pakiet_core::bms::{balance_pins, match_bms, required_current, wiring_steps, Balancing, Bms, Port};
use pakiet_core::calc::PackStats;
use pakiet_core::cells::Chemistry;
use pakiet_core::layout::Layout;

pub struct BmsEdit {
    /// Indeks własnego BMS; `None` = nowy
    pub target: Option<usize>,
    pub bms: Bms,
    pub error: String,
}

#[derive(Clone, Copy)]
enum Act {
    Select(bool, usize),
    Edit(usize),
    Copy(bool, usize),
    Delete(usize),
}

impl PakieciarkaApp {
    pub fn tab_bms(&mut self, ui: &mut Ui, layout: &Layout, st: &PackStats) {
        let chem = self.project.cell.chemistry;
        let need = required_current(self.project.load_a);
        ui.heading("BMS — dobór i podłączenie");
        ui.label(
            RichText::new(format!(
                "Pakiet {}S {} · prąd obciążenia {:.0} A · potrzebny BMS: {}S, {}, prąd ciągły ≥ {:.0} A, ładowanie do {:.1} V",
                st.s,
                chem.label(),
                self.project.load_a,
                st.s,
                chem.label(),
                need,
                st.v_max
            ))
            .strong(),
        );
        ui.label(
            RichText::new(
                "Lista pokazuje tylko BMS-y pasujące do pakietu. To pomoc w doborze, nie rekomendacja zakupowa: dane wbudowane są orientacyjne (⚠ do sprawdzenia) — zawsze sprawdź kartę katalogową konkretnego wariantu.",
            )
            .small()
            .weak(),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("➕ Dodaj własny BMS").clicked() {
                let bms = Bms { s_min: st.s, s_max: st.s, chemistries: vec![chem], currents_a: vec![need], ..Bms::default() };
                self.bms_edit = Some(BmsEdit { target: None, bms, error: String::new() });
            }
            if ui.button("📥 Importuj BMS-y…").clicked() {
                self.open_dialog(OpenKind::Bms, ui.ctx());
            }
            if ui.add_enabled(!self.custom_bms.is_empty(), egui::Button::new("📤 Eksportuj własne…")).clicked() {
                let json = serde_json::to_string_pretty(&self.custom_bms).unwrap_or_default();
                match platform::save_file("bms_pakieciarka.json", &json, platform::JSON) {
                    Ok(Some(m)) => self.status = format!("✔ {m}"),
                    Ok(None) => {}
                    Err(e) => self.status = format!("✖ {e}"),
                }
            }
        });
        ui.add_space(6.0);

        let mut act: Option<Act> = None;
        let selected = self.project.bms.clone();

        // pasujące: własne + wbudowane
        let mut rows: Vec<(bool, usize, &Bms, f64)> = Vec::new();
        for (custom, list) in [(true, &self.custom_bms), (false, &self.builtin_bms)] {
            for (i, b) in list.iter().enumerate() {
                let m = match_bms(b, st.s, chem, need);
                if let (true, Some(v)) = (m.ok(), m.variant_a) {
                    rows.push((custom, i, b, v));
                }
            }
        }

        card(ui, &format!("✔ Pasujące BMS-y ({})", rows.len()), |ui| {
            if rows.is_empty() {
                ui.label("Żaden BMS z bazy nie pasuje do tego pakietu. Dodaj własny (➕) z parametrami z karty katalogowej.");
                return;
            }
            egui::Grid::new("bms_match").striped(true).num_columns(9).spacing([14.0, 6.0]).show(ui, |ui| {
                for h in ["", "Model", "Producent", "Zakres", "Wariant", "Balansowanie", "Port", "Aplikacja", "Dane"] {
                    ui.strong(h);
                }
                ui.end_row();
                for &(custom, i, b, v) in &rows {
                    let is_sel = selected.as_ref() == Some(b);
                    if ui.add_enabled(!is_sel, egui::Button::new(if is_sel { "✔" } else { "Wybierz" })).clicked() {
                        act = Some(Act::Select(custom, i));
                    }
                    ui.label(if custom { format!("★ {}", b.name) } else { b.name.clone() });
                    ui.label(&b.manufacturer);
                    ui.label(b.s_label());
                    ui.label(RichText::new(format!("{v:.0} A")).strong()).on_hover_text(format!("Dostępne: {}", b.currents_label()));
                    ui.label(b.balance_label());
                    ui.label(b.port.label());
                    ui.label(if b.smart { "tak" } else { "nie" });
                    if b.verified {
                        ui.label("✔ sprawdzone");
                    } else {
                        ui.colored_label(Color32::from_rgb(230, 160, 0), "⚠ do sprawdzenia").on_hover_text(&b.notes);
                    }
                    ui.end_row();
                }
            });
        });

        egui::CollapsingHeader::new(format!("Moje BMS-y ({})", self.custom_bms.len())).show(ui, |ui| {
            if self.custom_bms.is_empty() {
                ui.label(RichText::new("Brak. Dodaj BMS, który masz, z parametrami i kolejnością pinów z jego dokumentacji.").weak());
            }
            for (i, b) in self.custom_bms.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&b.name).strong());
                    ui.label(format!("{} · {} · {} · {}", b.s_label(), b.chem_label(), b.currents_label(), b.port.label()));
                    if ui.small_button("✏").on_hover_text("Edytuj").clicked() {
                        act = Some(Act::Edit(i));
                    }
                    if ui.small_button("📋").on_hover_text("Duplikuj").clicked() {
                        act = Some(Act::Copy(true, i));
                    }
                    let armed = self.bms_delete_armed == Some(i);
                    let del = if armed {
                        ui.small_button(RichText::new("Usunąć?").color(Color32::from_rgb(220, 60, 60)))
                    } else {
                        ui.small_button("🗑").on_hover_text("Usuń")
                    };
                    if del.clicked() {
                        if armed {
                            act = Some(Act::Delete(i));
                        } else {
                            self.bms_delete_armed = Some(i);
                        }
                    }
                });
            }
        });

        ui.add_space(6.0);
        if let Some(b) = &selected {
            self.selected_bms_section(ui, b, layout, st);
        } else {
            ui.label(RichText::new("Wybierz BMS z listy, aby zobaczyć schemat podłączenia i kolejność pinów balansera.").weak());
        }

        match act {
            Some(Act::Select(custom, i)) => {
                self.project.bms = Some(if custom { self.custom_bms[i].clone() } else { self.builtin_bms[i].clone() });
            }
            Some(Act::Edit(i)) => {
                self.bms_edit = Some(BmsEdit { target: Some(i), bms: self.custom_bms[i].clone(), error: String::new() });
            }
            Some(Act::Copy(custom, i)) => {
                let mut b = if custom { self.custom_bms[i].clone() } else { self.builtin_bms[i].clone() };
                b.name = format!("{} (mój)", b.name);
                b.verified = true;
                self.bms_edit = Some(BmsEdit { target: None, bms: b, error: String::new() });
            }
            Some(Act::Delete(i)) => {
                let b = self.custom_bms.remove(i);
                self.bms_delete_armed = None;
                self.save_custom_bms();
                self.status = format!("usunięto BMS „{}”", b.name);
            }
            None => {}
        }
    }

    fn selected_bms_section(&mut self, ui: &mut Ui, b: &Bms, layout: &Layout, st: &PackStats) {
        let chem = self.project.cell.chemistry;
        let m = match_bms(b, st.s, chem, required_current(self.project.load_a));
        let mut clear = false;
        let mut copy = false;
        card(ui, &format!("🔌 Wybrany BMS: {}", b.name), |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} · {} · {} · balansowanie {} · port {} · NTC: {}",
                    b.s_label(),
                    b.chem_label(),
                    b.currents_label(),
                    b.balance_label(),
                    b.port.label(),
                    b.ntc
                ));
                if ui.small_button("Usuń wybór").clicked() {
                    clear = true;
                }
                if !self.custom_bms.contains(b) && ui.small_button("📋 Skopiuj do moich i popraw").clicked() {
                    copy = true;
                }
            });
            for p in &m.problems {
                ui.colored_label(Color32::from_rgb(220, 70, 60), format!("✖ Nie pasuje do pakietu: {p}"));
            }
            if !b.verified {
                ui.colored_label(
                    Color32::from_rgb(230, 160, 0),
                    "⚠ Dane orientacyjne — kolejność pinów i parametry sprawdź w dokumentacji swojego egzemplarza.",
                );
            }
            if !b.notes.is_empty() {
                ui.label(RichText::new(&b.notes).small());
            }
            ui.add_space(6.0);
            let w = ui.available_width();
            let h = (w * wiring_height(st.s) / 900.0).clamp(260.0, 1100.0);
            draw_bms_wiring(ui, b, st.s, vec2(w, h));
        });

        ui.columns(2, |cols| {
            card(&mut cols[0], "Wtyczka balansera", |ui| {
                ui.label(
                    RichText::new(if b.b0_first {
                        "Pin 1 = B0 (B−). Kolejność wg danych modelu — potwierdź w dokumentacji."
                    } else {
                        "Pin 1 = B+ (najwyższy odczep). Kolejność wg danych modelu — potwierdź w dokumentacji."
                    })
                    .small(),
                );
                egui::Grid::new("bms_pins").striped(true).num_columns(5).show(ui, |ui| {
                    for h in ["Pin", "Odczep", "Blaszka", "Nominalnie", "Naładowany"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    let cell = &self.project.cell;
                    for (pin, tap) in balance_pins(b, st.s) {
                        ui.label(pin.to_string());
                        ui.label(match tap {
                            0 => "B0 (B−)".to_string(),
                            t if t == st.s => format!("B{t} (B+)"),
                            t => format!("B{t}"),
                        });
                        let side = layout.plates.iter().find(|p| p.tap == tap).map(|p| p.side.label()).unwrap_or("—");
                        ui.label(side);
                        ui.label(format!("{:.1} V", tap as f64 * cell.nominal_v));
                        ui.label(format!("{:.2} V", tap as f64 * cell.max_v));
                        ui.end_row();
                    }
                });
            });
            card(&mut cols[1], "Kolejność podłączania", |ui| {
                for (i, s) in wiring_steps(b, st.s, self.project.cell.nominal_v).iter().enumerate() {
                    ui.label(format!("{}. {s}", i + 1));
                }
            });
        });

        if clear {
            self.project.bms = None;
        }
        if copy {
            let mut c = b.clone();
            c.name = format!("{} (mój)", c.name);
            c.verified = true;
            self.bms_edit = Some(BmsEdit { target: None, bms: c, error: String::new() });
        }
    }

    pub fn save_custom_bms(&mut self) {
        if let Err(e) = store::save_bms(&self.custom_bms) {
            self.status = format!("✖ nie udało się zapisać BMS-ów: {e}");
        }
    }

    pub fn import_bms(&mut self, file: &str, json: &str) {
        let list = serde_json::from_str::<Vec<Bms>>(json).or_else(|_| serde_json::from_str::<Bms>(json).map(|b| vec![b]));
        match list {
            Ok(list) => {
                let n = list.len();
                for b in list {
                    match self.custom_bms.iter_mut().find(|o| o.name == b.name) {
                        Some(o) => *o = b,
                        None => self.custom_bms.push(b),
                    }
                }
                self.save_custom_bms();
                self.status = format!("✔ zaimportowano {n} BMS z {file}");
            }
            Err(e) => self.status = format!("✖ {file}: niepoprawna lista BMS ({e})"),
        }
    }

    pub fn bms_edit_window(&mut self, ctx: &egui::Context) {
        let Some(mut edit) = self.bms_edit.take() else { return };
        let mut open = true;
        let mut result: Option<bool> = None;
        let mut cancel = false;
        let title = if edit.target.is_some() { "✏ Edycja BMS" } else { "➕ Nowy BMS" };
        egui::Window::new(title).open(&mut open).collapsible(false).default_width(420.0).show(ctx, |ui| {
            let b = &mut edit.bms;
            egui::Grid::new("bms_edit").num_columns(2).show(ui, |ui| {
                ui.label("Nazwa / model:");
                ui.text_edit_singleline(&mut b.name);
                ui.end_row();
                ui.label("Producent:");
                ui.text_edit_singleline(&mut b.manufacturer);
                ui.end_row();
                ui.label("Liczba ogniw S:");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut b.s_min).range(1..=32).prefix("od "));
                    ui.add(egui::DragValue::new(&mut b.s_max).range(1..=32).prefix("do "));
                });
                ui.end_row();
                ui.label("Chemia:");
                ui.horizontal(|ui| {
                    for ch in [Chemistry::LiIon, Chemistry::LiFePO4] {
                        let mut on = b.chemistries.contains(&ch);
                        if ui.checkbox(&mut on, ch.label()).changed() {
                            if on {
                                b.chemistries.push(ch);
                            } else {
                                b.chemistries.retain(|c| *c != ch);
                            }
                        }
                    }
                });
                ui.end_row();
                ui.label("Prąd ciągły:");
                let mut a = b.currents_a.first().copied().unwrap_or(30.0);
                if ui.add(egui::DragValue::new(&mut a).range(1.0..=1000.0).speed(1.0).suffix(" A")).changed() {
                    b.currents_a = vec![a];
                }
                ui.end_row();
                ui.label("Balansowanie:");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut b.balancing, Balancing::Passive, "pasywne");
                    ui.radio_value(&mut b.balancing, Balancing::Active, "aktywne");
                    ui.add(egui::DragValue::new(&mut b.balance_ma).range(1.0..=10000.0).speed(5.0).suffix(" mA"));
                });
                ui.end_row();
                ui.label("Port:");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut b.port, Port::Common, "wspólny");
                    ui.radio_value(&mut b.port, Port::Separate, "osobny (P−/C−)");
                });
                ui.end_row();
                ui.label("Aplikacja (BT/UART):");
                ui.checkbox(&mut b.smart, "");
                ui.end_row();
                ui.label("Czujniki NTC:");
                ui.add(egui::DragValue::new(&mut b.ntc).range(0..=8));
                ui.end_row();
                ui.label("Pin 1 wtyczki:");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut b.b0_first, true, "B0 (B−)");
                    ui.radio_value(&mut b.b0_first, false, "B+ (najwyższy)");
                });
                ui.end_row();
                ui.label("Uwagi:");
                ui.text_edit_multiline(&mut b.notes);
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut b.verified, "dane sprawdzone z dokumentacją");
                ui.end_row();
            });
            if !edit.error.is_empty() {
                ui.colored_label(Color32::from_rgb(220, 60, 60), &edit.error);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("💾 Zapisz").clicked() {
                    result = Some(false);
                }
                if ui.button("💾 Zapisz i wybierz").clicked() {
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
        if let Some(select) = result {
            let b = &mut edit.bms;
            b.name = b.name.trim().to_string();
            let err = if b.name.is_empty() {
                Some("Podaj nazwę BMS.")
            } else if b.s_min > b.s_max {
                Some("Zakres S: „od” nie może być większe niż „do”.")
            } else if b.chemistries.is_empty() {
                Some("Zaznacz co najmniej jedną chemię.")
            } else if self.custom_bms.iter().enumerate().any(|(i, o)| o.name == b.name && Some(i) != edit.target) {
                Some("Masz już BMS o tej nazwie.")
            } else {
                None
            };
            if let Some(e) = err {
                edit.error = e.into();
            } else {
                match edit.target {
                    Some(i) => self.custom_bms[i] = edit.bms.clone(),
                    None => self.custom_bms.push(edit.bms.clone()),
                }
                self.save_custom_bms();
                if select {
                    self.project.bms = Some(edit.bms.clone());
                }
                self.status = format!("✔ zapisano BMS „{}”", edit.bms.name);
                return;
            }
        }
        self.bms_edit = Some(edit);
    }
}

/// Wysokość schematu w jednostkach logicznych (szerokość = 900).
pub fn wiring_height(s: usize) -> f32 {
    100.0 + s as f32 * 24.0 + 90.0
}

/// Schemat: pakiet (grupy G1…GS z odczepami) — wtyczka balansera BMS — B−/P−/C− — odbiornik i ładowarka.
pub fn draw_bms_wiring(ui: &mut Ui, b: &Bms, s: usize, size: egui::Vec2) {
    let dark = ui.visuals().dark_mode;
    let (resp, painter) = ui.allocate_painter(size, Sense::hover());
    let rect = resp.rect;
    painter.rect_filled(rect, 6.0, if dark { Color32::from_gray(24) } else { Color32::from_gray(246) });
    let text = ui.visuals().text_color();
    let wire = if dark { Color32::from_gray(210) } else { Color32::from_gray(40) };

    // współrzędne logiczne → ekran
    let (lw, lh) = (900.0f32, wiring_height(s));
    let sc = (rect.width() / lw).min(rect.height() / lh);
    let off = rect.center() - vec2(lw * sc / 2.0, lh * sc / 2.0);
    let p = |x: f32, y: f32| off + vec2(x * sc, y * sc);
    let fs = (12.0 * sc).clamp(9.0, 14.0);

    let top = 100.0;
    let step = 24.0;
    let tap_y = |k: usize| top + (s - k) as f32 * step; // B0 na dole, BS na górze

    // pakiet
    let (px0, px1) = (60.0, 160.0);
    for g in 0..s {
        let r = Rect::from_min_max(p(px0, tap_y(g + 1)), p(px1, tap_y(g)));
        painter.rect_filled(r.shrink(1.0), 3.0, crate::draw::group_color(g, dark));
        painter.text(r.center(), Align2::CENTER_CENTER, format!("G{}", g + 1), FontId::proportional(fs), Color32::from_gray(25));
    }
    painter.text(p((px0 + px1) / 2.0, top - 16.0), Align2::CENTER_CENTER, "Pakiet", FontId::proportional(fs * 1.1), text);

    // BMS
    let (bx0, bx1) = (420.0, 560.0);
    let bms_rect = Rect::from_min_max(p(bx0, top - 20.0), p(bx1, tap_y(0) + 50.0));
    painter.rect_filled(bms_rect, 6.0, if dark { Color32::from_rgb(40, 60, 90) } else { Color32::from_rgb(200, 220, 245) });
    painter.text(bms_rect.center_top() + vec2(0.0, 14.0 * sc), Align2::CENTER_CENTER, "BMS", FontId::proportional(fs * 1.3), text);
    painter.text(bms_rect.center(), Align2::CENTER_CENTER, b.name.as_str(), FontId::proportional(fs * 0.85), text);

    // odczepy → wtyczka balansera
    let pins = balance_pins(b, s);
    for (pin, tap) in pins {
        let y = tap_y(tap);
        let from = p(px1, y);
        let to = p(bx0, y);
        let col = if tap == 0 { NEG } else if tap == s { POS } else { Color32::from_rgb(170, 120, 210) };
        painter.circle_filled(from, 3.0, col);
        painter.line_segment([from, to], Stroke::new(1.4, col));
        painter.text(p(px1 + 8.0, y - 2.0), Align2::LEFT_BOTTOM, format!("B{tap}"), FontId::proportional(fs * 0.85), text);
        painter.text(p(bx0 - 6.0, y - 2.0), Align2::RIGHT_BOTTOM, format!("pin {pin}"), FontId::proportional(fs * 0.8), text);
    }
    painter.text(p((px1 + bx0) / 2.0, top - 16.0), Align2::CENTER_CENTER, "przewody balansera", FontId::proportional(fs), text);

    // przewody mocy
    let thick = Stroke::new((4.0 * sc).max(2.5), wire);
    let red = Stroke::new((4.0 * sc).max(2.5), POS);
    let y_bminus = tap_y(0) + 30.0;
    // B− pakietu → B− BMS
    painter.add(Shape::line(vec![p(px0 + 20.0, tap_y(0)), p(px0 + 20.0, y_bminus), p(bx0 + 30.0, y_bminus)], thick));
    painter.text(p(bx0 + 34.0, y_bminus), Align2::LEFT_CENTER, "B−", FontId::proportional(fs), text);

    let (lx, cx) = (720.0, 720.0);
    let load = Rect::from_min_max(p(lx, top + 10.0), p(lx + 140.0, top + 70.0));
    let charger_y = (tap_y(0) - 10.0).max(top + 140.0);
    let charger = Rect::from_min_max(p(cx, charger_y - 30.0), p(cx + 140.0, charger_y + 30.0));
    for (r, name) in [(load, "Odbiornik"), (charger, "Ładowarka")] {
        painter.rect_stroke(r, 6.0, Stroke::new(1.5, text), egui::StrokeKind::Inside);
        painter.text(r.center(), Align2::CENTER_CENTER, name, FontId::proportional(fs), text);
    }

    // B+ pakietu → bezpiecznik → + odbiornika i ładowarki
    let y_plus = top - 62.0;
    let xr = lx - 20.0; // pion plusa
    painter.add(Shape::line(vec![p(px1 - 20.0, tap_y(s)), p(px1 - 20.0, y_plus), p(xr, y_plus)], red));
    let fuse = Rect::from_center_size(p(280.0, y_plus), vec2(44.0 * sc, 16.0 * sc));
    painter.rect_filled(fuse, 3.0, if dark { Color32::from_gray(60) } else { Color32::WHITE });
    painter.rect_stroke(fuse, 3.0, Stroke::new(1.5, POS), egui::StrokeKind::Inside);
    painter.text(fuse.center_top() - vec2(0.0, 4.0), Align2::CENTER_BOTTOM, "bezpiecznik", FontId::proportional(fs * 0.8), text);
    painter.add(Shape::line(vec![p(xr, y_plus), p(xr, top + 25.0), p(lx, top + 25.0)], red));
    painter.add(Shape::line(vec![p(xr, top + 25.0), p(xr, charger_y - 15.0), p(cx, charger_y - 15.0)], red));
    painter.text(p(lx + 6.0, top + 25.0), Align2::LEFT_CENTER, "+", FontId::monospace(fs * 1.2), POS);
    painter.text(p(cx + 6.0, charger_y - 15.0), Align2::LEFT_CENTER, "+", FontId::monospace(fs * 1.2), POS);

    // P− (i C−) → minusy
    let y_load_minus = top + 55.0;
    let y_ch_minus = charger_y + 15.0;
    match b.port {
        Port::Common => {
            painter.text(p(bx1 - 6.0, y_bminus - 20.0), Align2::RIGHT_CENTER, "P−", FontId::proportional(fs), text);
            painter.add(Shape::line(vec![p(bx1, y_bminus - 20.0), p(655.0, y_bminus - 20.0), p(655.0, y_load_minus), p(lx, y_load_minus)], thick));
            painter.add(Shape::line(vec![p(655.0, y_ch_minus), p(cx, y_ch_minus)], thick));
        }
        Port::Separate => {
            painter.text(p(bx1 - 6.0, y_bminus - 34.0), Align2::RIGHT_CENTER, "P−", FontId::proportional(fs), text);
            painter.text(p(bx1 - 6.0, y_bminus - 8.0), Align2::RIGHT_CENTER, "C−", FontId::proportional(fs), text);
            painter.add(Shape::line(vec![p(bx1, y_bminus - 34.0), p(630.0, y_bminus - 34.0), p(630.0, y_load_minus), p(lx, y_load_minus)], thick));
            painter.add(Shape::line(vec![p(bx1, y_bminus - 8.0), p(665.0, y_bminus - 8.0), p(665.0, y_ch_minus), p(cx, y_ch_minus)], thick));
        }
    }
    painter.text(p(lx + 6.0, y_load_minus), Align2::LEFT_CENTER, "−", FontId::monospace(fs * 1.2), NEG);
    painter.text(p(cx + 6.0, y_ch_minus), Align2::LEFT_CENTER, "−", FontId::monospace(fs * 1.2), NEG);

    // NTC — obrzeżem: od góry BMS, nad pakietem, z lewej do środka pakietu
    if b.ntc > 0 {
        let col = Color32::from_rgb(230, 160, 0);
        let y = top - 36.0;
        let mid = tap_y(s / 2) - step / 2.0;
        let path = [p(bx0 + 20.0, top - 20.0), p(bx0 + 20.0, y), p(30.0, y), p(30.0, mid), p(px0, mid)];
        painter.extend(Shape::dashed_line(&path, Stroke::new(1.2, col), 5.0, 4.0));
        painter.text(p(36.0, y + 4.0), Align2::LEFT_TOP, format!("NTC ×{}", b.ntc), FontId::proportional(fs * 0.85), col);
    }

    painter.text(
        rect.left_bottom() + vec2(8.0, -6.0),
        Align2::LEFT_BOTTOM,
        "czerwony = plus (przez bezpiecznik, BMS go nie przerywa) · czarny = minus przez BMS · fioletowy = przewody balansera",
        FontId::proportional(11.0),
        text,
    );
}
