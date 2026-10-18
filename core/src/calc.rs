//! Obliczenia elektryczne pakietu i dobór osprzętu (BMS, ładowarka, nikiel, przewody).
//! Wartości doboru to reguły kciuka — do weryfikacji z kartami katalogowymi.

use crate::cells::Cell;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: &'static str,
    pub s: usize,
    pub p: usize,
    pub note: &'static str,
}

pub fn presets() -> Vec<Preset> {
    vec![
        Preset { name: "1S2P powerbank / latarka", s: 1, p: 2, note: "3,7 V" },
        Preset { name: "2S2P 7,4 V (RC, głośnik)", s: 2, p: 2, note: "7,4 V" },
        Preset { name: "3S1P 12 V (narzędzie 10,8 V)", s: 3, p: 1, note: "10,8–12,6 V" },
        Preset { name: "3S4P 12 V (oświetlenie, router)", s: 3, p: 4, note: "11,1 V" },
        Preset { name: "4S1P 14,4 V", s: 4, p: 1, note: "14,4–16,8 V" },
        Preset { name: "5S1P 18 V (narzędzie)", s: 5, p: 1, note: "18–21 V" },
        Preset { name: "5S2P 18 V (narzędzie 4 Ah+)", s: 5, p: 2, note: "18–21 V" },
        Preset { name: "7S3P 24 V (hulajnoga)", s: 7, p: 3, note: "25,2 V" },
        Preset { name: "10S2P 36 V (hulajnoga)", s: 10, p: 2, note: "36 V" },
        Preset { name: "10S4P 36 V (e-bike)", s: 10, p: 4, note: "36 V" },
        Preset { name: "13S4P 48 V (e-bike)", s: 13, p: 4, note: "48 V" },
        Preset { name: "13S5P 48 V (e-bike)", s: 13, p: 5, note: "48 V" },
        Preset { name: "14S5P 52 V (e-bike)", s: 14, p: 5, note: "52 V" },
        Preset { name: "16S6P 60 V (skuter)", s: 16, p: 6, note: "57,6 V" },
        Preset { name: "20S6P 72 V (skuter / moto)", s: 20, p: 6, note: "72 V" },
        Preset { name: "14S10P 52 V (magazyn energii)", s: 14, p: 10, note: "≈1,8 kWh z 3,5 Ah" },
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct PackStats {
    pub s: usize,
    pub p: usize,
    pub cells: usize,
    pub v_min: f64,
    pub v_nom: f64,
    pub v_max: f64,
    pub capacity_ah: f64,
    pub energy_wh: f64,
    pub max_cont_a: f64,
    pub max_cont_w: f64,
    pub ir_mohm: f64,
    pub weight_cells_g: f64,
    pub weight_est_g: f64,
    /// Obciążenie zadane przez użytkownika
    pub load_a: f64,
    pub load_per_cell_a: f64,
    pub load_c_rate: f64,
    pub sag_v: f64,
    pub heat_w: f64,
    pub runtime_min: f64,
    pub load_ok: bool,
}

/// Szacunkowa masa całości: nikiel, BMS, przewody, obudowa ≈ +15%.
const OVERHEAD: f64 = 1.15;

pub fn pack_stats(cell: &Cell, s: usize, p: usize, load_a: f64) -> PackStats {
    let sf = s as f64;
    let pf = p as f64;
    let capacity_ah = pf * cell.capacity_mah / 1000.0;
    let v_nom = sf * cell.nominal_v;
    let max_cont_a = pf * cell.max_cont_a;
    let ir_mohm = sf * cell.ir_mohm / pf;
    let r = ir_mohm / 1000.0;
    let weight_cells_g = (s * p) as f64 * cell.weight_g;
    PackStats {
        s,
        p,
        cells: s * p,
        v_min: sf * cell.min_v,
        v_nom,
        v_max: sf * cell.max_v,
        capacity_ah,
        energy_wh: v_nom * capacity_ah,
        max_cont_a,
        max_cont_w: v_nom * max_cont_a,
        ir_mohm,
        weight_cells_g,
        weight_est_g: weight_cells_g * OVERHEAD,
        load_a,
        load_per_cell_a: load_a / pf,
        load_c_rate: if capacity_ah > 0.0 { load_a / capacity_ah } else { 0.0 },
        sag_v: load_a * r,
        heat_w: load_a * load_a * r,
        runtime_min: if load_a > 0.0 { capacity_ah / load_a * 60.0 } else { f64::INFINITY },
        load_ok: load_a <= max_cont_a,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StripMaterial {
    PureNickel,
    NickelPlatedSteel,
    CopperNickel,
}

impl StripMaterial {
    pub const ALL: [StripMaterial; 3] = [
        StripMaterial::PureNickel,
        StripMaterial::NickelPlatedSteel,
        StripMaterial::CopperNickel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StripMaterial::PureNickel => "Czysty nikiel (Ni 99,6%)",
            StripMaterial::NickelPlatedSteel => "Stal niklowana",
            StripMaterial::CopperNickel => "Kanapka miedź + nikiel",
        }
    }

    /// Dopuszczalna gęstość prądu [A/mm²] przy umiarkowanym nagrzewaniu (reguła kciuka).
    pub fn current_density(self) -> f64 {
        match self {
            StripMaterial::PureNickel => 5.0,
            StripMaterial::NickelPlatedSteel => 1.5,
            StripMaterial::CopperNickel => 15.0,
        }
    }
}

/// Grubości taśmy dostępne w handlu [mm].
pub const STRIP_THICKNESSES: [f64; 6] = [0.10, 0.12, 0.15, 0.20, 0.25, 0.30];
/// Szerokości taśmy dostępne w handlu [mm].
pub const STRIP_WIDTHS: [f64; 5] = [6.0, 7.0, 8.0, 10.0, 12.0];

impl StripMaterial {
    /// Rezystywność [Ω·mm²/m]. Dla kanapki Cu+Ni liczona warstwa miedzi o podanej grubości.
    pub fn resistivity(self) -> f64 {
        match self {
            StripMaterial::PureNickel => 0.0699,
            StripMaterial::NickelPlatedSteel => 0.13,
            StripMaterial::CopperNickel => 0.0172,
        }
    }
}

/// Typowa szerokość taśmy dla średnicy ogniwa (taśma mieści się na biegunie i między ogniwami).
pub fn auto_strip_width(cell_diameter_mm: f64) -> f64 {
    match cell_diameter_mm {
        d if d < 16.0 => 6.0,
        d if d < 20.0 => 8.0,
        d if d < 24.0 => 10.0,
        _ => 12.0,
    }
}

/// Ustawienia taśmy wybrane przez użytkownika.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StripSettings {
    pub material: StripMaterial,
    /// Największa grubość, którą zgrzewa zgrzewarka [mm]
    pub max_thickness_mm: f64,
    /// Szerokość taśmy [mm]; 0 = automatycznie wg średnicy ogniwa
    pub width_mm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StripChoice {
    pub thickness_mm: f64,
    pub width_mm: f64,
    pub layers: usize,
    /// Dopuszczalny prąd paska (wszystkie warstwy) [A]
    pub rating_a: f64,
    /// false = nawet 4 warstwy najgrubszej dozwolonej taśmy nie wystarczą
    pub ok: bool,
}

impl StripChoice {
    pub fn label(&self) -> String {
        let layers = if self.layers > 1 { format!("{} warstwy × ", self.layers) } else { String::new() };
        format!("{layers}{:.2} × {:.0} mm", self.thickness_mm, self.width_mm)
    }
}

/// Dobiera taśmę (grubość i liczbę warstw) dla prądu `current_a` płynącego jednym paskiem.
/// Preferuje jedną warstwę; grubość nie przekracza możliwości zgrzewarki.
pub fn recommend_strip(material: StripMaterial, current_a: f64, width_mm: f64, max_thickness_mm: f64) -> StripChoice {
    let j = material.current_density();
    let allowed: Vec<f64> = STRIP_THICKNESSES.iter().copied().filter(|&t| t <= max_thickness_mm + 1e-9).collect();
    let allowed = if allowed.is_empty() { vec![STRIP_THICKNESSES[0]] } else { allowed };
    for layers in 1..=4 {
        for &t in &allowed {
            let rating = t * width_mm * j * layers as f64;
            if rating >= current_a {
                return StripChoice { thickness_mm: t, width_mm, layers, rating_a: rating, ok: true };
            }
        }
    }
    let t = *allowed.last().unwrap();
    StripChoice { thickness_mm: t, width_mm, layers: 4, rating_a: t * width_mm * j * 4.0, ok: false }
}

/// Komplet łączników niklowych pakietu.
#[derive(Clone, Debug, PartialEq)]
pub struct StripPlan {
    pub material: StripMaterial,
    pub width_mm: f64,
    pub max_thickness_mm: f64,
    /// Połączenia szeregowe (między grupami) — pełny prąd dzielony na paski złącza
    pub series: StripChoice,
    pub series_current_a: f64,
    /// Paski na najsłabszym złączu szeregowym
    pub series_links: usize,
    /// Połączenia równoległe (w grupie) — wyrównywanie, projektowane na prąd jednego ogniwa
    pub parallel: StripChoice,
    pub parallel_current_a: f64,
    /// Blaszki biegunów B+ / B− — zbierają cały prąd do przewodu przylutowanego w środku blaszki
    pub terminal: StripChoice,
    pub terminal_current_a: f64,
    /// Spadek napięcia na jednym pasku złącza szeregowego [mV]
    pub series_drop_mv: f64,
    /// Straty ciepła we wszystkich złączach szeregowych [W]
    pub series_loss_w: f64,
}

pub fn strip_plan(set: &StripSettings, st: &PackStats, min_series_links: usize, cell_diameter_mm: f64, pitch_mm: f64) -> StripPlan {
    let width = if set.width_mm > 0.0 { set.width_mm } else { auto_strip_width(cell_diameter_mm) };
    let load = st.load_a.max(1.0);
    let links = min_series_links.max(1);
    let series_current_a = load / links as f64;
    let parallel_current_a = (load / st.p.max(1) as f64).max(1.0);
    let terminal_current_a = load / 2.0;
    let series = recommend_strip(set.material, series_current_a, width, set.max_thickness_mm);
    let parallel = recommend_strip(set.material, parallel_current_a, width, set.max_thickness_mm);
    let terminal = recommend_strip(set.material, terminal_current_a, width, set.max_thickness_mm);
    // rezystancja paska między środkami sąsiednich ogniw
    let area = series.thickness_mm * series.width_mm * series.layers as f64;
    let r_link = set.material.resistivity() * (pitch_mm / 1000.0) / area;
    let series_drop_mv = series_current_a * r_link * 1000.0;
    let series_loss_w = st.s.saturating_sub(1) as f64 * links as f64 * series_current_a.powi(2) * r_link;
    StripPlan {
        material: set.material,
        width_mm: width,
        max_thickness_mm: set.max_thickness_mm,
        series,
        series_current_a,
        series_links: links,
        parallel,
        parallel_current_a,
        terminal,
        terminal_current_a,
        series_drop_mv,
        series_loss_w,
    }
}

/// Przekrój przewodu (AWG, mm², dopuszczalny prąd ciągły dla przewodu silikonowego, krótki odcinek).
pub const WIRE_TABLE: [(u32, f64, f64); 11] = [
    (22, 0.33, 7.0),
    (20, 0.52, 11.0),
    (18, 0.82, 16.0),
    (16, 1.31, 22.0),
    (14, 2.08, 32.0),
    (12, 3.31, 41.0),
    (10, 5.26, 55.0),
    (8, 8.37, 73.0),
    (6, 13.3, 101.0),
    (4, 21.2, 135.0),
    (2, 33.6, 181.0),
];

pub fn recommend_wire(current_a: f64) -> (u32, f64, f64) {
    WIRE_TABLE
        .iter()
        .copied()
        .find(|&(_, _, a)| a >= current_a)
        .unwrap_or(WIRE_TABLE[WIRE_TABLE.len() - 1])
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recommendations {
    pub bms: String,
    pub charger: String,
    pub charge_current_a: f64,
    pub fuse_a: f64,
    pub strip: StripPlan,
    pub main_wire: (u32, f64, f64),
    pub balance_wire: &'static str,
    pub warnings: Vec<String>,
}

/// `min_series_links` — najmniejsza liczba połączeń między sąsiednimi grupami w układzie
/// (ile pasków niklu dzieli prąd na złączu szeregowym).
/// `pitch_mm` — rozstaw środków ogniw (długość paska między ogniwami).
pub fn recommendations(
    cell: &Cell,
    st: &PackStats,
    strip: &StripSettings,
    min_series_links: usize,
    pitch_mm: f64,
) -> Recommendations {
    let material = strip.material;
    let plan = strip_plan(strip, st, min_series_links, cell.diameter_mm, pitch_mm);
    let design_a = st.load_a.max(1.0);
    let bms_a = (design_a * 1.25 / 5.0).ceil() * 5.0;
    let charge_current_a = ((st.capacity_ah * 0.3) * 2.0).round() / 2.0;
    let mut warnings = Vec::new();

    if !st.load_ok {
        warnings.push(format!(
            "Obciążenie {:.1} A przekracza prąd ciągły pakietu {:.1} A — zwiększ P albo wybierz ogniwo wysokoprądowe.",
            st.load_a, st.max_cont_a
        ));
    }
    if st.load_c_rate > 3.0 {
        warnings.push(format!(
            "Obciążenie {:.1}C — duże nagrzewanie i spadek żywotności ogniw.",
            st.load_c_rate
        ));
    }
    if st.sag_v > st.v_nom * 0.1 {
        warnings.push(format!(
            "Spadek napięcia pod obciążeniem {:.1} V (>10% napięcia nominalnego).",
            st.sag_v
        ));
    }
    if material == StripMaterial::NickelPlatedSteel {
        warnings.push(
            "Stal niklowana ma ~4× większą rezystancję niż czysty nikiel — nadaje się tylko do małych prądów."
                .into(),
        );
    }
    if !plan.series.ok {
        warnings.push(format!(
            "Złącze szeregowe: {:.1} A na pasek to za dużo dla taśmy do {:.2} mm (nawet 4 warstwy) — użyj kanapki miedź + nikiel, zgrzewarki do grubszej taśmy albo zwiększ P.",
            plan.series_current_a, plan.max_thickness_mm
        ));
    }
    if !plan.terminal.ok || plan.terminal.layers > 2 {
        warnings.push(format!(
            "Bieguny B+/B−: {:.0} A — zamiast wielu warstw niklu przylutuj przewód główny wzdłuż blaszki albo użyj miedzianej szyny.",
            plan.terminal_current_a
        ));
    }
    if st.v_max > 60.0 {
        warnings.push(format!(
            "Napięcie maks. {:.1} V przekracza 60 V DC — strefa niebezpiecznego napięcia, zachowaj szczególną ostrożność.",
            st.v_max
        ));
    }
    if st.p >= 2 && min_series_links < st.p / 2 {
        warnings.push(format!(
            "Najsłabsze złącze szeregowe ma tylko {} połącz. — prąd skupia się na kilku paskach. Rozważ inny układ (wysokość kolumny = P).",
            min_series_links
        ));
    }

    Recommendations {
        bms: format!(
            "{}S {}, prąd ciągły ≥ {:.0} A, z balansowaniem",
            st.s,
            cell.chemistry.label(),
            bms_a
        ),
        charger: format!(
            "{:.1} V CC/CV ({}S × {:.2} V)",
            st.v_max, st.s, cell.max_v
        ),
        charge_current_a: charge_current_a.max(0.5),
        fuse_a: bms_a,
        strip: plan,
        main_wire: recommend_wire(design_a),
        balance_wire: "22–26 AWG (silikon)",
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cells::builtin_cells;

    #[test]
    fn ten_s_four_p() {
        let cell = builtin_cells()
            .into_iter()
            .find(|c| c.name.contains("30Q"))
            .unwrap();
        let st = pack_stats(&cell, 10, 4, 20.0);
        assert_eq!(st.cells, 40);
        assert!((st.v_nom - 36.0).abs() < 1e-9);
        assert!((st.v_max - 42.0).abs() < 1e-9);
        assert!((st.capacity_ah - 12.0).abs() < 1e-9);
        assert!((st.energy_wh - 432.0).abs() < 1e-9);
        assert!((st.max_cont_a - 60.0).abs() < 1e-9);
        assert!(st.load_ok);
    }

    #[test]
    fn strip_grows_with_current() {
        let a = recommend_strip(StripMaterial::PureNickel, 3.0, 8.0, 0.3);
        let b = recommend_strip(StripMaterial::PureNickel, 20.0, 8.0, 0.3);
        assert!(a.ok && a.rating_a >= 3.0);
        assert!(b.ok && b.rating_a >= 20.0);
        assert!(b.thickness_mm * b.width_mm * b.layers as f64 > a.thickness_mm * a.width_mm);
    }

    #[test]
    fn strip_respects_welder_limit() {
        // 10 A na pasek 8 mm czystego niklu (5 A/mm²): 1 × 0.25 mm albo 2 × 0.15 przy zgrzewarce do 0.15 mm
        let one = recommend_strip(StripMaterial::PureNickel, 10.0, 8.0, 0.3);
        assert_eq!((one.thickness_mm, one.layers), (0.25, 1));
        let two = recommend_strip(StripMaterial::PureNickel, 10.0, 8.0, 0.15);
        assert_eq!((two.thickness_mm, two.layers), (0.15, 2));
        assert!(two.ok);
        let too_much = recommend_strip(StripMaterial::PureNickel, 100.0, 8.0, 0.1);
        assert!(!too_much.ok);
    }

    #[test]
    fn plan_currents() {
        let cell = builtin_cells().into_iter().find(|c| c.name.contains("30Q")).unwrap();
        let st = pack_stats(&cell, 10, 4, 20.0);
        let set = StripSettings { material: StripMaterial::PureNickel, max_thickness_mm: 0.2, width_mm: 0.0 };
        let p = strip_plan(&set, &st, 4, cell.diameter_mm, 19.4);
        assert_eq!(p.width_mm, 8.0);
        assert!((p.series_current_a - 5.0).abs() < 1e-9);
        assert!((p.terminal_current_a - 10.0).abs() < 1e-9);
        assert!(p.series_drop_mv > 0.0 && p.series_loss_w > 0.0);
    }
}
