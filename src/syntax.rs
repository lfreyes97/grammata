//! Snippets del autocompletado `#…`.

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
