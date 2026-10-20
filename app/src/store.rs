//! Trwały magazyn: biblioteka projektów i własne ogniwa.
//! Natywnie pliki JSON w `$XDG_DATA_HOME/pakieciarka` (domyślnie `~/.local/share/pakieciarka`),
//! w przeglądarce `localStorage`.

use crate::app::Project;
use pakiet_core::cells::Cell;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedProject {
    pub id: String,
    /// Czas ostatniego zapisu [ms od epoki]
    pub modified_ms: i64,
    pub project: Project,
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn new_id() -> String {
    format!("{:x}", now_ms())
}

pub fn format_time(ms: i64) -> String {
    use chrono::TimeZone as _;
    chrono::Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

/// Wczytuje projekt z JSON: pełny zapis biblioteki albo sam projekt (eksport).
pub fn parse_project(json: &str) -> Result<Project, String> {
    serde_json::from_str::<SavedProject>(json)
        .map(|s| s.project)
        .or_else(|_| serde_json::from_str::<Project>(json))
        .map_err(|e| e.to_string())
}

fn sort(mut v: Vec<SavedProject>) -> Vec<SavedProject> {
    v.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    v
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use super::*;
    use std::path::PathBuf;

    /// `$XDG_DATA_HOME/pakieciarka`, a bez niej: Linux `~/.local/share/pakieciarka`,
    /// macOS `~/Library/Application Support/pakieciarka` (tam też eframe trzyma stan okna).
    fn data_dir() -> PathBuf {
        let default = if cfg!(target_os = "macos") { "Library/Application Support" } else { ".local/share" };
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(default)))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("pakieciarka")
    }

    fn projects_dir() -> PathBuf {
        data_dir().join("projects")
    }

    fn safe_id(id: &str) -> String {
        id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect()
    }

    /// Ścieżka do wyświetlenia — katalog domowy skrócony do `~`.
    fn pretty(p: PathBuf) -> String {
        let s = p.display().to_string();
        match std::env::var("HOME") {
            Ok(home) if !home.is_empty() && s.starts_with(&home) => format!("~{}", &s[home.len()..]),
            _ => s,
        }
    }

    pub fn location() -> String {
        pretty(projects_dir())
    }

    pub fn cells_location() -> String {
        pretty(data_dir().join("cells.json"))
    }

    pub fn list_projects() -> Vec<SavedProject> {
        let Ok(rd) = std::fs::read_dir(projects_dir()) else { return Vec::new() };
        let v = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|s| serde_json::from_str::<SavedProject>(&s).ok())
            .collect();
        sort(v)
    }

    pub fn save_project(sp: &SavedProject) -> Result<(), String> {
        let dir = projects_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(sp).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.json", safe_id(&sp.id)));
        // zapis atomowy: najpierw plik tymczasowy
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    pub fn delete_project(id: &str) -> Result<(), String> {
        std::fs::remove_file(projects_dir().join(format!("{}.json", safe_id(id)))).map_err(|e| e.to_string())
    }

    pub fn load_cells() -> Vec<Cell> {
        std::fs::read_to_string(data_dir().join("cells.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_cells(cells: &[Cell]) -> Result<(), String> {
        std::fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(cells).map_err(|e| e.to_string())?;
        std::fs::write(data_dir().join("cells.json"), json).map_err(|e| e.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
mod backend {
    use super::*;

    const PREFIX: &str = "pakieciarka.project.";
    const CELLS: &str = "pakieciarka.cells";

    fn storage() -> Result<web_sys::Storage, String> {
        web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .ok_or_else(|| "localStorage niedostępny".to_string())
    }

    pub fn location() -> String {
        "pamięć przeglądarki (localStorage)".into()
    }

    pub fn cells_location() -> String {
        location()
    }

    pub fn list_projects() -> Vec<SavedProject> {
        let Ok(st) = storage() else { return Vec::new() };
        let n = st.length().unwrap_or(0);
        let v = (0..n)
            .filter_map(|i| st.key(i).ok().flatten())
            .filter(|k| k.starts_with(PREFIX))
            .filter_map(|k| st.get_item(&k).ok().flatten())
            .filter_map(|s| serde_json::from_str::<SavedProject>(&s).ok())
            .collect();
        sort(v)
    }

    pub fn save_project(sp: &SavedProject) -> Result<(), String> {
        let json = serde_json::to_string(sp).map_err(|e| e.to_string())?;
        storage()?.set_item(&format!("{PREFIX}{}", sp.id), &json).map_err(|e| format!("{e:?}"))
    }

    pub fn delete_project(id: &str) -> Result<(), String> {
        storage()?.remove_item(&format!("{PREFIX}{id}")).map_err(|e| format!("{e:?}"))
    }

    pub fn load_cells() -> Vec<Cell> {
        storage()
            .ok()
            .and_then(|st| st.get_item(CELLS).ok().flatten())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_cells(cells: &[Cell]) -> Result<(), String> {
        let json = serde_json::to_string(cells).map_err(|e| e.to_string())?;
        storage()?.set_item(CELLS, &json).map_err(|e| format!("{e:?}"))
    }
}

pub use backend::{cells_location, delete_project, list_projects, load_cells, location, save_cells, save_project};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn library_roundtrip() {
        let dir = std::env::temp_dir().join(format!("pakieciarka-test-{}", std::process::id()));
        std::env::set_var("XDG_DATA_HOME", &dir);

        assert!(list_projects().is_empty());
        let mut a = SavedProject { id: "a1".into(), modified_ms: 1, project: Project::default() };
        a.project.name = "Pierwszy".into();
        let mut b = a.clone();
        b.id = "b2".into();
        b.modified_ms = 2;
        b.project.s = 13;
        save_project(&a).unwrap();
        save_project(&b).unwrap();
        let l = list_projects();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].id, "b2", "najnowszy pierwszy");
        assert_eq!(l[0].project.s, 13);

        delete_project("a1").unwrap();
        assert_eq!(list_projects().len(), 1);

        let mut c = pakiet_core::cells::Cell::default();
        c.name = "Moje".into();
        save_cells(&[c.clone()]).unwrap();
        assert_eq!(load_cells(), vec![c]);

        // eksport samego projektu też się wczytuje
        let json = serde_json::to_string(&b.project).unwrap();
        assert_eq!(parse_project(&json).unwrap().s, 13);

        std::fs::remove_dir_all(&dir).ok();
    }
}
