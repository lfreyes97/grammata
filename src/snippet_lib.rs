//! Bibliotecas de snippets escritas en typst: un `snippets.typ` global (en
//! la carpeta de configuración) y otro del proyecto (en la carpeta del
//! documento o en una superior, sin salir del gestor de archivos).
//!
//! Cada `#let` público (no empieza por `_`) es un snippet:
//! - Sin parámetros, se copia su valor: `[…]` → su marcado, `"…"` → el
//!   texto, `$…$` → la ecuación, otra expresión → `#expr`.
//! - Con parámetros (una función), se inserta la llamada. La definición se
//!   importa si la biblioteca está en la raíz del documento (typst no lee
//!   fuera de ella); si no, se copia al principio del documento.
//!
//! Los comentarios `///` justo encima dan el nombre (hasta el primer punto),
//! la descripción y, con «Grupo: X», el grupo.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use typst_syntax::ast::{self, AstNode};
use typst_syntax::{SyntaxKind as K, SyntaxNode};

pub const FILE: &str = "snippets.typ";

/// Cada cuánto se mira si el archivo cambió en disco.
const RECHECK: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    /// Texto que se copia tal cual.
    Text(String),
    /// Llamada a la función (con marcadores para los parámetros).
    Call(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LibSnippet {
    pub ident: String,
    pub name: String,
    pub desc: String,
    pub group: Option<String>,
    pub body: Body,
    /// Fuente del `#let …` (para copiarlo si no se puede importar).
    pub def: String,
}

impl LibSnippet {
    /// Lo que se inserta en el cursor (sin el `#import` o la definición).
    pub fn insert_text(&self) -> &str {
        match &self.body {
            Body::Text(t) | Body::Call(t) => t,
        }
    }
}

/// Analiza el fuente de una biblioteca. Devuelve los snippets y cuántos
/// errores de sintaxis tiene (un `#let` roto no aparece).
pub fn parse(src: &str) -> (Vec<LibSnippet>, usize) {
    let root = typst_syntax::parse(src);
    let errors = root.errors_and_warnings().0.len();
    let mut out = Vec::new();
    let mut docs: Vec<String> = Vec::new();
    let mut offset = 0;
    let mut hash_at = None;
    for n in root.children() {
        match n.kind() {
            K::LineComment => match n.leaf_text().strip_prefix("///") {
                Some(d) => docs.push(d.trim().to_string()),
                None => docs.clear(),
            },
            // Un salto de línea entre el comentario y el `#let` vale; una
            // línea en blanco (Parbreak) los separa.
            K::Space if !n.leaf_text().contains("\n\n") => {}
            K::Hash => hash_at = Some(offset),
            K::LetBinding => {
                let start = hash_at.unwrap_or(offset);
                let def = &src[start..offset + n.len()];
                if let Some(s) = snippet(n, &docs, def) {
                    out.push(s);
                }
                docs.clear();
                hash_at = None;
            }
            _ => {
                docs.clear();
                hash_at = None;
            }
        }
        offset += n.len();
    }
    (out, errors)
}

fn snippet(node: &SyntaxNode, docs: &[String], def: &str) -> Option<LibSnippet> {
    let lb = node.cast::<ast::LetBinding>()?;
    let init = lb.init()?;
    let (ident, body) = match lb.kind() {
        ast::LetBindingKind::Closure(id) => {
            let ast::Expr::Closure(c) = init else {
                return None;
            };
            (id.get().to_string(), Body::Call(call(id.get(), c)))
        }
        ast::LetBindingKind::Normal(ast::Pattern::Normal(ast::Expr::Ident(id))) => {
            (id.get().to_string(), Body::Text(value_text(init)))
        }
        _ => return None,
    };
    if ident.starts_with('_') {
        return None;
    }
    let (name, desc, group) = describe(docs, &ident);
    Some(LibSnippet {
        ident,
        name,
        desc,
        group,
        body,
        def: def.to_string(),
    })
}

/// Texto que inserta un `#let x = valor`.
fn value_text(e: ast::Expr) -> String {
    match e {
        ast::Expr::ContentBlock(b) => b.body().to_untyped().full_text().trim().to_string(),
        ast::Expr::Str(s) => s.get().to_string(),
        ast::Expr::Equation(_) => e.to_untyped().full_text().to_string(),
        _ => format!("#{}", e.to_untyped().full_text()),
    }
}

/// `#nombre(a: ${def}, ${pos})[${sel:texto}]`: los nombrados con su valor
/// por defecto como relleno; el último posicional, si es `body`, como
/// bloque de contenido que envuelve la selección.
fn call(name: &str, c: ast::Closure) -> String {
    let params: Vec<_> = c.params().children().collect();
    let pos_name = |p: &ast::Param| match p {
        ast::Param::Pos(ast::Pattern::Normal(ast::Expr::Ident(id))) => Some(id.get().to_string()),
        _ => None,
    };
    let trailing_body = params
        .last()
        .and_then(pos_name)
        .is_some_and(|n| n == "body");
    let mut args = Vec::new();
    for (i, p) in params.iter().enumerate() {
        if trailing_body && i + 1 == params.len() {
            break;
        }
        match p {
            ast::Param::Named(n) => args.push(format!(
                "{}: ${{{}}}",
                n.name().get(),
                n.expr().to_untyped().full_text()
            )),
            ast::Param::Pos(_) => {
                let n = pos_name(p).unwrap_or_else(|| "x".into());
                args.push(format!("${{{n}}}"));
            }
            ast::Param::Spread(_) => {}
        }
    }
    let mut s = format!("#{name}");
    if !args.is_empty() || !trailing_body {
        s += &format!("({})", args.join(", "));
    }
    if trailing_body {
        s += "[${sel:texto}]";
    }
    s
}

/// Nombre (primera frase), descripción y grupo a partir de los `///`.
fn describe(docs: &[String], ident: &str) -> (String, String, Option<String>) {
    let mut group = None;
    let mut text = Vec::new();
    for line in docs {
        // «Grupo: X» puede ir en su línea o al final de una frase.
        let lower = line.to_lowercase();
        let at = lower.find("grupo:").or_else(|| lower.find("group:"));
        match at {
            Some(i) => {
                let g = line[i..]
                    .split_once(':')
                    .map(|(_, g)| g.trim())
                    .unwrap_or("");
                if !g.is_empty() {
                    group = Some(g.trim_end_matches('.').to_string());
                }
                let before = line[..i].trim();
                if !before.is_empty() {
                    text.push(before.to_string());
                }
            }
            None => text.push(line.clone()),
        }
    }
    let desc = text.join(" ");
    let name = desc
        .split_once(". ")
        .map(|(a, _)| a)
        .unwrap_or(&desc)
        .trim_end_matches('.')
        .trim()
        .to_string();
    let name = if name.is_empty() {
        ident.to_string()
    } else {
        name
    };
    (name, desc, group)
}

// ---------- Archivos ----------

pub fn global_path() -> PathBuf {
    crate::session::config_dir().join(FILE)
}

/// `snippets.typ` del proyecto: en la carpeta del documento o en una
/// superior, sin subir por encima de `files_root` (si no hay gestor, solo
/// la del documento).
pub fn project_path(doc_dir: &Path, files_root: Option<&Path>) -> Option<PathBuf> {
    let inside = |d: &Path| files_root.is_some_and(|r| d.starts_with(r));
    let mut dir = Some(doc_dir);
    while let Some(d) = dir {
        let p = d.join(FILE);
        if p.is_file() {
            return Some(p);
        }
        if !inside(d) || Some(d) == files_root {
            break;
        }
        dir = d.parent();
    }
    None
}

/// Qué hay que añadir al documento (en `doc_root`) para poder usar `s`.
/// `None` si no hace falta nada (texto, o ya importado/definido).
pub fn preamble(s: &LibSnippet, lib: &Path, doc_root: &Path, doc_text: &str) -> Option<String> {
    if !matches!(s.body, Body::Call(_)) {
        return None;
    }
    match lib.strip_prefix(doc_root) {
        Ok(rel) => {
            let rel = rel.to_string_lossy().replace('\\', "/");
            let import = format!("#import \"{rel}\"");
            (!doc_text.contains(&import)).then(|| format!("{import}: *\n"))
        }
        Err(_) => {
            let defined = doc_text.contains(&format!("#let {}(", s.ident));
            (!defined).then(|| format!("{}\n\n", s.def.trim_end()))
        }
    }
}

/// Plantilla con ejemplos para una biblioteca nueva.
pub fn starter() -> &'static str {
    "// Biblioteca de snippets. Cada `#let` público aparece en el panel de
// snippets; los comentarios `///` de encima dan el nombre y, con
// «Grupo: X», el grupo. Los que empiezan por `_` no aparecen.
//
// Sin parámetros se copia su valor. Con parámetros se inserta la llamada,
// y el editor añade el `#import` (o copia la definición si este archivo
// no está en la carpeta del documento).

/// Ley de Ohm. Grupo: Física
#let ohm = $ V = I R $

/// Firma de una carta. Grupo: Cartas
#let firma = [Atentamente,

Nombre Apellido]

/// Caja de aviso con título. Grupo: Bloques
#let aviso(titulo: \"Aviso\", color: orange, body) = block(
  width: 100%,
  inset: 10pt,
  radius: 4pt,
  stroke: (left: 3pt + color),
  fill: color.lighten(90%),
)[*#titulo.* #body]
"
}

/// Una biblioteca cargada, que se recarga sola si cambia en disco.
pub struct Library {
    pub path: PathBuf,
    pub items: Vec<LibSnippet>,
    pub errors: usize,
    stamp: Option<SystemTime>,
    checked: Option<Instant>,
}

impl Library {
    pub fn new(path: PathBuf) -> Self {
        let mut l = Self {
            path,
            items: Vec::new(),
            errors: 0,
            stamp: None,
            checked: None,
        };
        l.refresh();
        l
    }

    pub fn exists(&self) -> bool {
        self.stamp.is_some()
    }

    /// Relee si cambió (lo comprueba como mucho cada `RECHECK`).
    pub fn refresh(&mut self) {
        if self.checked.is_some_and(|t| t.elapsed() < RECHECK) {
            return;
        }
        self.checked = Some(Instant::now());
        let stamp = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        if stamp == self.stamp {
            return;
        }
        self.stamp = stamp;
        (self.items, self.errors) = match std::fs::read_to_string(&self.path) {
            Ok(src) => parse(&src),
            Err(_) => (Vec::new(), 0),
        };
    }

    /// Fuerza la relectura en el próximo `refresh`.
    pub fn invalidate(&mut self) {
        self.checked = None;
        self.stamp = None;
    }
}

/// Las dos bibliotecas: global y la del proyecto del documento actual.
pub struct Libs {
    pub global: Library,
    pub project: Option<Library>,
}

impl Default for Libs {
    fn default() -> Self {
        Self {
            global: Library::new(global_path()),
            project: None,
        }
    }
}

impl Libs {
    /// Recarga lo que haya cambiado y sigue al documento actual.
    pub fn refresh(&mut self, doc_dir: Option<&Path>, files_root: Option<&Path>) {
        self.global.refresh();
        let want = doc_dir.and_then(|d| project_path(d, files_root));
        match (&mut self.project, want) {
            (Some(p), Some(w)) if p.path == w => p.refresh(),
            (_, w) => self.project = w.map(Library::new),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_ejemplos_de_la_plantilla() {
        let (items, errors) = parse(starter());
        assert_eq!(errors, 0);
        let names: Vec<_> = items.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Ley de Ohm",
                "Firma de una carta",
                "Caja de aviso con título"
            ]
        );
        assert_eq!(items[0].group.as_deref(), Some("Física"));
        assert_eq!(items[0].body, Body::Text("$ V = I R $".into()));
        assert_eq!(
            items[1].body,
            Body::Text("Atentamente,\n\nNombre Apellido".into())
        );
        assert_eq!(
            items[2].body,
            Body::Call("#aviso(titulo: ${\"Aviso\"}, color: ${orange})[${sel:texto}]".into())
        );
        assert!(items[2].def.starts_with("#let aviso(") && items[2].def.ends_with("#body]"));
    }

    #[test]
    fn valores_y_llamadas() {
        let src = "#let a = \"hola\"\n#let b = table(columns: 2)[x][y]\n#let _oculto = 1\n\
                   #let f(x, y) = x + y\n#let g() = 1\n#let h(body) = body\n#let (p, q) = (1, 2)";
        let (items, _) = parse(src);
        let got: Vec<_> = items
            .iter()
            .map(|s| (s.ident.as_str(), s.insert_text()))
            .collect();
        assert_eq!(
            got,
            [
                ("a", "hola"),
                ("b", "#table(columns: 2)[x][y]"),
                ("f", "#f(${x}, ${y})"),
                ("g", "#g()"),
                ("h", "#h[${sel:texto}]"),
            ]
        );
        // Sin `///`, el nombre es el identificador.
        assert_eq!(items[0].name, "a");
    }

    #[test]
    fn comentarios_solo_pegados() {
        let src = "/// Suelto\n\n#let a = 1\n// normal\n#let b = 2\n/// Uno.\n/// Más texto. Grupo: G\n#let c = 3";
        let (items, _) = parse(src);
        assert_eq!(items[0].name, "a");
        assert_eq!(items[1].name, "b");
        assert_eq!(items[2].name, "Uno");
        assert_eq!(items[2].desc, "Uno. Más texto.");
        assert_eq!(items[2].group.as_deref(), Some("G"));
    }

    #[test]
    fn preambulo_importa_o_copia() {
        let (items, _) = parse(starter());
        let aviso = &items[2];
        let root = Path::new("/p/tesis");
        let lib = root.join(FILE);
        assert_eq!(
            preamble(aviso, &lib, root, "= Hola").as_deref(),
            Some("#import \"snippets.typ\": *\n")
        );
        assert_eq!(
            preamble(aviso, &lib, root, "#import \"snippets.typ\": aviso\n"),
            None
        );
        // Fuera de la raíz (global): se copia la definición, una vez.
        let global = Path::new("/home/u/.config/grammata/snippets.typ");
        let p = preamble(aviso, global, root, "").unwrap();
        assert!(p.starts_with("#let aviso("));
        assert_eq!(preamble(aviso, global, root, &p), None);
        // Texto: nada que añadir.
        assert_eq!(preamble(&items[0], &lib, root, ""), None);
    }

    #[test]
    fn busca_la_del_proyecto_hacia_arriba() {
        let d = std::env::temp_dir().join(format!("grammata-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("cap/sub")).unwrap();
        std::fs::write(d.join(FILE), "").unwrap();
        assert_eq!(
            project_path(&d.join("cap/sub"), Some(&d)),
            Some(d.join(FILE))
        );
        // Sin gestor de archivos, solo la carpeta del documento.
        assert_eq!(project_path(&d.join("cap/sub"), None), None);
        assert_eq!(project_path(&d, None), Some(d.join(FILE)));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// La plantilla compila importada desde un documento que usa sus snippets.
    #[test]
    fn la_plantilla_compila() {
        let d = std::env::temp_dir().join(format!("grammata-libc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(FILE), starter()).unwrap();
        let (items, _) = parse(starter());
        let mut doc = String::new();
        for s in &items {
            if let Some(p) = preamble(s, &d.join(FILE), &d, &doc) {
                doc.insert_str(0, &p);
            }
            doc += &crate::snippets::expand(s.insert_text(), "x").0;
            doc += "\n\n";
        }
        let out = crate::engine::compile_pdf(&doc, &d, "main.typ");
        assert!(out.pdf.is_some(), "{doc}\n{:?}", out.diags);
        let _ = std::fs::remove_dir_all(&d);
    }
}
