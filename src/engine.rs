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
use typst::foundations::{Bytes, Datetime, Duration, NativeElement, Selector};
use typst::introspection::{Introspector, PagedPosition};
use typst::model::HeadingElem;
use typst::syntax::{FileId, RootedPath, Source, Span, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt as _, World, compile};
use typst_kit::datetime::Time;
use typst_kit::downloader::SystemDownloader;
use typst_kit::files::{FileStore, FsRoot, SystemFiles};
use typst_kit::fonts::FontStore;
use typst_kit::packages::SystemPackages;
use typst_layout::PagedDocument;

use crate::diagnostics::{self, Diagnostic};

// ---------- Fuentes compartidas ----------

static SHARED_FONTS: OnceLock<Arc<FontStore>> = OnceLock::new();

/// Catálogo de fuentes del sistema. Solo se escanean los metadatos; cada
/// fuente se lee del disco la primera vez que un documento la usa (cargarlas
/// todas de entrada eran GB de RAM con colecciones .ttc grandes).
fn shared_fonts() -> Arc<FontStore> {
    SHARED_FONTS
        .get_or_init(|| {
            let mut store = FontStore::new();
            store.extend(typst_kit::fonts::system());
            Arc::new(store)
        })
        .clone()
}

/// Familias disponibles para el font pick (antes: shell a `typst fonts`).
pub fn font_families() -> Vec<String> {
    let shared = shared_fonts();
    let mut v: Vec<String> = shared
        .book()
        .families()
        .map(|(name, _)| name.to_string())
        .collect();
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
    fonts: Arc<FontStore>,
    library: LazyHash<Library>,
    time: Time,
}

impl EngineWorld {
    fn new(root: PathBuf, filename: &str, text: String) -> Self {
        let vpath = VirtualPath::new(filename)
            .unwrap_or_else(|_| VirtualPath::new("main.typ").expect("vpath"));
        let main_id = RootedPath::new(VirtualRoot::Project, vpath).intern();
        let downloader = SystemDownloader::new(concat!("typst-editor/", env!("CARGO_PKG_VERSION")));
        let files = FileStore::new(SystemFiles::new(
            FsRoot::new(root),
            SystemPackages::new(downloader),
        ));
        Self {
            main_id,
            main_text: text,
            files,
            fonts: shared_fonts(),
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
        self.fonts.book()
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
        self.fonts.font(index)
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
    /// Encabezados con su página (0-based) e y en puntos: base del synctex.
    pub sync: Vec<SyncEntry>,
}

/// Un encabezado localizado: de aquí salen el scroll de la preview (forward)
/// y el salto al código desde un clic (inverso).
#[derive(Clone, Debug)]
pub struct SyncEntry {
    /// `None` = documento actual; `Some` = incluido (relativo a la raíz).
    pub file: Option<String>,
    pub line: usize,
    pub col: usize,
    pub page: usize,
    /// Y en puntos dentro de la página (coords de frame).
    pub y: f32,
}

pub fn compile_pdf(text: &str, root: &Path, filename: &str) -> Compiled {
    let t0 = Instant::now();
    let world = EngineWorld::new(root.to_path_buf(), filename, text.to_string());
    let warned = compile::<PagedDocument>(&world);
    let mut diags = diagnostics::from_warnings(&world, world.main_id, &warned.warnings);
    let (pdf, sync) = match warned.output {
        Ok(doc) => {
            let sync = extract_sync(&world, world.main_id, &doc);
            let pdf = typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).ok();
            (pdf, sync)
        }
        Err(errors) => {
            let mut d = diagnostics::from_source(&world, world.main_id, &errors);
            diags.append(&mut d);
            (None, Vec::new())
        }
    };
    // Como `typst watch`: sin esto la caché de memoización crece con cada
    // revisión compilada durante toda la sesión.
    comemo::evict(10);
    Compiled {
        pdf,
        diags,
        ms: t0.elapsed().as_millis(),
        sync,
    }
}

/// Encabezados del documento con página y posición, vía el introspector del
/// propio documento compilado (spans exactos, sin re-compilar ni regex).
fn extract_sync(world: &EngineWorld, main: FileId, doc: &PagedDocument) -> Vec<SyncEntry> {
    let intro = doc.introspector();
    let mut out = Vec::new();
    let mut last: Option<(Option<String>, usize, usize)> = None;
    for content in intro.query(&Selector::Elem(HeadingElem::ELEM, None)) {
        let (file, line, col) = locate_span(world, main, content.span());
        let (Some(line), Some(col)) = (line, col) else {
            continue;
        };
        let key = (file.clone(), line, col);
        // Un encabezado puede etiquetarse varias veces (cabeceras repetidas):
        // conserva la primera aparición.
        if last.as_ref() == Some(&key) {
            continue;
        }
        last = Some(key);
        let page_y = content
            .location()
            .and_then(|loc| intro.position(loc))
            .map(|p: PagedPosition| (p.page.get() - 1, p.point.y.to_pt() as f32));
        let Some((page, y)) = page_y else { continue };
        out.push(SyncEntry {
            file,
            line,
            col,
            page,
            y,
        });
    }
    out
}

/// Línea → (página, y): la última entrada del mismo fichero con
/// `line` anterior o igual. `None` si no hay entradas de ese fichero.
pub fn page_for_line(sync: &[SyncEntry], file: Option<&str>, line: usize) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    let mut first: Option<(usize, f32)> = None;
    for e in sync {
        if e.file.as_deref() != file {
            continue;
        }
        let py = (e.page, e.y);
        if first.is_none() {
            first = Some(py);
        }
        if e.line <= line {
            best = Some(py);
        } else {
            break;
        }
    }
    best.or(first)
}

/// (Página, y) → entrada: la última de esa página con `y` anterior o igual;
/// si no, la primera de la página; si no, la última de páginas anteriores.
pub fn line_for_pos(sync: &[SyncEntry], page: usize, y: f32) -> Option<&SyncEntry> {
    let mut best: Option<&SyncEntry> = None;
    let mut first_in_page: Option<&SyncEntry> = None;
    for e in sync {
        if e.page > page {
            break;
        }
        if e.page == page {
            if first_in_page.is_none() {
                first_in_page = Some(e);
            }
            if e.y <= y {
                best = Some(e);
            }
        } else {
            best = Some(e);
        }
    }
    best.or(first_in_page)
}
/// Atajo local al `locate_span` de diagnósticos.
fn locate_span(
    world: &EngineWorld,
    main: FileId,
    span: Span,
) -> (Option<String>, Option<usize>, Option<usize>) {
    crate::diagnostics::locate_span(world, main, span)
}

/// Compila y escribe el PDF entero, o PNG/SVG por página expandiendo `{p}`.
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
    if fmt == "pdf" {
        let bytes = typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).map_err(|errors| {
            errors
                .first()
                .map(|e| e.message.to_string())
                .unwrap_or_else(|| "falló la exportación a PDF".to_string())
        })?;
        return std::fs::write(dest_template, bytes)
            .map_err(|e| format!("no se pudo escribir {}: {e}", dest_template.display()));
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
                pix.encode_png()
                    .map_err(|e| format!("no se pudo codificar PNG: {e}"))?
            }
        };
        let dest = expand_dest(dest_template, fmt, multi, i + 1);
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear {dir:?}: {e}"))?;
        }
        std::fs::write(&dest, bytes)
            .map_err(|e| format!("no se pudo escribir {}: {e}", dest.display()))?;
    }
    Ok(())
}

fn expand_dest(template: &Path, fmt: &str, multi: bool, page: usize) -> PathBuf {
    let s = template.to_string_lossy();
    if s.contains("{p}") {
        return PathBuf::from(s.replace("{p}", &page.to_string()));
    }
    if multi {
        let stem = template
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
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

    fn sync_of(text: &str) -> Vec<SyncEntry> {
        let (w, main) = world_of(text);
        let warned = compile::<PagedDocument>(&w);
        let doc = warned.output.expect("debía compilar");
        extract_sync(&w, main, &doc)
    }

    #[test]
    fn synctex_documento_real() {
        let text = include_str!("../ejemplo.typ");
        let dir = std::env::temp_dir();
        let w = EngineWorld::new(dir, "ejemplo.typ", text.to_string());
        let warned = compile::<PagedDocument>(&w);
        assert!(warned.output.is_ok());
        let doc = warned.output.unwrap();
        let s = extract_sync(&w, w.main_id, &doc);
        assert!(!s.is_empty(), "ejemplo.typ debería dar entradas");
        for e in &s {
            println!("sync: {:?} L{} p{} y={:.1}", e.file, e.line, e.page, e.y);
        }
    }

    #[test]
    fn synctex_encabezados_paginas() {
        let s = sync_of("= Uno\nTexto\n#pagebreak()\n== Dos\nFin\n");
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].line, s[0].page), (1, 0));
        assert_eq!((s[1].line, s[1].page), (4, 1));
        assert!(s.iter().all(|e| e.file.is_none()));
        // Forward: línea 5 (Fin) cae en la página del último encabezado.
        assert_eq!(page_for_line(&s, None, 5), Some((1, s[1].y)));
        assert_eq!(page_for_line(&s, None, 1).map(|p| p.0), Some(0));
        // Inverso: clic en página 1 por encima del encabezado → el anterior.
        let e = line_for_pos(&s, 1, 0.0).unwrap();
        assert_eq!((e.line, e.page), (1, 0));
        let e = line_for_pos(&s, 1, s[1].y + 1.0).unwrap();
        assert_eq!(e.line, 4);
    }

    #[test]
    fn compila_ok_y_mapea_errores() {
        let (w, main) = world_of("Hola *mundo*");
        let warned = compile::<PagedDocument>(&w);
        assert!(warned.output.is_ok());
        let (w2, main2) = world_of("Hola #funcion_que_no_existe(1)");
        let warned = compile::<PagedDocument>(&w2);
        let Err(errors) = warned.output else {
            panic!("debía fallar")
        };
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
        assert!(out.pdf.is_some_and(|p| p.starts_with(b"%PDF")));
        assert!(out.diags.is_empty());
    }

    #[test]
    fn exporta_pdf_entero() {
        // PDF multipágina: un solo archivo PDF real, no PNG por página.
        let dir = std::env::temp_dir().join(format!("typst-editor-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("doc.pdf");
        let out = export_pages(
            "Uno\n#pagebreak()\nDos",
            &dir,
            "main.typ",
            &dest,
            "pdf",
            None,
        );
        assert!(out.is_ok(), "{out:?}");
        assert!(std::fs::read(&dest).unwrap().starts_with(b"%PDF"));
        assert!(!dir.join("doc-1.pdf").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
