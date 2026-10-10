//! Fuente typst de cada tipo: `template.typ` (una función con el formato
//! elegido) y `main.typ` (los datos y un arranque para escribir).
//!
//! Los datos van como parámetros de la función; el formato (papel, fuente,
//! columnas, encabezado…) queda fijo en la plantilla, donde se edita.

use super::*;

/// `refs.bib` de ejemplo (la cita del arranque apunta a esta entrada).
pub const SAMPLE_BIB: &str = "@book{knuth1984,
  author    = {Knuth, Donald E.},
  title     = {The TeXbook},
  publisher = {Addison-Wesley},
  year      = {1984},
}
";

const MESES: &str = "(\"enero\", \"febrero\", \"marzo\", \"abril\", \"mayo\", \"junio\",\n  \"julio\", \"agosto\", \"septiembre\", \"octubre\", \"noviembre\", \"diciembre\")";

/// Saltos a página impar del libro: las páginas en blanco que dejan (entre
/// un par de marcas `<salto-ini>`/`<salto-fin>`) van sin encabezado ni pie.
const BOOK_HELPERS: &str =
    "// Salto a página impar; la página en blanco que deje va sin encabezado ni pie.
#let _salto() = {
  [#metadata(none) <salto-ini>]
  pagebreak(to: \"odd\", weak: true)
  [#metadata(none) <salto-fin>]
}
#let _en_blanco() = {
  let p = here().page()
  query(<salto-ini>)
    .zip(query(<salto-fin>))
    .any(((a, b)) => a.location().page() < p and p < b.location().page())
}

";

/// Gris de los textos secundarios y color de títulos de la presentación.
const GRIS: &str = "luma(90)";
const TINTA: &str = "rgb(\"#1f5f7a\")";

/// Parámetro de la función de la plantilla: valor por defecto y el que
/// pasa `main.typ` (`None` = no se pasa).
struct Param {
    name: &'static str,
    default: String,
    value: Option<String>,
}

fn quoted(s: &str) -> String {
    format!("\"{}\"", esc(s))
}

/// Cadena o `none` si está en blanco.
fn opt(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        "none".into()
    } else {
        quoted(t)
    }
}

impl Options {
    fn es(&self) -> bool {
        self.lang == Lang::Es
    }

    fn tr(&self, es: &'static str, en: &'static str) -> &'static str {
        if self.es() { es } else { en }
    }

    fn has(&self, f: Field) -> bool {
        self.kind.has(f)
    }

    fn two_columns(&self) -> bool {
        self.has(Field::Columns) && self.columns == 2
    }

    fn params(&self) -> Vec<Param> {
        let p = |name, default: &str, value: Option<String>| Param {
            name,
            default: default.to_string(),
            value,
        };
        let title = self.title.trim();
        let mut v = vec![p(
            "titulo",
            &quoted(self.tr("Título", "Title")),
            (!title.is_empty()).then(|| quoted(title)),
        )];
        if self.has(Field::Subtitle) {
            v.push(p("subtitulo", "none", Some(opt(&self.subtitle))));
        }
        if self.has(Field::Course) {
            v.push(p("curso", "none", Some(opt(&self.course))));
        }
        v.push(p("autor", "none", Some(opt(&self.author))));
        if self.has(Field::Institution) {
            v.push(p("institucion", "none", Some(opt(&self.institution))));
        }
        let fecha = match self.date {
            DateMode::Today => "auto".to_string(),
            DateMode::Custom => opt(&self.custom_date),
            DateMode::None => "none".to_string(),
        };
        v.push(p("fecha", "auto", Some(fecha)));
        if self.has(Field::Abstract) {
            v.push(p("resumen", "none", Some(opt(&self.abstract_text))));
        }
        if self.has(Field::Toc) {
            v.push(p("indice", "true", Some(self.toc.to_string())));
        }
        if self.has(Field::Solutions) {
            v.push(p("soluciones", "true", Some(self.solutions.to_string())));
        }
        if self.has(Field::Bib) {
            let value = self
                .bib
                .code()
                .map(|s| format!("bibliography(\"{BIB_FILE}\", style: \"{s}\")"));
            v.push(p("bibliografia", "none", value));
        }
        v
    }

    /// `template.typ`: la función del tipo con el formato elegido.
    pub fn build_template(&self) -> String {
        let k = self.kind;
        let mut s = format!(
            "// Plantilla «{}» generada por el editor. Se aplica desde main.typ\n\
             // con `#show: {}.with(...)`; el formato se cambia aquí.\n\n",
            k.name(),
            k.func()
        );
        if self.es() {
            s += &format!("#let _meses = {MESES}\n\n");
        }
        match k {
            Kind::Notes => s += &self.notes_envs(),
            Kind::Book => s += BOOK_HELPERS,
            _ => {}
        }
        s += "// `fecha`: auto = hoy, none = sin fecha, o un texto.\n";
        s += &format!("#let {}(\n", k.func());
        for p in self.params() {
            s += &format!("  {}: {},\n", p.name, p.default);
        }
        s += "  body,\n) = {\n";
        s += &self.sets();
        s += &self.date_code();
        s += &match k {
            Kind::Article => self.article(),
            Kind::Report => self.report(),
            Kind::Book => self.book(),
            Kind::Notes => self.notes(),
            Kind::Slides => self.slides(),
        };
        s += "  body\n";
        if self.has(Field::Bib) {
            s += "  if bibliografia != none {\n    bibliografia\n  }\n";
        }
        s += "}\n";
        s
    }

    /// `main.typ`: importa la plantilla, le pasa los datos y deja un
    /// arranque. Los textos van como cadenas (`"…"`): `$`, `@`, `]`, `*`…
    /// se muestran tal cual. Un campo en blanco queda en `none`.
    pub fn build_main(&self) -> String {
        let k = self.kind;
        let what = if k == Kind::Notes { "*" } else { k.func() };
        let mut s = format!(
            "#import \"{TEMPLATE_FILE}\": {what}\n\n#show: {}.with(\n",
            k.func()
        );
        for p in self.params() {
            if let Some(v) = p.value {
                s += &format!("  {}: {v},\n", p.name);
            }
        }
        s += ")\n\n";
        s += &self.starter();
        s
    }

    fn starter(&self) -> String {
        let cite = if self.uses_bib() {
            self.tr(
                "\nTexto con una cita @knuth1984.\n",
                "\nText with a citation @knuth1984.\n",
            )
        } else {
            ""
        };
        match self.kind {
            Kind::Article | Kind::Report => {
                format!("= {}\n{cite}", self.tr("Introducción", "Introduction"))
            }
            Kind::Book => format!("= {}\n{cite}", self.tr("Primer capítulo", "First chapter")),
            Kind::Notes => format!(
                "= {}\n{cite}\n#definicion[{}]\n\n#ejercicio[{}]\n\n#solucion[{}]\n",
                self.tr("Introducción", "Introduction"),
                self.tr("Texto de la definición.", "Definition text."),
                self.tr("Enunciado del ejercicio.", "Exercise statement."),
                self.tr("Desarrollo de la solución.", "Worked solution."),
            ),
            Kind::Slides => self
                .tr(
                    "= Primera sección\n\n== Primera diapositiva\n\n- Primer punto\n- Segundo punto\n",
                    "= First section\n\n== First slide\n\n- First point\n- Second point\n",
                )
                .to_string(),
        }
    }

    // ---------- Partes comunes ----------

    fn sets(&self) -> String {
        let k = self.kind;
        let mut s = String::from("  set document(title: titulo)\n");
        let m = self.margins.cm();
        let margin = match k {
            Kind::Slides => "(x: 2.2cm, top: 1.8cm, bottom: 1.6cm)".to_string(),
            Kind::Book => format!(
                "(inside: {}cm, outside: {}cm, y: {}cm)",
                fmt_pt(m + 0.5),
                fmt_pt(m - 0.5),
                fmt_pt(m)
            ),
            _ => format!("{}cm", fmt_pt(m)),
        };
        let paper = if k == Kind::Slides {
            "presentation-16-9"
        } else {
            self.paper.code()
        };
        s += &format!("  set page(\n    paper: \"{paper}\",\n    margin: {margin},\n");
        // El índice usa esta numeración; el pie la dibuja a su manera.
        s += "    numbering: \"1\",\n";
        if self.two_columns() && k != Kind::Report {
            s += "    columns: 2,\n";
        }
        s += &format!(
            "    header: {},\n    footer: {},\n  )\n",
            self.header_code(),
            self.footer_code()
        );
        let font = match self.font.trim() {
            "" => String::new(),
            f => format!("font: {}, ", quoted(f)),
        };
        s += &format!(
            "  set text({font}size: {}pt, lang: \"{}\")\n",
            fmt_pt(self.font_size),
            self.lang.code()
        );
        let mut par = Vec::new();
        if self.has(Field::Justify) && self.justify {
            par.push("justify: true".to_string());
        }
        if let Some((l, sp)) = self.leading.code() {
            par.push(format!("leading: {l}, spacing: {sp}"));
        }
        if !par.is_empty() {
            s += &format!("  set par({})\n", par.join(", "));
        }
        if self.has(Field::Numbering) && self.numbering {
            s += "  set heading(numbering: \"1.1\")\n";
        }
        s
    }

    /// Encabezado: se omite en la primera página (la portada).
    fn header_code(&self) -> String {
        if !self.has(Field::Header) {
            return "none".into();
        }
        let cond = self.page_cond();
        let right = |x: &str| {
            format!(
                "context if {cond} {{\n      align(right, text(size: 0.85em, fill: {GRIS}, {x}))\n    }}"
            )
        };
        match self.header {
            HeaderMode::None => "none".into(),
            HeaderMode::Title => right("titulo"),
            HeaderMode::Custom if self.header_text.trim().is_empty() => "none".into(),
            HeaderMode::Custom => right(&quoted(self.header_text.trim())),
            HeaderMode::Chapter if !self.has(Field::Chapter) => "none".into(),
            // Último capítulo empezado antes de esta página; nada en la
            // página donde empieza uno.
            HeaderMode::Chapter => format!(
                "context {{\n      let p = here().page()\n      \
                 let caps = query(heading.where(level: 1, outlined: true))\n      \
                 let antes = caps.filter(h => h.location().page() < p)\n      \
                 if {cond} and antes.len() > 0 and not caps.any(h => h.location().page() == p) {{\n        \
                 align(right, text(size: 0.85em, fill: {GRIS}, antes.last().body))\n      }}\n    }}"
            ),
        }
    }

    /// Páginas con encabezado (y, en el libro, con pie): no la portada ni
    /// las que dejan en blanco los saltos a impar.
    fn page_cond(&self) -> &'static str {
        if self.kind == Kind::Book {
            "here().page() > 1 and not _en_blanco()"
        } else {
            "here().page() > 1"
        }
    }

    fn footer_code(&self) -> String {
        let pos = match self.page_num {
            PageNum::None => return "none".into(),
            PageNum::Center => "center",
            PageNum::Right => "right",
            PageNum::Outside => "if calc.odd(here().page()) { right } else { left }",
        };
        let num = match self.page_fmt {
            PageFmt::Plain => "counter(page).display()".to_string(),
            PageFmt::OfTotal => "counter(page).display(\"1 / 1\", both: true)".to_string(),
            PageFmt::Word => format!("[{} #counter(page).display()]", self.tr("Página", "Page")),
        };
        if self.kind == Kind::Book {
            // Romanos hasta el cuerpo (marcado con <libro-cuerpo>).
            return format!(
                "context if {} {{\n      let cuerpo = query(<libro-cuerpo>)\n      \
                 let romano = cuerpo.len() > 0 and here().page() < cuerpo.first().location().page()\n      \
                 let n = if romano {{ counter(page).display(\"i\") }} else {{ {num} }}\n      \
                 align({pos}, text(size: 0.85em, n))\n    }}",
                self.page_cond()
            );
        }
        format!("context align({pos}, text(size: 0.85em, {num}))")
    }

    /// Fecha de hoy: typst no traduce los meses, en español van a mano.
    fn date_code(&self) -> String {
        let hoy = if self.es() {
            "    [#hoy.day() de #_meses.at(hoy.month() - 1) de #hoy.year()]\n"
        } else {
            "    hoy.display(\"[month repr:long] [day padding:none], [year]\")\n"
        };
        format!(
            "\n  let fecha = if fecha == auto {{\n    let hoy = datetime.today()\n{hoy}  }} else {{ fecha }}\n\n"
        )
    }

    fn toc(&self, indent: &str, then: &str) -> String {
        format!(
            "{indent}if indice {{\n{indent}  outline(title: [{}])\n{then}{indent}}}\n",
            self.tr("Índice", "Contents")
        )
    }

    /// Portada a página completa (informe y libro).
    fn cover_page(&self) -> String {
        let inst = "    #if institucion != none { text(size: 13pt, smallcaps(institucion)) }\n";
        let book = self.kind == Kind::Book;
        let mut s = String::from(if book {
            "  block(width: 100%, height: 100%)[\n    #set align(center)\n"
        } else {
            "  page(header: none, footer: none)[\n    #set align(center)\n"
        });
        if !book {
            s += inst;
        }
        s += "    #v(1fr)\n";
        s += "    #text(size: 24pt, weight: \"bold\", titulo)\n";
        s += &format!(
            "    #if subtitulo != none [ \\ #v(0.3em) #text(size: 15pt, fill: {GRIS}, subtitulo)]\n"
        );
        s += "    #v(2fr)\n";
        s += "    #if autor != none [#text(size: 13pt, autor) \\ ]\n";
        s += "    #if fecha != none [#fecha]\n";
        s += "    #v(1fr)\n";
        if book {
            s += inst;
        }
        s += "  ]\n";
        s
    }

    // ---------- Tipos ----------

    fn article(&self) -> String {
        let mut s = String::from(
            "  let portada = {\n    align(center)[\n      \
             #text(size: 20pt, weight: \"bold\", titulo)\n      \
             #if autor != none [ \\ #text(size: 12pt, autor)]\n      \
             #if institucion != none [ \\ #emph(institucion)]\n      \
             #if fecha != none [ \\ #fecha]\n    ]\n",
        );
        s += &format!(
            "    if resumen != none {{\n      block(inset: (x: 2em))[*{}.* #resumen]\n    }}\n",
            self.tr("Resumen", "Abstract")
        );
        s += &self.toc("    ", "");
        s += "  }\n";
        s += self.place_or_inline("portada");
        s
    }

    /// A dos columnas, la portada flota a todo el ancho sobre ellas.
    fn place_or_inline(&self, what: &str) -> &'static str {
        if self.two_columns() {
            debug_assert!(matches!(what, "portada" | "cabecera"));
            if what == "portada" {
                "  place(top + center, float: true, scope: \"parent\", clearance: 2em, portada)\n"
            } else {
                "  place(top, float: true, scope: \"parent\", clearance: 1.5em, cabecera)\n"
            }
        } else if what == "portada" {
            "  portada\n"
        } else {
            "  cabecera\n"
        }
    }

    fn report(&self) -> String {
        let mut s = self.cover_page();
        s += "  counter(page).update(1)\n";
        s += &format!(
            "  if resumen != none {{\n    heading(outlined: false, numbering: none, [{}])\n    resumen\n    pagebreak(weak: true)\n  }}\n",
            self.tr("Resumen", "Abstract")
        );
        s += &self.toc("  ", "    pagebreak(weak: true)\n");
        if self.two_columns() {
            s += "  set page(columns: 2)\n";
        }
        s += "  show heading.where(level: 1): it => {\n    pagebreak(weak: true)\n    v(1.5em)\n    it\n    v(0.8em)\n  }\n";
        s
    }

    fn book(&self) -> String {
        let mut s = self.cover_page();
        // Dorso de la portada en blanco; preliminares en romanos.
        s += "  _salto()\n  counter(page).update(1)\n";
        s += &self.toc("  ", "    _salto()\n");
        s += "  [#metadata(none) <libro-cuerpo>]\n  counter(page).update(1)\n";
        s += &format!(
            "  show heading.where(level: 1): it => {{\n    \
             _salto()\n    v(12%)\n    \
             if it.numbering != none {{\n      \
             text(size: 13pt, fill: {GRIS})[{} #context numbering(it.numbering, ..counter(heading).at(it.location()))]\n      \
             parbreak()\n    }}\n    \
             text(size: 24pt, weight: \"bold\", it.body)\n    v(2em)\n  }}\n",
            self.tr("Capítulo", "Chapter")
        );
        s
    }

    /// Entornos de los apuntes (a nivel de fichero: `main.typ` los importa).
    fn notes_envs(&self) -> String {
        let t = |es, en| self.tr(es, en);
        format!(
            "// Entornos: #definicion[…], #teorema[…], #ejemplo[…], #ejercicio[…] y\n\
             // #solucion[…]. Con `nota: [...]` se añade un paréntesis al título.\n\
             #let _soluciones = state(\"soluciones\", true)\n\
             #let _num = counter(\"apuntes-entorno\")\n\
             #let _ej = counter(\"apuntes-ejercicio\")\n\n\
             #let _entorno(nombre, tinta, contador, nota, body) = {{\n  \
             contador.step()\n  \
             block(\n    width: 100%,\n    inset: (x: 10pt, y: 8pt),\n    radius: 3pt,\n    \
             fill: tinta.lighten(92%),\n    stroke: (left: 2.5pt + tinta),\n    {{\n      \
             text(weight: \"bold\", fill: tinta.darken(25%), {{\n        \
             nombre + \" \"\n        context contador.display()\n        \
             if nota != none [ (#nota)]\n        \".\"\n      }})\n      \
             h(0.4em)\n      body\n    }},\n  )\n}}\n\n\
             #let definicion(nota: none, body) = _entorno(\"{}\", rgb(\"#2f855a\"), _num, nota, body)\n\
             #let teorema(nota: none, body) = _entorno(\"{}\", rgb(\"#2b6cb0\"), _num, nota, body)\n\
             #let ejemplo(nota: none, body) = _entorno(\"{}\", rgb(\"#b7791f\"), _num, nota, body)\n\
             #let ejercicio(nota: none, body) = _entorno(\"{}\", rgb(\"#c05621\"), _ej, nota, body)\n\
             #let solucion(body) = context if _soluciones.get() {{\n  \
             block(width: 100%, inset: (left: 10pt, y: 4pt), stroke: (left: 1pt + luma(160)))[_{}._ #body]\n}}\n\n",
            t("Definición", "Definition"),
            t("Teorema", "Theorem"),
            t("Ejemplo", "Example"),
            t("Ejercicio", "Exercise"),
            t("Solución", "Solution"),
        )
    }

    fn notes(&self) -> String {
        let mut s = String::from("  _soluciones.update(soluciones)\n");
        s += &format!(
            "  let cabecera = {{\n    grid(\n      columns: (1fr, auto),\n      \
             align: (left + bottom, right + bottom),\n      column-gutter: 1em,\n      {{\n        \
             if curso != none {{ text(size: 10pt, fill: {GRIS}, upper(curso)); linebreak() }}\n        \
             text(size: 18pt, weight: \"bold\", titulo)\n      }},\n      {{\n        \
             set text(size: 10pt)\n        \
             if autor != none {{ autor; linebreak() }}\n        \
             if institucion != none {{ institucion; linebreak() }}\n        \
             if fecha != none {{ fecha }}\n      }},\n    )\n    \
             line(length: 100%, stroke: 0.6pt + luma(120))\n"
        );
        s += &self.toc("    ", "");
        s += "  }\n";
        s += self.place_or_inline("cabecera");
        s
    }

    fn slides(&self) -> String {
        let mut s = format!(
            "  page(header: none, footer: none)[\n    #set align(center + horizon)\n    \
             #text(size: 1.8em, weight: \"bold\", fill: {TINTA}, titulo)\n    \
             #if subtitulo != none [ \\ #text(size: 1.1em, fill: {GRIS}, subtitulo)]\n    \
             #v(1em)\n    \
             #if autor != none [#autor \\ ]\n    \
             #if institucion != none [#text(size: 0.85em, institucion) \\ ]\n    \
             #if fecha != none [#text(size: 0.85em, fecha)]\n  ]\n"
        );
        s += &format!(
            "  if indice {{\n    text(size: 1.25em, weight: \"bold\", fill: {TINTA}, [{}])\n    \
             v(0.6em)\n    outline(title: none, depth: 1)\n  }}\n",
            self.tr("Contenido", "Contents")
        );
        // «=» sección: diapositiva con el título centrado. «==»: título de
        // diapositiva con una raya debajo.
        s += &format!(
            "  show heading.where(level: 1): it => {{\n    pagebreak(weak: true)\n    \
             block(width: 100%, height: 100%, align(center + horizon, text(size: 1.6em, weight: \"bold\", fill: {TINTA}, it.body)))\n  }}\n\
             \x20 show heading.where(level: 2): it => {{\n    pagebreak(weak: true)\n    \
             block(\n      width: 100%,\n      below: 0.8em,\n      inset: (bottom: 0.3em),\n      \
             stroke: (bottom: 1.5pt + {TINTA}),\n      \
             text(size: 1.25em, weight: \"bold\", fill: {TINTA}, it.body),\n    )\n  }}\n"
        );
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workdir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("typst-editor-tmpl-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Escribe el proyecto en una carpeta propia y compila `main.typ`.
    /// Devuelve el número de páginas o los errores.
    fn compile(o: &Options) -> Result<usize, String> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = workdir().join(N.fetch_add(1, Ordering::Relaxed).to_string());
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let main = o.write_project(&dir).unwrap();
        let src = std::fs::read_to_string(main).unwrap();
        let out = crate::engine::compile_pdf(&src, &dir, MAIN_FILE);
        let _ = std::fs::remove_dir_all(&dir);
        match out.pdf {
            Some(pdf) => Ok(count_pages(&pdf)),
            None => Err(format!(
                "{:?}\n--- template.typ\n{}\n--- main.typ\n{}",
                out.diags.iter().map(|d| &d.message).collect::<Vec<_>>(),
                o.build_template(),
                o.build_main()
            )),
        }
    }

    fn count_pages(pdf: &[u8]) -> usize {
        let pdf = hayro::hayro_syntax::Pdf::new(std::sync::Arc::new(pdf.to_vec())).unwrap();
        pdf.pages().len()
    }

    fn ok(o: &Options) {
        if let Err(e) = compile(o) {
            panic!("no compila ({:?}):\n{e}", o.kind);
        }
    }

    #[test]
    fn escapa_comillas_y_barras() {
        assert_eq!(esc("di \"hola\" \\ adiós"), "di \\\"hola\\\" \\\\ adiós");
        assert_eq!(esc("a\r\nb"), "a\\nb");
    }

    #[test]
    fn cada_tipo_por_defecto_compila() {
        for k in Kind::ALL {
            ok(&Options::of(k));
        }
    }

    /// Para cada tipo, combinaciones que recorren todos los valores de cada
    /// opción (con pasos distintos, para que no vayan siempre juntos).
    #[test]
    fn combinaciones_compilan() {
        fn nth<T: Choice>(i: usize, step: usize) -> T {
            T::ALL[(i * step) % T::ALL.len()]
        }
        for k in Kind::ALL {
            for i in 0..12 {
                let mut o = Options::of(k);
                o.title = "El \"niño\" y la \\barra\\ $5 #f".into();
                o.subtitle = if i % 2 == 0 {
                    "Sub ]título".into()
                } else {
                    String::new()
                };
                o.author = if i % 3 == 0 {
                    String::new()
                } else {
                    "Ana \"La\" Autora".into()
                };
                o.institution = if i % 2 == 1 {
                    "Univ. @x".into()
                } else {
                    String::new()
                };
                o.course = if i % 2 == 0 {
                    "Cálculo I".into()
                } else {
                    String::new()
                };
                o.custom_date = "1º de enero de \"2026\"".into();
                o.abstract_text = if i % 2 == 0 {
                    "Resumen *con* $x$.\n\nOtro párrafo".into()
                } else {
                    String::new()
                };
                o.date = nth(i, 1);
                o.lang = nth(i, 1);
                o.paper = nth(i, 2);
                o.margins = nth(i, 1);
                o.columns = 1 + (i % 2) as u8;
                o.font = [
                    "",
                    "New Computer Modern",
                    "DejaVu Sans Mono",
                    "Fuente Que No Existe",
                ][i % 4]
                    .into();
                o.font_size = k.font_sizes()[i % 3];
                o.leading = nth(i, 2);
                o.justify = i % 3 != 1;
                o.numbering = i % 4 != 2;
                o.header = nth(i, 1);
                o.header_text = "Cabecera \"propia\"".into();
                o.page_num = nth(i, 3);
                o.page_fmt = nth(i, 1);
                o.toc = i % 3 != 2;
                o.bib = nth(i, 1);
                o.solutions = i % 2 == 0;
                ok(&o);
            }
        }
    }

    #[test]
    fn main_importa_la_plantilla() {
        for k in Kind::ALL {
            let main = Options::of(k).build_main();
            let import = if k == Kind::Notes { "*" } else { k.func() };
            assert!(
                main.starts_with(&format!("#import \"template.typ\": {import}\n")),
                "{main}"
            );
            assert!(
                main.contains(&format!("#show: {}.with(", k.func())),
                "{main}"
            );
        }
    }

    #[test]
    fn markup_es_literal() {
        let o = Options {
            title: "Costo $5 * _x_ // nota ] #f".into(),
            author: "ana@uni.edu <etq>".into(),
            date: DateMode::Custom,
            custom_date: "9 de octubre".into(),
            abstract_text: "Ver [1] y $5 ] ana@uni.edu\n\nSegundo párrafo".into(),
            ..Options::default()
        };
        let main = o.build_main();
        assert!(main.contains("fecha: \"9 de octubre\""), "{main}");
        ok(&o);
    }

    #[test]
    fn campos_vacios_y_titulo_por_defecto() {
        let o = Options {
            title: "  ".into(),
            author: "".into(),
            date: DateMode::None,
            ..Options::default()
        };
        let main = o.build_main();
        assert!(!main.contains("titulo:"), "{main}");
        assert!(
            main.contains("autor: none") && main.contains("resumen: none"),
            "{main}"
        );
        assert!(o.build_template().contains("titulo: \"Título\""));
        let en = Options {
            lang: Lang::En,
            ..o.clone()
        };
        assert!(en.build_template().contains("titulo: \"Title\""));
        ok(&o);
        ok(&en);
    }

    #[test]
    fn bibliografia_crea_refs_y_cita() {
        let o = Options {
            bib: BibStyle::Ieee,
            ..Options::of(Kind::Report)
        };
        assert_eq!(o.files(), [TEMPLATE_FILE, MAIN_FILE, BIB_FILE]);
        let main = o.build_main();
        assert!(
            main.contains("bibliography(\"refs.bib\", style: \"ieee\")"),
            "{main}"
        );
        assert!(main.contains("@knuth1984"), "{main}");
        ok(&o);
        // Sin bibliografía, ni fichero ni parámetro.
        let sin = Options::of(Kind::Report);
        assert_eq!(sin.files(), [TEMPLATE_FILE, MAIN_FILE]);
        assert!(!sin.build_main().contains("bibliografia"));
        // La presentación no tiene bibliografía aunque se haya elegido.
        let s = Options {
            bib: BibStyle::Apa,
            ..Options::of(Kind::Slides)
        };
        assert_eq!(s.files(), [TEMPLATE_FILE, MAIN_FILE]);
    }

    #[test]
    fn no_sobrescribe() {
        let dir = workdir().join("ocupado");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(BIB_FILE), "mío").unwrap();
        let o = Options {
            bib: BibStyle::Apa,
            ..Options::default()
        };
        let err = o.write_project(&dir).unwrap_err();
        assert!(err.contains(BIB_FILE), "{err}");
        assert_eq!(std::fs::read_to_string(dir.join(BIB_FILE)).unwrap(), "mío");
        assert!(!dir.join(TEMPLATE_FILE).exists() && !dir.join(MAIN_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn libro_empieza_capitulos_en_impar() {
        // Portada, dorso, índice, blanco, capítulo 1 → 5 páginas.
        let mut o = Options::of(Kind::Book);
        assert_eq!(compile(&o), Ok(5));
        // Sin índice: portada, dorso, capítulo 1.
        o.toc = false;
        assert_eq!(compile(&o), Ok(3));
    }

    #[test]
    fn apuntes_ocultan_soluciones() {
        let mut o = Options::of(Kind::Notes);
        assert!(o.build_main().contains("soluciones: true"));
        o.solutions = false;
        assert!(o.build_main().contains("soluciones: false"));
        ok(&o);
    }

    #[test]
    fn cambiar_de_tipo_ajusta_tamano_y_pie() {
        let mut o = Options::default();
        o.set_kind(Kind::Slides);
        assert_eq!(o.font_size, 20.0);
        assert_eq!(o.page_num, PageNum::Right);
        o.set_kind(Kind::Book);
        assert_eq!(o.font_size, 11.0);
        assert_eq!(o.header, HeaderMode::Chapter);
        assert_eq!(o.page_num, PageNum::Outside);
    }
}

/// Escribe un proyecto de cada tipo en `$SAMPLES_DIR/<tipo>` para revisarlos
/// a ojo (`typst compile`). `cargo test muestras -- --ignored`.
#[test]
#[ignore = "escribe en SAMPLES_DIR"]
fn muestras() {
    let Ok(root) = std::env::var("SAMPLES_DIR") else {
        return;
    };
    for k in Kind::ALL {
        let dir = std::path::Path::new(&root).join(k.func());
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let o = Options {
            subtitle: "Un subtítulo".into(),
            institution: "Universidad Nacional".into(),
            course: "Cálculo I".into(),
            abstract_text: "Este es el resumen del trabajo.".into(),
            bib: BibStyle::Apa,
            ..Options::of(k)
        };
        o.write_project(&dir).unwrap();
    }
    // Artículo a dos columnas con encabezado y pie «1 / N».
    let dir = std::path::Path::new(&root).join("articulo-2col");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let o = Options {
        columns: 2,
        header: HeaderMode::Title,
        page_fmt: PageFmt::OfTotal,
        abstract_text: "Este es el resumen del trabajo.".into(),
        ..Options::default()
    };
    o.write_project(&dir).unwrap();
}
