//! Baza BMS, dopasowanie do pakietu i opis podłączenia (B−/P−/C−, wtyczka balansera).
//!
//! Dane wbudowane pochodzą z ogólnodostępnych opisów sprzedawców i producentów — są orientacyjne
//! i oznaczone jako „do sprawdzenia”. Kolejność pinów balansera bywa różna nawet w obrębie
//! jednej rodziny, dlatego zawsze trzeba ją potwierdzić w dokumentacji i miernikiem.

use crate::cells::Chemistry;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Port {
    /// Ładowanie i rozładowanie przez ten sam przewód P−
    Common,
    /// Osobne przewody: P− do odbiornika, C− do ładowarki
    Separate,
}

impl Port {
    pub fn label(self) -> &'static str {
        match self {
            Port::Common => "wspólny (P−)",
            Port::Separate => "osobny (P− i C−)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Balancing {
    /// Rozładowywanie najwyższych ogniw na rezystorach (zwykle 30–100 mA)
    Passive,
    /// Przenoszenie energii między ogniwami (zwykle 0,4–2 A)
    Active,
}

impl Balancing {
    pub fn label(self) -> &'static str {
        match self {
            Balancing::Passive => "pasywne",
            Balancing::Active => "aktywne",
        }
    }
}

/// Jak opisane są punkty balansera na płytce / przewodach.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PadLabels {
    /// B0 (B−), B1, B2 … BS (B+)
    #[default]
    Taps,
    /// Narastające napięcie nominalne: 0V, 3.7V, 7.4V … (LiFePO4: 3.2V, 6.4V …) — typowe w tanich BMS
    Voltages,
    /// Numer ogniwa: B−, 1S, 2S …
    Cells,
}

impl PadLabels {
    pub const ALL: [PadLabels; 3] = [PadLabels::Taps, PadLabels::Voltages, PadLabels::Cells];

    pub fn label(self) -> &'static str {
        match self {
            PadLabels::Taps => "B0, B1, B2 … (numery odczepów)",
            PadLabels::Voltages => "0V, 3.7V, 7.4V … (napięcia)",
            PadLabels::Cells => "B−, 1S, 2S … (numery ogniw)",
        }
    }
}

/// Napięcie „na etykiecie” jednego ogniwa — tanie płytki opisują punkty wielokrotnością tej wartości.
pub fn label_cell_voltage(chem: Chemistry) -> f64 {
    match chem {
        Chemistry::LiIon => 3.7,
        Chemistry::LiFePO4 => 3.2,
    }
}

/// Napis przy punkcie balansera dla odczepu `tap` (0…s).
pub fn pad_label(b: &Bms, tap: usize, s: usize, chem: Chemistry) -> String {
    match b.pad_labels {
        PadLabels::Taps => match tap {
            0 => "B0 / B−".into(),
            t if t == s => format!("B{t} / B+"),
            t => format!("B{t}"),
        },
        PadLabels::Voltages => {
            let v = tap as f64 * label_cell_voltage(chem);
            let txt = format!("{v:.1}").replace(".0", "");
            format!("{txt}V")
        }
        PadLabels::Cells => if tap == 0 { "B−".into() } else { format!("{tap}S") },
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bms {
    /// Nazwa modelu / rodziny
    pub name: String,
    pub manufacturer: String,
    pub chemistries: Vec<Chemistry>,
    pub s_min: usize,
    pub s_max: usize,
    /// Dostępne warianty prądu ciągłego [A]
    pub currents_a: Vec<f64>,
    pub balancing: Balancing,
    /// Prąd balansowania [mA]
    pub balance_ma: f64,
    pub port: Port,
    /// Bluetooth / UART, aplikacja
    pub smart: bool,
    /// Liczba czujników temperatury
    pub ntc: u8,
    /// true: pin 1 wtyczki balansera = B0 (B−); false: pin 1 = BS (B+)
    pub b0_first: bool,
    /// Opis punktów balansera na płytce
    pub pad_labels: PadLabels,
    /// Dane potwierdzone z dokumentacją producenta
    pub verified: bool,
    pub notes: String,
}

impl Default for Bms {
    fn default() -> Self {
        Bms {
            name: "Mój BMS".into(),
            manufacturer: String::new(),
            chemistries: vec![Chemistry::LiIon],
            s_min: 10,
            s_max: 10,
            currents_a: vec![30.0],
            balancing: Balancing::Passive,
            balance_ma: 50.0,
            port: Port::Common,
            smart: false,
            ntc: 1,
            b0_first: true,
            pad_labels: PadLabels::Taps,
            verified: true,
            notes: String::new(),
        }
    }
}

impl Bms {
    pub fn s_label(&self) -> String {
        if self.s_min == self.s_max { format!("{}S", self.s_min) } else { format!("{}–{}S", self.s_min, self.s_max) }
    }

    pub fn currents_label(&self) -> String {
        self.currents_a.iter().map(|a| format!("{a:.0}")).collect::<Vec<_>>().join(" / ") + " A"
    }

    pub fn chem_label(&self) -> String {
        self.chemistries.iter().map(|c| c.label()).collect::<Vec<_>>().join(", ")
    }

    pub fn balance_label(&self) -> String {
        if self.balance_ma >= 1000.0 {
            format!("{} {:.1} A", self.balancing.label(), self.balance_ma / 1000.0)
        } else {
            format!("{} {:.0} mA", self.balancing.label(), self.balance_ma)
        }
    }
}

fn family(
    manufacturer: &str,
    name: &str,
    s: (usize, usize),
    currents: &[f64],
    balancing: Balancing,
    balance_ma: f64,
    port: Port,
    smart: bool,
    notes: &str,
) -> Bms {
    Bms {
        name: name.into(),
        manufacturer: manufacturer.into(),
        chemistries: vec![Chemistry::LiIon, Chemistry::LiFePO4],
        s_min: s.0,
        s_max: s.1,
        currents_a: currents.to_vec(),
        balancing,
        balance_ma,
        port,
        smart,
        ntc: if smart { 2 } else { 1 },
        b0_first: true,
        pad_labels: PadLabels::Taps,
        verified: false,
        notes: notes.into(),
    }
}

/// Wbudowane rodziny BMS — dane orientacyjne, do sprawdzenia z dokumentacją konkretnego wariantu.
pub fn builtin_bms() -> Vec<Bms> {
    use Balancing::*;
    use Port::*;
    let mut generic = family(
        "—",
        "BMS bez marki (moduł z polami lutowniczymi)",
        (2, 8),
        &[10.0, 20.0, 30.0, 40.0],
        Passive,
        30.0,
        Common,
        false,
        "Tanie moduły (np. do elektronarzędzi): pola lutownicze opisane narastającym napięciem — 0V, 3.7V, 7.4V … zamiast B0, B1, B2. Często bez balansowania albo z bardzo małym prądem — sprawdź opis. Osobne płytki dla Li-ion i LiFePO4.",
    );
    generic.pad_labels = PadLabels::Voltages;
    generic.ntc = 0;
    vec![
        generic,
        family(
            "Daly",
            "Daly standard (port wspólny)",
            (3, 24),
            &[15.0, 20.0, 30.0, 40.0, 60.0, 80.0, 100.0],
            Passive,
            35.0,
            Common,
            false,
            "Osobne płytki dla Li-ion i LiFePO4 oraz dla każdej liczby S — zamów wariant na konkretne S i chemię. Instrukcja Daly: najpierw B−, potem B1…BS.",
        ),
        family(
            "Daly",
            "Daly standard (port osobny)",
            (3, 24),
            &[15.0, 20.0, 30.0, 40.0, 60.0],
            Passive,
            35.0,
            Separate,
            false,
            "Prąd ładowania zwykle niższy niż rozładowania — sprawdź na etykiecie. Osobne płytki dla Li-ion i LiFePO4.",
        ),
        family(
            "Daly",
            "Daly Smart (Bluetooth)",
            (4, 24),
            &[40.0, 60.0, 80.0, 100.0, 150.0, 200.0],
            Passive,
            35.0,
            Common,
            true,
            "Konfiguracja przez aplikację; opcjonalny moduł aktywnego balansera.",
        ),
        family(
            "JBD (Jiabaida)",
            "JBD SP17S (10–17S)",
            (10, 17),
            &[50.0, 80.0],
            Passive,
            50.0,
            Common,
            true,
            "Ładowanie i rozładowanie przez ten sam port. Prąd balansowania do sprawdzenia w karcie wariantu.",
        ),
        family(
            "JBD (Jiabaida)",
            "JBD SP24S (8–24S)",
            (8, 24),
            &[40.0, 50.0, 60.0, 80.0, 100.0, 200.0],
            Passive,
            50.0,
            Common,
            true,
            "Ładowanie i rozładowanie przez ten sam port. Prąd balansowania do sprawdzenia w karcie wariantu.",
        ),
        family(
            "JK (Jikong)",
            "JK B1A8S (4–8S)",
            (4, 8),
            &[100.0],
            Active,
            1000.0,
            Common,
            true,
            "Aktywny balanser 1 A, Bluetooth.",
        ),
        family(
            "JK (Jikong)",
            "JK BD6A17S (8–17S)",
            (8, 17),
            &[60.0, 80.0],
            Active,
            600.0,
            Common,
            true,
            "Aktywny balanser (0,4–1 A zależnie od wariantu), Bluetooth.",
        ),
        family(
            "JK (Jikong)",
            "JK B2A24S (8–24S)",
            (8, 24),
            &[150.0, 200.0],
            Active,
            2000.0,
            Common,
            true,
            "Aktywny balanser 2 A, Bluetooth. Wariant 30P (300 A) obsługuje 7–24S.",
        ),
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct BmsMatch {
    /// Najmniejszy wariant prądu spełniający wymaganie
    pub variant_a: Option<f64>,
    /// Lista powodów niedopasowania (pusta = pasuje)
    pub problems: Vec<String>,
}

impl BmsMatch {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Wymagany prąd ciągły BMS: 1,25 × prąd obciążenia, zaokrąglony w górę do 5 A.
pub fn required_current(load_a: f64) -> f64 {
    (load_a.max(1.0) * 1.25 / 5.0).ceil() * 5.0
}

pub fn match_bms(b: &Bms, s: usize, chem: Chemistry, required_a: f64) -> BmsMatch {
    let mut problems = Vec::new();
    if s < b.s_min || s > b.s_max {
        problems.push(format!("obsługuje {}, pakiet ma {s}S", b.s_label()));
    }
    if !b.chemistries.contains(&chem) {
        problems.push(format!("nie dla {}", chem.label()));
    }
    let variant_a = b.currents_a.iter().copied().filter(|&a| a >= required_a).fold(None, |m: Option<f64>, a| {
        Some(m.map_or(a, |m| m.min(a)))
    });
    if variant_a.is_none() {
        let max = b.currents_a.iter().copied().fold(0.0, f64::max);
        problems.push(format!("maks. {max:.0} A, potrzeba ≥ {required_a:.0} A"));
    }
    BmsMatch { variant_a, problems }
}

/// Wtyczka balansera: (numer pinu od 1, numer odczepu Bk).
pub fn balance_pins(b: &Bms, s: usize) -> Vec<(usize, usize)> {
    (0..=s).map(|i| (i + 1, if b.b0_first { i } else { s - i })).collect()
}

/// Opis podłączenia krok po kroku.
pub fn wiring_steps(b: &Bms, s: usize, nominal_v: f64) -> Vec<String> {
    let mut v = Vec::new();
    if b.pad_labels != PadLabels::Taps {
        v.push(match b.pad_labels {
            PadLabels::Voltages => "Punkty na płytce są opisane napięciem (0V, 3.7V, 7.4V …): to narastające napięcie odczepu względem B−, czyli 3.7V = B1, 7.4V = B2 itd. — kolumna „Na płytce” w tabeli pinów. Opis jest nominalny; zmierzone napięcie zależy od naładowania ogniw.".to_string(),
            _ => "Punkty na płytce są opisane numerami ogniw (B−, 1S, 2S …): 1S = B1, 2S = B2 itd. — kolumna „Na płytce” w tabeli pinów.".to_string(),
        });
    }
    v.extend([
        "Pakiet bez podłączonego BMS: zmierz miernikiem napięcia odczepów narastająco od B− (B1 ≈ 1 ogniwo, B2 ≈ 2 ogniwa …) i porównaj z tabelą odczepów.".to_string(),
        format!(
            "Przygotuj przewody balansera: {} żył (B0…B{s}). Kolejność pinów we wtyczce sprawdź w dokumentacji BMS — bywa odwrócona; pomyłka niszczy płytkę.",
            s + 1
        ),
        "Wtyczka balansera ODŁĄCZONA od BMS: przylutuj/zgrzej żyły do blaszek wg etykiet B0…BS z rysunku pakietu.".to_string(),
        format!(
            "Na wtyczce (nadal odłączonej) zmierz kolejne piny względem pinu B0 — napięcie ma rosnąć o ≈ {nominal_v:.1} V na pin, aż do pełnego napięcia pakietu na B{s}."
        ),
        "Gruby przewód B− BMS przykręć/przylutuj do minusa pakietu (blaszka B0).".to_string(),
    ]);
    match b.port {
        Port::Common => v.push("P− BMS to nowy minus pakietu: do niego podłącz minus odbiornika i ładowarki.".into()),
        Port::Separate => {
            v.push("P− BMS podłącz do minusa odbiornika (rozładowanie).".into());
            v.push("C− BMS podłącz do minusa ładowarki (ładowanie) — prąd ładowania nie może przekroczyć wartości z karty BMS.".into());
        }
    }
    v.push("Plus pakietu (B+) przez bezpiecznik prosto do plusa odbiornika i ładowarki — BMS nie przerywa plusa.".into());
    if b.ntc > 0 {
        v.push(format!("Czujnik{} temperatury (NTC) przyklej do ogniw w środku pakietu, z dala od blaszek z dużym prądem.", if b.ntc > 1 { "i" } else { "" }));
    }
    v.push("Dopiero na końcu wepnij wtyczkę balansera do BMS (u niektórych modeli wymagana jest kolejność: najpierw B−, potem balanser — patrz instrukcja). Sprawdź napięcie na P−/P+ i działanie BMS.".into());
    if b.smart {
        v.push("Skonfiguruj w aplikacji: chemię, liczbę ogniw, napięcia odcięcia i prądy zgodne z ogniwami.".into());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching() {
        let jk = builtin_bms().into_iter().find(|b| b.name.starts_with("JK B2A24S")).unwrap();
        let m = match_bms(&jk, 13, Chemistry::LiIon, 35.0);
        assert!(m.ok());
        assert_eq!(m.variant_a, Some(150.0));
        let m = match_bms(&jk, 4, Chemistry::LiIon, 35.0);
        assert!(!m.ok());
        assert_eq!(required_current(25.0), 35.0);
    }

    #[test]
    fn pad_labels() {
        let b = Bms { pad_labels: PadLabels::Voltages, ..Default::default() };
        assert_eq!(pad_label(&b, 0, 4, Chemistry::LiIon), "0V");
        assert_eq!(pad_label(&b, 2, 4, Chemistry::LiIon), "7.4V");
        assert_eq!(pad_label(&b, 3, 4, Chemistry::LiIon), "11.1V");
        assert_eq!(pad_label(&b, 1, 4, Chemistry::LiFePO4), "3.2V");
        let c = Bms { pad_labels: PadLabels::Cells, ..Default::default() };
        assert_eq!(pad_label(&c, 3, 4, Chemistry::LiIon), "3S");
    }

    #[test]
    fn pins_order() {
        let mut b = Bms { s_min: 4, s_max: 4, ..Default::default() };
        assert_eq!(balance_pins(&b, 4)[0], (1, 0));
        b.b0_first = false;
        assert_eq!(balance_pins(&b, 4)[0], (1, 4));
        assert_eq!(balance_pins(&b, 4).len(), 5);
    }
}
