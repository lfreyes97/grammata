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
mod instance;
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
    let run = match cli::parse(std::env::args_os().skip(1), &cwd) {
        Ok(cli::Parsed::Run(run)) => run,
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
    // The paths go to the running instance, if any, and this process is done.
    let claim = if run.new_window {
        instance::Claim::Guest
    } else {
        instance::claim(&run.paths)
    };
    if matches!(claim, instance::Claim::Forwarded) {
        return Ok(());
    }
    let paths = run.paths;
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
        Box::new(move |cc| {
            let instance = match claim {
                instance::Claim::Owner(owner) => {
                    let ctx = cc.egui_ctx.clone();
                    owner.serve(move || ctx.request_repaint())
                }
                _ => instance::Instance::guest(),
            };
            Ok(Box::new(app::App::with_paths(cc, paths, instance)))
        }),
    )
}
