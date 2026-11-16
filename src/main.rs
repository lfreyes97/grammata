mod actions;
mod app;
mod chrome;
mod cli;
mod code_view;
mod compile;
mod diagnostics;
mod doc;
mod editor;
mod engine;
mod files;
mod formatbar;
mod highlight;
mod i18n;
mod ide;
mod image_view;
mod layout;
mod outline;
mod panels;
mod pdf_render;
mod plugins;
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
    let cwd = std::env::current_dir().unwrap_or_default();
    let paths = match cli::parse(std::env::args_os().skip(1), &cwd) {
        Ok(cli::Parsed::Run(paths)) => paths,
        Ok(cli::Parsed::Help) => {
            print!("{}", cli::HELP);
            return Ok(());
        }
        Ok(cli::Parsed::Version) => {
            println!("{}", cli::version());
            return Ok(());
        }
        Err(e) => {
            eprintln!("grammata: {e}\nTry 'grammata --help'.");
            std::process::exit(2);
        }
    };
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
    eframe::run_native(
        "grammata",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::with_paths(cc, paths)))),
    )
}
