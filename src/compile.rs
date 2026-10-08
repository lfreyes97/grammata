//! Compilación con la CLI de typst en un hilo aparte.
//!
//! El texto va por stdin y el PDF sale por stdout: sin temporales, y con
//! `--root` = carpeta del documento para que las rutas relativas funcionen.

use crate::diagnostics::{self, Diagnostic};
use crate::doc::DocId;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

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
}

pub struct Output {
    pub key: Key,
    pub pdf: Option<Arc<Vec<u8>>>,
    pub diagnostics: Vec<Diagnostic>,
    pub ms: u128,
}

/// Último resultado por documento (para cambiar de pestaña sin recompilar).
pub struct Built {
    pub key: Key,
    /// Último PDF válido (puede ser de una rev anterior si la última falló).
    pub pdf: Option<(Key, Arc<Vec<u8>>)>,
    pub diagnostics: Vec<Diagnostic>,
    pub ms: u128,
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
        Self { tx, rx, in_flight: None, built: HashMap::new() }
    }

    pub fn is_built(&self, key: Key) -> bool {
        self.built.get(&key.doc).is_some_and(|b| b.key == key)
    }

    pub fn request(&mut self, key: Key, text: String, root: PathBuf) {
        if self.in_flight == Some(key) {
            return;
        }
        if self.tx.send(Job { key, text, root }).is_ok() {
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
            let prev_pdf = self.built.remove(&out.key.doc).and_then(|b| b.pdf);
            let pdf = match out.pdf {
                Some(p) => Some((out.key, p)),
                None => prev_pdf,
            };
            self.built.insert(
                out.key.doc,
                Built { key: out.key, pdf, diagnostics: out.diagnostics, ms: out.ms },
            );
            got.push(out.key);
        }
        got
    }

    pub fn forget(&mut self, doc: DocId) {
        self.built.remove(&doc);
    }
}

fn run(job: &Job) -> Output {
    let t0 = Instant::now();
    let res = typst(&job.root, &job.text, &["-"], &[]);
    let (pdf, diagnostics) = match res {
        Ok((stdout, stderr, ok)) => {
            let diags = diagnostics::parse(&stderr);
            let pdf = (ok && stdout.starts_with(b"%PDF")).then(|| Arc::new(stdout));
            (pdf, diags)
        }
        Err(e) => (None, diagnostics::parse(&format!("error: {e}"))),
    };
    Output { key: job.key, pdf, diagnostics, ms: t0.elapsed().as_millis() }
}

/// Ejecuta `typst compile - <out>` con `text` por stdin.
/// Devuelve (stdout, stderr, éxito).
pub fn typst(
    root: &Path,
    text: &str,
    output: &[&str],
    extra: &[&str],
) -> Result<(Vec<u8>, String, bool), String> {
    let mut child = Command::new("typst")
        .arg("compile")
        .args(["--diagnostic-format", "short", "--root"])
        .arg(root)
        .args(extra)
        .arg("-")
        .args(output)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("no se pudo ejecutar `typst` (¿está en el PATH?): {e}"))?;
    // stdin en otro hilo: si typst llenara stdout antes de leer todo, no hay bloqueo.
    let mut stdin = child.stdin.take().expect("stdin piped");
    let text = text.to_owned();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(text.as_bytes());
    });
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = writer.join();
    Ok((out.stdout, String::from_utf8_lossy(&out.stderr).into_owned(), out.status.success()))
}

pub fn typst_version() -> Option<String> {
    let o = Command::new("typst").arg("--version").output().ok()?;
    let s = String::from_utf8_lossy(&o.stdout);
    // "typst 0.15.1 (9dfd3a08)" → "typst 0.15.1"
    Some(s.split(" (").next().unwrap_or(&s).trim().to_string())
}

/// Familias de fuentes de `typst fonts` (una por línea), ordenadas y sin
/// duplicados. Tarda ~0.2 s: llamar una vez y cachear (ver `FormatBar`).
pub fn typst_fonts() -> Vec<String> {
    let out = Command::new("typst").arg("fonts").output();
    let Ok(out) = out else { return Vec::new() };
    parse_fonts(&String::from_utf8_lossy(&out.stdout))
}

fn parse_fonts(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_fuentes() {
        let v = parse_fonts("DejaVu Sans\nDejaVu Sans\n  \nLibertinus Serif\n");
        assert_eq!(v, vec!["DejaVu Sans".to_string(), "Libertinus Serif".to_string()]);
    }
}
