//! Compilación Typst embebida en un hilo aparte (sin CLI).
//!
//! El buffer vivo del editor se compila en proceso con el mismo `root` que
//! antes (carpeta del documento para rutas relativas). El hilo usa stack
//! generoso y `catch_unwind`: un fallo del compilador no tumba la app.

use crate::diagnostics::Diagnostic;
use crate::doc::DocId;
use crate::engine::SyncEntry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

/// Identifica una versión concreta de un documento.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key {
    pub doc: DocId,
    pub rev: u64,
}

struct Job {
    key: Key,
    text: String,
    root: PathBuf,
    filename: String,
}

pub struct Output {
    pub key: Key,
    pub pdf: Option<Arc<Vec<u8>>>,
    pub diagnostics: Vec<Diagnostic>,
    pub ms: u128,
    /// Entradas synctex de esta compilación (vacío si falló).
    pub sync: Vec<SyncEntry>,
}

/// Último resultado por documento (para cambiar de pestaña sin recompilar).
pub struct Built {
    pub key: Key,
    /// Último PDF válido (puede ser de una rev anterior si la última falló).
    pub pdf: Option<(Key, Arc<Vec<u8>>)>,
    pub diagnostics: Vec<Diagnostic>,
    pub ms: u128,
    /// Synctex del último PDF válido (como `pdf`: de la rev anterior si falla).
    pub sync: Vec<SyncEntry>,
}

pub struct Compiler {
    tx: Sender<Job>,
    rx: Receiver<Output>,
    pub in_flight: Option<Key>,
    pub built: HashMap<DocId, Built>,
}

impl Compiler {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx_job) = channel::<Job>();
        let (tx_out, rx) = channel::<Output>();
        std::thread::Builder::new()
            .name("typst-compile".into())
            // El compilador evalúa recursivamente: stack generoso como el CLI.
            .stack_size(64 << 20)
            .spawn(move || {
                while let Ok(mut job) = rx_job.recv() {
                    // Si se encolaron varias, solo cuenta la última.
                    while let Ok(newer) = rx_job.try_recv() {
                        job = newer;
                    }
                    let out = run(&job);
                    if tx_out.send(out).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("hilo typst-compile");
        Self {
            tx,
            rx,
            in_flight: None,
            built: HashMap::new(),
        }
    }

    pub fn is_built(&self, key: Key) -> bool {
        self.built.get(&key.doc).is_some_and(|b| b.key == key)
    }

    pub fn request(&mut self, key: Key, text: String, root: PathBuf, filename: String) {
        if self.in_flight == Some(key) {
            return;
        }
        if self
            .tx
            .send(Job {
                key,
                text,
                root,
                filename,
            })
            .is_ok()
        {
            self.in_flight = Some(key);
        }
    }

    /// Recoge resultados (no bloquea). Devuelve los que llegaron.
    pub fn poll(&mut self) -> Vec<Key> {
        let mut got = Vec::new();
        while let Ok(out) = self.rx.try_recv() {
            if self.in_flight == Some(out.key) {
                self.in_flight = None;
            }
            let prev = self.built.remove(&out.key.doc);
            let prev_pdf = prev.as_ref().and_then(|b| b.pdf.clone());
            let prev_sync = prev.map(|b| b.sync).unwrap_or_default();
            let has_pdf = out.pdf.is_some();
            let pdf = match out.pdf {
                Some(p) => Some((out.key, p)),
                None => prev_pdf,
            };
            let sync = if has_pdf { out.sync } else { prev_sync };
            self.built.insert(
                out.key.doc,
                Built {
                    key: out.key,
                    pdf,
                    diagnostics: out.diagnostics,
                    ms: out.ms,
                    sync,
                },
            );
            got.push(out.key);
        }
        got
    }

    pub fn forget(&mut self, doc: DocId) {
        self.built.remove(&doc);
    }
}

/// Compila el buffer en proceso (sin CLI). Un panic del compilador no puede
/// tumbar la app: se convierte en diagnóstico.
fn run(job: &Job) -> Output {
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::engine::compile_pdf(&job.text, &job.root, &job.filename)
    }));
    match out {
        Ok(c) => {
            let pdf = c.pdf.map(Arc::new);
            Output {
                key: job.key,
                pdf,
                diagnostics: c.diags,
                ms: c.ms,
                sync: c.sync,
            }
        }
        Err(_) => {
            let diagnostics = vec![crate::diagnostics::Diagnostic {
                severity: crate::diagnostics::Severity::Error,
                file: None,
                line: None,
                col: None,
                message: "error interno del compilador (panic)".to_string(),
                hints: Vec::new(),
            }];
            Output {
                key: job.key,
                pdf: None,
                diagnostics,
                ms: 0,
                sync: Vec::new(),
            }
        }
    }
}

/// Versión del compilador embebido (fijada en Cargo.toml, sin deriva CLI).
pub fn typst_version() -> Option<String> {
    Some("typst 0.15.1 (embebido)".to_string())
}

/// Familias de fuentes del motor embebido (antes: shell a `typst fonts`).
pub fn typst_fonts() -> Vec<String> {
    crate::engine::font_families()
}
