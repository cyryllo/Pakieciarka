//! Generator G-code dla zgrzewarki opartej o FluidNC / GRBL.
//!
//! Dla każdego punktu zgrzewu: dojazd XY na wysokości bezpiecznej, opuszczenie
//! elektrod (Z), odczekanie na docisk, wyzwolenie impulsu (szablon), powrót Z.

use crate::layout::{weld_spots, Layout, Side};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    FluidncSync,
    FluidncImmediate,
    GrblCoolant,
    Spindle,
    Custom,
}

impl Trigger {
    pub const ALL: [Trigger; 5] = [
        Trigger::FluidncSync,
        Trigger::FluidncImmediate,
        Trigger::GrblCoolant,
        Trigger::Spindle,
        Trigger::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Trigger::FluidncSync => "FluidNC: M62/M63 P0 (wyjście synchroniczne)",
            Trigger::FluidncImmediate => "FluidNC: M64/M65 P0 (wyjście natychmiastowe)",
            Trigger::GrblCoolant => "GRBL: M8/M9 (wyjście chłodzenia)",
            Trigger::Spindle => "M3/M5 (wyjście wrzeciona / PWM)",
            Trigger::Custom => "Własny szablon",
        }
    }

    /// Szablon wyzwolenia. `{pulse}` = czas impulsu [s], `{n}` = numer zgrzewu, `{cell}` = numer ogniwa.
    pub fn template(self) -> &'static str {
        match self {
            Trigger::FluidncSync => "M62 P0\nG4 P{pulse}\nM63 P0",
            Trigger::FluidncImmediate => "G4 P0\nM64 P0\nG4 P{pulse}\nM65 P0",
            Trigger::GrblCoolant => "M8\nG4 P{pulse}\nM9",
            Trigger::Spindle => "M3 S1000\nG4 P{pulse}\nM5",
            Trigger::Custom => "M62 P0\nG4 P{pulse}\nM63 P0",
        }
    }
}

/// Jak pakiet jest obracany, by zgrzać stronę dolną.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Flip {
    /// Wokół osi pionowej (jak kartka w książce): lewo ↔ prawo
    LeftRight,
    /// Wokół osi poziomej (do siebie): przód ↔ tył
    FrontBack,
}

impl Flip {
    pub const ALL: [Flip; 2] = [Flip::LeftRight, Flip::FrontBack];

    pub fn label(self) -> &'static str {
        match self {
            Flip::LeftRight => "na bok — lewo ↔ prawo (jak kartka)",
            Flip::FrontBack => "do siebie — przód ↔ tył",
        }
    }

    /// Punkt widziany z góry → ten sam punkt widziany po obróceniu pakietu.
    pub fn apply(self, w: f64, h: f64, x: f64, y: f64) -> (f64, f64) {
        match self {
            Flip::LeftRight => (w - x, y),
            Flip::FrontBack => (x, h - y),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    /// Kolejność wężyka z układu (kolumnami)
    Snake,
    /// Najbliższy sąsiad (krótsze przejazdy)
    Nearest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GcodeSettings {
    pub side: Side,
    /// Sposób obrócenia pakietu przed zgrzewaniem dołu
    pub bottom_flip: Flip,
    /// Góra rysunku = Y+ maszyny (Y rośnie od operatora); bez tego góra rysunku jest przy Y=0
    pub flip_y: bool,
    /// Pozycja maszyny narożnika obrysu ogniw o najmniejszych X i Y (lewy-przedni róg na stole)
    pub origin_x: f64,
    pub origin_y: f64,
    pub use_z: bool,
    pub z_safe: f64,
    pub z_weld: f64,
    pub feed_xy: f64,
    pub feed_z: f64,
    /// Odczekanie po opuszczeniu elektrod [ms]
    pub settle_ms: f64,
    /// Czas sygnału wyzwalającego [ms]
    pub pulse_ms: f64,
    /// Odczekanie po zgrzewie [ms]
    pub after_ms: f64,
    pub trigger: Trigger,
    pub custom_template: String,
    pub spots_per_cell: usize,
    pub spot_offset_mm: f64,
    pub spot_angle_deg: f64,
    pub order: Order,
    pub home_first: bool,
    /// Bez wyzwalania (próba ruchów)
    pub dry_run: bool,
    /// M0 co N ogniw (0 = nigdy) — np. kontrola elektrod
    pub pause_every: usize,
    pub header: String,
    pub footer: String,
}

impl Default for GcodeSettings {
    fn default() -> Self {
        GcodeSettings {
            side: Side::Top,
            bottom_flip: Flip::LeftRight,
            flip_y: true,
            origin_x: 10.0,
            origin_y: 10.0,
            use_z: true,
            z_safe: 5.0,
            z_weld: 0.0,
            feed_xy: 3000.0,
            feed_z: 600.0,
            settle_ms: 150.0,
            pulse_ms: 80.0,
            after_ms: 150.0,
            trigger: Trigger::FluidncSync,
            custom_template: Trigger::Custom.template().into(),
            spots_per_cell: 2,
            spot_offset_mm: 2.5,
            spot_angle_deg: 0.0,
            order: Order::Snake,
            home_first: false,
            dry_run: false,
            pause_every: 0,
            header: String::new(),
            footer: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GcodeOutput {
    pub text: String,
    /// Kolejne punkty zgrzewów we współrzędnych maszyny
    pub path: Vec<(f64, f64)>,
    pub welds: usize,
    pub travel_mm: f64,
    pub est_time_s: f64,
}

fn f(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// Przelicza punkt z układu (widok z góry) na współrzędne maszyny dla danej strony.
pub fn to_machine(layout: &Layout, st: &GcodeSettings, x: f64, y: f64) -> (f64, f64) {
    let (x, y) = if st.side == Side::Bottom { st.bottom_flip.apply(layout.width, layout.height, x, y) } else { (x, y) };
    let y = if st.flip_y { layout.height - y } else { y };
    (st.origin_x + x, st.origin_y + y)
}

fn cell_order(layout: &Layout, order: Order, st: &GcodeSettings) -> Vec<usize> {
    let n = layout.cells.len();
    match order {
        Order::Snake => (0..n).collect(),
        Order::Nearest => {
            let pts: Vec<(f64, f64)> =
                layout.cells.iter().map(|c| to_machine(layout, st, c.x, c.y)).collect();
            let mut left: Vec<usize> = (0..n).collect();
            let mut out = Vec::with_capacity(n);
            let mut cur = (st.origin_x, st.origin_y);
            while !left.is_empty() {
                let (k, _) = left
                    .iter()
                    .enumerate()
                    .map(|(k, &i)| (k, (pts[i].0 - cur.0).powi(2) + (pts[i].1 - cur.1).powi(2)))
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .unwrap();
                let i = left.swap_remove(k);
                cur = pts[i];
                out.push(i);
            }
            out
        }
    }
}

pub fn generate(layout: &Layout, st: &GcodeSettings) -> GcodeOutput {
    let template = match st.trigger {
        Trigger::Custom => st.custom_template.as_str(),
        t => t.template(),
    };
    let spots = weld_spots(layout, st.spots_per_cell, st.spot_offset_mm, st.spot_angle_deg);
    let per_cell = st.spots_per_cell.max(1);
    let order = cell_order(layout, st.order, st);

    let mut g = String::new();
    let mut out = GcodeOutput::default();
    let line = |g: &mut String, s: &str| {
        g.push_str(s);
        g.push('\n');
    };

    line(&mut g, &format!("; Pakieciarka — {}S{}P, strona: {}", layout.s, layout.p, st.side.label()));
    line(
        &mut g,
        &format!(
            "; ogniw: {}, punktów na ogniwo: {}, obrys {}×{} mm{}",
            layout.cells.len(),
            per_cell,
            f(layout.width),
            f(layout.height),
            match (st.side, st.bottom_flip) {
                (Side::Top, _) => "",
                (Side::Bottom, Flip::LeftRight) => ", pakiet obrócony lewo↔prawo (lustro X)",
                (Side::Bottom, Flip::FrontBack) => ", pakiet obrócony przód↔tył (lustro Y)",
            }
        ),
    );
    if st.dry_run {
        line(&mut g, "; TRYB PRÓBNY — bez wyzwalania zgrzewu");
    }
    line(&mut g, "G21 ; mm");
    line(&mut g, "G90 ; współrzędne absolutne");
    if st.home_first {
        line(&mut g, "$H");
    }
    for l in st.header.lines().filter(|l| !l.trim().is_empty()) {
        line(&mut g, l);
    }
    if st.use_z {
        line(&mut g, &format!("G0 Z{}", f(st.z_safe)));
    }

    let pulse_s = f(st.pulse_ms / 1000.0);
    let mut cur = (st.origin_x, st.origin_y);
    let z_time = if st.use_z { 2.0 * (st.z_safe - st.z_weld).abs() / st.feed_z.max(1.0) * 60.0 } else { 0.0 };

    for (ci, &cell) in order.iter().enumerate() {
        if st.pause_every > 0 && ci > 0 && ci % st.pause_every == 0 {
            line(&mut g, "M0 ; pauza — sprawdź elektrody");
        }
        let c = &layout.cells[cell];
        let pol = if layout.positive_on(cell, st.side) { "+" } else { "-" };
        line(&mut g, &format!("; ogniwo {} (grupa {}, biegun {})", cell + 1, c.group + 1, pol));
        for spot in &spots[cell * per_cell..(cell + 1) * per_cell] {
            let (x, y) = to_machine(layout, st, spot.x, spot.y);
            out.travel_mm += ((x - cur.0).powi(2) + (y - cur.1).powi(2)).sqrt();
            cur = (x, y);
            out.path.push((x, y));
            out.welds += 1;
            line(&mut g, &format!("G0 X{} Y{} F{}", f(x), f(y), f(st.feed_xy)));
            if st.use_z {
                line(&mut g, &format!("G1 Z{} F{}", f(st.z_weld), f(st.feed_z)));
            }
            if st.settle_ms > 0.0 {
                line(&mut g, &format!("G4 P{}", f(st.settle_ms / 1000.0)));
            }
            let body = template
                .replace("{pulse}", &pulse_s)
                .replace("{n}", &out.welds.to_string())
                .replace("{cell}", &(cell + 1).to_string());
            for l in body.lines().filter(|l| !l.trim().is_empty()) {
                if st.dry_run {
                    line(&mut g, &format!("; {l}"));
                } else {
                    line(&mut g, l);
                }
            }
            if st.after_ms > 0.0 {
                line(&mut g, &format!("G4 P{}", f(st.after_ms / 1000.0)));
            }
            if st.use_z {
                line(&mut g, &format!("G0 Z{}", f(st.z_safe)));
            }
        }
    }

    for l in st.footer.lines().filter(|l| !l.trim().is_empty()) {
        line(&mut g, l);
    }
    if st.use_z {
        line(&mut g, &format!("G0 Z{}", f(st.z_safe)));
    }
    line(&mut g, &format!("G0 X{} Y{}", f(st.origin_x), f(st.origin_y)));
    line(&mut g, "M2");

    out.est_time_s = out.travel_mm / st.feed_xy.max(1.0) * 60.0
        + out.welds as f64 * (z_time + (st.settle_ms + st.pulse_ms + st.after_ms) / 1000.0);
    out.text = g;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{build_layout, LayoutParams};

    #[test]
    fn generates_all_welds() {
        let l = build_layout(4, 2, 18.4, &LayoutParams::default());
        let st = GcodeSettings::default();
        let out = generate(&l, &st);
        assert_eq!(out.welds, 8 * 2);
        assert_eq!(out.text.matches("M62 P0").count(), 16);
        assert!(out.text.ends_with("M2\n"));
    }

    #[test]
    fn dry_run_comments_trigger() {
        let l = build_layout(2, 1, 18.4, &LayoutParams::default());
        let st = GcodeSettings { dry_run: true, ..Default::default() };
        let out = generate(&l, &st);
        assert!(out.text.lines().all(|l| !l.starts_with("M62")));
    }

    #[test]
    fn bottom_is_mirrored() {
        let l = build_layout(3, 1, 18.4, &LayoutParams::default());
        let top = GcodeSettings { spots_per_cell: 1, order: Order::Snake, ..Default::default() };
        let bottom = GcodeSettings { side: Side::Bottom, ..top.clone() };
        let a = generate(&l, &top).path[0];
        let b = generate(&l, &bottom).path[0];
        assert!((a.0 + b.0 - 2.0 * top.origin_x - l.width).abs() < 1e-9);
        assert!((a.1 - b.1).abs() < 1e-9);

        let fb = GcodeSettings { bottom_flip: Flip::FrontBack, ..bottom };
        let c = generate(&l, &fb).path[0];
        assert!((a.0 - c.0).abs() < 1e-9);
        assert!((a.1 + c.1 - 2.0 * top.origin_y - l.height).abs() < 1e-9);
    }
}
