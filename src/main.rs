mod actions;
mod app;
mod chrome;
mod code_view;
mod compile;
mod diagnostics;
mod doc;
mod editor;
mod engine;
mod files;
mod formatbar;
mod highlight;
mod image_view;
mod layout;
mod outline;
mod panels;
mod pdf_render;
mod preview;
mod project;
mod scratch;
mod session;
mod snippet_lib;
mod snippets;
mod symbols;
mod syntax;
mod templates;
mod text;
mod theme;
mod widgets;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 840.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("Grammata")
            .with_app_id("grammata")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/grammata-256.png"))
                    .expect("icono embebido"),
            ),
        ..Default::default()
    };
    // El nombre interno sigue siendo `typst-editor`: de él cuelgan las rutas de
    // la configuración del usuario (`~/.config/typst-editor/`).
    eframe::run_native(
        "typst-editor",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
