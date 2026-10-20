mod app;
mod bms_ui;
mod draw;
mod library;
mod pdf;
mod platform;
mod store;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    // --rozmiar 1500x950 — rozmiar okna (np. do zrzutów ekranu)
    let args: Vec<String> = std::env::args().collect();
    let size = args
        .iter()
        .skip_while(|a| *a != "--rozmiar")
        .nth(1)
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?]))
        .unwrap_or([1400.0, 900.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pakieciarka")
            .with_app_id("pakieciarka")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/pakieciarka-256.png"))
                    .expect("niepoprawna ikona"),
            )
            .with_inner_size(size)
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Pakieciarka",
        options,
        Box::new(|cc| Ok(Box::new(app::PakieciarkaApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().expect("brak window").document().expect("brak document");
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("brak elementu canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id nie jest canvasem");
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(app::PakieciarkaApp::new(cc)))),
            )
            .await;
        if let Some(el) = document.get_element_by_id("loading_text") {
            match result {
                Ok(_) => el.remove(),
                Err(e) => {
                    el.set_inner_html("<p>Nie udało się uruchomić aplikacji. Szczegóły w konsoli.</p>");
                    panic!("Błąd uruchamiania: {e:?}");
                }
            }
        }
    });
}
