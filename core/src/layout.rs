//! Rozmieszczenie ogniw, przydział do grup równoległych, polaryzacja oraz połączenia niklem.
//!
//! Współrzędne w mm, widok z góry, początek w lewym górnym rogu obrysu, oś Y w dół.
//! Ogniwa są układane w kolumny „wężykiem” (kolumna w dół, następna w górę...),
//! a kolejne grupy po P ogniw są wycinane z tej ścieżki — dzięki temu każda grupa
//! jest spójna i sąsiaduje z następną.
//!
//! Polaryzacja: grupa 0 ma + na górze, każda kolejna jest odwrócona. Połączenia
//! szeregowe wypadają więc naprzemiennie na górze (G0–G1, G2–G3…) i na dole (G1–G2, G3–G4…).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arrangement {
    /// Siatka prostokątna
    Grid,
    /// Plaster miodu (kolumny przesunięte o pół rozstawu)
    Honeycomb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Top,
    Bottom,
}

impl Side {
    pub fn label(self) -> &'static str {
        match self {
            Side::Top => "Góra",
            Side::Bottom => "Dół",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutParams {
    pub arrangement: Arrangement,
    /// Liczba ogniw w kolumnie; 0 = automatycznie (= P)
    pub column_height: usize,
    /// Odstęp między ogniwami (koszyki/separatory) [mm]
    pub gap_mm: f64,
    /// Grupy ułożone w wierszach zamiast kolumn
    pub horizontal: bool,
}

impl Default for LayoutParams {
    fn default() -> Self {
        LayoutParams { arrangement: Arrangement::Grid, column_height: 0, gap_mm: 1.0, horizontal: false }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedCell {
    pub x: f64,
    pub y: f64,
    pub group: usize,
    /// Kolejny numer na ścieżce wężyka
    pub order: usize,
    pub positive_top: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plate {
    pub side: Side,
    /// Grupy połączone tą blaszką (1 albo 2)
    pub groups: Vec<usize>,
    pub cells: Vec<usize>,
    /// Numer odczepu balansera B0..BS (B0 = B-, BS = B+)
    pub tap: usize,
    pub cx: f64,
    pub cy: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub side: Side,
    pub a: usize,
    pub b: usize,
    /// true = połączenie szeregowe (między grupami), false = równoległe
    pub series: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub s: usize,
    pub p: usize,
    pub radius: f64,
    pub pitch: f64,
    pub width: f64,
    pub height: f64,
    pub cells: Vec<PlacedCell>,
    pub plates: Vec<Plate>,
    pub links: Vec<Link>,
    /// Najmniejsza liczba połączeń szeregowych na jednym złączu grup
    pub min_series_links: usize,
    pub columns: usize,
    pub column_height: usize,
}

impl Layout {
    /// Czy na danej stronie ogniwo ma biegun dodatni.
    pub fn positive_on(&self, cell: usize, side: Side) -> bool {
        self.cells[cell].positive_top == (side == Side::Top)
    }

    pub fn plates_on(&self, side: Side) -> impl Iterator<Item = &Plate> {
        self.plates.iter().filter(move |pl| pl.side == side)
    }

    pub fn links_on(&self, side: Side) -> impl Iterator<Item = &Link> {
        self.links.iter().filter(move |l| l.side == side)
    }

    /// Liczba połączeń szeregowych między grupą `k-1` i `k`.
    pub fn junction_links(&self, k: usize) -> usize {
        self.links
            .iter()
            .filter(|l| {
                l.series && {
                    let (ga, gb) = (self.cells[l.a].group, self.cells[l.b].group);
                    ga.min(gb) + 1 == k && ga.max(gb) == k
                }
            })
            .count()
    }
}

pub fn build_layout(s: usize, p: usize, cell_diameter_mm: f64, params: &LayoutParams) -> Layout {
    let s = s.max(1);
    let p = p.max(1);
    let n = s * p;
    let radius = cell_diameter_mm / 2.0;
    let pitch = cell_diameter_mm + params.gap_mm.max(0.0);
    let h = if params.column_height == 0 { p } else { params.column_height }.clamp(1, n);
    let columns = n.div_ceil(h);

    let (col_dx, stagger) = match params.arrangement {
        Arrangement::Grid => (pitch, 0.0),
        Arrangement::Honeycomb => (pitch * 3f64.sqrt() / 2.0, pitch / 2.0),
    };

    let mut cells = Vec::with_capacity(n);
    'outer: for c in 0..columns {
        for i in 0..h {
            let order = cells.len();
            if order >= n {
                break 'outer;
            }
            let r = if c % 2 == 0 { i } else { h - 1 - i };
            let mut x = radius + c as f64 * col_dx;
            let mut y = radius + r as f64 * pitch + if c % 2 == 1 { stagger } else { 0.0 };
            if params.horizontal {
                std::mem::swap(&mut x, &mut y);
            }
            let group = order / p;
            cells.push(PlacedCell { x, y, group, order, positive_top: group % 2 == 0 });
        }
    }

    let width = cells.iter().map(|c| c.x).fold(0.0, f64::max) + radius;
    let height = cells.iter().map(|c| c.y).fold(0.0, f64::max) + radius;

    // Blaszki: na górze grupy {2k, 2k+1} (odczep 2k+1), na dole {2k-1, 2k} (odczep 2k).
    let mut plates = Vec::new();
    for side in [Side::Bottom, Side::Top] {
        let mut k = 0;
        loop {
            let groups: Vec<usize> = match side {
                Side::Top => vec![2 * k, 2 * k + 1],
                Side::Bottom if k == 0 => vec![0],
                Side::Bottom => vec![2 * k - 1, 2 * k],
            }
            .into_iter()
            .filter(|&g| g < s)
            .collect();
            if groups.is_empty() {
                break;
            }
            let tap = match side {
                Side::Top => 2 * k + 1,
                Side::Bottom => 2 * k,
            };
            let members: Vec<usize> =
                (0..n).filter(|&i| groups.contains(&cells[i].group)).collect();
            let cx = members.iter().map(|&i| cells[i].x).sum::<f64>() / members.len() as f64;
            let cy = members.iter().map(|&i| cells[i].y).sum::<f64>() / members.len() as f64;
            plates.push(Plate { side, groups, cells: members, tap, cx, cy });
            k += 1;
        }
    }

    // Połączenia między sąsiadującymi ogniwami tej samej blaszki.
    let max_d2 = (pitch * 1.08).powi(2);
    let mut links = Vec::new();
    for plate in &plates {
        for (ia, &a) in plate.cells.iter().enumerate() {
            for &b in &plate.cells[ia + 1..] {
                let (ca, cb) = (&cells[a], &cells[b]);
                let d2 = (ca.x - cb.x).powi(2) + (ca.y - cb.y).powi(2);
                if d2 <= max_d2 {
                    links.push(Link { side: plate.side, a, b, series: ca.group != cb.group });
                }
            }
        }
    }

    let mut layout = Layout {
        s,
        p,
        radius,
        pitch,
        width,
        height,
        cells,
        plates,
        links,
        min_series_links: p,
        columns,
        column_height: h,
    };
    if s > 1 {
        layout.min_series_links = (1..s).map(|k| layout.junction_links(k)).min().unwrap_or(p);
    }
    layout
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeldSpot {
    pub x: f64,
    pub y: f64,
    pub cell: usize,
}

/// Punkty zgrzewów dla jednej strony: `per_cell` punktów na okręgu o promieniu `offset_mm`
/// wokół środka bieguna, obrócone o `angle_deg`.
pub fn weld_spots(layout: &Layout, per_cell: usize, offset_mm: f64, angle_deg: f64) -> Vec<WeldSpot> {
    let per_cell = per_cell.max(1);
    let a0 = angle_deg.to_radians();
    let mut out = Vec::with_capacity(layout.cells.len() * per_cell);
    for (i, c) in layout.cells.iter().enumerate() {
        if per_cell == 1 {
            out.push(WeldSpot { x: c.x, y: c.y, cell: i });
            continue;
        }
        for k in 0..per_cell {
            let a = a0 + std::f64::consts::TAU * k as f64 / per_cell as f64;
            out.push(WeldSpot { x: c.x + offset_mm * a.cos(), y: c.y + offset_mm * a.sin(), cell: i });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lay(s: usize, p: usize, h: usize, arr: Arrangement) -> Layout {
        build_layout(s, p, 18.4, &LayoutParams { arrangement: arr, column_height: h, ..Default::default() })
    }

    #[test]
    fn taps_cover_zero_to_s() {
        for s in 1..8 {
            let l = lay(s, 3, 0, Arrangement::Grid);
            let mut taps: Vec<usize> = l.plates.iter().map(|p| p.tap).collect();
            taps.sort();
            assert_eq!(taps, (0..=s).collect::<Vec<_>>(), "s={s}");
        }
    }

    #[test]
    fn column_layout_has_p_series_links() {
        let l = lay(10, 4, 0, Arrangement::Grid);
        assert_eq!(l.cells.len(), 40);
        assert_eq!(l.columns, 10);
        assert_eq!(l.min_series_links, 4);
        // w każdej grupie 3 połączenia równoległe w kolumnie (strona +) i 3 (strona -)
        let par = l.links.iter().filter(|k| !k.series).count();
        assert_eq!(par, 10 * 3 * 2);
    }

    #[test]
    fn snake_groups_are_connected() {
        for arr in [Arrangement::Grid, Arrangement::Honeycomb] {
            let l = lay(13, 5, 7, arr);
            for k in 1..13 {
                assert!(l.junction_links(k) >= 1, "{arr:?} złącze {k}");
            }
        }
    }

    #[test]
    fn polarity_alternates() {
        let l = lay(3, 2, 0, Arrangement::Grid);
        assert!(l.positive_on(0, Side::Top));
        assert!(!l.positive_on(2, Side::Top));
        assert!(l.positive_on(4, Side::Top));
    }
}
