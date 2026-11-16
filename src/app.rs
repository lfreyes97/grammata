//! Estado de la app, bucle por frame y ejecución de acciones.

use crate::i18n::{tr, trf};
use crate::actions::Action;
use crate::chrome;
use crate::compile::{self, Compiler, Key};
use crate::diagnostics;
use crate::doc::{Doc, DocId, Docs};
use crate::editor::{self, EditorUi, Goto};
use crate::files;
use crate::formatbar::{FormatBar, Mark};
use crate::layout::{self, Panel, Preset, Tab};
use crate::panels::{self, OutlineCache};
use crate::preview::{self, Preview};
use crate::scratch;
use crate::session::{self, Prefs, Session, SessionTab};
use crate::snippets::{self, UserSnippet};
use crate::symbols;
use crate::templates;
use crate::text;
use crate::theme;
use egui::{Key as K, Modifiers as M};
use egui_dock::{DockArea, DockState, TabViewer, tab_viewer::OnCloseResponse};
use egui_phosphor::regular as icon;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEFAULT_FONT: f32 = 14.0;
const MAX_OPEN_BYTES: u64 = 5_000_000;
/// Pausa al escribir antes de autoguardar.
const AUTOSAVE_DELAY: Duration = Duration::from_millis(800);
/// Intervalo mínimo entre dos escrituras del PDF junto al fuente.
const PDF_EVERY: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, PartialEq)]
pub enum StatusKind {
    Ok,
    Err,
    Info,
}

/// Mensaje transitorio de la barra de estado.
pub struct Status {
    msg: String,
    kind: StatusKind,
    at: Instant,
}

impl Status {
    pub fn visible(&self) -> Option<(&str, StatusKind)> {
        let ttl = if self.kind == StatusKind::Err { 15 } else { 6 };
        (!self.msg.is_empty() && self.at.elapsed() < Duration::from_secs(ttl))
            .then_some((self.msg.as_str(), self.kind))
    }
}

pub enum Modal {
    ConfirmClose(DocId),
    ConfirmDelete(PathBuf),
    GotoLine(String),
    About,
    EditSnippet(snippets::Editing),
    NewProject(crate::project::NewProject),
    Export(ExportDlg),
}

/// Opciones del diálogo de exportación. `total`: páginas de la vista previa
/// (0 si aún no hay), para validar el rango.
pub struct ExportDlg {
    pub fmt: String,
    pub pages: String,
    pub ppi: u32,
    pub total: usize,
}

const MAX_RECENT_SYMBOLS: usize = 18;

/// Recuentos de la barra de estado, cacheados por (doc, rev[, cursor]).
#[derive(Default)]
pub struct Stats {
    key: Option<(DocId, u64)>,
    counts: (usize, usize),
    cursor_key: Option<(DocId, u64, usize)>,
    line_col: (usize, usize),
}

impl Stats {
    pub fn get(&mut self, doc: &Doc) -> (usize, usize) {
        if self.key != Some((doc.id, doc.rev)) {
            self.counts = text::content_stats(&doc.text);
            self.key = Some((doc.id, doc.rev));
        }
        self.counts
    }

    pub fn line_col(&mut self, doc: &Doc, c: usize) -> (usize, usize) {
        if self.cursor_key != Some((doc.id, doc.rev, c)) {
            self.line_col = text::char_to_line_col(&doc.text, c);
            self.cursor_key = Some((doc.id, doc.rev, c));
        }
        self.line_col
    }
}

pub struct State {
    pub docs: Docs,
    pub prefs: Prefs,
    pub compiler: Compiler,
    pub ide: crate::ide::Ide,
    pub preview: Preview,
    pub editor: EditorUi,
    pub outline: OutlineCache,
    pub stats: Stats,
    pub status: Status,
    pub actions: Vec<Action>,
    pub modal: Option<Modal>,
    pub typst_version: Option<String>,
    pub needs_persist: bool,
    pub symbol_query: String,
    pub recent_symbols: Vec<String>,
    pub snippet_query: String,
    pub template: templates::Options,
    /// Filtro del desplegable de fuentes del builder.
    pub template_font_filter: String,
    /// Carpeta del proyecto donde escribe el builder (sin ella, pregunta).
    pub template_dir: Option<PathBuf>,
    pub files: files::FileMgr,
    pub scratch: scratch::Scratch,
    pub user_snippets: Vec<UserSnippet>,
    /// Bibliotecas `snippets.typ` (global y del proyecto).
    pub snippet_libs: crate::snippet_lib::Libs,
    /// Plugins instalados (paquetes, snippets, temas y plantillas).
    pub plugins: crate::plugins::Plugins,
    pub fmt: FormatBar,
    force_compile: bool,
    /// PDF junto al fuente: hay uno nuevo por escribir, cuándo se escribió el
    /// último y si ya se avisó en la barra de estado.
    pdf_pending: bool,
    last_pdf: Option<std::time::Instant>,
    pdf_announced: bool,
    /// Documento fijado en la vista previa: se compila y muestra él aunque
    /// se edite otro (p. ej. `main.typ` mientras se escribe un capítulo).
    pub pinned: Option<DocId>,
    /// Sube al guardar un archivo que no es el principal (ver `Key::disk`).
    pub disk_gen: u64,
    /// Pestañas del visor de imágenes.
    pub images: crate::image_view::Images,
    /// Exportación en curso (hilo) y última carpeta elegida.
    export_rx: Option<std::sync::mpsc::Receiver<Result<PathBuf, String>>>,
    last_export_dir: Option<PathBuf>,
    /// Programa de un plugin en marcha (hilo) y qué hacer con su salida.
    plugin_job: Option<PluginJob>,
}

struct PluginJob {
    rx: std::sync::mpsc::Receiver<Result<crate::plugins::run::Done, String>>,
    then: Then,
    /// Para la barra de estado: «Importar Markdown».
    label: String,
}

/// Destino del resultado de un programa de plugin.
enum Then {
    NewDoc,
    /// El texto se escribe en ese archivo.
    Write(PathBuf),
    /// El programa ya escribió el archivo.
    Wrote(PathBuf),
    Replace {
        doc: DocId,
        start: usize,
        expect: String,
    },
}

impl State {
    pub(crate) fn set_status(&mut self, msg: impl Into<String>, kind: StatusKind) {
        self.status = Status {
            msg: msg.into(),
            kind,
            at: Instant::now(),
        };
    }

    /// Raíz efectiva de compilación del documento actual (ver
    /// `Doc::compile_root`).
    pub(crate) fn compile_root(&self) -> PathBuf {
        self.docs
            .cur()
            .compile_root(self.prefs.files_root.as_deref())
    }

    /// Documento que se compila y se ve en la vista previa: el fijado o,
    /// sin fijar, el actual.
    pub(crate) fn main_doc(&self) -> &Doc {
        self.pinned
            .and_then(|id| self.docs.get(id))
            .unwrap_or_else(|| self.docs.cur())
    }

    pub(crate) fn main_key(&self) -> Key {
        let doc = self.main_doc();
        Key {
            doc: doc.id,
            rev: doc.rev,
            disk: self.disk_gen,
        }
    }

    pub(crate) fn main_root(&self) -> PathBuf {
        self.main_doc()
            .compile_root(self.prefs.files_root.as_deref())
    }

    /// Nombre del documento fijado (si lo hay y sigue abierto).
    pub(crate) fn pinned_name(&self) -> Option<String> {
        self.pinned.and_then(|id| self.docs.get(id)).map(Doc::fname)
    }

    /// Ruta del documento actual en las entradas synctex del principal:
    /// `None` si es el principal, `Some(relativa a la raíz)` si es un
    /// incluido. `Err` si no está bajo la raíz (no puede aparecer).
    pub(crate) fn sync_file(&self) -> Result<Option<String>, ()> {
        let (cur, main) = (self.docs.cur(), self.main_doc());
        if cur.id == main.id {
            return Ok(None);
        }
        let root = self.main_root();
        cur.path
            .as_deref()
            .and_then(|p| p.strip_prefix(&root).ok())
            .map(|r| {
                Some(
                    r.components()
                        .map(|c| c.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/"),
                )
            })
            .ok_or(())
    }

    fn preview_info(&self) -> preview::Info {
        let doc = self.main_doc();
        let built = self.compiler.built.get(&doc.id);
        preview::Info {
            compiling: self.compiler.in_flight.is_some_and(|k| k.doc == doc.id),
            auto: self.prefs.auto_compile,
            errors: built
                .map(|b| diagnostics::count(&b.diagnostics).0)
                .unwrap_or(0),
            stale: self.preview.loaded.is_some_and(|k| k != self.main_key()),
            has_build: built.is_some(),
            sync: built.map(|b| b.sync.clone()).unwrap_or_default(),
            root: self.main_root(),
            main_path: (doc.id != self.docs.cur_id())
                .then(|| doc.path.clone())
                .flatten(),
            pinned: self.pinned_name(),
            can_pin: self.docs.cur().path.is_some(),
        }
    }
}

pub struct App {
    st: State,
    dock: DockState<Tab>,
    dock_style: Option<(bool, egui_dock::Style)>,
    last_persist: Instant,
    title: String,
    /// Última rev de cada doc que el autoguardado ya revisó.
    autosave_seen: std::collections::HashMap<DocId, u64>,
    /// Owns the session or not, and the paths later launches send.
    instance: crate::instance::Instance,
}

impl App {
    /// Starts the app and opens the paths given on the command line, on top of
    /// the restored session.
    pub fn with_paths(
        cc: &eframe::CreationContext,
        paths: Vec<PathBuf>,
        instance: crate::instance::Instance,
    ) -> Self {
        let mut app = Self::start(cc, instance);
        app.open_paths(paths);
        app
    }

    /// Opens what was asked on the command line (or by a later launch).
    fn open_paths(&mut self, paths: Vec<PathBuf>) {
        let app = self;
        for p in paths {
            if p.is_dir() {
                files::set_root(&mut app.st, p);
                app.st.actions.push(Action::ShowPanel(Panel::Files));
            } else if p.exists() || !p.parent().is_some_and(Path::is_dir) {
                app.open_path(p);
            } else {
                // Doesn't exist yet: an empty tab that saves to that path.
                let cur = app.st.docs.cur_id();
                let id = app.st.docs.push(Some(p.clone()), String::new());
                layout::add_doc(&mut app.dock, id, cur);
                app.switch_to_id(id);
                app.st
                    .set_status(trf!("Nuevo archivo {}", p.display()), StatusKind::Info);
            }
        }
    }

    /// An app that owns the session and listens to nobody (tests).
    #[allow(dead_code)]
    pub fn new(cc: &eframe::CreationContext) -> Self {
        Self::start(cc, crate::instance::Instance::solo())
    }

    fn start(cc: &eframe::CreationContext, instance: crate::instance::Instance) -> Self {
        let ctx = cc.egui_ctx.clone();
        theme::install_fonts(&ctx);
        // Ctrl+= / Ctrl+- / Ctrl+0 son para la fuente del editor, no para el zoom de la UI.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);

        let mut session = session::load().unwrap_or_default();
        if !instance.owns_session {
            // A guest takes the preferences but not the owner's tabs.
            session.tabs.clear();
            session.layout = None;
            session.current = 0;
            session.prefs.pinned = None;
        }
        let mut docs = Docs::new();
        let mut ids: Vec<Option<DocId>> = Vec::new();
        for t in session.tabs.into_iter().take(session::MAX_TABS) {
            ids.push(restore_tab(&mut docs, t));
        }
        if docs.list.is_empty() {
            docs.push(None, include_str!("../ejemplo.typ").to_string());
            ids.clear();
        }
        docs.current = session.current.min(docs.list.len() - 1);

        let mut dock = session
            .layout
            .as_ref()
            .filter(|_| !ids.is_empty())
            .map(|l| session::restore_layout(l, &ids))
            .filter(|d| d.iter_all_tabs().any(|(_, t)| matches!(t, Tab::Doc(_))))
            .unwrap_or_else(|| {
                Preset::Classic.build(docs.list.iter().map(|d| Tab::Doc(d.id)).collect())
            });
        // Docs que el diseño guardado no conocía (o diseño nuevo).
        let first = docs.list[0].id;
        for d in &docs.list {
            if dock.find_tab(&Tab::Doc(d.id)).is_none() {
                layout::add_doc(&mut dock, d.id, first);
            }
        }
        layout::focus_doc(&mut dock, docs.cur_id());

        let (scratch_text, scratch_equation, scratch_block) = (
            session.prefs.scratch_text.clone(),
            session.prefs.scratch_equation,
            session.prefs.scratch_block,
        );
        let mut prefs = session.prefs;
        crate::i18n::init(prefs.lang.as_deref());
        // Antes de tocar las fuentes del motor: los plugins aportan las suyas.
        let plugins = crate::plugins::Plugins::load();
        match prefs.theme.as_deref().map(|t| plugins.theme(t)) {
            Some(Some(p)) => {
                prefs.dark = p.dark;
                theme::set_custom(Some(p));
            }
            // El plugin del tema ya no está.
            Some(None) => prefs.theme = None,
            None => {}
        }
        theme::apply(&ctx, prefs.dark);
        let mut st = State {
            docs,
            prefs,
            compiler: Compiler::new(ctx.clone()),
            ide: crate::ide::Ide::new(ctx.clone()),
            preview: Preview::new(ctx.clone()),
            editor: EditorUi::default(),
            outline: OutlineCache::default(),
            stats: Stats::default(),
            status: Status {
                msg: String::new(),
                kind: StatusKind::Info,
                at: Instant::now(),
            },
            actions: Vec::new(),
            modal: None,
            typst_version: compile::typst_version(),
            needs_persist: false,
            symbol_query: String::new(),
            recent_symbols: session.recent_symbols,
            snippet_query: String::new(),
            user_snippets: snippets::load_user(),
            snippet_libs: crate::snippet_lib::Libs::default(),
            plugins,
            template: templates::Options::default(),
            template_font_filter: String::new(),
            template_dir: None,
            files: files::FileMgr::default(),
            scratch: scratch::Scratch::from_session(scratch_text, scratch_equation, scratch_block),
            fmt: FormatBar::preloaded(&ctx),
            force_compile: false,
            pdf_pending: false,
            last_pdf: None,
            pdf_announced: false,
            export_rx: None,
            last_export_dir: None,
            plugin_job: None,
            pinned: None,
            disk_gen: 0,
            images: crate::image_view::Images::new(ctx.clone()),
        };
        st.pinned = st
            .prefs
            .pinned
            .as_deref()
            .and_then(|p| st.docs.by_path(p))
            .map(|i| st.docs.list[i].id);

        Self {
            st,
            dock,
            dock_style: None,
            last_persist: Instant::now(),
            title: String::new(),
            autosave_seen: Default::default(),
            instance,
        }
    }

    // ---------- Sesión ----------

    fn persist(&mut self) {
        if !self.instance.owns_session {
            // Guests leave the owner's session alone.
            self.st.needs_persist = false;
            self.last_persist = Instant::now();
            return;
        }
        let docs = &self.st.docs.list;
        let order: Vec<DocId> = docs.iter().take(session::MAX_TABS).map(|d| d.id).collect();
        let tabs = docs
            .iter()
            .take(session::MAX_TABS)
            .map(|d| SessionTab {
                path: d.path.clone(),
                // Archivo guardado y sin cambios: se relee del disco al abrir.
                text: if d.path.is_some() && !d.modified() {
                    String::new()
                } else {
                    session::tab_text(&d.text)
                },
            })
            .collect();
        let layout = session::save_layout(&self.dock, &order);
        let recent = self.st.recent_symbols.clone();
        self.st.prefs.scratch_text = self.st.scratch.text.clone();
        self.st.prefs.scratch_equation = self.st.scratch.equation;
        self.st.prefs.scratch_block = self.st.scratch.block;
        self.st.prefs.pinned = self
            .st
            .pinned
            .and_then(|id| self.st.docs.get(id))
            .and_then(|d| d.path.clone());
        let s = Session::new(
            tabs,
            self.st.docs.current,
            self.st.prefs.clone(),
            layout,
            recent,
        );
        match session::save(&s) {
            Ok(()) => self.st.needs_persist = false,
            Err(e) => self.st.set_status(
                trf!("No se pudo guardar la sesión: {}", e),
                StatusKind::Err,
            ),
        }
        self.last_persist = Instant::now();
    }

    // ---------- Entrada ----------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.st.modal.is_some() {
            return;
        }
        let a = &mut self.st.actions;
        let cs = M::CTRL | M::SHIFT;
        ctx.input_mut(|i| {
            let mut k = |m: M, key: K| i.consume_key(m, key);
            // Con Mayús primero: `consume_key(CTRL, …)` también acepta Ctrl+Mayús.
            if k(cs, K::S) {
                a.push(Action::SaveAs)
            }
            if k(cs, K::Tab) {
                a.push(Action::PrevDoc)
            }
            if k(cs, K::O) {
                a.push(Action::TogglePanel(Panel::Outline))
            }
            if k(cs, K::V) {
                a.push(Action::TogglePanel(Panel::Preview))
            }
            if k(cs, K::M) {
                a.push(Action::TogglePanel(Panel::Problems))
            }
            if k(cs, K::Y) {
                a.push(Action::TogglePanel(Panel::Symbols))
            }
            if k(cs, K::I) {
                a.push(Action::TogglePanel(Panel::Snippets))
            }
            if k(cs, K::T) {
                a.push(Action::TogglePanel(Panel::Templates))
            }
            if k(cs, K::F) {
                a.push(Action::TogglePanel(Panel::Files))
            }
            if k(cs, K::B) {
                a.push(Action::TogglePanel(Panel::Scratch))
            }
            if k(cs, K::U) {
                a.push(Action::TogglePanel(Panel::Plugins))
            }
            if k(M::SHIFT, K::F3) {
                self.st.editor.pending_step = Some(-1)
            }
            if k(cs, K::N) {
                a.push(Action::NewProject)
            }
            if k(M::CTRL, K::N) || k(M::CTRL, K::T) {
                a.push(Action::NewDoc)
            }
            if k(M::CTRL, K::O) {
                a.push(Action::Open)
            }
            if k(M::CTRL, K::S) {
                a.push(Action::Save)
            }
            if k(M::CTRL, K::W) {
                a.push(Action::CloseCurrent)
            }
            if k(M::CTRL, K::Tab) || k(M::CTRL, K::PageDown) {
                a.push(Action::NextDoc)
            }
            if k(M::CTRL, K::PageUp) {
                a.push(Action::PrevDoc)
            }
            if k(M::CTRL, K::F) {
                a.push(Action::Find)
            }
            if k(M::CTRL, K::G) {
                a.push(Action::GotoLinePrompt)
            }
            if k(M::CTRL, K::E) {
                a.push(Action::Export("pdf".into()))
            }
            if k(M::NONE, K::F5) {
                a.push(Action::Compile)
            }
            if k(M::NONE, K::F11) {
                let on = !self.st.preview.presentation;
                self.st.preview.set_presentation(ctx, on);
            }
            // La condición primero: `k` consume la tecla y Esc la necesitan
            // la búsqueda, el autocompletado y los campos de los paneles.
            if self.st.preview.presentation && k(M::NONE, K::Escape) {
                self.st.preview.set_presentation(ctx, false);
            }
            if k(M::NONE, K::F3) {
                self.st.editor.pending_step = Some(1)
            }
            if k(M::CTRL, K::Equals) || k(M::CTRL, K::Plus) {
                a.push(Action::FontDelta(1.0))
            }
            if k(M::CTRL, K::Minus) {
                a.push(Action::FontDelta(-1.0))
            }
            if k(M::CTRL, K::Num0) {
                a.push(Action::FontReset)
            }
            if k(M::ALT, K::Z) {
                a.push(Action::ToggleWrap)
            }
            if k(M::CTRL, K::Space) {
                self.st.editor.ac.forced = true
            }
            for (key, mark) in [
                (K::B, Mark::Bold),
                (K::I, Mark::Italic),
                (K::U, Mark::Underline),
            ] {
                if k(M::CTRL, key) {
                    a.push(Action::ToggleMark(mark))
                }
            }
            if k(M::CTRL, K::Slash) {
                a.push(Action::PrefixLines("// ".into()))
            }
            let nums = [
                K::Num1,
                K::Num2,
                K::Num3,
                K::Num4,
                K::Num5,
                K::Num6,
                K::Num7,
                K::Num8,
                K::Num9,
            ];
            for (n, key) in nums.into_iter().enumerate() {
                if k(M::CTRL, key) {
                    a.push(Action::SwitchTo(n))
                }
            }
        });
        // Archivos soltados sobre la ventana.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        for p in dropped {
            self.st.actions.push(Action::OpenPath(p));
        }
    }

    // ---------- Compilación ----------

    fn compile_tick(&mut self, ctx: &egui::Context) {
        let st = &mut self.st;
        st.ide.poll();
        for key in st.compiler.poll() {
            if key.doc == st.main_doc().id
                && let Some(e) = st.compiler.built.get(&key.doc).and_then(|b| {
                    b.diagnostics
                        .iter()
                        .find(|d| d.severity == diagnostics::Severity::Error)
                })
            {
                let msg = e.message.lines().next().unwrap_or("").to_string();
                st.set_status(format!("{}: {msg}", e.location()), StatusKind::Err);
            }
            // Compiló bien el principal: su PDF está listo para copiarse junto al fuente.
            let main = st.main_doc();
            if st.prefs.pdf_alongside
                && key.doc == main.id
                && main.path.is_some()
                && st
                    .compiler
                    .built
                    .get(&key.doc)
                    .and_then(|b| b.pdf.as_ref())
                    .is_some_and(|(k, _)| *k == key)
            {
                st.pdf_pending = true;
            }
        }
        if st.pdf_pending {
            let wait = st
                .last_pdf
                .map_or(Duration::ZERO, |t| PDF_EVERY.saturating_sub(t.elapsed()));
            if !st.prefs.pdf_alongside {
                st.pdf_pending = false;
            } else if wait.is_zero() {
                st.pdf_pending = false;
                let main = st.main_doc();
                let dest = main.path.as_deref().map(pdf_beside);
                let data = st.compiler.built.get(&main.id).and_then(|b| b.pdf.clone());
                if let (Some(dest), Some((_, data))) = (dest, data) {
                    match write_atomic(&dest, &data) {
                        Ok(()) => {
                            st.last_pdf = Some(Instant::now());
                            if !std::mem::replace(&mut st.pdf_announced, true) {
                                st.set_status(
                                    trf!("PDF guardado junto al fuente: {}", dest.display()),
                                    StatusKind::Ok,
                                );
                            }
                        }
                        Err(e) => st.set_status(
                            trf!("No se pudo guardar el PDF: {}", e),
                            StatusKind::Err,
                        ),
                    }
                }
            } else {
                ctx.request_repaint_after(wait);
            }
        }
        // Vista previa ← último PDF válido del doc principal.
        let main = st.main_doc().id;
        match st.compiler.built.get(&main).and_then(|b| b.pdf.clone()) {
            Some((k, data)) if st.preview.loaded != Some(k) => st.preview.load(k, data),
            None if st.preview.loaded.is_some() => st.preview.clear(),
            _ => {}
        }
        st.preview.poll(ctx);
        if let Some(rx) = &st.export_rx {
            use std::sync::mpsc::TryRecvError;
            match rx.try_recv() {
                Ok(Ok(p)) => {
                    st.export_rx = None;
                    st.set_status(trf!("Exportado {}", p.display()), StatusKind::Ok);
                }
                Ok(Err(e)) => {
                    st.export_rx = None;
                    st.set_status(trf!("Falló la exportación: {}", e), StatusKind::Err);
                }
                Err(TryRecvError::Disconnected) => st.export_rx = None,
                Err(TryRecvError::Empty) => {}
            }
        }

        if st.compiler.in_flight.is_some() {
            return;
        }
        let key = st.main_key();
        let doc = st.docs.get(key.doc).expect("doc principal");
        let go = if st.force_compile {
            st.force_compile = false;
            true
        } else if st.prefs.auto_compile && !st.compiler.is_built(key) {
            let first = !st.compiler.built.contains_key(&doc.id);
            let wait = debounce(doc.text.len());
            let elapsed = doc.last_edit.elapsed();
            if !first && elapsed < wait {
                ctx.request_repaint_after(wait - elapsed);
            }
            first || elapsed >= wait
        } else {
            false
        };
        if go {
            let (text, root) = (doc.text.clone(), st.main_root());
            let filename =
                crate::engine::EngineWorld::filename_for_job(doc.path.as_deref(), &root);
            st.compiler.request(key, text, root, filename);
        }
    }

    // ---------- Acciones ----------

    fn run_actions(&mut self, ctx: &egui::Context) {
        let mut guard = 0;
        while !self.st.actions.is_empty() && guard < 8 {
            guard += 1;
            for a in std::mem::take(&mut self.st.actions) {
                self.run(a, ctx);
            }
        }
    }

    fn run(&mut self, action: Action, ctx: &egui::Context) {
        let cur = self.st.docs.cur_id();
        match action {
            Action::NewDoc => {
                let id = self.st.docs.push(None, String::new());
                layout::add_doc(&mut self.dock, id, cur);
                self.switch_to_id(id);
                self.st.needs_persist = true;
            }
            Action::NewDocWith(text) => {
                let id = self.st.docs.push(None, text);
                // Sin guardar: al cerrar pregunta, como cualquier texto nuevo.
                if let Some(d) = self.st.docs.get_mut(id) {
                    d.saved.clear();
                }
                layout::add_doc(&mut self.dock, id, cur);
                self.switch_to_id(id);
                self.st.needs_persist = true;
            }
            Action::PluginsReload => {
                self.st.plugins.reload();
                self.plugins_changed(ctx);
                let n = self.st.plugins.list.len();
                self.st.set_status(format!("Plugins: {n}"), StatusKind::Info);
            }
            Action::PluginEnable(id, on) => {
                self.st.plugins.set_enabled(&id, on);
                self.plugins_changed(ctx);
            }
            Action::PluginInstall(id) => match self.st.plugins.install(id) {
                Ok(manifest) => {
                    self.plugins_changed(ctx);
                    self.open_path(manifest);
                }
                Err(e) => self
                    .st
                    .set_status(trf!("No se pudo crear el plugin: {}", e), StatusKind::Err),
            },
            Action::PluginAllow(id, on) => self.st.plugins.set_allowed(&id, on),
            Action::Import => self.import_dialog(),
            Action::ImportPath(p) => self.import_path(ctx, p),
            Action::PluginRun { plugin, kind, id } => self.plugin_run(ctx, &plugin, kind, &id),
            Action::SetTheme(key) => self.set_theme(ctx, key),
            Action::NewProject => {
                let np = crate::project::NewProject::new(self.st.prefs.files_root.as_deref());
                self.st.modal = Some(Modal::NewProject(np));
            }
            Action::OpenProject { dir, kind } => {
                files::set_root(&mut self.st, dir.clone());
                match kind {
                    crate::project::Kind::Empty => {
                        self.open_path(dir.join(templates::MAIN_FILE));
                        self.st.actions.push(Action::ShowPanel(Panel::Files));
                    }
                    crate::project::Kind::Template(k) => {
                        self.st.template.set_kind(k);
                        self.st.template_dir = Some(dir);
                        self.st.actions.push(Action::ShowPanel(Panel::Templates));
                    }
                }
            }
            Action::NewFromTemplate(opts) => {
                // Con proyecto vinculado escribe ahí; si no, pide carpeta.
                let dir = self.st.template_dir.clone().or_else(|| {
                    let mut dlg = rfd::FileDialog::new()
                        .set_title(trf!("Carpeta del documento ({})", opts.kind.name()));
                    let start = self.st.prefs.files_root.clone().or_else(|| {
                        self.st
                            .docs
                            .cur()
                            .path
                            .as_deref()
                            .and_then(Path::parent)
                            .map(Path::to_path_buf)
                    });
                    if let Some(dir) = start {
                        dlg = dlg.set_directory(dir);
                    }
                    dlg.pick_folder()
                });
                if let Some(dir) = dir {
                    match opts.write_project(&dir) {
                        Ok(main) => {
                            self.st.template_dir = None;
                            if self.open_path(main) {
                                self.st.set_status(
                                    trf!("Creados {} en {}", opts.files().join(", "), dir.display()),
                                    StatusKind::Ok,
                                );
                            }
                        }
                        Err(e) => self.st.set_status(e, StatusKind::Err),
                    }
                }
            }
            Action::PickFolder => {
                crate::files::pick_root(&mut self.st);
                self.st.actions.push(Action::ShowPanel(Panel::Files));
            }
            Action::Open => {
                let mut dlg = rfd::FileDialog::new()
                    .add_filter("Typst", &["typ"])
                    .add_filter("Todos", &["*"]);
                if let Some(dir) = self.st.docs.cur().path.as_deref().and_then(Path::parent) {
                    dlg = dlg.set_directory(dir);
                }
                for p in dlg.pick_files().unwrap_or_default() {
                    self.open_path(p);
                }
            }
            Action::OpenPath(p) => {
                self.open_path(p);
            }
            Action::OpenAt { path, line, col } => {
                if self.open_path(path) {
                    self.jump_to(line, col);
                }
            }
            Action::Save => {
                self.save(cur, false);
            }
            Action::SaveAs => {
                self.save(cur, true);
            }
            Action::Close(id) => match self.st.docs.get(id) {
                Some(d) if d.modified() && !(d.path.is_none() && d.text.trim().is_empty()) => {
                    self.switch_to_id(id);
                    self.st.modal = Some(Modal::ConfirmClose(id));
                }
                Some(_) => self.discard(id),
                None => {}
            },
            Action::CloseCurrent => self.st.actions.push(Action::Close(cur)),
            Action::Discard(id) => self.discard(id),
            Action::SaveAndClose(id) => {
                if self.save(id, false) {
                    self.discard(id);
                }
            }
            Action::NextDoc | Action::PrevDoc => {
                let n = self.st.docs.list.len();
                let i = self.st.docs.current;
                let j = if matches!(action, Action::NextDoc) {
                    (i + 1) % n
                } else {
                    (i + n - 1) % n
                };
                let id = self.st.docs.list[j].id;
                self.switch_to_id(id);
            }
            Action::SwitchTo(i) => {
                if let Some(d) = self.st.docs.list.get(i) {
                    let id = d.id;
                    self.switch_to_id(id);
                }
            }
            Action::Compile => {
                self.st.force_compile = true;
                // F5 es una petición expresa: el PDF no espera al intervalo.
                self.st.last_pdf = None;
            }
            Action::Export(fmt) => {
                self.st.modal = Some(Modal::Export(ExportDlg {
                    fmt,
                    pages: String::new(),
                    ppi: 144,
                    total: self.st.preview.page_count(),
                }));
            }
            Action::ExportRun { fmt, ppi, pages } => self.export_with(ctx, &fmt, ppi, &pages),
            Action::Find => {
                let f = &mut self.st.editor.find;
                f.open = true;
                f.want_focus = true;
            }
            Action::GotoLinePrompt => {
                let (l, _) = self
                    .st
                    .editor
                    .cursor
                    .map(|c| text::char_to_line_col(&self.st.docs.cur().text, c))
                    .unwrap_or((1, 1));
                self.st.modal = Some(Modal::GotoLine(l.to_string()));
            }
            Action::JumpTo { line, col } => self.jump_to(line, col),
            Action::GotoPreviewPage { page, y } => self.st.preview.goto_position(page, y),
            Action::SyncToPdf => {
                let doc = self.st.docs.cur();
                let at = match self.st.editor.selection {
                    Some((a, _)) => a,
                    None => self.st.editor.cursor.unwrap_or(0),
                };
                let (line, _) = text::char_to_line_col(&doc.text, at);
                let file = self.st.sync_file().ok();
                let pos = file.and_then(|f| {
                    let b = self.st.compiler.built.get(&self.st.main_doc().id)?;
                    crate::engine::page_for_line(&b.sync, f.as_deref(), line)
                });
                match pos {
                    Some((page, y)) => {
                        layout::show_panel(&mut self.dock, Panel::Preview, cur);
                        self.st.preview.goto_position(page, y);
                    }
                    None => self.st.set_status(
                        tr("Sin posición en el PDF: compila el documento primero"),
                        StatusKind::Info,
                    ),
                }
            }
            Action::Select { start, end } => {
                self.st.editor.goto = Some(Goto {
                    doc: cur,
                    start,
                    end,
                    focus: true,
                });
                layout::focus_doc(&mut self.dock, cur);
            }
            Action::TogglePanel(p) => layout::toggle_panel(&mut self.dock, p, cur),
            Action::ShowPanel(p) => layout::show_panel(&mut self.dock, p, cur),
            Action::ApplyPreset(p) => {
                let tabs = (self.st.docs.list.iter().map(|d| Tab::Doc(d.id)))
                    .chain(self.st.images.list.iter().map(|t| Tab::Image(t.id)))
                    .collect();
                self.dock = p.build(tabs);
                layout::focus_doc(&mut self.dock, cur);
                self.st.needs_persist = true;
            }
            Action::ToggleLang => {
                let next = match crate::i18n::current() {
                    crate::i18n::Lang::Es => crate::i18n::Lang::En,
                    crate::i18n::Lang::En => crate::i18n::Lang::Es,
                };
                crate::i18n::set(next);
                self.st.prefs.lang = Some(next.code().to_string());
                self.st.needs_persist = true;
            }
            Action::ToggleTheme => {
                // Alternar claro/oscuro deja el tema de plugin, si lo había.
                self.st.prefs.theme = None;
                theme::set_custom(None);
                self.st.prefs.dark = !self.st.prefs.dark;
                theme::apply(ctx, self.st.prefs.dark);
                self.dock_style = None;
                self.st.needs_persist = true;
            }
            Action::FontDelta(d) => {
                self.st.prefs.font_size = (self.st.prefs.font_size + d).clamp(9.0, 28.0);
                self.st.needs_persist = true;
            }
            Action::FontReset => {
                self.st.prefs.font_size = DEFAULT_FONT;
                self.st.needs_persist = true;
            }
            Action::ToggleWrap => {
                self.st.prefs.wrap = !self.st.prefs.wrap;
                self.st.needs_persist = true;
            }
            Action::TogglePin => {
                self.st.pinned = match self.st.pinned {
                    Some(_) => None,
                    None => self.st.docs.cur().path.is_some().then_some(cur),
                };
                self.st.needs_persist = true;
            }
            Action::ToggleAutoSave => {
                self.st.prefs.auto_save = !self.st.prefs.auto_save;
                self.st.needs_persist = true;
            }
            Action::TogglePdfAlongside => {
                self.st.prefs.pdf_alongside = !self.st.prefs.pdf_alongside;
                self.st.pdf_pending = self.st.prefs.pdf_alongside;
                self.st.pdf_announced = false;
                self.st.needs_persist = true;
            }
            Action::NewWindow => {
                let spawned = std::env::current_exe()
                    .and_then(|exe| std::process::Command::new(exe).arg("--new-window").spawn());
                match spawned {
                    // Reaped in the background so it never lingers as a zombie.
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                    Err(e) => self.st.set_status(
                        trf!("No se pudo abrir otra ventana: {}", e),
                        StatusKind::Err,
                    ),
                }
            }
            Action::ToggleAuto => {
                self.st.prefs.auto_compile = !self.st.prefs.auto_compile;
                self.st.needs_persist = true;
            }
            Action::ToggleFormatBar => {
                self.st.prefs.format_bar = !self.st.prefs.format_bar;
                self.st.needs_persist = true;
            }
            Action::About => self.st.modal = Some(Modal::About),
            Action::InsertMath(insert) => {
                self.insert(&insert, true);
                let r = &mut self.st.recent_symbols;
                r.retain(|x| *x != insert);
                r.insert(0, insert);
                r.truncate(MAX_RECENT_SYMBOLS);
                self.st.needs_persist = true;
            }
            Action::InsertSnippet(body) => self.insert(&body, false),
            Action::InsertImage(img) => {
                let doc = self.st.docs.cur();
                match crate::image_view::image_ref(&img, doc.path.as_deref(), &self.st.compile_root())
                {
                    Some(r) => {
                        let r = r.replace('\\', "\\\\").replace('"', "\\\"");
                        self.insert(&format!("#image(\"{r}\")${{}}"), false);
                        layout::focus_doc(&mut self.dock, cur);
                    }
                    None => self.st.set_status(
                        tr("La imagen está fuera de la carpeta del proyecto: typst no puede leerla"),
                        StatusKind::Err,
                    ),
                }
            }
            Action::InsertLibSnippet { body, preamble } => {
                if let Some(p) = preamble {
                    self.prepend(&p);
                }
                self.insert(&body, false);
            }
            Action::CreateSnippetLib(path) => {
                if !path.exists() {
                    let made = path
                        .parent()
                        .map_or(Ok(()), std::fs::create_dir_all)
                        .and_then(|_| std::fs::write(&path, crate::snippet_lib::starter()));
                    if let Err(e) = made {
                        self.st.set_status(
                            trf!("No se pudo crear {}: {}", path.display(), e),
                            StatusKind::Err,
                        );
                        return;
                    }
                }
                // Que el panel la vea ya, sin esperar a la próxima comprobación.
                self.st.snippet_libs.global.invalidate();
                self.st.snippet_libs.project = None;
                self.open_path(path);
            }
            Action::ReplaceText {
                doc,
                start,
                expect,
                text,
            } => self.replace_text(doc, start, &expect, text),
            Action::PrefixLines(prefix) => self.prefix_lines(&prefix),
            Action::ToggleMark(mark) => self.toggle_mark(mark),
            Action::InsertSetRule(rule) => self.insert_set_rule(&rule),
            Action::NewSnippet => {
                let body = self.selected_text();
                self.st.modal = Some(Modal::EditSnippet(snippets::Editing {
                    index: None,
                    name: String::new(),
                    body,
                }));
            }
            Action::EditSnippet(i) => {
                if let Some(s) = self.st.user_snippets.get(i) {
                    let e = snippets::Editing {
                        index: Some(i),
                        name: s.name.clone(),
                        body: s.body.clone(),
                    };
                    self.st.modal = Some(Modal::EditSnippet(e));
                }
            }
            Action::DeleteSnippet(i) => {
                if i < self.st.user_snippets.len() {
                    let s = self.st.user_snippets.remove(i);
                    self.save_snippets();
                    self.st
                        .set_status(trf!("Snippet «{}» eliminado", s.name), StatusKind::Info);
                }
            }
        }
    }

    fn selected_text(&self) -> String {
        let doc = self.st.docs.cur();
        match self.st.editor.selection {
            Some((a, b)) if a < b => doc.text.chars().skip(a).take(b - a).collect(),
            _ => String::new(),
        }
    }

    /// Alterna un prefijo de línea en el rango seleccionado (ver
    /// `formatbar::toggle_prefix`). Mantiene la selección sobre el bloque.
    fn prefix_lines(&mut self, prefix: &str) {
        let doc = self.st.docs.cur();
        let len = doc.text.chars().count();
        let (a, b) = match (self.st.editor.selection, self.st.editor.cursor) {
            (Some((a, b)), _) => (a.min(len), b.min(len)),
            (None, Some(c)) => (c.min(len), c.min(len)),
            (None, None) => (len, len),
        };
        let id = doc.id;
        let (text, na, nb) = crate::formatbar::toggle_prefix(&doc.text, a, b, prefix);
        let d = self.st.docs.cur_mut();
        d.text = text;
        d.touch();
        self.st.editor.goto = Some(Goto {
            doc: id,
            start: na,
            end: nb,
            focus: true,
        });
        layout::focus_doc(&mut self.dock, id);
        self.st.needs_persist = true;
    }

    fn insert_set_rule(&mut self, rule: &str) {
        let doc = self.st.docs.cur();
        let len = doc.text.chars().count();
        let at = match (self.st.editor.selection, self.st.editor.cursor) {
            (Some((a, b)), _) => a.min(b).min(len),
            (None, Some(c)) => c.min(len),
            (None, None) => len,
        };
        let id = doc.id;
        let (text, cur) = crate::formatbar::insert_set_line(&doc.text, at, rule);
        let d = self.st.docs.cur_mut();
        d.text = text;
        d.touch();
        self.st.editor.goto = Some(Goto {
            doc: id,
            start: cur,
            end: cur,
            focus: true,
        });
        layout::focus_doc(&mut self.dock, id);
        self.st.needs_persist = true;
    }

    fn toggle_mark(&mut self, mark: Mark) {
        let doc = self.st.docs.cur();
        let len = doc.text.chars().count();
        let (a, b) = match (self.st.editor.selection, self.st.editor.cursor) {
            (Some((a, b)), _) => (a.min(len), b.min(len)),
            (None, Some(c)) => (c.min(len), c.min(len)),
            (None, None) => (len, len),
        };
        let id = doc.id;
        let (text, na, nb) = crate::formatbar::toggle_wrap(&doc.text, a, b, mark);
        let d = self.st.docs.cur_mut();
        d.text = text;
        d.touch();
        self.st.editor.goto = Some(Goto {
            doc: id,
            start: na,
            end: nb,
            focus: true,
        });
        layout::focus_doc(&mut self.dock, id);
        self.st.needs_persist = true;
    }

    /// Inserta una plantilla en el cursor (sustituyendo la selección).
    /// `math`: envuelve en `$…$` si no se está ya en una ecuación, y separa con
    /// espacios para no pegar el nombre a identificadores vecinos.
    fn insert(&mut self, template: &str, math: bool) {
        let sel_text = self.selected_text();
        let doc = self.st.docs.cur();
        let len = doc.text.chars().count();
        let (a, b) = match (self.st.editor.selection, self.st.editor.cursor) {
            (Some((a, b)), _) => (a.min(len), b.min(len)),
            (None, Some(c)) => (c.min(len), c.min(len)),
            (None, None) => (len, len),
        };
        let (mut text, mut sa, mut sb) = snippets::expand(template, &sel_text);
        let in_math = symbols::in_math(&doc.text, a);
        // Un snippet que solo son ecuaciones, dentro de otra ecuación: su
        // interior (sin `$` anidados), con el espaciado de los símbolos.
        let mut math = math;
        if !math
            && in_math
            && let Some((t, x, y)) = snippets::strip_equations(&text, sa, sb)
        {
            (text, sa, sb) = (t, x, y);
            math = true;
        }
        if math {
            if !in_math {
                // Envuelve; sin relleno, el cursor queda antes del `$` final
                // para seguir escribiendo dentro de la ecuación.
                text = format!("${text}$");
                sa += 1;
                sb += 1;
            } else {
                let word = |c: Option<char>| {
                    c.is_some_and(|c| c.is_alphanumeric() || c == '.' || c == ')')
                };
                let before = a.checked_sub(1).and_then(|i| doc.text.chars().nth(i));
                let after = doc.text.chars().nth(b);
                if word(before) {
                    text.insert(0, ' ');
                    sa += 1;
                    sb += 1;
                }
                if word(after) {
                    text.push(' ');
                }
            }
        }
        let id = doc.id;
        let d = self.st.docs.cur_mut();
        let (ba, bb) = (
            text::char_to_byte(&d.text, a),
            text::char_to_byte(&d.text, b),
        );
        d.text.replace_range(ba..bb, &text);
        d.touch();
        self.st.editor.goto = Some(Goto {
            doc: id,
            start: a + sa,
            end: a + sb,
            focus: true,
        });
        layout::focus_doc(&mut self.dock, id);
        self.st.needs_persist = true;
    }

    /// Añade `text` al principio del documento actual, desplazando cursor y
    /// selección para que lo que se inserte después caiga donde estaba.
    fn prepend(&mut self, text: &str) {
        let n = text.chars().count();
        let d = self.st.docs.cur_mut();
        d.text.insert_str(0, text);
        d.touch();
        let e = &mut self.st.editor;
        e.cursor = e.cursor.map(|c| c + n);
        e.selection = e.selection.map(|(a, b)| (a + n, b + n));
    }

    /// Reemplaza el `$…$` que el borrador está editando. Si el documento
    /// cambió y la ecuación se movió, se busca la aparición más cercana a su
    /// sitio; si ya no está, no se toca nada y se deja de editar.
    fn replace_text(&mut self, doc: DocId, start: usize, expect: &str, text: String) {
        let found = self
            .st
            .docs
            .get(doc)
            .and_then(|d| text::nearest_occurrence(&d.text, expect, start));
        let Some(start) = found else {
            let msg = if self.st.scratch.editing.take().is_some() {
                tr("La ecuación ya no está en el documento: el borrador deja de editarla")
            } else {
                tr("El texto cambió mientras tanto: no se reemplazó")
            };
            self.st.set_status(msg, StatusKind::Info);
            return;
        };
        let end = start + expect.chars().count();
        if doc != self.st.docs.cur_id() {
            self.switch_to_id(doc);
        }
        let Some(d) = self.st.docs.get_mut(doc) else {
            return;
        };
        let (ba, bb) = (
            text::char_to_byte(&d.text, start),
            text::char_to_byte(&d.text, end),
        );
        d.text.replace_range(ba..bb, &text);
        d.touch();
        let new_end = start + text.chars().count();
        self.st.editor.goto = Some(Goto {
            doc,
            start,
            end: new_end,
            focus: true,
        });
        layout::focus_doc(&mut self.dock, doc);
        // Sigue editando la misma ecuación, ya con su texto nuevo.
        self.st.scratch.editing = Some(scratch::Editing {
            doc,
            start,
            original: text,
        });
        self.st.needs_persist = true;
    }

    fn save_snippets(&mut self) {
        if let Err(e) = snippets::save_user(&self.st.user_snippets) {
            self.st.set_status(
                trf!("No se pudieron guardar los snippets: {}", e),
                StatusKind::Err,
            );
        }
    }

    /// Cierra el diálogo de snippet guardándolo (o no).
    pub fn finish_snippet_edit(&mut self) {
        let Some(Modal::EditSnippet(e)) = self.st.modal.take() else {
            return;
        };
        let s = UserSnippet {
            name: e.name.trim().to_string(),
            body: e.body,
        };
        let name = s.name.clone();
        match e.index {
            Some(i) if i < self.st.user_snippets.len() => self.st.user_snippets[i] = s,
            _ => self.st.user_snippets.push(s),
        }
        self.save_snippets();
        self.st
            .set_status(trf!("Snippet «{}» guardado", name), StatusKind::Ok);
    }

    fn switch_to_id(&mut self, id: DocId) {
        if let Some(i) = self.st.docs.index_of(id) {
            if i != self.st.docs.current {
                self.st.docs.current = i;
                self.on_doc_changed();
            }
            layout::focus_doc(&mut self.dock, id);
        }
    }

    fn on_doc_changed(&mut self) {
        self.st.editor.cursor = None;
        self.st.editor.selection = None;
        self.st.editor.find.current = None;
        self.st.editor.ac.forced = false;
        self.st.needs_persist = true;
    }

    fn jump_to(&mut self, line: usize, col: usize) {
        let doc = self.st.docs.cur();
        let c = text::line_col_to_char(&doc.text, line, col);
        self.st.editor.goto = Some(Goto {
            doc: doc.id,
            start: c,
            end: c,
            focus: true,
        });
        layout::focus_doc(&mut self.dock, doc.id);
    }

    /// Abre `p` (o enfoca su pestaña). Devuelve si quedó como doc actual.
    fn open_path(&mut self, p: PathBuf) -> bool {
        let p = std::fs::canonicalize(&p).unwrap_or(p);
        if crate::image_view::is_viewable(&p) {
            let tab = Tab::Image(self.st.images.open(p));
            if self.dock.find_tab(&tab).is_none() {
                layout::add_tab(&mut self.dock, tab, self.st.docs.cur_id());
            }
            layout::focus_tab(&mut self.dock, tab);
            return false;
        }
        if let Some(i) = self.st.docs.by_path(&p) {
            let id = self.st.docs.list[i].id;
            self.switch_to_id(id);
            return true;
        }
        if std::fs::metadata(&p).is_ok_and(|m| m.len() > MAX_OPEN_BYTES) {
            self.st.set_status(
                trf!("{} es demasiado grande (> 5 MB)", p.display()),
                StatusKind::Err,
            );
            return false;
        }
        let text = match std::fs::read_to_string(&p) {
            Ok(t) => t,
            Err(e) => {
                self.st.set_status(
                    trf!("No se pudo abrir {}: {}", p.display(), e),
                    StatusKind::Err,
                );
                return false;
            }
        };
        let cur = self.st.docs.cur();
        let cur_id = cur.id;
        // Reutiliza una pestaña «sin título» vacía.
        if cur.path.is_none() && cur.text.is_empty() {
            let id = cur.id;
            let d = self.st.docs.cur_mut();
            d.path = Some(p.clone());
            d.saved = text.clone();
            d.text = text;
            d.touch();
            self.st.compiler.forget(id);
            self.on_doc_changed();
        } else {
            let id = self.st.docs.push(Some(p.clone()), text);
            layout::add_doc(&mut self.dock, id, cur_id);
            self.switch_to_id(id);
        }
        self.st
            .set_status(trf!("Abierto {}", p.display()), StatusKind::Info);
        self.st.needs_persist = true;
        true
    }

    fn save(&mut self, id: DocId, save_as: bool) -> bool {
        let Some(doc) = self.st.docs.get(id) else {
            return false;
        };
        let dest = match (&doc.path, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => {
                let mut dlg = rfd::FileDialog::new()
                    .add_filter("Typst", &["typ"])
                    .set_file_name(doc.fname());
                if let Some(dir) = doc.path.as_deref().and_then(Path::parent) {
                    dlg = dlg.set_directory(dir);
                }
                dlg.save_file()
            }
        };
        let Some(dest) = dest else { return false };
        match self.write_doc(id, &dest) {
            Ok(()) => {
                self.st
                    .set_status(trf!("Guardado {}", dest.display()), StatusKind::Ok);
                true
            }
            Err(e) => {
                self.st
                    .set_status(trf!("No se pudo guardar: {}", e), StatusKind::Err);
                false
            }
        }
    }

    /// Escribe el doc en `dest` y lo marca guardado (sin mensaje).
    fn write_doc(&mut self, id: DocId, dest: &Path) -> std::io::Result<()> {
        let text = self.st.docs.get(id).expect("doc").text.clone();
        std::fs::write(dest, &text)?;
        let d = self.st.docs.get_mut(id).expect("doc");
        let moved = d.path.as_deref() != Some(dest);
        d.saved = text;
        d.path = Some(dest.to_path_buf());
        if moved {
            // Cambió la carpeta raíz: las rutas relativas se resuelven distinto.
            d.touch();
        }
        // Un incluido guardado cambia lo que compila el principal.
        if id != self.st.main_doc().id {
            self.st.disk_gen += 1;
        }
        self.st.needs_persist = true;
        Ok(())
    }

    /// Autoguardado: los docs con ruta y cambios se escriben tras
    /// `AUTOSAVE_DELAY` sin teclear. Un fallo se avisa una vez por versión.
    fn autosave(&mut self, ctx: &egui::Context) {
        if !self.st.prefs.auto_save {
            return;
        }
        let mut due = Vec::new();
        for d in &self.st.docs.list {
            if d.path.is_none() || self.autosave_seen.get(&d.id) == Some(&d.rev) {
                continue;
            }
            let elapsed = d.last_edit.elapsed();
            if elapsed < AUTOSAVE_DELAY {
                ctx.request_repaint_after(AUTOSAVE_DELAY - elapsed);
                continue;
            }
            due.push((d.id, d.rev, d.path.clone().expect("ruta")));
        }
        for (id, rev, path) in due {
            self.autosave_seen.insert(id, rev);
            if !self.st.docs.get(id).is_some_and(Doc::modified) {
                continue;
            }
            if let Err(e) = self.write_doc(id, &path) {
                self.st.set_status(
                    trf!("No se pudo autoguardar {}: {}", path.display(), e),
                    StatusKind::Err,
                );
            }
        }
    }

    fn discard(&mut self, id: DocId) {
        if self.st.docs.list.len() == 1 {
            // Último documento: su pestaña pasa a ser un «sin título» vacío, en
            // el mismo sitio del dock.
            let new = self.st.docs.push(None, String::new());
            for (_, t) in self.dock.iter_all_tabs_mut() {
                if *t == Tab::Doc(id) {
                    *t = Tab::Doc(new);
                }
            }
        } else if let Some(path) = self.dock.find_tab(&Tab::Doc(id)) {
            self.dock.remove_tab(path);
        }
        self.st.docs.remove(id);
        self.st.compiler.forget(id);
        if self.st.pinned == Some(id) {
            self.st.pinned = None;
        }
        let cur = self.st.docs.cur_id();
        layout::focus_doc(&mut self.dock, cur);
        self.on_doc_changed();
    }

    /// Exporta el documento principal (el fijado si lo hay: lo que se ve).
    /// `fmt`: `pdf`, `pdfa`, `png` o `svg`. La compilación va en un hilo; el
    /// resultado llega por `export_rx` (ver `compile_tick`).
    fn export_with(&mut self, ctx: &egui::Context, fmt: &str, ppi: Option<u32>, pages: &[usize]) {
        if self.st.export_rx.is_some() {
            self.st
                .set_status(tr("Ya hay una exportación en curso"), StatusKind::Info);
            return;
        }
        let ext = if fmt == "pdfa" { "pdf" } else { fmt };
        let doc = self.st.main_doc();
        let (doc_path, doc_id) = (doc.path.clone(), doc.id);
        let stem = doc_path
            .as_deref()
            .and_then(Path::file_stem)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "documento".into());
        let n_pages = if pages.is_empty() {
            self.st.preview.page_count()
        } else {
            pages.len()
        };
        let multi = ext != "pdf" && n_pages > 1;
        let name = match (multi, fmt) {
            (true, _) => format!("{stem}-{{p}}.{ext}"),
            (_, "pdfa") => format!("{stem}-pdfa.{ext}"),
            _ => format!("{stem}.{ext}"),
        };
        let mut dlg = rfd::FileDialog::new()
            .add_filter(ext.to_uppercase(), &[ext])
            .set_file_name(name);
        // Última carpeta de exportación; si no, la del documento.
        let dir = self.st.last_export_dir.clone().or_else(|| {
            doc_path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf)
        });
        if let Some(dir) = dir {
            dlg = dlg.set_directory(dir);
        }
        let Some(mut dest) = dlg.save_file() else {
            return;
        };
        self.st.last_export_dir = dest.parent().map(Path::to_path_buf);
        // Varias páginas en PNG/SVG: typst exige una plantilla `{p}` en el nombre.
        if multi && !dest.to_string_lossy().contains("{p}") {
            let s = dest
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            dest.set_file_name(format!("{s}-{{p}}.{ext}"));
        }

        // PDF al día: se copia el de la vista previa, sin recompilar.
        let key = self.st.main_key();
        if fmt == "pdf"
            && pages.is_empty()
            && let Some((k, data)) = self
                .st
                .compiler
                .built
                .get(&doc_id)
                .and_then(|b| b.pdf.clone())
            && k == key
        {
            match std::fs::write(&dest, data.as_slice()) {
                Ok(()) => self
                    .st
                    .set_status(trf!("Exportado {}", dest.display()), StatusKind::Ok),
                Err(e) => self
                    .st
                    .set_status(trf!("No se pudo exportar: {}", e), StatusKind::Err),
            }
            return;
        }
        // PDF/A, PDF desactualizado, PNG o SVG: se recompila en un hilo.
        let root = self.st.main_root();
        let filename = crate::engine::EngineWorld::filename_for_job(doc_path.as_deref(), &root);
        let (text, fmt, pages) = (
            self.st.main_doc().text.clone(),
            fmt.to_string(),
            pages.to_vec(),
        );
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let shown = dest.clone();
        let spawned = std::thread::Builder::new()
            .name("export".into())
            .spawn(move || {
                let r =
                    crate::engine::export_pages(&text, &root, &filename, &dest, &fmt, ppi, &pages);
                let _ = tx.send(r.map(|()| shown));
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => {
                self.st.export_rx = Some(rx);
                self.st.set_status(tr("Exportando…"), StatusKind::Info);
            }
            Err(e) => self
                .st
                .set_status(trf!("Falló la exportación: {}", e), StatusKind::Err),
        }
    }

    // ---------- Programas de plugins ----------

    fn import_dialog(&mut self) {
        use crate::plugins::run::Kind;
        let mut dlg = rfd::FileDialog::new();
        let mut any = false;
        for (_, t) in self.st.plugins.tools(Kind::Import) {
            dlg = dlg.add_filter(&t.name, &t.extensions);
            any = true;
        }
        if !any {
            self.st.set_status(
                tr("Ningún plugin activo sabe importar: añade el de Pandoc en el panel Plugins"),
                StatusKind::Err,
            );
            self.st.actions.push(Action::ShowPanel(Panel::Plugins));
            return;
        }
        if let Some(dir) = self.st.docs.cur().path.as_deref().and_then(Path::parent) {
            dlg = dlg.set_directory(dir);
        }
        if let Some(p) = dlg.pick_file() {
            self.st.actions.push(Action::ImportPath(p));
        }
    }

    fn import_path(&mut self, ctx: &egui::Context, path: PathBuf) {
        use crate::plugins::run::Kind;
        let plugins = &self.st.plugins;
        let Some((plugin, tool)) = plugins.importer(&path) else {
            self.st.set_status(
                trf!("Ningún plugin activo importa {}", path.display()),
                StatusKind::Err,
            );
            return;
        };
        // Con `{input}` lo lee el programa (puede ser binario).
        let text = if tool.reads_file() {
            Ok(String::new())
        } else {
            std::fs::read_to_string(&path)
        };
        let label = trf!("Importar {}", tool.name);
        match text {
            Ok(text) => {
                let job = plugins.job(plugin.id(), Kind::Import, &tool.id, text, Some(path), None);
                self.start_job(ctx, job, Then::NewDoc, label);
            }
            Err(e) => self.st.set_status(
                trf!("No se pudo leer {}: {}", path.display(), e),
                StatusKind::Err,
            ),
        }
    }

    /// Exportador o comando de un plugin sobre el documento actual.
    fn plugin_run(&mut self, ctx: &egui::Context, plugin: &str, kind: crate::plugins::run::Kind, id: &str) {
        use crate::plugins::run::{Input, Kind, Output};
        let Some((_, tool)) = self
            .st
            .plugins
            .tools(kind)
            .find(|(p, t)| p.id() == plugin && t.id == id)
        else {
            return;
        };
        let tool = tool.clone();
        let doc = self.st.docs.cur();
        let (doc_id, doc_path) = (doc.id, doc.path.clone());
        if kind == Kind::Export {
            let ext = tool.extensions.first().cloned().unwrap_or_default();
            let stem = doc_path
                .as_deref()
                .and_then(Path::file_stem)
                .map_or("documento".into(), |s| s.to_string_lossy().into_owned());
            let mut dlg = rfd::FileDialog::new()
                .add_filter(&tool.name, &[&ext])
                .set_file_name(format!("{stem}.{ext}"));
            let dir = self
                .st
                .last_export_dir
                .clone()
                .or_else(|| doc_path.as_deref().and_then(Path::parent).map(Path::to_path_buf));
            if let Some(dir) = dir {
                dlg = dlg.set_directory(dir);
            }
            let Some(out) = dlg.save_file() else {
                return;
            };
            self.st.last_export_dir = out.parent().map(Path::to_path_buf);
            let then = if tool.writes_file() {
                Then::Wrote(out.clone())
            } else {
                Then::Write(out.clone())
            };
            let text = self.st.docs.cur().text.clone();
            let job = self.st.plugins.job(plugin, kind, id, text, doc_path, Some(out));
            self.start_job(ctx, job, then, trf!("Exportar {}", tool.name));
            return;
        }
        let selected = self.selected_text();
        let whole = tool.input == Input::Document || selected.is_empty();
        let (start, text) = match self.st.editor.selection {
            Some((a, _)) if !whole => (a, selected),
            _ => (0, self.st.docs.cur().text.clone()),
        };
        let then = match tool.output {
            Output::Replace => Then::Replace {
                doc: doc_id,
                start,
                expect: text.clone(),
            },
            Output::New => Then::NewDoc,
        };
        let job = self.st.plugins.job(plugin, kind, id, text, doc_path, None);
        self.start_job(ctx, job, then, tool.name.clone());
    }

    /// Lanza el programa en un hilo; `plugin_tick` recoge el resultado.
    fn start_job(
        &mut self,
        ctx: &egui::Context,
        job: Option<crate::plugins::run::Job>,
        then: Then,
        label: String,
    ) {
        let Some(job) = job else {
            return;
        };
        if self.st.plugin_job.is_some() {
            self.st
                .set_status(tr("Hay otro plugin trabajando: espera a que termine"), StatusKind::Err);
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("plugin".into())
            .spawn(move || {
                let _ = tx.send(job.run());
                ctx.request_repaint();
            })
            .expect("hilo plugin");
        self.st.set_status(format!("{}…", label), StatusKind::Info);
        self.st.plugin_job = Some(PluginJob { rx, then, label });
    }

    /// Recoge el resultado del programa de plugin, si ya terminó.
    fn plugin_tick(&mut self) {
        use std::sync::mpsc::TryRecvError;
        let st = &mut self.st;
        let Some(job) = &st.plugin_job else {
            return;
        };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(tr("el programa se interrumpió").to_string()),
        };
        let PluginJob { then, label, .. } = st.plugin_job.take().expect("trabajo");
        let done = match result {
            Ok(d) => d,
            Err(e) => {
                st.set_status(format!("{label}: {e}"), StatusKind::Err);
                return;
            }
        };
        let mut status = done.message.unwrap_or_else(|| trf!("{}: hecho", label));
        match then {
            Then::NewDoc => st.actions.push(Action::NewDocWith(done.text)),
            Then::Replace { doc, start, expect } => st.actions.push(Action::ReplaceText {
                doc,
                start,
                expect,
                text: done.text,
            }),
            Then::Wrote(path) => status = trf!("Exportado {}", path.display()),
            Then::Write(path) => match std::fs::write(&path, done.text) {
                Ok(()) => status = trf!("Exportado {}", path.display()),
                Err(e) => {
                    st.set_status(
                        trf!("No se pudo escribir {}: {}", path.display(), e),
                        StatusKind::Err,
                    );
                    return;
                }
            },
        }
        st.set_status(status, StatusKind::Info);
    }

    /// Pone el tema `plugin/nombre` (o vuelve a los de serie con `None`).
    fn set_theme(&mut self, ctx: &egui::Context, key: Option<String>) {
        let pal = key.as_deref().and_then(|k| self.st.plugins.theme(k));
        self.st.prefs.theme = key.filter(|_| pal.is_some());
        if let Some(p) = pal {
            self.st.prefs.dark = p.dark;
        }
        theme::set_custom(pal);
        theme::apply(ctx, self.st.prefs.dark);
        self.dock_style = None;
        self.st.needs_persist = true;
    }

    /// Tras activar, desactivar o recargar plugins: sus paquetes y fuentes
    /// ya están en el motor; falta recompilar y soltar el tema que ya no esté.
    fn plugins_changed(&mut self, ctx: &egui::Context) {
        self.st.fmt.reload_fonts(ctx);
        self.st.force_compile = true;
        if let Some(key) = self.st.prefs.theme.clone() {
            self.set_theme(ctx, Some(key));
        }
    }

    fn dock_style(&mut self, ctx: &egui::Context) -> egui_dock::Style {
        let dark = self.st.prefs.dark;
        match &self.dock_style {
            Some((d, s)) if *d == dark => s.clone(),
            _ => {
                let s = theme::dock_style(&ctx.global_style(), &theme::palette_for(dark));
                self.dock_style = Some((dark, s.clone()));
                s
            }
        }
    }
}

/// `dir/doc.typ` → `dir/doc.pdf`.
fn pdf_beside(src: &Path) -> PathBuf {
    src.with_extension("pdf")
}

/// Escritura atómica: quien tenga el PDF abierto no ve nunca uno a medias.
fn write_atomic(dest: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = dest.with_extension("pdf.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, dest).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn debounce(len: usize) -> Duration {
    Duration::from_millis(match len {
        0..=50_000 => 350,
        50_001..=200_000 => 700,
        _ => 1200,
    })
}

fn restore_tab(docs: &mut Docs, t: SessionTab) -> Option<DocId> {
    match t.path {
        Some(p) => {
            let disk = std::fs::metadata(&p)
                .ok()
                .filter(|m| m.len() <= MAX_OPEN_BYTES)
                .and_then(|_| std::fs::read_to_string(&p).ok());
            match disk {
                Some(content) => {
                    let id = docs.push(Some(p), content);
                    // Cambios sin guardar de la sesión anterior.
                    if !t.text.is_empty() {
                        let d = docs.get_mut(id).expect("doc");
                        if d.text != t.text {
                            d.text = t.text;
                        }
                    }
                    Some(id)
                }
                // El archivo ya no está: conserva el texto como modificado.
                None if !t.text.is_empty() => {
                    let id = docs.push(Some(p), String::new());
                    docs.get_mut(id).expect("doc").text = t.text;
                    Some(id)
                }
                None => None,
            }
        }
        None if !t.text.is_empty() => {
            let id = docs.push(None, String::new());
            docs.get_mut(id).expect("doc").text = t.text;
            Some(id)
        }
        None => None,
    }
}

impl eframe::App for App {
    fn on_exit(&mut self) {
        self.persist();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let pal = theme::palette_for(self.st.prefs.dark);

        // Paths sent by a later launch: open them and bring the window forward.
        let sent: Vec<Vec<PathBuf>> = self
            .instance
            .inbox
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for paths in sent {
            self.open_paths(paths);
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        self.shortcuts(&ctx);
        if let Some(dir) = self.st.editor.pending_step.take() {
            editor::find_step(&mut self.st, dir);
        }
        self.plugin_tick();
        self.run_actions(&ctx);
        self.autosave(&ctx);
        self.compile_tick(&ctx);

        egui::Panel::top("top-bar")
            .frame(chrome::top_frame(&pal))
            .resizable(false)
            .show(ui, |ui| chrome::top_bar(&mut self.st, &self.dock, ui));
        egui::Panel::bottom("status-bar")
            .frame(chrome::status_frame(&pal))
            .resizable(false)
            .show(ui, |ui| chrome::status_bar(&mut self.st, ui));

        // Presentación: solo el lienzo del PDF a pantalla completa.
        if self.st.preview.presentation {
            let info = self.st.preview_info();
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(pal.chrome))
                .show(ui, |ui| {
                    self.st
                        .preview
                        .present(ui, &mut self.st.prefs, &info, &mut self.st.actions);
                });
        } else {
            let style = self.dock_style(&ctx);
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(pal.chrome))
                .show(ui, |ui| {
                    editor::find_bar(&mut self.st, ui);
                    DockArea::new(&mut self.dock)
                        .style(style)
                        .show_add_buttons(false)
                        .show_leaf_collapse_buttons(false)
                        .show_leaf_close_all_buttons(false)
                        .show_inside(ui, &mut Viewer { st: &mut self.st });
                });
        }

        // Doc actual = pestaña de documento activa en el nodo con foco.
        if let Some((_, Tab::Doc(id))) = self.dock.find_active_focused() {
            let id = *id;
            if id != self.st.docs.cur_id()
                && let Some(i) = self.st.docs.index_of(id)
            {
                self.st.docs.current = i;
                self.on_doc_changed();
            }
        }

        // Barra flotante de formato (tras el dock: usa el ancla del frame).
        crate::formatbar::show(&mut self.st, &ctx);

        if let Some(Modal::EditSnippet(e)) = &mut self.st.modal {
            match snippets::edit_modal(&ctx, e) {
                Some(true) => self.finish_snippet_edit(),
                Some(false) => self.st.modal = None,
                None => {}
            }
        } else {
            chrome::modals(&mut self.st, &ctx);
        }
        self.run_actions(&ctx);

        let doc = self.st.docs.cur();
        let title = format!("{}{} — Typst Editor", doc.fname(), if doc.modified() { " •" } else { "" });
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        let since = self.last_persist.elapsed();
        // Cambios marcados: a los 3 s. El resto (p. ej. arrastrar divisores del
        // dock) no se marca: se guarda cada 2 min y al salir.
        if (self.st.needs_persist && since > Duration::from_secs(3))
            || since > Duration::from_secs(120)
        {
            self.persist();
        }
        if self.st.needs_persist {
            ctx.request_repaint_after(Duration::from_secs(3));
        }
    }
}

/// Dibuja cada pestaña del dock.
struct Viewer<'a> {
    st: &'a mut State,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(("dock-tab", *tab))
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        match *tab {
            Tab::Doc(id) => match self.st.docs.get(id) {
                Some(d) => {
                    let dot = if d.modified() { "  ●" } else { "" };
                    format!("{} {}{dot}", icon::FILE_TEXT, d.fname()).into()
                }
                None => tr("(cerrado)").into(),
            },
            Tab::Panel(Panel::Problems) => {
                let main = self.st.main_doc().id;
                let n = self
                    .st
                    .compiler
                    .built
                    .get(&main)
                    .map(|b| b.diagnostics.len())
                    .unwrap_or(0);
                let p = Panel::Problems;
                if n > 0 {
                    format!("{} {}  {n}", p.icon(), p.title()).into()
                } else {
                    format!("{} {}", p.icon(), p.title()).into()
                }
            }
            Tab::Panel(p) => format!("{} {}", p.icon(), p.title()).into(),
            Tab::Image(id) => match self.st.images.get(id) {
                Some(t) => format!("{} {}", icon::IMAGE, t.name()).into(),
                None => tr("(cerrada)").into(),
            },
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        let st = &mut *self.st;
        match *tab {
            Tab::Doc(id) => editor::doc_tab(st, ui, id),
            Tab::Panel(Panel::Preview) => {
                let info = st.preview_info();
                st.preview.ui(ui, &mut st.prefs, &info, &mut st.actions);
            }
            Tab::Panel(Panel::Outline) => panels::outline(st, ui),
            Tab::Panel(Panel::Problems) => panels::problems(st, ui),
            Tab::Panel(Panel::Symbols) => symbols::panel(st, ui),
            Tab::Panel(Panel::Snippets) => snippets::panel(st, ui),
            Tab::Panel(Panel::Templates) => templates::panel(st, ui),
            Tab::Panel(Panel::Files) => files::panel(st, ui),
            Tab::Panel(Panel::Scratch) => scratch::panel(st, ui),
            Tab::Panel(Panel::Plugins) => crate::plugins::panel(st, ui),
            Tab::Image(id) => crate::image_view::tab(st, ui, id),
        }
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut Tab, _path: egui_dock::NodePath) {
        if let Tab::Doc(id) = *tab {
            let Some(d) = self.st.docs.get(id) else {
                return;
            };
            let path = d.path.clone();
            if ui
                .button(trf!("{}  Guardar", icon::FLOPPY_DISK))
                .clicked()
            {
                self.st
                    .actions
                    .push(Action::SwitchTo(self.st.docs.index_of(id).unwrap_or(0)));
                self.st.actions.push(Action::Save);
                ui.close();
            }
            if let Some(p) = path
                && ui.button(trf!("{}  Copiar ruta", icon::COPY)).clicked()
            {
                ui.ctx().copy_text(p.display().to_string());
                ui.close();
            }
            ui.separator();
            if ui.button(trf!("{}  Cerrar", icon::X)).clicked() {
                self.st.actions.push(Action::Close(id));
                ui.close();
            }
        }
    }

    fn on_close(&mut self, tab: &mut Tab) -> OnCloseResponse {
        match *tab {
            // El cierre de docs pasa por `Action::Close` (confirma si hay cambios).
            Tab::Doc(id) => {
                self.st.actions.push(Action::Close(id));
                OnCloseResponse::Ignore
            }
            Tab::Panel(_) => OnCloseResponse::Close,
            Tab::Image(id) => {
                self.st.images.close(id);
                OnCloseResponse::Close
            }
        }
    }

    fn scroll_bars(&self, _tab: &Tab) -> [bool; 2] {
        [false, false]
    }
}

#[cfg(test)]
mod pdf_junto_tests {
    use super::*;

    #[test]
    fn el_pdf_va_junto_al_fuente() {
        assert_eq!(pdf_beside(Path::new("/a/b/informe.typ")), Path::new("/a/b/informe.pdf"));
        assert_eq!(pdf_beside(Path::new("sin-ext")), Path::new("sin-ext.pdf"));
    }

    #[test]
    fn escribe_y_reemplaza_sin_dejar_temporales() {
        let d = std::env::temp_dir().join(format!("grammata-wa-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let dest = d.join("x.pdf");
        write_atomic(&dest, b"%PDF-1").unwrap();
        write_atomic(&dest, b"%PDF-2").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"%PDF-2");
        assert!(!d.join("x.pdf.tmp").exists());
        assert!(write_atomic(&d.join("no-existe").join("x.pdf"), b"x").is_err());
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod ui_tests;
