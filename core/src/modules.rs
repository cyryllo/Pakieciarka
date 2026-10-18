//! Duże pakiety złożone z modułów (np. 2× 10S4P w szeregu = 20S4P).

use crate::calc::PackStats;

#[derive(Clone, Debug, PartialEq)]
pub struct ModulePlan {
    pub module_s: usize,
    pub module_p: usize,
    /// Moduły połączone szeregowo w jednym łańcuchu
    pub series: usize,
    /// Łańcuchy połączone równolegle
    pub parallel: usize,
}

impl ModulePlan {
    pub fn total_s(&self) -> usize {
        self.module_s * self.series
    }
    pub fn total_p(&self) -> usize {
        self.module_p * self.parallel
    }
    pub fn modules(&self) -> usize {
        self.series * self.parallel
    }
    /// Zakres odczepów balansera w całym pakiecie dla modułu nr `idx` w łańcuchu.
    pub fn taps(&self, idx: usize) -> (usize, usize) {
        (idx * self.module_s, (idx + 1) * self.module_s)
    }

    /// Instrukcja łączenia krok po kroku.
    pub fn steps(&self, total: &PackStats) -> Vec<String> {
        let mut v = Vec::new();
        v.push(format!(
            "Zbuduj {} jednakowych modułów {}S{}P z ogniw z tej samej partii (dopasowane pojemnością i IR).",
            self.modules(),
            self.module_s,
            self.module_p
        ));
        v.push("Przed łączeniem naładuj/rozładuj wszystkie moduły do tego samego napięcia (różnica ≤ 0,05 V na grupę).".into());
        if self.parallel > 1 {
            v.push(format!(
                "Równolegle: połącz {} moduły na każdym poziomie grupy — nie tylko + i −, ale też wszystkie odczepy balansera B1…B{} (grupy muszą być fizycznie równoległe). Moduły w jednej kolumnie schematu tworzą jeden „super-moduł” {}S{}P.",
                self.parallel,
                self.module_s.saturating_sub(1),
                self.module_s,
                self.total_p()
            ));
            v.push("Połączenia równoległe wykonuj przewodami/szynami o równej długości do wspólnej szyny + i −.".into());
        }
        if self.series > 1 {
            v.push(format!(
                "Szeregowo: + modułu n łącz z − modułu n+1 przewodem dobranym na pełny prąd pakietu ({:.0} A).",
                total.load_a.max(1.0)
            ));
            v.push(format!(
                "Balanser: jeden BMS {}S na cały pakiet. Odczepy modułu nr k numeruj od B{}·k — np. B0 modułu 2 = B{} całego pakietu (to ten sam węzeł co B+ modułu 1).",
                self.total_s(),
                self.module_s,
                self.module_s
            ));
        }
        v.push(format!(
            "Wynik: {}S{}P — {:.1} V nominalnie (maks. {:.1} V), {:.1} Ah, {:.0} Wh.",
            self.total_s(),
            self.total_p(),
            total.v_nom,
            total.v_max,
            total.capacity_ah,
            total.energy_wh
        ));
        v.push("Bezpiecznik główny na przewodzie B+ możliwie blisko pakietu; między modułami izolacja (fishpaper) i mocowanie przeciw przetarciu.".into());
        v
    }
}
