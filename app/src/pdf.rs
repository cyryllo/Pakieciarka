//! Eksport układu i połączeń do PDF (A4 poziomo) — do druku przy ręcznym zgrzewaniu.
//!
//! Strony: góra i dół pakietu (skala 1:1, jeśli się mieści) oraz dane pakietu z listą
//! kontrolną blaszek. Czcionka Ubuntu Light (ta sama co w interfejsie) jest osadzana
//! w PDF, więc polskie znaki drukują się poprawnie.

use crate::app::Project;
use crate::draw::group_color;
use pakiet_core::calc::{pack_stats, recommendations};
use pakiet_core::gcode::Flip;
use pakiet_core::layout::{weld_spots, Layout, Side};
use pdf_writer::types::{CidFontType, FontFlags, LineCapStyle, SystemInfo, TextRenderingMode, UnicodeCmap};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str};
use std::collections::BTreeMap;

const PAGE_W: f32 = 297.0;
const PAGE_H: f32 = 210.0;
const MARGIN: f32 = 12.0;
const K: f32 = 72.0 / 25.4; // mm -> pt

type Rgb = [f32; 3];
const BLACK: Rgb = [0.1, 0.1, 0.12];
const GRAY: Rgb = [0.45, 0.45, 0.48];
const LIGHT: Rgb = [0.85, 0.86, 0.88];
const WHITE: Rgb = [1.0, 1.0, 1.0];
const NICKEL: Rgb = [0.69, 0.71, 0.75];
const SERIES: Rgb = [0.84, 0.47, 0.16];
const POS: Rgb = [0.86, 0.2, 0.18];
const NEG: Rgb = [0.15, 0.43, 0.78];
const SPOT: Rgb = [0.08, 0.6, 0.75];
const PIN1: Rgb = [0.98, 0.8, 0.08];

pub struct PdfOptions {
    pub numbers: bool,
    pub labels: bool,
    pub spots: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Align {
    Left,
    Center,
    Right,
}

struct Font<'a> {
    face: ttf_parser::Face<'a>,
    data: &'a [u8],
    upem: f32,
    used: BTreeMap<u16, char>,
}

impl<'a> Font<'a> {
    fn new(data: &'a [u8]) -> Self {
        let face = ttf_parser::Face::parse(data, 0).expect("czcionka");
        let upem = face.units_per_em() as f32;
        Font { face, data, upem, used: BTreeMap::new() }
    }

    fn gid(&self, ch: char) -> u16 {
        self.face
            .glyph_index(ch)
            .or_else(|| self.face.glyph_index('?'))
            .map(|g| g.0)
            .unwrap_or(0)
    }

    /// Tekst → identyfikatory glifów (Identity-H, 2 bajty big-endian).
    fn encode(&mut self, s: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(s.len() * 2);
        for ch in s.chars() {
            let g = self.gid(ch);
            self.used.entry(g).or_insert(ch);
            out.extend_from_slice(&g.to_be_bytes());
        }
        out
    }

    fn advance(&self, gid: u16) -> f32 {
        self.face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap_or(0) as f32 / self.upem
    }

    /// Szerokość tekstu [mm] dla rozmiaru w punktach.
    fn width(&self, s: &str, size_pt: f32) -> f32 {
        s.chars().map(|c| self.advance(self.gid(c))).sum::<f32>() * size_pt / K
    }
}

struct Doc<'a> {
    font: Font<'a>,
    pages: Vec<Content>,
}

impl Doc<'_> {
    fn page(&mut self) {
        self.pages.push(Content::new());
    }

    fn c(&mut self) -> &mut Content {
        self.pages.last_mut().expect("brak strony")
    }

    fn p(x: f32, y: f32) -> (f32, f32) {
        (x * K, (PAGE_H - y) * K)
    }

    fn circle_path(&mut self, x: f32, y: f32, r: f32) {
        const KAPPA: f32 = 0.552_284_8;
        let k = r * KAPPA;
        let c = self.c();
        let pt = |px: f32, py: f32| Doc::p(px, py);
        let (sx, sy) = pt(x + r, y);
        c.move_to(sx, sy);
        for (a, b, e) in [
            ((x + r, y - k), (x + k, y - r), (x, y - r)),
            ((x - k, y - r), (x - r, y - k), (x - r, y)),
            ((x - r, y + k), (x - k, y + r), (x, y + r)),
            ((x + k, y + r), (x + r, y + k), (x + r, y)),
        ] {
            let (a, b, e) = (pt(a.0, a.1), pt(b.0, b.1), pt(e.0, e.1));
            c.cubic_to(a.0, a.1, b.0, b.1, e.0, e.1);
        }
        c.close_path();
    }

    fn circle(&mut self, x: f32, y: f32, r: f32, fill: Option<Rgb>, stroke: Option<(Rgb, f32)>) {
        self.circle_path(x, y, r);
        self.paint(fill, stroke);
    }

    fn paint(&mut self, fill: Option<Rgb>, stroke: Option<(Rgb, f32)>) {
        let c = self.c();
        if let Some(f) = fill {
            c.set_fill_rgb(f[0], f[1], f[2]);
        }
        if let Some((s, w)) = stroke {
            c.set_stroke_rgb(s[0], s[1], s[2]);
            c.set_line_width(w * K);
        }
        match (fill.is_some(), stroke.is_some()) {
            (true, true) => c.fill_nonzero_and_stroke(),
            (true, false) => c.fill_nonzero(),
            (false, true) => c.stroke(),
            (false, false) => c.end_path(),
        };
    }

    fn line(&mut self, a: (f32, f32), b: (f32, f32), w: f32, col: Rgb) {
        let (a, b) = (Doc::p(a.0, a.1), Doc::p(b.0, b.1));
        let c = self.c();
        c.set_line_cap(LineCapStyle::RoundCap);
        c.set_stroke_rgb(col[0], col[1], col[2]);
        c.set_line_width(w * K);
        c.move_to(a.0, a.1);
        c.line_to(b.0, b.1);
        c.stroke();
        c.set_line_cap(LineCapStyle::ButtCap);
    }

    fn polygon(&mut self, pts: &[(f32, f32)], fill: Option<Rgb>, stroke: Option<(Rgb, f32)>) {
        for (i, &(x, y)) in pts.iter().enumerate() {
            let (px, py) = Doc::p(x, y);
            if i == 0 {
                self.c().move_to(px, py);
            } else {
                self.c().line_to(px, py);
            }
        }
        self.c().close_path();
        self.paint(fill, stroke);
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, fill: Option<Rgb>, stroke: Option<(Rgb, f32)>) {
        self.polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], fill, stroke);
    }

    /// Tekst; `y` to środek wysokości wielkich liter.
    fn text(&mut self, x: f32, y: f32, s: &str, size: f32, col: Rgb, align: Align, bold: bool) {
        let w = self.font.width(s, size);
        let x = match align {
            Align::Left => x,
            Align::Center => x - w / 2.0,
            Align::Right => x - w,
        };
        let baseline = y + size / K * 0.35;
        let (px, py) = Doc::p(x, baseline);
        let bytes = self.font.encode(s);
        let c = self.c();
        c.begin_text();
        c.set_font(Name(b"F1"), size);
        c.set_fill_rgb(col[0], col[1], col[2]);
        if bold {
            c.set_stroke_rgb(col[0], col[1], col[2]);
            c.set_line_width(size * 0.045);
            c.set_text_rendering_mode(TextRenderingMode::FillStroke);
        } else {
            c.set_text_rendering_mode(TextRenderingMode::Fill);
        }
        c.set_text_matrix([1.0, 0.0, 0.0, 1.0, px, py]);
        c.show(Str(&bytes));
        c.end_text();
    }

    fn tag(&mut self, x: f32, y: f32, s: &str, size: f32, fg: Rgb, bg: Rgb) {
        let w = self.font.width(s, size) + 2.4;
        let h = size / K * 1.25;
        self.rect(x - w / 2.0, y - h / 2.0, w, h, Some(bg), None);
        self.text(x, y, s, size, fg, Align::Center, false);
    }

    /// Zawijanie tekstu do szerokości; zwraca linie.
    fn wrap(&self, s: &str, size: f32, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut cur = String::new();
        for word in s.split(' ') {
            let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if self.font.width(&cand, size) > width && !cur.is_empty() {
                lines.push(std::mem::replace(&mut cur, word.to_string()));
            } else {
                cur = cand;
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        lines
    }

    fn checkbox(&mut self, x: f32, y: f32, size: f32) {
        self.rect(x, y - size / 2.0, size, size, Some(WHITE), Some((BLACK, 0.25)));
    }
}

fn rgb(c: egui::Color32) -> Rgb {
    [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0]
}

/// Zakresy numerów: [1,2,3,5] → "1–3, 5".
fn ranges(mut v: Vec<usize>) -> String {
    v.sort_unstable();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1] == v[j] + 1 {
            j += 1;
        }
        out.push(if j > i { format!("{}–{}", v[i], v[j]) } else { v[i].to_string() });
        i = j + 1;
    }
    out.join(", ")
}

fn flip_label(f: Flip) -> &'static str {
    match f {
        Flip::LeftRight => "obróć pakiet na bok (lewa krawędź przechodzi na prawą)",
        Flip::FrontBack => "obróć pakiet do siebie (tylna krawędź przechodzi do przodu)",
    }
}

struct Area {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// Rysuje jedną stronę pakietu w obszarze; zwraca użytą skalę (1.0 = 1:1).
fn draw_side(d: &mut Doc, layout: &Layout, side: Side, flip: Flip, opts: &PdfOptions, project: &Project, area: Area) -> f32 {
    let m = layout.radius * 0.35;
    let (lw, lh) = ((layout.width + 2.0 * m) as f32, (layout.height + 2.0 * m) as f32);
    let sc = (area.w / lw).min(area.h / lh).min(1.0);
    let ox = area.x + (area.w - lw * sc) / 2.0;
    let oy = area.y + (area.h - lh * sc) / 2.0;
    let tr = |x: f64, y: f64| -> (f32, f32) {
        let (x, y) = if side == Side::Bottom { flip.apply(layout.width, layout.height, x, y) } else { (x, y) };
        (ox + (x + m) as f32 * sc, oy + (y + m) as f32 * sc)
    };
    let r = layout.radius as f32 * sc;

    // obrys ze ściętym rogiem przy ogniwie #1
    let corners = [tr(-m, -m), tr(layout.width + m, -m), tr(layout.width + m, layout.height + m), tr(-m, layout.height + m)];
    let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.0), a.1.max(c.0)));
    let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.1), a.1.max(c.1)));
    let ring = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    let first = tr(layout.cells[0].x, layout.cells[0].y);
    let dist = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let notch = (0..4).min_by(|&a, &b| dist(ring[a], first).total_cmp(&dist(ring[b], first))).unwrap_or(0);
    let cut = (r * 1.1).clamp(3.0, 12.0);
    let mut outline = Vec::new();
    for (i, &c) in ring.iter().enumerate() {
        if i == notch {
            for n in [ring[(i + 3) % 4], ring[(i + 1) % 4]] {
                let l = dist(n, c);
                outline.push((c.0 + (n.0 - c.0) / l * cut, c.1 + (n.1 - c.1) / l * cut));
            }
        } else {
            outline.push(c);
        }
    }
    d.polygon(&outline, None, Some((GRAY, 0.35)));

    for c in &layout.cells {
        let (x, y) = tr(c.x, c.y);
        d.circle(x, y, r, Some(rgb(group_color(c.group, false))), Some((GRAY, 0.2)));
    }
    let strip_w = (8.0 * sc).min(r * 0.95);
    for l in layout.links_on(side) {
        let (a, b) = (&layout.cells[l.a], &layout.cells[l.b]);
        d.line(tr(a.x, a.y), tr(b.x, b.y), strip_w, if l.series { SERIES } else { NICKEL });
    }
    for c in &layout.cells {
        let (x, y) = tr(c.x, c.y);
        d.circle(x, y, strip_w * 0.62, Some(NICKEL), None);
    }

    let fs = (r * K * 0.75).clamp(5.0, 16.0);
    for (i, c) in layout.cells.iter().enumerate() {
        let (x, y) = tr(c.x, c.y);
        if layout.positive_on(i, side) {
            d.circle(x, y, r * 0.36, Some([0.95, 0.95, 0.95]), Some((POS, 0.3)));
            d.text(x, y, "+", fs * 0.8, POS, Align::Center, true);
        } else {
            d.text(x, y, "−", fs, NEG, Align::Center, true);
        }
        if opts.numbers && r >= 3.0 {
            d.text(x, y - r * 0.64, &format!("{}", i + 1), (r * K * 0.3).clamp(4.5, 11.0), BLACK, Align::Center, true);
        }
        if opts.labels && r >= 5.0 {
            d.text(x, y + r * 0.64, &format!("G{}", c.group + 1), (r * K * 0.26).clamp(4.0, 9.0), BLACK, Align::Center, false);
        }
    }

    if opts.spots {
        let g = &project.gcode;
        for s in weld_spots(layout, g.spots_per_cell, g.spot_offset_mm, g.spot_angle_deg) {
            let (x, y) = tr(s.x, s.y);
            d.circle(x, y, (0.6 * sc).max(0.35), Some(SPOT), None);
        }
    }

    // znacznik #1
    let corner = ring[notch];
    let l = dist(corner, first);
    let dot = (first.0 + (corner.0 - first.0) / l * r * 0.78, first.1 + (corner.1 - first.1) / l * r * 0.78);
    d.circle(dot.0, dot.1, (r * 0.2).clamp(0.8, 2.5), Some(PIN1), Some((BLACK, 0.2)));

    if opts.labels {
        let size = (r * K * 0.42).clamp(5.0, 10.0);
        for plate in layout.plates_on(side) {
            let (x, y) = tr(plate.cx, plate.cy);
            let (txt, bg) = if plate.tap == 0 {
                ("B− (B0)".to_string(), BLACK)
            } else if plate.tap == layout.s {
                (format!("B+ (B{})", plate.tap), POS)
            } else {
                (format!("B{}", plate.tap), [0.3, 0.3, 0.35])
            };
            d.tag(x, y, &txt, size, WHITE, bg);
        }
    }
    sc
}

fn legend(d: &mut Doc, x: f32, y: f32) {
    let mut x = x;
    let mut item = |d: &mut Doc, col: Option<Rgb>, label: &str| -> f32 {
        if let Some(col) = col {
            d.line((x, y), (x + 7.0, y), 2.2, col);
            x += 9.0;
        }
        d.text(x, y, label, 7.5, BLACK, Align::Left, false);
        x += d.font.width(label, 7.5) + 6.0;
        x
    };
    item(d, Some(NICKEL), "nikiel — równolegle");
    item(d, Some(SERIES), "nikiel — szeregowo");
    item(d, None, "+ biegun dodatni   − biegun ujemny");
    let x = item(d, None, "Bn = odczep balansera");
    d.circle(x + 1.2, y, 1.2, Some(PIN1), Some((BLACK, 0.2)));
    d.text(x + 4.0, y, "i ścięty róg obrysu = ogniwo #1", 7.5, BLACK, Align::Left, false);
}

/// Linijka kontrolna 50 mm — do sprawdzenia, czy wydruk ma skalę 100%.
fn ruler(d: &mut Doc, x: f32, y: f32, sc: f32) {
    let len = 50.0 * sc;
    d.line((x, y), (x + len, y), 0.4, BLACK);
    for i in 0..=5 {
        let xx = x + i as f32 * 10.0 * sc;
        let h = if i % 5 == 0 { 2.5 } else { 1.5 };
        d.line((xx, y - h), (xx, y), 0.3, BLACK);
    }
    let label = if (sc - 1.0).abs() < 1e-3 {
        "50 mm — skala 1:1 (drukuj w 100%, bez dopasowania do strony)".to_string()
    } else {
        format!("50 mm — skala 1:{:.2} (pakiet nie mieści się na A4 w 1:1)", 1.0 / sc)
    };
    d.text(x + len + 3.0, y - 1.0, &label, 7.0, BLACK, Align::Left, false);
}

pub fn layout_pdf(project: &Project, layout: &Layout, opts: &PdfOptions) -> Vec<u8> {
    let mut d = Doc { font: Font::new(epaint_default_fonts::UBUNTU_LIGHT), pages: Vec::new() };
    let flip = project.gcode.bottom_flip;
    let area = || Area { x: MARGIN, y: 30.0, w: PAGE_W - 2.0 * MARGIN, h: PAGE_H - 30.0 - 26.0 };

    // --- strona: góra
    d.page();
    d.text(MARGIN, 32.0, "GÓRA — widok z góry", 11.0, BLACK, Align::Left, true);
    let sc = draw_side(&mut d, layout, Side::Top, flip, opts, project, Area { y: 37.0, h: PAGE_H - 37.0 - 24.0, ..area() });
    legend(&mut d, MARGIN, PAGE_H - 18.0);
    ruler(&mut d, MARGIN, PAGE_H - 11.0, sc);

    // --- strona: dół
    d.page();
    d.text(MARGIN, 32.0, &format!("DÓŁ — {}", flip_label(flip)), 11.0, BLACK, Align::Left, true);
    let sc = draw_side(&mut d, layout, Side::Bottom, flip, opts, project, Area { y: 37.0, h: PAGE_H - 37.0 - 24.0, ..area() });
    legend(&mut d, MARGIN, PAGE_H - 18.0);
    ruler(&mut d, MARGIN, PAGE_H - 11.0, sc);

    // --- strona: dane i lista kontrolna
    info_pages(&mut d, project, layout);

    // nagłówki
    let n = d.pages.len();
    let st = pack_stats(&project.cell, project.s, project.p, project.load_a);
    let date = crate::store::format_time(crate::store::now_ms());
    for i in 0..n {
        d.pages.swap(i, n - 1);
        d.text(MARGIN, 14.0, &project.name, 15.0, BLACK, Align::Left, true);
        d.text(
            MARGIN,
            21.0,
            &format!(
                "{}S{}P · {} · {:.1} V ({:.1}–{:.1} V) · {:.2} Ah · {:.0} Wh · {} ogniw",
                st.s, st.p, project.cell.name, st.v_nom, st.v_min, st.v_max, st.capacity_ah, st.energy_wh, st.cells
            ),
            9.0,
            GRAY,
            Align::Left,
            false,
        );
        d.text(PAGE_W - MARGIN, 14.0, &format!("Strona {}/{}", i + 1, n), 9.0, GRAY, Align::Right, false);
        d.text(PAGE_W - MARGIN, 21.0, &format!("Pakieciarka {} · {date}", env!("CARGO_PKG_VERSION")), 7.5, GRAY, Align::Right, false);
        d.line((MARGIN, 25.0), (PAGE_W - MARGIN, 25.0), 0.25, LIGHT);
        d.text(MARGIN, PAGE_H - 4.5, pakiet_core::disclaimer::SHORT, 6.0, GRAY, Align::Left, false);
        d.pages.swap(i, n - 1);
    }

    finish(d)
}

fn info_pages(d: &mut Doc, project: &Project, layout: &Layout) {
    let cell = &project.cell;
    let st = pack_stats(cell, project.s, project.p, project.load_a);
    let rec = recommendations(cell, &st, &project.strip_settings(), layout.min_series_links, layout.pitch);
    d.page();
    d.text(MARGIN, 32.0, "Dane pakietu i lista kontrolna zgrzewania", 11.0, BLACK, Align::Left, true);

    // lewa kolumna: parametry + odczepy
    let col_w = (PAGE_W - 2.0 * MARGIN - 10.0) / 2.0;
    let mut y = 40.0;
    let kv = |d: &mut Doc, y: &mut f32, k: &str, v: String| {
        d.text(MARGIN, *y, k, 8.0, GRAY, Align::Left, false);
        for (i, line) in d.wrap(&v, 8.0, col_w - 52.0).iter().enumerate() {
            d.text(MARGIN + 52.0, *y + i as f32 * 3.8, line, 8.0, BLACK, Align::Left, false);
            if i > 0 {
                *y += 3.8;
            }
        }
        *y += 4.6;
    };
    let sp = &rec.strip;
    let (awg, mm2, _) = rec.main_wire;
    for (k, v) in [
        ("Ogniwo", format!("{} ({}, {:.0} mAh, {:.1} A)", cell.name, cell.chemistry.label(), cell.capacity_mah, cell.max_cont_a)),
        ("Konfiguracja", format!("{}S{}P — {} ogniw", st.s, st.p, st.cells)),
        ("Napięcie", format!("{:.1} V nom. · {:.1} V maks. · {:.1} V min.", st.v_nom, st.v_max, st.v_min)),
        ("Pojemność / energia", format!("{:.2} Ah · {:.0} Wh", st.capacity_ah, st.energy_wh)),
        ("Prąd ciągły maks.", format!("{:.1} A (projektowy {:.1} A)", st.max_cont_a, st.load_a)),
        ("Wymiary bloku", format!("{:.0} × {:.0} × {:.0} mm", layout.width, layout.height, cell.length_mm)),
        ("Masa (szac.)", format!("{:.2} kg", st.weight_est_g / 1000.0)),
        ("BMS", rec.bms.clone()),
        ("Ładowarka", format!("{} · {:.1} A", rec.charger, rec.charge_current_a)),
        ("Nikiel — szeregowo", format!("{} ({:.1} A na pasek, {} pask. na złączu)", sp.series.label(), sp.series_current_a, sp.series_links)),
        ("Nikiel — równolegle", format!("{} ({:.1} A na pasek)", sp.parallel.label(), sp.parallel_current_a)),
        ("Nikiel — bieguny B+/B−", format!("{} ({:.1} A, przewód w środku blaszki)", sp.terminal.label(), sp.terminal_current_a)),
        ("Taśma", format!("{} · zgrzewarka do {:.2} mm", project.strip.label(), sp.max_thickness_mm)),
        ("Przewody", format!("główne {awg} AWG ({mm2} mm²) · balanser {}", rec.balance_wire)),
        ("Bezpiecznik", format!("{:.0} A na B+", rec.fuse_a)),
    ] {
        kv(d, &mut y, k, v);
    }

    y += 3.0;
    d.text(MARGIN, y, "Odczepy balansera (mierz narastająco od B−)", 9.0, BLACK, Align::Left, true);
    y += 5.0;
    let mut plates: Vec<_> = layout.plates.iter().collect();
    plates.sort_by_key(|p| p.tap);
    let cols = [0.0, 22.0, 38.0, 60.0];
    for (i, h) in ["Odczep", "Strona", "Nominalnie", "Naładowany"].iter().enumerate() {
        d.text(MARGIN + cols[i], y, h, 7.5, GRAY, Align::Left, true);
    }
    y += 4.0;
    for pl in &plates {
        if y > PAGE_H - 12.0 {
            break;
        }
        let name = match pl.tap {
            0 => "B0 (B−)".to_string(),
            t if t == layout.s => format!("B{t} (B+)"),
            t => format!("B{t}"),
        };
        for (i, v) in [
            name,
            pl.side.label().to_string(),
            format!("{:.1} V", pl.tap as f64 * cell.nominal_v),
            format!("{:.2} V", pl.tap as f64 * cell.max_v),
        ]
        .iter()
        .enumerate()
        {
            d.text(MARGIN + cols[i], y, v, 7.5, BLACK, Align::Left, false);
        }
        y += 3.6;
    }

    // prawa kolumna: lista kontrolna blaszek
    let x = MARGIN + col_w + 10.0;
    let mut y = 40.0;
    d.text(x, y, "Lista kontrolna — blaszki niklowe", 9.0, BLACK, Align::Left, true);
    y += 5.0;
    let steps = [
        "Posegreguj ogniwa (napięcie, IR); ułóż wg strony GÓRA: ogniwo #1 przy ściętym rogu.".to_string(),
        format!("Zgrzej górę, potem {}, i zgrzej dół.", flip_label(project.gcode.bottom_flip)),
        "Pomarańczowe = połączenia szeregowe (pełny prąd) — nie oszczędzaj niklu.".to_string(),
    ];
    for s in &steps {
        for line in d.wrap(&format!("• {s}"), 7.5, col_w) {
            d.text(x, y, &line, 7.5, BLACK, Align::Left, false);
            y += 3.6;
        }
    }
    y += 2.0;
    for side in [Side::Top, Side::Bottom] {
        if y > PAGE_H - 16.0 {
            d.page();
            y = 40.0;
        }
        d.text(x, y, side.label(), 8.5, BLACK, Align::Left, true);
        y += 4.5;
        let mut side_plates: Vec<_> = layout.plates_on(side).collect();
        side_plates.sort_by_key(|p| p.tap);
        for pl in side_plates {
            let groups = pl
                .groups
                .iter()
                .map(|&g| {
                    let cells: Vec<usize> = pl.cells.iter().filter(|&&c| layout.cells[c].group == g).map(|&c| c + 1).collect();
                    format!("G{} [{}]", g + 1, ranges(cells))
                })
                .collect::<Vec<_>>()
                .join(" + ");
            let kind = if pl.groups.len() > 1 { "szeregowo" } else if pl.tap == 0 { "biegun B−" } else { "biegun B+" };
            let strip = if pl.groups.len() > 1 { sp.series.label() } else { sp.terminal.label() };
            let text = format!("B{} — {kind}: {groups} · nikiel {strip}", pl.tap);
            let lines = d.wrap(&text, 7.5, col_w - 7.0);
            if y + lines.len() as f32 * 3.6 > PAGE_H - 10.0 {
                d.page();
                d.text(MARGIN, 32.0, "Lista kontrolna — ciąg dalszy", 11.0, BLACK, Align::Left, true);
                y = 40.0;
            }
            d.checkbox(x, y, 2.8);
            for line in lines {
                d.text(x + 5.0, y, &line, 7.5, BLACK, Align::Left, false);
                y += 3.6;
            }
            y += 0.8;
        }
        y += 2.0;
    }
}

fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

fn finish(d: Doc) -> Vec<u8> {
    let Doc { font, pages } = d;
    let mut pdf = Pdf::new();
    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog_id = alloc();
    let pages_id = alloc();
    let font_id = alloc();
    let cid_id = alloc();
    let desc_id = alloc();
    let file_id = alloc();
    let cmap_id = alloc();
    let page_ids: Vec<(Ref, Ref)> = pages.iter().map(|_| (alloc(), alloc())).collect();

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id).kids(page_ids.iter().map(|p| p.0)).count(page_ids.len() as i32);

    for (content, &(page_id, content_id)) in pages.into_iter().zip(&page_ids) {
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, PAGE_W * K, PAGE_H * K));
        page.parent(pages_id);
        page.contents(content_id);
        page.resources().fonts().pair(Name(b"F1"), font_id);
        page.finish();
        let data = deflate(&content.finish());
        pdf.stream(content_id, &data).filter(Filter::FlateDecode);
    }

    // czcionka: Type0 / CIDFontType2, kodowanie Identity-H (identyfikatory glifów)
    let base = Name(b"UbuntuLight");
    let sys = SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 };
    pdf.type0_font(font_id)
        .base_font(base)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid_id)
        .to_unicode(cmap_id);
    {
        let mut cid = pdf.cid_font(cid_id);
        cid.subtype(CidFontType::Type2);
        cid.base_font(base);
        cid.system_info(sys);
        cid.font_descriptor(desc_id);
        cid.cid_to_gid_map_predefined(Name(b"Identity"));
        let mut w = cid.widths();
        for &gid in font.used.keys() {
            w.consecutive(gid, [font.advance(gid) * 1000.0]);
        }
        w.finish();
    }
    let face = &font.face;
    let s = 1000.0 / font.upem;
    let bb = face.global_bounding_box();
    pdf.font_descriptor(desc_id)
        .name(base)
        .flags(FontFlags::NON_SYMBOLIC)
        .bbox(Rect::new(bb.x_min as f32 * s, bb.y_min as f32 * s, bb.x_max as f32 * s, bb.y_max as f32 * s))
        .italic_angle(0.0)
        .ascent(face.ascender() as f32 * s)
        .descent(face.descender() as f32 * s)
        .cap_height(face.capital_height().unwrap_or(700) as f32 * s)
        .stem_v(70.0)
        .font_file2(file_id);
    let font_data = deflate(font.data);
    pdf.stream(file_id, &font_data)
        .filter(Filter::FlateDecode)
        .pair(Name(b"Length1"), font.data.len() as i32);

    let mut cmap = UnicodeCmap::new(Name(b"Custom"), sys);
    for (&gid, &ch) in &font.used {
        cmap.pair(gid, ch);
    }
    let cmap_data = cmap.finish();
    pdf.cmap(cmap_id, &cmap_data);

    pdf.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pakiet_core::layout::build_layout;

    #[test]
    fn generates_pdf() {
        let p = Project::default();
        let l = build_layout(p.s, p.p, p.cell.diameter_mm, &p.layout);
        let bytes = layout_pdf(&p, &l, &PdfOptions { numbers: true, labels: true, spots: true });
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 10_000);
        assert_eq!(ranges(vec![5, 1, 2, 3, 8]), "1–3, 5, 8");
        // PAKIECIARKA_PDF_OUT=katalog — zapis przykładowych PDF do obejrzenia
        if let Some(dir) = std::env::var_os("PAKIECIARKA_PDF_OUT") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::write(dir.join("10s4p.pdf"), &bytes).unwrap();
            let big = Project { s: 14, p: 10, ..Project::default() };
            let l = build_layout(big.s, big.p, big.cell.diameter_mm, &big.layout);
            std::fs::write(dir.join("14s10p.pdf"), layout_pdf(&big, &l, &PdfOptions { numbers: true, labels: true, spots: false })).unwrap();
        }
    }
}
