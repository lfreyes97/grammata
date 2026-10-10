//! Documentos abiertos y su colección.

use std::path::{Path, PathBuf};
use std::time::Instant;

pub type DocId = u64;

pub struct Doc {
    pub id: DocId,
    pub path: Option<PathBuf>,
    pub text: String,
    /// Contenido en disco (o al crear); `text != saved` ⇒ modificado.
    pub saved: String,
    /// Se incrementa en cada edición; clave de cachés y de compilación.
    pub rev: u64,
    pub last_edit: Instant,
}

impl Doc {
    pub fn fname(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "sin título.typ".into())
    }

    pub fn modified(&self) -> bool {
        self.text != self.saved
    }

    /// Directorio raíz para compilar: las rutas relativas (`#image`, `#include`)
    /// se resuelven desde aquí.
    pub fn root_dir(&self) -> PathBuf {
        self.path
            .as_deref()
            .and_then(Path::parent)
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(std::env::temp_dir)
    }

    /// Raíz efectiva de compilación: la carpeta del gestor si contiene al
    /// documento (proyectos como Instatypst, con `#import "../…"` por encima
    /// de la carpeta del fichero), si no la del propio documento.
    pub fn compile_root(&self, files_root: Option<&Path>) -> PathBuf {
        let own = self.root_dir();
        match (self.path.as_deref(), files_root) {
            (Some(p), Some(r)) if r.is_dir() && p.strip_prefix(r).is_ok() => r.to_path_buf(),
            _ => own,
        }
    }

    pub fn touch(&mut self) {
        self.rev = self.rev.wrapping_add(1);
        self.last_edit = Instant::now();
    }
}

/// Colección de documentos. Siempre hay al menos uno.
pub struct Docs {
    pub list: Vec<Doc>,
    pub current: usize,
    next_id: DocId,
}

impl Docs {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            current: 0,
            next_id: 0,
        }
    }

    pub fn push(&mut self, path: Option<PathBuf>, text: String) -> DocId {
        let id = self.next_id;
        self.next_id += 1;
        self.list.push(Doc {
            id,
            path,
            saved: text.clone(),
            text,
            rev: 1,
            last_edit: Instant::now(),
        });
        id
    }

    pub fn cur(&self) -> &Doc {
        &self.list[self.current]
    }

    pub fn cur_mut(&mut self) -> &mut Doc {
        &mut self.list[self.current]
    }

    pub fn cur_id(&self) -> DocId {
        self.cur().id
    }

    pub fn index_of(&self, id: DocId) -> Option<usize> {
        self.list.iter().position(|d| d.id == id)
    }

    pub fn get(&self, id: DocId) -> Option<&Doc> {
        self.list.iter().find(|d| d.id == id)
    }

    pub fn get_mut(&mut self, id: DocId) -> Option<&mut Doc> {
        self.list.iter_mut().find(|d| d.id == id)
    }

    pub fn by_path(&self, p: &Path) -> Option<usize> {
        self.list.iter().position(|d| d.path.as_deref() == Some(p))
    }

    /// Quita el doc `id` (nunca el último: debe quedar al menos uno).
    pub fn remove(&mut self, id: DocId) {
        let Some(idx) = self.index_of(id) else { return };
        if self.list.len() <= 1 {
            return;
        }
        self.list.remove(idx);
        if idx < self.current {
            self.current -= 1;
        }
        self.current = self.current.min(self.list.len() - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raiz_efectiva() {
        let dir = std::env::temp_dir().join(format!("typst-editor-root-{}", std::process::id()));
        let sub = dir.join("campana");
        std::fs::create_dir_all(&sub).unwrap();
        let f = sub.join("a.typ");
        std::fs::write(&f, "= A\n").unwrap();
        let mut docs = Docs::new();
        let id = docs.push(Some(f), "= A\n".to_string());
        let doc = docs.get(id).expect("doc");
        // Sin gestor: carpeta del documento.
        assert_eq!(doc.compile_root(None), sub);
        // Gestor que no contiene al documento: carpeta del documento.
        let fuera = std::env::temp_dir().join(format!("typst-editor-fuera-{}", std::process::id()));
        std::fs::create_dir_all(&fuera).unwrap();
        assert_eq!(doc.compile_root(Some(fuera.as_path())), sub);
        // Gestor que contiene al documento: el gestor manda.
        assert_eq!(doc.compile_root(Some(dir.as_path())), dir);
        // Raíz inexistente: se ignora.
        assert_eq!(doc.compile_root(Some(dir.join("nope").as_path())), sub);
        // Sin archivo: temporal, sin gestor posible.
        let mut bare_docs = Docs::new();
        let bid = bare_docs.push(None, "x".to_string());
        let bare = bare_docs.get(bid).expect("doc");
        assert_eq!(
            bare.compile_root(Some(dir.as_path())),
            std::env::temp_dir()
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fuera);
    }
}
