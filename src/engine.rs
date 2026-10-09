//! Compilador Typst embebido (sin depender del CLI `typst`).
//!
//! - `EngineWorld`: entorno de compilación sobre `typst-kit` (archivos del
//!   proyecto + paquetes del sistema/Universe, fuentes del sistema, fecha).
//!   El documento principal es siempre el buffer vivo del editor (shadow),
//!   como antes con stdin; el resto sale del disco bajo `--root`.
//! - Las fuentes se cargan una vez por proceso (`shared_fonts`) y se calientan
//!   en el hilo de precarga de la barra de formato.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use typst::diag::FileError;
use typst::foundations::{Bytes, Datetime, Duration};
use typst::layout::{Frame, FrameItem, Point, Transform};
use typst::syntax::{FileId, LinkedNode, RootedPath, Source, Span, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, TextItem};
use typst::utils::{LazyHash, Scalar};
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

/// Catálogo de fuentes del sistema más las de serie de typst. De las del
/// sistema solo se escanean los metadatos; cada una se lee del disco la
/// primera vez que un documento la usa (cargarlas todas de entrada eran GB
/// de RAM con colecciones .ttc grandes).
fn shared_fonts() -> Arc<FontStore> {
    SHARED_FONTS
        .get_or_init(|| {
            let mut store = FontStore::new();
            store.extend(typst_kit::fonts::system());
            // Las de serie de typst (Libertinus, New Computer Modern…) tras
            // las del sistema, como el CLI: sin ellas un documento sin
            // `#set text(font:)` sale con una fuente de reserva cualquiera.
            store.extend(typst_kit::fonts::embedded());
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
    /// Fuente principal parseada una vez: `source()` se llama por cada
    /// consulta de span (diagnósticos, synctex) y re-parsear costaba O(n).
    main: Source,
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
            main: Source::new(main_id, text),
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
            Ok(self.main.clone())
        } else {
            self.files.source(id)
        }
    }

    fn file(&self, id: FileId) -> Result<Bytes, FileError> {
        if id == self.main_id {
            Ok(Bytes::from_string(self.main.text().to_string()))
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
    /// Texto del documento con su página y posición: base del synctex.
    pub sync: Vec<SyncEntry>,
}

/// Un tramo de texto colocado: una línea fuente en un renglón de una página.
/// De aquí salen el scroll de la preview (forward) y el salto al código
/// desde un clic (inverso).
#[derive(Clone, Debug)]
pub struct SyncEntry {
    /// `None` = documento actual; `Some` = incluido (relativo a la raíz).
    pub file: Option<String>,
    pub line: usize,
    pub col: usize,
    pub page: usize,
    /// Inicio del tramo en puntos dentro de la página (coords de frame).
    pub x: f32,
    /// Línea base del renglón, en puntos.
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

/// Renderiza `src` (documento completo) a imagen para vistas previas
/// pequeñas (p. ej. el borrador): primera página a `scale` px/pt.
/// `Err` = primer mensaje de error.
pub fn render_preview(src: &str, root: &Path, scale: f32) -> Result<egui::ColorImage, String> {
    let world = EngineWorld::new(root.to_path_buf(), "borrador.typ", src.to_string());
    let warned = compile::<PagedDocument>(&world);
    let mut diags = diagnostics::from_warnings(&world, world.main_id, &warned.warnings);
    let out = match warned.output {
        Ok(doc) => {
            let pix = doc.pages().first().map(|page| {
                typst_render::render(
                    page,
                    &typst_render::RenderOptions {
                        pixel_per_pt: Scalar::new(scale.clamp(0.5, 4.0) as f64),
                        render_bleed: false,
                    },
                )
            });
            pix.map(|p| pixmap_to_image(&p))
                .ok_or_else(|| "página vacía".to_string())
        }
        Err(errors) => {
            let mut d = diagnostics::from_source(&world, world.main_id, &errors);
            diags.append(&mut d);
            Err(diags
                .first()
                .and_then(|d| d.message.lines().next())
                .unwrap_or("no compila")
                .to_string())
        }
    };
    comemo::evict(10);
    out
}

/// Pixmap premultiplicado de tiny-skia → `ColorImage` sobre fondo blanco
/// (como la vista previa del PDF).
fn pixmap_to_image(pix: &tiny_skia::Pixmap) -> egui::ColorImage {
    let (w, h) = (pix.width() as usize, pix.height() as usize);
    let mut out = Vec::with_capacity(w * h * 4);
    for px in pix.data().chunks_exact(4) {
        let (r, g, b, a) = (px[0] as f32, px[1] as f32, px[2] as f32, px[3] as f32);
        if a <= 0.0 {
            out.extend_from_slice(&[255, 255, 255, 255]);
        } else {
            // Despremultiplica mezclando sobre blanco.
            let t = 255.0 / a;
            out.push((r * t + 255.0 - a).clamp(0.0, 255.0) as u8);
            out.push((g * t + 255.0 - a).clamp(0.0, 255.0) as u8);
            out.push((b * t + 255.0 - a).clamp(0.0, 255.0) as u8);
            out.push(255);
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([w, h], &out)
}

/// Cada tramo de texto de cada página con su línea fuente, recorriendo los
/// frames (con sus transformaciones) como hace `typst-ide`. Los tramos
/// contiguos de una misma línea fuente en un mismo renglón se funden.
fn extract_sync(world: &EngineWorld, main: FileId, doc: &PagedDocument) -> Vec<SyncEntry> {
    let mut out: Vec<SyncEntry> = Vec::new();
    // Span → offset del fichero principal en una pasada: `World::range` busca
    // desde la raíz en cada llamada (~30 µs; miles de spans por documento).
    let mut offsets = HashMap::new();
    index_spans(LinkedNode::new(world.main.root()), &mut offsets);
    // El mismo nodo de texto produce muchos tramos (y cabeceras repetidas).
    let mut located: HashMap<Span, Option<(Option<String>, usize, usize)>> = HashMap::new();
    for (page, p) in doc.pages().iter().enumerate() {
        walk_text(&p.frame, Transform::identity(), &mut |pos, text| {
            let Some(span) = text
                .glyphs
                .iter()
                .map(|g| g.span.0)
                .find(|s| !s.is_detached())
            else {
                return;
            };
            let loc = located
                .entry(span)
                .or_insert_with(|| match offsets.get(&span) {
                    Some(&b) => {
                        let (l, c) = world.main.lines().byte_to_line_column(b)?;
                        Some((None, l + 1, c + 1))
                    }
                    None => {
                        let (file, line, col) = locate_span(world, main, span);
                        Some((file, line?, col?))
                    }
                });
            let Some((file, line, col)) = loc.clone() else {
                return;
            };
            let (x, y) = (pos.x.to_pt() as f32, pos.y.to_pt() as f32);
            if let Some(last) = out.last_mut()
                && last.page == page
                && last.line == line
                && last.file == file
                && (last.y - y).abs() < 0.5
            {
                last.x = last.x.min(x);
                return;
            }
            out.push(SyncEntry {
                file,
                line,
                col,
                page,
                x,
                y,
            });
        });
    }
    out
}

fn index_spans(node: LinkedNode, out: &mut HashMap<Span, usize>) {
    out.insert(node.span(), node.offset());
    for child in node.children() {
        index_spans(child, out);
    }
}

/// Visita los `TextItem` del frame con su posición en coords de página.
fn walk_text<F: FnMut(Point, &TextItem)>(frame: &Frame, ts: Transform, f: &mut F) {
    for (pos, item) in frame.items() {
        match item {
            FrameItem::Group(g) => {
                let ts = ts
                    .pre_concat(Transform::translate(pos.x, pos.y))
                    .pre_concat(g.transform);
                walk_text(&g.frame, ts, f);
            }
            FrameItem::Text(t) => f(pos.transform(ts), t),
            _ => {}
        }
    }
}

/// Línea → (página, y): la primera aparición de esa línea; si no produce
/// texto (código, línea vacía), la de la línea anterior más cercana que sí;
/// si no hay, la siguiente. No supone orden: las cabeceras repetidas de cada
/// página apuntan a líneas del principio.
pub fn page_for_line(sync: &[SyncEntry], file: Option<&str>, line: usize) -> Option<(usize, f32)> {
    let mut before: Option<&SyncEntry> = None;
    let mut after: Option<&SyncEntry> = None;
    for e in sync.iter().filter(|e| e.file.as_deref() == file) {
        if e.line == line {
            return Some((e.page, e.y));
        }
        if e.line < line && before.is_none_or(|b| e.line > b.line) {
            before = Some(e);
        }
        if e.line > line && after.is_none_or(|b| e.line < b.line) {
            after = Some(e);
        }
    }
    before.or(after).map(|e| (e.page, e.y))
}

/// (Página, x, y) → tramo bajo el punto: el renglón cuya línea base es la
/// primera por debajo de `y` (o el último de la página), y en él el tramo
/// que empieza más a la derecha sin pasar de `x`. Sin texto en la página,
/// el último tramo de las anteriores.
pub fn line_for_pos(sync: &[SyncEntry], page: usize, x: f32, y: f32) -> Option<&SyncEntry> {
    let on_page = || sync.iter().filter(move |e| e.page == page);
    // Tolerancia: un clic sobre los descendentes queda bajo la línea base.
    let row_y = on_page()
        .map(|e| e.y)
        .filter(|&ey| ey >= y - 3.0)
        .min_by(f32::total_cmp)
        .or_else(|| on_page().map(|e| e.y).max_by(f32::total_cmp));
    let Some(row_y) = row_y else {
        return sync.iter().rfind(|e| e.page < page);
    };
    let row = || on_page().filter(move |e| (e.y - row_y).abs() < 1.0);
    row()
        .filter(|e| e.x <= x + 1.0)
        .max_by(|a, b| a.x.total_cmp(&b.x))
        .or_else(|| row().min_by(|a, b| a.x.total_cmp(&b.x)))
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
            println!(
                "sync: {:?} L{} p{} x={:.1} y={:.1}",
                e.file, e.line, e.page, e.x, e.y
            );
        }
    }

    #[test]
    fn synctex_por_linea() {
        let s = sync_of("= Uno\nTexto\n#pagebreak()\n== Dos\nFin\n");
        let lines: Vec<(usize, usize)> = s.iter().map(|e| (e.line, e.page)).collect();
        assert_eq!(lines, [(1, 0), (2, 0), (4, 1), (5, 1)]);
        assert!(s.iter().all(|e| e.file.is_none()));
        // Forward exacto por línea; sin texto (#pagebreak), la anterior.
        assert_eq!(page_for_line(&s, None, 5), Some((1, s[3].y)));
        assert_eq!(page_for_line(&s, None, 2), Some((0, s[1].y)));
        assert_eq!(page_for_line(&s, None, 3), Some((0, s[1].y)));
        // Inverso: el renglón bajo el clic, no el encabezado de la sección.
        let e = line_for_pos(&s, 1, 0.0, s[3].y - 4.0).unwrap();
        assert_eq!(e.line, 5);
        let e = line_for_pos(&s, 1, 0.0, s[2].y - 4.0).unwrap();
        assert_eq!(e.line, 4);
        // Bajo el último renglón: el último de la página.
        let e = line_for_pos(&s, 0, 0.0, 9999.0).unwrap();
        assert_eq!(e.line, 2);
    }

    #[test]
    fn synctex_cabecera_y_columnas() {
        // La cabecera de cada página apunta a la línea 1: el forward de la
        // línea 4 no debe quedarse en la página 0 por ella.
        let s = sync_of("#set page(header: [Cabecera])\nA\n#pagebreak()\nB\n");
        assert_eq!(page_for_line(&s, None, 4).map(|p| p.0), Some(1));
        // Dos tramos en el mismo renglón: decide la x del clic.
        let s = sync_of("#grid(columns: (1fr, 1fr),\n  [Izquierda],\n  [Derecha])\n");
        let izq = s.iter().find(|e| e.line == 2).unwrap();
        let der = s.iter().find(|e| e.line == 3).unwrap();
        assert!((izq.y - der.y).abs() < 1.0 && der.x > izq.x);
        let y = izq.y - 4.0;
        assert_eq!(line_for_pos(&s, 0, der.x + 5.0, y).unwrap().line, 3);
        assert_eq!(line_for_pos(&s, 0, izq.x + 5.0, y).unwrap().line, 2);
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
