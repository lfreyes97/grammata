//! Estado de la app, bucle por frame y ejecución de acciones.

use crate::actions::Action;
use crate::chrome;
use crate::compile::{self, Compiler, Key};
use crate::diagnostics;
use crate::doc::{Doc, DocId, Docs};
use crate::editor::{self, EditorUi, Goto};
use crate::formatbar::FormatBar;
use crate::layout::{self, Panel, Preset, Tab};
use crate::panels::{self, OutlineCache};
use crate::preview::{self, Preview};
use crate::session::{self, Prefs, Session, SessionTab};
use crate::snippets::{self, UserSnippet};
use crate::symbols;
use crate::text;
use crate::theme;
use egui::{Key as K, Modifiers as M};
use egui_dock::{DockArea, DockState, TabViewer, tab_viewer::OnCloseResponse};
use egui_phosphor::regular as icon;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEFAULT_FONT: f32 = 14.0;
const MAX_OPEN_BYTES: u64 = 5_000_000;

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
    GotoLine(String),
    About,
    EditSnippet(snippets::Editing),
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
    pub user_snippets: Vec<UserSnippet>,
    pub fmt: FormatBar,
    force_compile: bool,
}

impl State {
    pub(crate) fn set_status(&mut self, msg: impl Into<String>, kind: StatusKind) {
        self.status = Status {
            msg: msg.into(),
            kind,
            at: Instant::now(),
        };
    }

    fn preview_info(&self) -> preview::Info {
        let doc = self.docs.cur();
        let built = self.compiler.built.get(&doc.id);
        preview::Info {
            compiling: self.compiler.in_flight.is_some_and(|k| k.doc == doc.id),
            auto: self.prefs.auto_compile,
            errors: built
                .map(|b| diagnostics::count(&b.diagnostics).0)
                .unwrap_or(0),
            stale: self.preview.loaded.is_some_and(|k| k.rev != doc.rev),
            has_build: built.is_some(),
            sync: built.map(|b| b.sync.clone()).unwrap_or_default(),
            root: doc.root_dir(),
        }
    }
}

pub struct App {
    st: State,
    dock: DockState<Tab>,
    dock_style: Option<(bool, egui_dock::Style)>,
    last_persist: Instant,
    title: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let ctx = cc.egui_ctx.clone();
        theme::install_fonts(&ctx);
        // Ctrl+= / Ctrl+- / Ctrl+0 son para la fuente del editor, no para el zoom de la UI.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);

        let session = session::load().unwrap_or_default();
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

        let prefs = session.prefs;
        theme::apply(&ctx, prefs.dark);
        let st = State {
            docs,
            prefs,
            compiler: Compiler::new(ctx.clone()),
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
            fmt: FormatBar::preloaded(&ctx),
            force_compile: false,
        };

        Self {
            st,
            dock,
            dock_style: None,
            last_persist: Instant::now(),
            title: String::new(),
        }
    }

    // ---------- Sesión ----------

    fn persist(&mut self) {
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
                format!("No se pudo guardar la sesión: {e}"),
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
            if k(M::SHIFT, K::F3) {
                self.st.editor.pending_step = Some(-1)
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
                a.push(Action::ExportPdf)
            }
            if k(M::NONE, K::F5) {
                a.push(Action::Compile)
            }
            if k(M::NONE, K::F11) {
                let on = !self.st.preview.presentation;
                self.st.preview.set_presentation(ctx, on);
            }
            if k(M::NONE, K::Escape) && self.st.preview.presentation {
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
        for key in st.compiler.poll() {
            if key.doc == st.docs.cur_id()
                && let Some(e) = st.compiler.built.get(&key.doc).and_then(|b| {
                    b.diagnostics
                        .iter()
                        .find(|d| d.severity == diagnostics::Severity::Error)
                })
            {
                let msg = e.message.lines().next().unwrap_or("").to_string();
                st.set_status(format!("{}: {msg}", e.location()), StatusKind::Err);
            }
        }
        // Vista previa ← último PDF válido del doc actual.
        let cur = st.docs.cur_id();
        match st.compiler.built.get(&cur).and_then(|b| b.pdf.clone()) {
            Some((k, data)) if st.preview.loaded != Some(k) => st.preview.load(k, data),
            None if st.preview.loaded.is_some() => st.preview.clear(),
            _ => {}
        }
        st.preview.poll(ctx);

        if st.compiler.in_flight.is_some() {
            return;
        }
        let doc = st.docs.cur();
        let key = Key {
            doc: doc.id,
            rev: doc.rev,
        };
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
            let (text, root) = (doc.text.clone(), doc.root_dir());
            let filename = crate::engine::EngineWorld::filename_for_job(doc.path.as_deref());
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
            }
            Action::ExportPdf => self.export("pdf"),
            Action::ExportPng(ppi) => self.export_with("png", Some(ppi)),
            Action::ExportSvg => self.export("svg"),
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
                let pos = self
                    .st
                    .compiler
                    .built
                    .get(&doc.id)
                    .and_then(|b| crate::engine::page_for_line(&b.sync, None, line));
                match pos {
                    Some((page, y)) => {
                        layout::show_panel(&mut self.dock, Panel::Preview, cur);
                        self.st.preview.goto_position(page, y);
                    }
                    None => self.st.set_status(
                        "Sin posición en el PDF: compila un documento con encabezados",
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
                let tabs = self.st.docs.list.iter().map(|d| Tab::Doc(d.id)).collect();
                self.dock = p.build(tabs);
                layout::focus_doc(&mut self.dock, cur);
                self.st.needs_persist = true;
            }
            Action::ToggleTheme => {
                self.st.prefs.dark = !self.st.prefs.dark;
                theme::apply(ctx, self.st.prefs.dark);
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
            Action::PrefixLines(prefix) => self.prefix_lines(&prefix),
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
                        .set_status(format!("Snippet «{}» eliminado", s.name), StatusKind::Info);
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
        if math {
            if !symbols::in_math(&doc.text, a) {
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

    fn save_snippets(&mut self) {
        if let Err(e) = snippets::save_user(&self.st.user_snippets) {
            self.st.set_status(
                format!("No se pudieron guardar los snippets: {e}"),
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
            .set_status(format!("Snippet «{name}» guardado"), StatusKind::Ok);
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
        if let Some(i) = self.st.docs.by_path(&p) {
            let id = self.st.docs.list[i].id;
            self.switch_to_id(id);
            return true;
        }
        if std::fs::metadata(&p).is_ok_and(|m| m.len() > MAX_OPEN_BYTES) {
            self.st.set_status(
                format!("{} es demasiado grande (> 5 MB)", p.display()),
                StatusKind::Err,
            );
            return false;
        }
        let text = match std::fs::read_to_string(&p) {
            Ok(t) => t,
            Err(e) => {
                self.st.set_status(
                    format!("No se pudo abrir {}: {e}", p.display()),
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
            .set_status(format!("Abierto {}", p.display()), StatusKind::Info);
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
        let text = doc.text.clone();
        match std::fs::write(&dest, &text) {
            Ok(()) => {
                let d = self.st.docs.get_mut(id).expect("doc");
                let moved = d.path.as_ref() != Some(&dest);
                d.saved = text;
                d.path = Some(dest.clone());
                if moved {
                    // Cambió la carpeta raíz: las rutas relativas se resuelven distinto.
                    d.touch();
                }
                self.st
                    .set_status(format!("Guardado {}", dest.display()), StatusKind::Ok);
                self.st.needs_persist = true;
                true
            }
            Err(e) => {
                self.st
                    .set_status(format!("No se pudo guardar: {e}"), StatusKind::Err);
                false
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
        let cur = self.st.docs.cur_id();
        layout::focus_doc(&mut self.dock, cur);
        self.on_doc_changed();
    }

    fn export(&mut self, fmt: &str) {
        self.export_with(fmt, None);
    }

    fn export_with(&mut self, fmt: &str, ppi: Option<u32>) {
        let doc = self.st.docs.cur();
        let stem = doc
            .path
            .as_deref()
            .and_then(Path::file_stem)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "documento".into());
        let multi = fmt != "pdf" && self.st.preview.page_count() > 1;
        let name = if multi {
            format!("{stem}-{{p}}.{fmt}")
        } else {
            format!("{stem}.{fmt}")
        };
        let mut dlg = rfd::FileDialog::new()
            .add_filter(fmt.to_uppercase(), &[fmt])
            .set_file_name(name);
        if let Some(dir) = doc.path.as_deref().and_then(Path::parent) {
            dlg = dlg.set_directory(dir);
        }
        let Some(mut dest) = dlg.save_file() else {
            return;
        };
        // Varias páginas en PNG/SVG: typst exige una plantilla `{p}` en el nombre.
        if multi && !dest.to_string_lossy().contains("{p}") {
            let s = dest
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            dest.set_file_name(format!("{s}-{{p}}.{fmt}"));
        }

        // PDF al día: se copia el de la vista previa, sin recompilar.
        let key = Key {
            doc: doc.id,
            rev: doc.rev,
        };
        if fmt == "pdf"
            && let Some((k, data)) = self
                .st
                .compiler
                .built
                .get(&doc.id)
                .and_then(|b| b.pdf.clone())
            && k == key
        {
            match std::fs::write(&dest, data.as_slice()) {
                Ok(()) => self
                    .st
                    .set_status(format!("Exportado {}", dest.display()), StatusKind::Ok),
                Err(e) => self
                    .st
                    .set_status(format!("No se pudo exportar: {e}"), StatusKind::Err),
            }
            return;
        }
        // PDF desactualizado, PNG o SVG: se recompila en proceso.
        let filename = crate::engine::EngineWorld::filename_for_job(doc.path.as_deref());
        match crate::engine::export_pages(&doc.text, &doc.root_dir(), &filename, &dest, fmt, ppi) {
            Ok(()) => self
                .st
                .set_status(format!("Exportado {}", dest.display()), StatusKind::Ok),
            Err(e) => self
                .st
                .set_status(format!("Falló la exportación: {e}"), StatusKind::Err),
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

        self.shortcuts(&ctx);
        if let Some(dir) = self.st.editor.pending_step.take() {
            editor::find_step(&mut self.st, dir);
        }
        self.run_actions(&ctx);
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
        let title = format!(
            "{}{} — Typst Editor",
            doc.fname(),
            if doc.modified() { " •" } else { "" }
        );
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
                None => "(cerrado)".into(),
            },
            Tab::Panel(Panel::Problems) => {
                let cur = self.st.docs.cur_id();
                let n = self
                    .st
                    .compiler
                    .built
                    .get(&cur)
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
        }
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut Tab, _path: egui_dock::NodePath) {
        if let Tab::Doc(id) = *tab {
            let Some(d) = self.st.docs.get(id) else {
                return;
            };
            let path = d.path.clone();
            if ui
                .button(format!("{}  Guardar", icon::FLOPPY_DISK))
                .clicked()
            {
                self.st
                    .actions
                    .push(Action::SwitchTo(self.st.docs.index_of(id).unwrap_or(0)));
                self.st.actions.push(Action::Save);
                ui.close();
            }
            if let Some(p) = path
                && ui.button(format!("{}  Copiar ruta", icon::COPY)).clicked()
            {
                ui.ctx().copy_text(p.display().to_string());
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{}  Cerrar", icon::X)).clicked() {
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
        }
    }

    fn scroll_bars(&self, _tab: &Tab) -> [bool; 2] {
        [false, false]
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod ui_tests;
