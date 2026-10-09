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
mod layout;
mod outline;
mod panels;
mod pdf_render;
mod preview;
mod scratch;
mod session;
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
            .with_title("Typst Editor")
            .with_app_id("typst-editor"),
        ..Default::default()
    };
    eframe::run_native(
        "typst-editor",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
