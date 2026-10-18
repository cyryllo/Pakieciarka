//! Baza ogniw. Wartości z kart katalogowych producentów (zaokrąglone);
//! rezystancja wewnętrzna to typowa wartość ACIR 1 kHz dla nowego ogniwa.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Chemistry {
    /// Li-ion NMC/NCA (3.6 V nominalnie)
    LiIon,
    /// LiFePO4 (3.2 V nominalnie)
    LiFePO4,
}

impl Chemistry {
    pub fn label(self) -> &'static str {
        match self {
            Chemistry::LiIon => "Li-ion",
            Chemistry::LiFePO4 => "LiFePO4",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub name: String,
    /// Format, np. "18650"
    pub format: String,
    pub chemistry: Chemistry,
    pub diameter_mm: f64,
    pub length_mm: f64,
    pub capacity_mah: f64,
    pub nominal_v: f64,
    pub max_v: f64,
    pub min_v: f64,
    /// Maksymalny prąd ciągły rozładowania [A]
    pub max_cont_a: f64,
    /// Rezystancja wewnętrzna [mΩ]
    pub ir_mohm: f64,
    pub weight_g: f64,
}

impl Cell {
    #[allow(clippy::too_many_arguments)]
    fn li(
        name: &str,
        format: &str,
        capacity_mah: f64,
        max_cont_a: f64,
        ir_mohm: f64,
        weight_g: f64,
    ) -> Self {
        let (d, l) = format_dims(format);
        Cell {
            name: name.into(),
            format: format.into(),
            chemistry: Chemistry::LiIon,
            diameter_mm: d,
            length_mm: l,
            capacity_mah,
            nominal_v: 3.6,
            max_v: 4.2,
            min_v: 2.5,
            max_cont_a,
            ir_mohm,
            weight_g,
        }
    }

    fn lfp(
        name: &str,
        format: &str,
        capacity_mah: f64,
        max_cont_a: f64,
        ir_mohm: f64,
        weight_g: f64,
    ) -> Self {
        let (d, l) = format_dims(format);
        Cell {
            name: name.into(),
            format: format.into(),
            chemistry: Chemistry::LiFePO4,
            diameter_mm: d,
            length_mm: l,
            capacity_mah,
            nominal_v: 3.2,
            max_v: 3.65,
            min_v: 2.5,
            max_cont_a,
            ir_mohm,
            weight_g,
        }
    }

    pub fn energy_wh(&self) -> f64 {
        self.capacity_mah / 1000.0 * self.nominal_v
    }
}

/// Średnica i długość dla standardowych formatów.
pub fn format_dims(format: &str) -> (f64, f64) {
    match format {
        "18650" => (18.4, 65.2),
        "21700" => (21.2, 70.4),
        "26650" => (26.2, 65.4),
        "32700" => (32.2, 70.5),
        "14500" => (14.2, 50.5),
        _ => (18.4, 65.2),
    }
}

pub fn builtin_cells() -> Vec<Cell> {
    vec![
        Cell::li("Samsung INR18650-25R", "18650", 2500.0, 20.0, 15.0, 45.0),
        Cell::li("Samsung INR18650-30Q", "18650", 3000.0, 15.0, 15.0, 48.0),
        Cell::li("Samsung INR18650-35E", "18650", 3400.0, 8.0, 35.0, 50.0),
        Cell::li("LG INR18650-HG2", "18650", 3000.0, 20.0, 15.0, 47.0),
        Cell::li("LG INR18650-MJ1", "18650", 3500.0, 10.0, 35.0, 46.0),
        Cell::li("Molicel INR-18650-P26A", "18650", 2600.0, 35.0, 12.0, 45.0),
        Cell::li("Molicel INR-18650-P28A", "18650", 2800.0, 35.0, 12.0, 46.0),
        Cell::li("Molicel INR-18650-M35A", "18650", 3500.0, 10.0, 30.0, 47.0),
        Cell::li("Sony/Murata US18650VTC5A", "18650", 2600.0, 25.0, 13.0, 47.0),
        Cell::li("Sony/Murata US18650VTC6", "18650", 3000.0, 15.0, 13.0, 46.6),
        Cell::li("Panasonic NCR18650B", "18650", 3350.0, 4.9, 45.0, 48.5),
        Cell::li("Panasonic NCR18650GA", "18650", 3450.0, 10.0, 35.0, 48.0),
        Cell::li("EVE INR18650-33V", "18650", 3200.0, 10.0, 30.0, 48.0),
        Cell::li("Samsung INR21700-40T", "21700", 4000.0, 35.0, 9.0, 66.5),
        Cell::li("Samsung INR21700-50E", "21700", 5000.0, 9.8, 25.0, 69.0),
        Cell::li("Molicel INR-21700-P42A", "21700", 4200.0, 45.0, 10.0, 70.0),
        Cell::li("Molicel INR-21700-P45B", "21700", 4500.0, 45.0, 9.0, 70.0),
        Cell::li("LG INR21700-M50LT", "21700", 5000.0, 14.4, 22.0, 68.0),
        Cell::lfp("A123 ANR26650M1B (LFP)", "26650", 2500.0, 50.0, 6.0, 76.0),
        Cell::lfp("LiFePO4 32700 6Ah (generyczne)", "32700", 6000.0, 18.0, 8.0, 145.0),
    ]
}

impl Default for Cell {
    fn default() -> Self {
        builtin_cells().remove(1)
    }
}
