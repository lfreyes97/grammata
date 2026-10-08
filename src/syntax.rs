//! Resaltado Typst para egui_code_editor y snippets de autocompletado.

use std::collections::BTreeSet;
use std::sync::LazyLock;

/// `#` inicia palabra para que `#table` sea un token (keyword con `#`).
pub static TYPST: LazyLock<egui_code_editor::Syntax> = LazyLock::new(|| {
    egui_code_editor::Syntax::new("typst")
        .with_comment("//")
        .with_comment_multiline(["/*", "*/"])
        .with_word_start(BTreeSet::from(['#']))
        .with_keywords(BTreeSet::from([
            // control y declaración (con y sin #)
            "#let", "#set", "#show", "#import", "#include", "#for", "#while", "#if",
            "#else", "#context", "#return", "#break", "#continue",
            "let", "set", "show", "import", "include", "for", "while", "if", "else",
            "context", "as", "in", "and", "or", "not", "is", "return", "break",
            "continue", "none", "auto", "true", "false",
            // funciones habituales (forma #nombre)
            "#table", "#figure", "#image", "#text", "#link", "#heading", "#list",
            "#enum", "#math", "#equation", "#cite", "#ref", "#label", "#outline",
            "#page", "#block", "#grid", "#stack", "#counter", "#state", "#query",
            "#align", "#pad", "#place", "#hide", "#line", "#rect", "#circle",
            "#ellipse", "#polygon", "#path", "#curve", "#caption", "#footnote",
            "#bibliography", "#terms", "#par", "#quote", "#strong", "#emph",
            "#highlight", "#strike", "#super", "#sub", "#smallcaps", "#box",
            "#v", "#h", "#pagebreak", "#colbreak", "#columns", "#raw", "#lorem",
        ]))
        .with_types(BTreeSet::from([
            "int", "float", "length", "angle", "ratio", "color", "str", "bool",
            "array", "dictionary", "content", "stroke", "alignment", "direction",
        ]))
});

pub struct Completion {
    pub name: &'static str,
    pub snippet: &'static str,
    pub detail: &'static str,
}

const fn c(name: &'static str, snippet: &'static str, detail: &'static str) -> Completion {
    Completion { name, snippet, detail }
}

pub const COMPLETIONS: &[Completion] = &[
    c("align", "align(center)[..]", "alinear contenido"),
    c("bibliography", "bibliography(\"refs.bib\")", "bibliografía"),
    c("block", "block[..]", "bloque"),
    c("box", "box[..]", "caja en línea"),
    c("cite", "cite(<clave>)", "cita"),
    c("columns", "columns(2)[..]", "columnas"),
    c("context", "context [..]", "contexto"),
    c("counter", "counter(..)", "contador"),
    c("emph", "emph[..]", "cursiva"),
    c("enum", "+ item\n+ item", "enumeración"),
    c("figure", "figure(caption: [], image(\"img.png\"))", "figura"),
    c("footnote", "footnote[..]", "nota al pie"),
    c("for", "for x in (1, 2) [..]", "bucle"),
    c("grid", "grid(columns: 2, [..], [..])", "rejilla"),
    c("h", "h(1em)", "espacio horizontal"),
    c("heading", "= Título", "encabezado"),
    c("if", "if x > 1 [..]", "condicional"),
    c("image", "image(\"img.png\", width: 80%)", "imagen"),
    c("import", "import \"..\": *", "importar"),
    c("include", "include \"cap.typ\"", "incluir"),
    c("label", "<mi-etiqueta>", "etiqueta"),
    c("let", "let x = 1", "variable"),
    c("link", "link(\"https://..\")[..]", "enlace"),
    c("list", "- item\n- item", "lista"),
    c("lorem", "lorem(30)", "texto de relleno"),
    c("math", "$ x^2 + y^2 = z^2 $", "ecuación"),
    c("outline", "outline()", "índice"),
    c("page", "set page(paper: \"a4\", margin: 2cm)", "página"),
    c("pagebreak", "pagebreak()", "salto de página"),
    c("query", "query(heading)", "consulta"),
    c("quote", "quote(block: true)[..]", "cita en bloque"),
    c("raw", "raw(\"..\", lang: \"rust\")", "código"),
    c("ref", "ref(<lbl>)", "referencia"),
    c("set", "set text(size: 11pt)", "regla set"),
    c("show", "show heading: it => [#it]", "regla show"),
    c("stack", "stack[..]", "pila"),
    c("state", "state(..)", "estado"),
    c("strong", "strong[..]", "negrita"),
    c("table", "table(columns: 2, [a], [b])", "tabla"),
    c("text", "text(red)[..]", "texto con estilo"),
    c("v", "v(1em)", "espacio vertical"),
];

/// Coincidencias por prefijo primero, luego por subcadena.
pub fn matching(prefix: &str) -> Vec<&'static Completion> {
    let p = prefix.to_lowercase();
    let mut out: Vec<_> = COMPLETIONS.iter().filter(|c| c.name.starts_with(&p)).collect();
    out.extend(
        COMPLETIONS
            .iter()
            .filter(|c| !c.name.starts_with(&p) && c.name.contains(&p)),
    );
    out
}
