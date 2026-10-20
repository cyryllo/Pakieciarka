//! Okna plików: natywnie systemowe okno wyboru (portal XDG), w przeglądarce
//! wybór pliku do wczytania i pobranie przy zapisie.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenKind {
    Project,
    Cells,
}

pub struct OpenedFile {
    pub kind: OpenKind,
    pub name: String,
    pub content: String,
}

/// Wczytane pliki czekające na obsłużenie w następnej klatce.
pub type Inbox = Rc<RefCell<Vec<OpenedFile>>>;

pub const JSON: (&str, &[&str]) = ("JSON", &["json"]);
pub const GCODE: (&str, &[&str]) = ("G-code", &["gcode", "nc", "ngc"]);
pub const PDF: (&str, &[&str]) = ("PDF", &["pdf"]);

/// Okno „Otwórz”. Wynik trafia do `inbox` (natywnie od razu, w przeglądarce asynchronicznie).
#[cfg(not(target_arch = "wasm32"))]
pub fn open_file(kind: OpenKind, inbox: &Inbox, _ctx: &egui::Context) -> Result<(), String> {
    let Some(path) = rfd::FileDialog::new().add_filter(JSON.0, JSON.1).pick_file() else {
        return Ok(());
    };
    let content = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    inbox.borrow_mut().push(OpenedFile { kind, name, content });
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn open_file(kind: OpenKind, inbox: &Inbox, ctx: &egui::Context) -> Result<(), String> {
    let inbox = inbox.clone();
    let ctx = ctx.clone();
    wasm_bindgen_futures::spawn_local(async move {
        if let Some(h) = rfd::AsyncFileDialog::new().add_filter(JSON.0, JSON.1).pick_file().await {
            let bytes = h.read().await;
            inbox.borrow_mut().push(OpenedFile {
                kind,
                name: h.file_name(),
                content: String::from_utf8_lossy(&bytes).into_owned(),
            });
            ctx.request_repaint();
        }
    });
    Ok(())
}

/// „Zapisz jako”. `Ok(None)` = anulowano, `Ok(Some(opis))` = zapisano.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_file(default_name: &str, content: &str, filter: (&str, &[&str])) -> Result<Option<String>, String> {
    save_bytes(default_name, content.as_bytes(), filter, false)
}

/// Zapis danych binarnych; natywnie opcjonalnie otwiera plik w domyślnym programie.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_bytes(default_name: &str, content: &[u8], filter: (&str, &[&str]), open_after: bool) -> Result<Option<String>, String> {
    let mut dlg = rfd::FileDialog::new().set_file_name(default_name).add_filter(filter.0, filter.1);
    if let Some(home) = std::env::var_os("HOME") {
        dlg = dlg.set_directory(home);
    }
    let Some(path) = dlg.save_file() else { return Ok(None) };
    std::fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
    if open_after {
        // podgląd do druku w domyślnej przeglądarce PDF
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(opener).arg(&path).spawn();
    }
    Ok(Some(format!("zapisano {}", path.display())))
}

#[cfg(target_arch = "wasm32")]
pub fn save_file(default_name: &str, content: &str, filter: (&str, &[&str])) -> Result<Option<String>, String> {
    save_bytes(default_name, content.as_bytes(), filter, false)
}

#[cfg(target_arch = "wasm32")]
pub fn save_bytes(default_name: &str, content: &[u8], filter: (&str, &[&str]), _open_after: bool) -> Result<Option<String>, String> {
    use wasm_bindgen::JsCast as _;

    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let window = web_sys::window().ok_or("brak window")?;
    let document = window.document().ok_or("brak document")?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(content));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(if filter.1.contains(&"pdf") { "application/pdf" } else { "text/plain;charset=utf-8" });
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let a: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(err)?
        .dyn_into()
        .map_err(|_| "nie można utworzyć odnośnika".to_string())?;
    a.set_href(&url);
    a.set_download(default_name);
    a.click();
    web_sys::Url::revoke_object_url(&url).ok();
    Ok(Some(format!("pobrano {default_name}")))
}

/// Nazwa pliku z nazwy projektu (bez znaków problematycznych w systemach plików).
pub fn file_stem(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if s.is_empty() { "pakiet".into() } else { s }
}
