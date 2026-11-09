//! Consultas de `typst-ide` en un hilo aparte (autocompletado semántico).
//!
//! El hilo conserva su propio `EngineWorld` (fuentes, librería y caché de
//! archivos) y atiende solo la última consulta encolada. La UI pregunta
//! con el texto y el cursor y recoge la respuesta en el frame siguiente.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use typst_layout::PagedDocument;

use crate::doc::DocId;
use crate::engine::EngineWorld;

struct Query {
    doc: DocId,
    rev: u64,
    /// Cursor en bytes dentro de `text`.
    cursor: usize,
    explicit: bool,
    text: String,
    root: PathBuf,
    filename: String,
}

#[derive(Clone)]
pub struct Item {
    pub label: String,
    /// Cuerpo con marcadores `${nombre}`; si falta, el propio `label`.
    pub apply: Option<String>,
    pub detail: Option<String>,
}

/// Respuesta a una consulta: sirve mientras doc, rev y cursor coincidan.
#[derive(Clone)]
pub struct Reply {
    pub doc: DocId,
    pub rev: u64,
    pub cursor: usize,
    /// Byte donde empieza el tramo que sustituye el elemento elegido.
    pub from: usize,
    pub items: Vec<Item>,
}

pub struct Ide {
    tx: Sender<Query>,
    rx: Receiver<Reply>,
    pub last: Option<Reply>,
    asked: Option<(DocId, u64, usize, bool)>,
}

impl Ide {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx_q) = channel::<Query>();
        let (tx_r, rx) = channel::<Reply>();
        std::thread::Builder::new()
            .name("typst-ide".into())
            .stack_size(64 << 20)
            .spawn(move || {
                let mut cache: Option<(PathBuf, String, EngineWorld, Instant)> = None;
                while let Ok(mut q) = rx_q.recv() {
                    while let Ok(newer) = rx_q.try_recv() {
                        q = newer;
                    }
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        answer(&mut cache, &q)
                    }));
                    if let Ok(reply) = r
                        && tx_r.send(reply).is_err()
                    {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("hilo typst-ide");
        Self {
            tx,
            rx,
            last: None,
            asked: None,
        }
    }

    /// Pide completar en `cursor` (byte). No repite una consulta idéntica.
    #[allow(clippy::too_many_arguments)]
    pub fn request(
        &mut self,
        doc: DocId,
        rev: u64,
        cursor: usize,
        explicit: bool,
        text: &str,
        root: PathBuf,
        filename: String,
    ) {
        let key = (doc, rev, cursor, explicit);
        if self.asked == Some(key) {
            return;
        }
        self.asked = Some(key);
        let _ = self.tx.send(Query {
            doc,
            rev,
            cursor,
            explicit,
            text: text.to_string(),
            root,
            filename,
        });
    }

    /// Recoge la respuesta más reciente. `true` si cambió algo.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(r) = self.rx.try_recv() {
            self.last = Some(r);
            changed = true;
        }
        changed
    }
}

fn answer(cache: &mut Option<(PathBuf, String, EngineWorld, Instant)>, q: &Query) -> Reply {
    let stale = cache
        .as_ref()
        .is_none_or(|(r, f, _, _)| *r != q.root || *f != q.filename);
    if stale {
        let mut w = EngineWorld::new(q.root.clone(), &q.filename, q.text.clone());
        w.refresh_known_files();
        *cache = Some((q.root.clone(), q.filename.clone(), w, Instant::now()));
    }
    let (_, _, world, scanned) = cache.as_mut().expect("mundo del IDE");
    world.set_main(q.text.clone());
    if scanned.elapsed() > Duration::from_secs(5) {
        world.refresh_known_files();
        *scanned = Instant::now();
    }
    let source = world.main_source();
    let cursor = q.cursor.min(source.text().len());
    let (from, items) =
        typst_ide::autocomplete(&*world, None::<&PagedDocument>, &source, cursor, q.explicit)
            .unwrap_or((cursor, Vec::new()));
    let items = items
        .into_iter()
        .map(|c| Item {
            label: c.label.to_string(),
            apply: c.apply.map(|a| a.to_string()),
            detail: c.detail.map(|d| d.to_string()),
        })
        .collect();
    Reply {
        doc: q.doc,
        rev: q.rev,
        cursor: q.cursor,
        from,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `|` marca el cursor; sin marca, el final del texto.
    fn ask(dir: &std::path::Path, text: &str, explicit: bool) -> Reply {
        let cursor = text.find('|').unwrap_or(text.len());
        let text = text.replacen('|', "", 1);
        let text = text.as_str();
        let mut cache = None;
        let q = Query {
            doc: DocId::default(),
            rev: 0,
            cursor,
            explicit,
            text: text.to_string(),
            root: dir.to_path_buf(),
            filename: "main.typ".to_string(),
        };
        answer(&mut cache, &q)
    }

    #[test]
    fn completa_funciones_y_variables() {
        let dir = std::env::temp_dir();
        let r = ask(&dir, "#let numero = 3\n#ali", false);
        assert!(r.items.iter().any(|i| i.label == "align"));
        assert_eq!(&"#let numero = 3\n#ali"[r.from..], "ali");
        let r = ask(&dir, "#let numero = 3\n#num", false);
        assert!(r.items.iter().any(|i| i.label == "numero"));
    }

    #[test]
    fn completa_parametros_y_rutas() {
        let dir = std::env::temp_dir().join(format!("grammata-ide-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("foto.png"), b"x").unwrap();
        let r = ask(&dir, "#table(col", false);
        assert!(r.items.iter().any(|i| i.label == "columns"));
        let r = ask(&dir, "#image(\"fo|\")", false);
        assert!(r.items.iter().any(|i| i.label.contains("foto.png")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
