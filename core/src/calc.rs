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

#[derive(Clone, Debug, PartialEq)]
pub struct StripChoice {
    pub thickness_mm: f64,
    pub width_mm: f64,
    pub layers: usize,
    pub rating_a: f64,
}

/// Dobiera taśmę dla jednego połączenia szeregowego (między dwoma sąsiednimi ogniwami
/// różnych grup), przez które płynie `current_a`.
pub fn recommend_strip(material: StripMaterial, current_a: f64) -> StripChoice {
    const SIZES: [(f64, f64); 8] = [
        (0.10, 7.0),
        (0.12, 8.0),
        (0.15, 8.0),
        (0.20, 8.0),
        (0.15, 10.0),
        (0.20, 10.0),
        (0.25, 10.0),
        (0.30, 10.0),
    ];
    let j = material.current_density();
    for layers in 1..=4 {
        for &(t, w) in SIZES.iter() {
            let rating = t * w * j * layers as f64;
            if rating >= current_a {
                return StripChoice { thickness_mm: t, width_mm: w, layers, rating_a: rating };
            }
        }
    }
    let (t, w) = SIZES[SIZES.len() - 1];
    StripChoice { thickness_mm: t, width_mm: w, layers: 4, rating_a: t * w * j * 4.0 }
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
    pub strip: StripChoice,
    pub strip_link_current_a: f64,
    pub main_wire: (u32, f64, f64),
    pub balance_wire: &'static str,
    pub warnings: Vec<String>,
}

/// `min_series_links` — najmniejsza liczba połączeń między sąsiednimi grupami w układzie
/// (ile pasków niklu dzieli prąd na złączu szeregowym).
pub fn recommendations(
    cell: &Cell,
    st: &PackStats,
    material: StripMaterial,
    min_series_links: usize,
) -> Recommendations {
    let design_a = st.load_a.max(1.0);
    let bms_a = (design_a * 1.25 / 5.0).ceil() * 5.0;
    let charge_current_a = ((st.capacity_ah * 0.3) * 2.0).round() / 2.0;
    let links = min_series_links.max(1) as f64;
    let strip_link_current_a = design_a / links;
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
        strip: recommend_strip(material, strip_link_current_a),
        strip_link_current_a,
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
        let a = recommend_strip(StripMaterial::PureNickel, 3.0);
        let b = recommend_strip(StripMaterial::PureNickel, 20.0);
        assert!(a.rating_a >= 3.0);
        assert!(b.rating_a >= 20.0);
        assert!(b.thickness_mm * b.width_mm * b.layers as f64 > a.thickness_mm * a.width_mm);
    }
}
