//! Compilador Typst embebido (sin depender del CLI `typst`).
//!
//! - `EngineWorld`: entorno de compilación sobre `typst-kit` (archivos del
//!   proyecto + paquetes del sistema/Universe, fuentes del sistema, fecha).
//!   El documento principal es siempre el buffer vivo del editor (shadow),
//!   como antes con stdin; el resto sale del disco bajo `--root`.
//! - Las fuentes se cargan una vez por proceso (`shared_fonts`) y se calientan
//!   en el hilo de precarga de la barra de formato.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use typst::diag::FileError;
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt as _, World, compile};
use typst_layout::PagedDocument;
use typst_kit::datetime::Time;
use typst_kit::downloader::SystemDownloader;
use typst_kit::files::{FileStore, FsRoot, SystemFiles};
use typst_kit::fonts::FontSource as _;
use typst_kit::packages::SystemPackages;

use crate::diagnostics::{self, Diagnostic};

// ---------- Fuentes compartidas ----------

struct SharedFonts {
    book: FontBook,
    fonts: Vec<Font>,
}

static SHARED_FONTS: OnceLock<Arc<SharedFonts>> = OnceLock::new();

fn shared_fonts() -> Arc<SharedFonts> {
    SHARED_FONTS
        .get_or_init(|| {
            let fonts: Vec<Font> = typst_kit::fonts::system().filter_map(|(p, _)| p.load()).collect();
            let book = FontBook::from_infos(fonts.iter().map(|f| f.info().clone()));
            Arc::new(SharedFonts { book, fonts })
        })
        .clone()
}

/// Familias disponibles para el font pick (antes: shell a `typst fonts`).
pub fn font_families() -> Vec<String> {
    let shared = shared_fonts();
    let mut v: Vec<String> = shared.book.families().map(|(name, _)| name.to_string()).collect();
    v.sort();
    v.dedup();
    v
}

/// Calienta las fuentes en segundo plano (hilo `typst-fonts`).
/// Hoy la precarga la hace la barra de formato llamando a `font_families`;
/// se conserva para hilos dedicados futuros.
#[allow(dead_code)]
pub fn warm_fonts(ctx: egui::Context) {
    std::thread::Builder::new()
        .name("typst-fonts".into())
        .spawn(move || {
            let _ = shared_fonts();
            ctx.request_repaint();
        })
        .expect("hilo typst-fonts");
}

// ---------- World ----------

/// Entorno de una compilación. Se crea de nuevo en cada `compile_*` (como un
/// proceso CLI fresco); lo caro (fuentes, librería) se comparte.
pub(crate) struct EngineWorld {
    main_id: FileId,
    main_text: String,
    files: FileStore<SystemFiles>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    library: LazyHash<Library>,
    time: Time,
}

impl EngineWorld {
    fn new(root: PathBuf, filename: &str, text: String) -> Self {
        let vpath = VirtualPath::new(filename).unwrap_or_else(|_| VirtualPath::new("main.typ").expect("vpath"));
        let main_id = RootedPath::new(VirtualRoot::Project, vpath).intern();
        let shared = shared_fonts();
        let downloader = SystemDownloader::new(concat!("typst-editor/", env!("CARGO_PKG_VERSION")));
        let files = FileStore::new(SystemFiles::new(FsRoot::new(root), SystemPackages::new(downloader)));
        Self {
            main_id,
            main_text: text,
            files,
            book: LazyHash::new(shared.book.clone()),
            fonts: shared.fonts.clone(),
            library: LazyHash::new(Library::default()),
            time: Time::system(),
        }
    }

    /// Nombre virtual del main: el fichero real si está guardado, `main.typ`
    /// si no. Los `#include` relativos resuelven igual que con el CLI.
    pub(crate) fn filename_for_job(path: Option<&Path>) -> String {
        path.and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| VirtualPath::new(s).is_ok())
            .unwrap_or_else(|| "main.typ".to_string())
    }
}

impl World for EngineWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> Result<Source, FileError> {
        if id == self.main_id {
            Ok(Source::new(id, self.main_text.clone()))
        } else {
            self.files.source(id)
        }
    }

    fn file(&self, id: FileId) -> Result<Bytes, FileError> {
        if id == self.main_id {
            Ok(Bytes::from_string(self.main_text.clone()))
        } else {
            self.files.file(id)
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.time.today(offset)
    }
}

// ---------- Compilación ----------

/// Salida de compilar el buffer a PDF en memoria.
pub struct Compiled {
    pub pdf: Option<Vec<u8>>,
    pub diags: Vec<Diagnostic>,
    pub ms: u128,
}

pub fn compile_pdf(text: &str, root: &Path, filename: &str) -> Compiled {
    let t0 = Instant::now();
    let world = EngineWorld::new(root.to_path_buf(), filename, text.to_string());
    let warned = compile::<PagedDocument>(&world);
    let mut diags = diagnostics::from_warnings(&world, world.main_id, &warned.warnings);
    let pdf = match warned.output {
        Ok(doc) => typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).ok(),
        Err(errors) => {
            let mut d = diagnostics::from_source(&world, world.main_id, &errors);
            diags.append(&mut d);
            None
        }
    };
    Compiled { pdf, diags, ms: t0.elapsed().as_millis() }
}

/// Compila y escribe PNG/SVG por página, expandiendo `{p}` en el destino.
/// Bloqueante como antes (la exportación es una acción explícita).
pub fn export_pages(
    text: &str,
    root: &Path,
    filename: &str,
    dest_template: &Path,
    fmt: &str,
    ppi: Option<u32>,
) -> Result<(), String> {
    let world = EngineWorld::new(root.to_path_buf(), filename, text.to_string());
    let warned = compile::<PagedDocument>(&world);
    let doc = warned.output.map_err(|errors| {
        diagnostics::from_source(&world, world.main_id, &errors)
            .into_iter()
            .next()
            .map(|d| d.message)
            .unwrap_or_else(|| "falló la compilación".to_string())
    })?;
    if doc.pages().is_empty() {
        return Err("el documento no tiene páginas".to_string());
    }
    let multi = doc.pages().len() > 1;
    for (i, page) in doc.pages().iter().enumerate() {
        let bytes: Vec<u8> = match fmt {
            "svg" => typst_svg::svg(page, &typst_svg::SvgOptions::default()).into_bytes(),
            _ => {
                let ppi = ppi.unwrap_or(144) as f64;
                let pix = typst_render::render(
                    page,
                    &typst_render::RenderOptions {
                        pixel_per_pt: typst::utils::Scalar::new(ppi / 72.0),
                        ..Default::default()
                    },
                );
                pix.encode_png().map_err(|e| format!("no se pudo codificar PNG: {e}"))?
            }
        };
        let dest = expand_dest(dest_template, fmt, multi, i + 1);
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear {dir:?}: {e}"))?;
        }
        std::fs::write(&dest, bytes).map_err(|e| format!("no se pudo escribir {}: {e}", dest.display()))?;
    }
    Ok(())
}

fn expand_dest(template: &Path, fmt: &str, multi: bool, page: usize) -> PathBuf {
    let s = template.to_string_lossy();
    if s.contains("{p}") {
        return PathBuf::from(s.replace("{p}", &page.to_string()));
    }
    if multi {
        let stem = template.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        return template.with_file_name(format!("{stem}-{page}.{fmt}"));
    }
    template.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_of(text: &str) -> (EngineWorld, FileId) {
        let dir = std::env::temp_dir();
        let w = EngineWorld::new(dir, "main.typ", text.to_string());
        let main = w.main_id;
        (w, main)
    }

    #[test]
    fn compila_ok_y_mapea_errores() {
        let (w, main) = world_of("Hola *mundo*");
        let warned = compile::<PagedDocument>(&w);
        assert!(warned.output.is_ok());
        let (w2, main2) = world_of("Hola #funcion_que_no_existe(1)");
        let warned = compile::<PagedDocument>(&w2);
        let Err(errors) = warned.output else { panic!("debía fallar") };
        let d = diagnostics::from_source(&w2, main2, &errors);
        assert!(!d.is_empty());
        assert!(d.iter().all(|x| x.severity == diagnostics::Severity::Error));
        assert!(d.iter().all(|x| x.file.is_none() && x.line == Some(1)));
        let _ = (main, w);
    }

    #[test]
    fn compila_sin_cli() {
        // El engine no depende del PATH: compila aunque no haya CLI.
        let out = compile_pdf("= Título\nHola", std::env::temp_dir().as_path(), "main.typ");
        assert!(out.pdf.map_or(false, |p| p.starts_with(b"%PDF")));
        assert!(out.diags.is_empty());
    }
}
