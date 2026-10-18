//! Rysowanie: układ ogniw z blaszkami, schemat modułów, podgląd ścieżki G-code.

use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use pakiet_core::gcode::{to_machine, Flip, GcodeOutput, GcodeSettings};
use pakiet_core::layout::{Layout, Side, WeldSpot};
use pakiet_core::modules::ModulePlan;

pub const NICKEL: Color32 = Color32::from_rgba_premultiplied(176, 182, 190, 215);
pub const SERIES: Color32 = Color32::from_rgba_premultiplied(214, 120, 40, 225);
pub const POS: Color32 = Color32::from_rgb(220, 50, 47);
pub const NEG: Color32 = Color32::from_rgb(38, 110, 200);
pub const SPOT: Color32 = Color32::from_rgb(20, 200, 230);

pub fn group_color(g: usize, dark: bool) -> Color32 {
    let h = (g as f32 * 0.618_034).fract();
    let (s, v) = if dark { (0.45, 0.55) } else { (0.35, 0.92) };
    egui::ecolor::Hsva::new(h, s, v, 1.0).into()
}

fn canvas_bg(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_gray(24)
    } else {
        Color32::from_gray(246)
    }
}

/// Dopasowanie obiektu `w`×`h` [mm] do prostokąta z marginesem.
struct Fit {
    off: Pos2,
    sc: f32,
}

impl Fit {
    fn new(rect: Rect, w: f64, h: f64, margin: f32) -> Self {
        let avail = rect.shrink(margin);
        let sc = (avail.width() / w.max(1.0) as f32).min(avail.height() / h.max(1.0) as f32);
        let off = avail.center() - vec2(w as f32 * sc / 2.0, h as f32 * sc / 2.0);
        Fit { off, sc }
    }
    fn p(&self, x: f64, y: f64) -> Pos2 {
        pos2(self.off.x + x as f32 * self.sc, self.off.y + y as f32 * self.sc)
    }
}

fn tag(painter: &egui::Painter, pos: Pos2, text: &str, size: f32, fg: Color32, bg: Color32) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(size), fg);
    let rect = Rect::from_center_size(pos, galley.size() + vec2(8.0, 4.0));
    painter.rect_filled(rect, 4.0, bg);
    painter.galley(rect.min + vec2(4.0, 2.0), galley, fg);
}

pub struct PackView<'a> {
    pub side: Side,
    /// Widok dołu jak po obróceniu pakietu wokół osi pionowej
    /// `Some` = dół pokazany tak, jak go widać po obróceniu pakietu
    pub flip: Option<Flip>,
    pub spots: Option<&'a [WeldSpot]>,
    pub labels: bool,
    /// Numery ogniw #1…#N
    pub numbers: bool,
}

/// Rysuje jedną stronę pakietu. Zwraca indeks ogniwa pod kursorem.
pub fn draw_pack(ui: &mut Ui, layout: &Layout, view: &PackView, size: Vec2) -> Option<usize> {
    let dark = ui.visuals().dark_mode;
    let (resp, painter) = ui.allocate_painter(size, Sense::hover());
    let rect = resp.rect;
    painter.rect_filled(rect, 6.0, canvas_bg(ui));
    // margines obrysu wokół ogniw [mm]
    let m = layout.radius * 0.35;
    let fit = Fit::new(rect, layout.width + 2.0 * m, layout.height + 2.0 * m, 26.0);
    let flip = view.flip.filter(|_| view.side == Side::Bottom);
    let tr = |x: f64, y: f64| {
        let (x, y) = match flip {
            Some(f) => f.apply(layout.width, layout.height, x, y),
            None => (x, y),
        };
        fit.p(x + m, y + m)
    };
    let r = layout.radius as f32 * fit.sc;
    let text_col = ui.visuals().text_color();

    let title = match (view.side, flip) {
        (Side::Top, _) => "GÓRA — widok z góry",
        (Side::Bottom, Some(Flip::LeftRight)) => "DÓŁ — po obróceniu na bok (lewo ↔ prawo)",
        (Side::Bottom, Some(Flip::FrontBack)) => "DÓŁ — po obróceniu do siebie (przód ↔ tył)",
        (Side::Bottom, None) => "DÓŁ — prześwietlony z góry (bez obracania)",
    };
    painter.text(rect.left_top() + vec2(8.0, 6.0), Align2::LEFT_TOP, title, FontId::proportional(13.0), text_col);

    // obrys pakietu ze ściętym rogiem przy ogniwie #1 (jak „pin 1” w układach scalonych)
    let corners = [tr(-m, -m), tr(layout.width + m, -m), tr(layout.width + m, layout.height + m), tr(-m, layout.height + m)];
    let first = tr(layout.cells[0].x, layout.cells[0].y);
    let pin1 = pack_outline(&painter, corners, first, layout.radius as f32 * fit.sc, dark);

    // ogniwa
    for c in &layout.cells {
        painter.circle(tr(c.x, c.y), r, group_color(c.group, dark), Stroke::new(1.0, Color32::from_black_alpha(90)));
    }

    // blaszki: pod biegunem każdego ogniwa oraz paski między sąsiadami
    let strip_w = (8.0 * fit.sc).min(r * 0.95).max(1.5);
    for link in layout.links_on(view.side) {
        let (a, b) = (&layout.cells[link.a], &layout.cells[link.b]);
        let col = if link.series { SERIES } else { NICKEL };
        painter.line_segment([tr(a.x, a.y), tr(b.x, b.y)], Stroke::new(strip_w, col));
    }
    for c in &layout.cells {
        painter.circle_filled(tr(c.x, c.y), strip_w * 0.62, NICKEL);
    }

    // bieguny
    let fs = (r * 0.9).clamp(6.0, 22.0);
    if r >= 4.0 {
        for (i, c) in layout.cells.iter().enumerate() {
            let p = tr(c.x, c.y);
            if layout.positive_on(i, view.side) {
                painter.circle(p, r * 0.36, Color32::from_gray(235), Stroke::new(1.0, POS));
                painter.text(p, Align2::CENTER_CENTER, "+", FontId::monospace(fs * 0.8), POS);
            } else {
                painter.text(p, Align2::CENTER_CENTER, "−", FontId::monospace(fs), NEG);
            }
            if view.numbers && r >= 9.0 {
                painter.text(
                    p - vec2(0.0, r * 0.64),
                    Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    FontId::proportional((r * 0.36).clamp(7.0, 13.0)),
                    Color32::from_gray(25),
                );
            }
            if view.labels && r >= 14.0 {
                painter.text(
                    p + vec2(0.0, r * 0.62),
                    Align2::CENTER_CENTER,
                    format!("G{}", c.group + 1),
                    FontId::proportional((r * 0.38).clamp(8.0, 13.0)),
                    Color32::from_gray(30),
                );
            }
        }
    }

    pin1_dot(&painter, first, pin1, r);

    // punkty zgrzewów
    if let Some(spots) = view.spots {
        let sr = (0.9 * fit.sc).clamp(1.2, 3.5);
        for s in spots {
            painter.circle_filled(tr(s.x, s.y), sr, SPOT);
        }
    }

    // odczepy balansera
    if view.labels {
        let size = (r * 0.55).clamp(9.0, 14.0);
        for plate in layout.plates_on(view.side) {
            let p = tr(plate.cx, plate.cy);
            let (txt, bg) = if plate.tap == 0 {
                ("B− (B0)".to_string(), Color32::from_rgb(30, 30, 30))
            } else if plate.tap == layout.s {
                (format!("B+ (B{})", plate.tap), POS)
            } else {
                (format!("B{}", plate.tap), Color32::from_rgba_unmultiplied(70, 70, 80, 230))
            };
            tag(&painter, p, &txt, size, Color32::WHITE, bg);
        }
    }

    // kursor
    let hover = resp.hover_pos().and_then(|hp| {
        layout
            .cells
            .iter()
            .enumerate()
            .find(|(_, c)| tr(c.x, c.y).distance(hp) <= r)
            .map(|(i, _)| i)
    });
    if let Some(i) = hover {
        let c = &layout.cells[i];
        painter.circle_stroke(tr(c.x, c.y), r + 2.0, Stroke::new(2.0, ui.visuals().selection.stroke.color));
    }
    hover
}

/// Obrys pakietu ze ściętym rogiem najbliżej ogniwa #1. `corners` — narożniki obrysu
/// na ekranie (w dowolnej kolejności obiegu). Zwraca pozycję ściętego narożnika.
fn pack_outline(painter: &egui::Painter, corners: [Pos2; 4], first: Pos2, r: f32, dark: bool) -> Pos2 {
    let bb = Rect::from_points(&corners);
    let ring = [bb.left_top(), bb.right_top(), bb.right_bottom(), bb.left_bottom()];
    let notch = (0..4).min_by(|&a, &b| ring[a].distance(first).total_cmp(&ring[b].distance(first))).unwrap_or(0);
    let cut = (r * 1.1).clamp(6.0, 34.0);
    let mut outline = Vec::with_capacity(5);
    for (i, &c) in ring.iter().enumerate() {
        if i == notch {
            outline.push(c + (ring[(i + 3) % 4] - c).normalized() * cut);
            outline.push(c + (ring[(i + 1) % 4] - c).normalized() * cut);
        } else {
            outline.push(c);
        }
    }
    let col = if dark { Color32::from_gray(150) } else { Color32::from_gray(90) };
    painter.add(Shape::closed_line(outline, Stroke::new(1.5, col)));
    ring[notch]
}

/// Żółta kropka na ogniwie #1, od strony ściętego rogu.
fn pin1_dot(painter: &egui::Painter, first: Pos2, corner: Pos2, r: f32) {
    let dot = first + (corner - first).normalized() * r * 0.78;
    painter.circle(dot, (r * 0.2).clamp(3.0, 7.0), Color32::from_rgb(250, 204, 21), Stroke::new(1.0, Color32::from_gray(20)));
}

pub fn draw_legend(ui: &mut Ui) {
    let item = |ui: &mut Ui, col: Color32, w: f32, txt: &str| {
        let (rect, _) = ui.allocate_exact_size(vec2(26.0, 14.0), Sense::hover());
        ui.painter().line_segment([rect.left_center(), rect.right_center()], Stroke::new(w, col));
        ui.label(txt);
    };
    ui.horizontal_wrapped(|ui| {
        item(ui, NICKEL, 7.0, "nikiel — połączenie równoległe");
        ui.add_space(10.0);
        item(ui, SERIES, 7.0, "nikiel — połączenie szeregowe");
        ui.add_space(10.0);
        ui.colored_label(POS, "+ biegun dodatni");
        ui.colored_label(NEG, "− biegun ujemny");
        ui.add_space(10.0);
        ui.colored_label(SPOT, "• punkt zgrzewu");
        ui.add_space(10.0);
        ui.label("Bn = odczep balansera");
        ui.add_space(10.0);
        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        ui.painter().circle(rect.center(), 5.0, Color32::from_rgb(250, 204, 21), Stroke::new(1.0, Color32::from_gray(20)));
        ui.label("+ ścięty róg obrysu = ogniwo #1");
    });
}

/// Schemat łączenia modułów: kolumny = łańcuch szeregowy, wiersze = łańcuchy równoległe.
pub fn draw_modules(ui: &mut Ui, plan: &ModulePlan, size: Vec2) {
    let dark = ui.visuals().dark_mode;
    let (resp, painter) = ui.allocate_painter(size, Sense::hover());
    let rect = resp.rect;
    painter.rect_filled(rect, 6.0, canvas_bg(ui));
    let text_col = ui.visuals().text_color();

    let (cols, rows) = (plan.series.max(1), plan.parallel.max(1));
    let (bw, bh, gx, gy) = (150.0f64, 80.0f64, 90.0f64, 46.0f64);
    let total_w = cols as f64 * bw + (cols - 1) as f64 * gx + 2.0 * 70.0;
    let total_h = rows as f64 * bh + (rows - 1) as f64 * gy + 60.0;
    let fit = Fit::new(rect, total_w, total_h, 16.0);
    let sc = fit.sc;
    let box_rect = |c: usize, r: usize| {
        let x = 70.0 + c as f64 * (bw + gx);
        let y = 30.0 + r as f64 * (bh + gy);
        Rect::from_min_max(fit.p(x, y), fit.p(x + bw, y + bh))
    };
    let wire = |w: f32| Stroke::new((w * sc).max(1.5), if dark { Color32::from_gray(200) } else { Color32::from_gray(60) });
    let fs = (14.0 * sc).clamp(8.0, 18.0);

    for r in 0..rows {
        for c in 0..cols {
            let b = box_rect(c, r);
            painter.rect_filled(b, 6.0, group_color(c, dark));
            painter.rect_stroke(b, 6.0, Stroke::new(1.5, Color32::from_black_alpha(120)), egui::StrokeKind::Inside);
            let (t0, t1) = plan.taps(c);
            painter.text(
                b.center() - vec2(0.0, fs * 0.7),
                Align2::CENTER_CENTER,
                format!("M{}  {}S{}P", r * cols + c + 1, plan.module_s, plan.module_p),
                FontId::proportional(fs),
                Color32::from_gray(20),
            );
            painter.text(
                b.center() + vec2(0.0, fs * 0.7),
                Align2::CENTER_CENTER,
                format!("B{t0}…B{t1}"),
                FontId::proportional(fs * 0.85),
                Color32::from_gray(40),
            );
            painter.text(b.left_center() + vec2(8.0 * sc, 0.0), Align2::LEFT_CENTER, "−", FontId::monospace(fs * 1.3), NEG);
            painter.text(b.right_center() - vec2(8.0 * sc, 0.0), Align2::RIGHT_CENTER, "+", FontId::monospace(fs * 1.3), POS);
            // szeregowo do następnego modułu
            if c + 1 < cols {
                let nb = box_rect(c + 1, r);
                painter.line_segment([b.right_center(), nb.left_center()], Stroke::new((4.0 * sc).max(2.0), SERIES));
            }
        }
    }

    // szyny równoległe: na końcach oraz między modułami (połączone węzły grup)
    if rows > 1 {
        let top = box_rect(0, 0);
        let bot = box_rect(0, rows - 1);
        let x_neg = top.left() - 30.0 * sc;
        painter.line_segment([pos2(x_neg, top.center().y), pos2(x_neg, bot.center().y)], wire(4.0));
        for r in 0..rows {
            let b = box_rect(0, r);
            painter.line_segment([pos2(x_neg, b.center().y), b.left_center()], wire(4.0));
        }
        let top = box_rect(cols - 1, 0);
        let bot = box_rect(cols - 1, rows - 1);
        let x_pos = top.right() + 30.0 * sc;
        painter.line_segment([pos2(x_pos, top.center().y), pos2(x_pos, bot.center().y)], wire(4.0));
        for r in 0..rows {
            let b = box_rect(cols - 1, r);
            painter.line_segment([b.right_center(), pos2(x_pos, b.center().y)], wire(4.0));
        }
        for c in 0..cols.saturating_sub(1) {
            let x = (box_rect(c, 0).right() + box_rect(c + 1, 0).left()) / 2.0;
            let y0 = box_rect(c, 0).center().y;
            let y1 = box_rect(c, rows - 1).center().y;
            painter.extend(Shape::dashed_line(&[pos2(x, y0), pos2(x, y1)], wire(2.0), 6.0, 4.0));
        }
        // balanser między modułami równoległymi
        for c in 0..cols {
            for r in 0..rows - 1 {
                let a = box_rect(c, r);
                let b = box_rect(c, r + 1);
                painter.extend(Shape::dashed_line(
                    &[a.center_bottom(), b.center_top()],
                    Stroke::new(1.5, Color32::from_rgb(140, 90, 200)),
                    4.0,
                    3.0,
                ));
            }
        }
        let first = box_rect(0, 0);
        let last = box_rect(cols - 1, 0);
        painter.text(pos2(x_neg, first.top() - 6.0), Align2::CENTER_BOTTOM, "B−", FontId::proportional(fs), NEG);
        painter.text(pos2(x_pos, last.top() - 6.0), Align2::CENTER_BOTTOM, "B+", FontId::proportional(fs), POS);
    } else {
        let first = box_rect(0, 0);
        let last = box_rect(cols - 1, 0);
        let l = first.left_center() - vec2(40.0 * sc, 0.0);
        let rr = last.right_center() + vec2(40.0 * sc, 0.0);
        painter.line_segment([l, first.left_center()], wire(4.0));
        painter.line_segment([last.right_center(), rr], wire(4.0));
        painter.text(l, Align2::RIGHT_CENTER, "B− ", FontId::proportional(fs), NEG);
        painter.text(rr, Align2::LEFT_CENTER, " B+", FontId::proportional(fs), POS);
    }
    painter.text(
        rect.left_bottom() + vec2(8.0, -6.0),
        Align2::LEFT_BOTTOM,
        "pomarańczowe = szeregowo · szare = szyny równoległe · przerywane = łączenie węzłów i balansera",
        FontId::proportional(12.0),
        text_col,
    );
}

/// Podgląd ścieżki głowicy we współrzędnych maszyny (X w prawo, Y w górę).
pub fn draw_gcode_preview(ui: &mut Ui, layout: &Layout, st: &GcodeSettings, out: &GcodeOutput, size: Vec2) {
    let dark = ui.visuals().dark_mode;
    let (resp, painter) = ui.allocate_painter(size, Sense::hover());
    let rect = resp.rect;
    painter.rect_filled(rect, 6.0, canvas_bg(ui));
    let text_col = ui.visuals().text_color();

    let max_x = st.origin_x + layout.width + 5.0;
    let max_y = st.origin_y + layout.height + 5.0;
    let fit = Fit::new(rect, max_x, max_y, 26.0);
    // Y maszyny w górę
    let tr = |x: f64, y: f64| fit.p(x, max_y - y);

    // osie
    let o = tr(0.0, 0.0);
    painter.line_segment([o, tr(max_x, 0.0)], Stroke::new(1.0, Color32::from_rgb(200, 60, 60)));
    painter.line_segment([o, tr(0.0, max_y)], Stroke::new(1.0, Color32::from_rgb(60, 170, 60)));
    painter.text(tr(max_x, 0.0), Align2::RIGHT_TOP, "X", FontId::proportional(12.0), text_col);
    painter.text(tr(0.0, max_y), Align2::LEFT_TOP, " Y", FontId::proportional(12.0), text_col);

    let r = layout.radius as f32 * fit.sc;
    let m = layout.radius * 0.35;
    let mc = |x: f64, y: f64| {
        let (x, y) = to_machine(layout, st, x, y);
        tr(x, y)
    };
    let corners = [mc(-m, -m), mc(layout.width + m, -m), mc(layout.width + m, layout.height + m), mc(-m, layout.height + m)];
    let first = mc(layout.cells[0].x, layout.cells[0].y);
    let pin1 = pack_outline(&painter, corners, first, r, dark);
    for (i, c) in layout.cells.iter().enumerate() {
        let p = mc(c.x, c.y);
        let pos = layout.positive_on(i, st.side);
        let col = group_color(c.group, dark).gamma_multiply(0.7);
        painter.circle(p, r, col, Stroke::new(1.0, if pos { POS } else { NEG }));
        if r >= 9.0 {
            painter.text(p, Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional((r * 0.5).clamp(7.0, 13.0)), Color32::from_gray(25));
        }
    }
    pin1_dot(&painter, first, pin1, r);

    if out.path.len() > 1 {
        let pts: Vec<Pos2> = out.path.iter().map(|&(x, y)| tr(x, y)).collect();
        painter.add(Shape::line(pts, Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 120, 255, 160))));
    }
    for &(x, y) in &out.path {
        painter.circle_filled(tr(x, y), 2.0, SPOT);
    }
    if let (Some(&first), Some(&last)) = (out.path.first(), out.path.last()) {
        painter.circle_stroke(tr(first.0, first.1), 6.0, Stroke::new(2.0, Color32::from_rgb(40, 180, 60)));
        painter.circle_stroke(tr(last.0, last.1), 6.0, Stroke::new(2.0, Color32::from_rgb(220, 60, 60)));
    }
    painter.text(
        rect.left_top() + vec2(8.0, 6.0),
        Align2::LEFT_TOP,
        format!("Widok maszyny — strona: {} (zielony = start, czerwony = koniec)", st.side.label()),
        FontId::proportional(13.0),
        text_col,
    );
}
