//! Panel de snippets: biblioteca de serie + snippets propios del usuario
//! (en `~/.config/typst-editor/snippets.json`).
//!
//! Marcadores en el cuerpo (ver `expand`):
//! - `${sel}`: el texto seleccionado (el snippet lo envuelve).
//! - `${sel:x}`: la selección, o `x` si no hay.
//! - `${x}`: texto de relleno `x`; el primero queda seleccionado para escribir encima.
//! - `${}`: posición del cursor.

use crate::actions::Action;
use crate::app::State;
use crate::theme;
use crate::widgets;
use egui::RichText;
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};

pub struct Snippet {
    pub name: &'static str,
    pub body: &'static str,
}

pub struct Group {
    pub name: &'static str,
    pub icon: &'static str,
    pub items: &'static [Snippet],
}

const fn sn(name: &'static str, body: &'static str) -> Snippet {
    Snippet { name, body }
}

pub const BUILTIN: &[Group] = &[
    Group {
        name: "Documento",
        icon: icon::FILE_TEXT,
        items: &[
            sn(
                "Configuración básica",
                "#set page(paper: \"a4\", margin: 2.5cm)\n#set text(size: 11pt, lang: \"es\")\n#set par(justify: true)\n#set heading(numbering: \"1.1\")\n${}",
            ),
            sn(
                "Título y autor",
                "#align(center)[\n  #text(size: 20pt, weight: \"bold\")[${Título}]\n\n  Autor · #datetime.today().display()\n]\n",
            ),
            sn("Índice", "#outline(title: [${Índice}])\n"),
            sn(
                "Encabezado y pie de página",
                "#set page(\n  header: align(right)[${Encabezado}],\n  numbering: \"1 / 1\",\n)\n",
            ),
            sn(
                "Resumen",
                "#block(inset: (x: 2em))[\n  *Resumen.* ${sel:Texto del resumen.}\n]\n",
            ),
        ],
    },
    Group {
        name: "Texto",
        icon: icon::TEXT_T,
        items: &[
            sn("Negrita", "*${sel:texto}*"),
            sn("Cursiva", "_${sel:texto}_"),
            sn("Código en línea", "`${sel:código}`"),
            sn("Enlace", "#link(\"${https://typst.app}\")[${sel:enlace}]"),
            sn("Nota al pie", "#footnote[${sel:nota}]"),
            sn(
                "Cita en bloque",
                "#quote(block: true, attribution: [${Autor}])[\n  ${sel:Cita.}\n]\n",
            ),
            sn("Resaltado", "#highlight[${sel:texto}]"),
            sn("Texto en color", "#text(fill: ${blue})[${sel:texto}]"),
            sn("Versalitas", "#smallcaps[${sel:texto}]"),
        ],
    },
    Group {
        name: "Listas",
        icon: icon::LIST_NUMBERS,
        items: &[
            sn("Viñetas", "- ${Elemento}\n- Elemento\n- Elemento\n"),
            sn("Numerada", "+ ${Paso}\n+ Paso\n+ Paso\n"),
            sn("Términos", "/ ${Término}: definición\n/ Otro: definición\n"),
            sn("Lista anidada", "- ${Uno}\n  - Uno a\n  - Uno b\n- Dos\n"),
        ],
    },
    Group {
        name: "Figuras y tablas",
        icon: icon::TABLE,
        items: &[
            sn(
                "Imagen con pie",
                "#figure(\n  image(\"${imagen.png}\", width: 80%),\n  caption: [Descripción],\n) <fig:nombre>\n",
            ),
            sn(
                "Tabla",
                "#table(\n  columns: 3,\n  table.header[*A*][*B*][*C*],\n  [${1}], [2], [3],\n  [4], [5], [6],\n)\n",
            ),
            sn(
                "Tabla con pie",
                "#figure(\n  table(\n    columns: 2,\n    table.header[*Nombre*][*Valor*],\n    [${a}], [1],\n  ),\n  caption: [Descripción],\n) <tab:nombre>\n",
            ),
        ],
    },
    Group {
        name: "Matemáticas",
        icon: icon::SIGMA,
        items: &[
            sn("Ecuación en línea", "$${sel:x}$"),
            sn("Ecuación en bloque", "$ ${sel:x} $"),
            sn(
                "Numerar ecuaciones",
                "#set math.equation(numbering: \"(1)\")\n${}",
            ),
            sn(
                "Ecuaciones alineadas",
                "$ ${f(x)} &= (x + 1)^2 \\\n  &= x^2 + 2x + 1 $",
            ),
            sn(
                "Sistema de casos",
                "$ f(x) = cases(\n  ${1} &\"si\" x > 0,\n  0 &\"si no\",\n) $",
            ),
            sn("Matriz", "$ A = mat(\n  ${1}, 2;\n  3, 4;\n) $"),
            sn("Matriz 3×3", "$ mat(${1}, 2, 3;\n  4, 5, 6;\n  7, 8, 9) $"),
            sn("Vector columna", "$ vec(${x}, ${y}, ${z}) $"),
            sn(
                "Sistema de ecuaciones",
                "$ cases(x + y &= ${7},\n  x - y &= ${1}) $",
            ),
            sn("Fracción", "$ frac(${a}, ${b}) $"),
            sn("Límite", "$ lim_(${x} -> ${0}) ${f(x)} $"),
            sn("Derivada", "$ (dif ${f}) / (dif ${x}) $"),
            sn("Integral", "$ integral_${a}^${b} ${f(x)} dif x $"),
            sn("Sumatorio", "$ sum_(${i} = 1)^${n} ${a_i} $"),
            sn("Productorio", "$ product_(${i} = 1)^${n} ${a_i} $"),
            sn("Raíz n-ésima", "$ root(${n}, ${x}) $"),
            sn("Valor absoluto", "$ abs(${x}) $"),
            sn("Norma", "$ norm(${v}) $"),
            sn("Cancelar término", "$ cancel(${x + y}) $"),
            sn("Llave inferior", "$ underbrace(${x + y}, \"${suma}\") $"),
            sn("Texto en ecuación", "$ ${x} \"donde\" ${x > 0} $"),
            sn("Ecuación etiquetada", "$ ${x} $ <${eq}>"),
            sn(
                "Fórmula cuadrática",
                "$ x = (-b plus.minus sqrt(b^2 - 4 a c)) / (2 a) $",
            ),
            sn(
                "Raíces por promedio",
                "$ u = -(${B}) / 2 = ${u} $\n$ d = sqrt((${u})^2 - ${C}) = ${d} $\n$ x = ${u} plus.minus ${d} $",
            ),
            sn(
                "Teorema",
                "#block(stroke: (left: 2pt + gray), inset: (left: 1em, y: 4pt))[\n  *Teorema.* ${sel:Enunciado.}\n]\n",
            ),
            sn(
                "Definición",
                "#block(stroke: (left: 2pt + gray), inset: (left: 1em, y: 4pt))[\n  *Definición.* ${sel:Término.}\n]\n",
            ),
            sn(
                "Demostración",
                "_Demostración._ ${sel:Razonamiento.} $square$",
            ),
        ],
    },
    Group {
        name: "Cálculo",
        icon: icon::CALCULATOR,
        items: &[
            sn("Redondear", "#calc.round(${3.14159}, digits: ${2})"),
            sn("Potencia", "#calc.pow(${2}, ${10})"),
            sn(
                "Discriminante",
                "#let a = ${2}\n#let b = ${8}\n#let c = ${6}\n#let D = calc.pow(b, 2) - 4 * a * c\n$Delta = #D$",
            ),
            sn(
                "Raíces cuadrática",
                "#let a = ${2}\n#let b = ${8}\n#let c = ${6}\n#let D = calc.pow(b, 2) - 4 * a * c\n#let r = calc.sqrt(D)\n$x = (#(-b) plus.minus #r) / #(2 * a)$",
            ),
            sn(
                "Po-Shen Loh",
                "#let B = ${4}\n#let C = ${3}\n#let u = -B / 2\n#let d = calc.sqrt(calc.pow(u, 2) - C)\n$u = #u, quad d = #d, quad x = #u plus.minus #d$",
            ),
        ],
    },
    Group {
        name: "Código",
        icon: icon::CODE,
        items: &[
            sn("Bloque de código", "```${rust}\n${sel}\n```\n"),
            sn("Variable", "#let ${nombre} = 1\n"),
            sn("Función", "#let ${nombre}(x) = {\n  x\n}\n"),
            sn("Regla show", "#show heading: it => {\n  ${it}\n}\n"),
            sn("Bucle", "#for i in range(${3}) [\n  - Elemento #i\n]\n"),
        ],
    },
    Group {
        name: "Diseño",
        icon: icon::LAYOUT,
        items: &[
            sn(
                "Dos columnas",
                "#columns(2)[\n  ${sel:Columna izquierda.}\n  #colbreak()\n  Columna derecha.\n]\n",
            ),
            sn(
                "Rejilla",
                "#grid(\n  columns: (1fr, 1fr),\n  gutter: 1em,\n  [${A}], [B],\n)\n",
            ),
            sn("Centrado", "#align(center)[${sel:texto}]"),
            sn(
                "Recuadro",
                "#rect(width: 100%, inset: 1em, radius: 4pt)[\n  ${sel:texto}\n]\n",
            ),
            sn("Salto de página", "#pagebreak()\n${}"),
            sn("Espacio vertical", "#v(${1em})\n"),
        ],
    },
    Group {
        name: "Referencias",
        icon: icon::BOOKMARK_SIMPLE,
        items: &[
            sn("Etiqueta", "<${etiqueta}>"),
            sn("Referencia", "@${etiqueta}"),
            sn(
                "Bibliografía",
                "#bibliography(\"${refs.bib}\", style: \"apa\")\n",
            ),
            sn("Cita", "#cite(<${clave}>)"),
        ],
    },
];

/// Expande los marcadores. Devuelve (texto, inicio, fin) de la selección
/// final en chars: el primer relleno o `${}`; si no hay, el final.
pub fn expand(body: &str, selection: &str) -> (String, usize, usize) {
    let mut out = String::new();
    let mut target: Option<(usize, usize)> = None;
    let mut rest = body;
    while let Some(i) = rest.find("${") {
        let Some(j) = rest[i + 2..].find('}') else {
            break;
        };
        out.push_str(&rest[..i]);
        let inner = &rest[i + 2..i + 2 + j];
        rest = &rest[i + 3 + j..];
        let fill = match inner {
            "sel" => {
                // Sin selección, el cursor queda donde iría (dentro de
                // `#text(...)[]`, entre `**`…) para seguir escribiendo.
                if selection.is_empty() && target.is_none() {
                    let at = out.chars().count();
                    target = Some((at, at));
                }
                out.push_str(selection);
                continue;
            }
            _ if inner.starts_with("sel:") && !selection.is_empty() => {
                out.push_str(selection);
                continue;
            }
            _ => inner.strip_prefix("sel:").unwrap_or(inner),
        };
        let start = out.chars().count();
        out.push_str(fill);
        if target.is_none() {
            target = Some((start, start + fill.chars().count()));
        }
    }
    out.push_str(rest);
    let n = out.chars().count();
    let (a, b) = target.unwrap_or((n, n));
    (out, a, b)
}

// ---------- Snippets del usuario ----------

#[derive(Clone, Serialize, Deserialize)]
pub struct UserSnippet {
    pub name: String,
    pub body: String,
}

fn path() -> std::path::PathBuf {
    crate::session::config_dir().join("snippets.json")
}

pub fn load_user() -> Vec<UserSnippet> {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_user(list: &[UserSnippet]) -> std::io::Result<()> {
    let p = path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(list).map_err(std::io::Error::other)?;
    std::fs::write(p, json)
}

/// Snippet en edición (diálogo). `index` = `None` para uno nuevo.
pub struct Editing {
    pub index: Option<usize>,
    pub name: String,
    pub body: String,
}

// ---------- Panel ----------

pub fn panel(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::icon_button(ui, icon::PLUS, "Nuevo snippet (con la selección actual)")
                .clicked()
            {
                st.actions.push(Action::NewSnippet);
            }
            ui.add(
                egui::TextEdit::singleline(&mut st.snippet_query)
                    .id_salt("snippet-query")
                    .hint_text(format!("{}  Buscar snippets", icon::MAGNIFYING_GLASS))
                    .desired_width(f32::INFINITY),
            );
        });
    });
    let q = st.snippet_query.trim().to_lowercase();
    let matches = |name: &str, body: &str| {
        q.is_empty() || name.to_lowercase().contains(&q) || body.to_lowercase().contains(&q)
    };
    let mut act: Option<Action> = None;

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            // Propios.
            let mine: Vec<(usize, &UserSnippet)> =
                st.user_snippets.iter().enumerate().filter(|(_, s)| matches(&s.name, &s.body)).collect();
            egui::CollapsingHeader::new(RichText::new(format!("{}  Mis snippets", icon::STAR)).color(pal.text))
                .default_open(true)
                .show(ui, |ui| {
                    if st.user_snippets.is_empty() {
                        ui.label(
                            RichText::new("Selecciona texto en el editor y pulsa + para guardarlo como snippet.")
                                .small()
                                .color(pal.text_faint),
                        );
                    }
                    for (i, s) in mine {
                        let size = egui::vec2(ui.available_width(), 24.0);
                        ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::icon_button(ui, icon::TRASH, "Eliminar").clicked() {
                                act = Some(Action::DeleteSnippet(i));
                            }
                            if widgets::icon_button(ui, icon::PENCIL_SIMPLE, "Editar").clicked() {
                                act = Some(Action::EditSnippet(i));
                            }
                            let w = ui.available_width();
                            if row(ui, &pal, &s.name, &s.body, w).clicked() {
                                act = Some(Action::InsertSnippet(s.body.clone()));
                            }
                        });
                    }
                });
            // De serie.
            for g in BUILTIN {
                let items: Vec<&Snippet> = g.items.iter().filter(|s| matches(s.name, s.body)).collect();
                if items.is_empty() {
                    continue;
                }
                egui::CollapsingHeader::new(RichText::new(format!("{}  {}", g.icon, g.name)).color(pal.text))
                    .default_open(!q.is_empty())
                    .show(ui, |ui| {
                        for s in items {
                            let w = ui.available_width();
                            if row(ui, &pal, s.name, s.body, w).clicked() {
                                act = Some(Action::InsertSnippet(s.body.to_string()));
                            }
                        }
                    });
            }
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(
                    RichText::new("Clic para insertar en el cursor. Los que usan la selección la envuelven.")
                        .small()
                        .color(pal.text_faint),
                )
                .wrap(),
            );
        });
    });
    if let Some(a) = act {
        st.actions.push(a);
    }
}

/// Fila de snippet: nombre a la izquierda, primera línea del cuerpo atenuada.
fn row(
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    name: &str,
    body: &str,
    width: f32,
) -> egui::Response {
    let preview: String = expand(body, "…")
        .0
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(28)
        .collect();
    ui.add_sized(
        [width, 24.0],
        egui::Button::new((
            RichText::new(name).color(pal.text),
            egui::Atom::grow(),
            RichText::new(preview)
                .monospace()
                .small()
                .color(pal.text_faint),
        ))
        .frame_when_inactive(false)
        .truncate(),
    )
    .on_hover_ui(|ui| {
        ui.label(
            RichText::new(expand(body, "⟨selección⟩").0)
                .monospace()
                .color(pal.text),
        );
    })
}

/// Diálogo de creación/edición. Devuelve `Some(guardar)` al cerrarse.
pub fn edit_modal(ctx: &egui::Context, e: &mut Editing) -> Option<bool> {
    let pal = theme::palette(ctx);
    let mut done = None;
    widgets::modal(ctx, "edit-snippet", 480.0, |ui| {
        let title = if e.index.is_some() {
            "Editar snippet"
        } else {
            "Nuevo snippet"
        };
        widgets::modal_header(
            ui,
            &pal,
            icon::BRACKETS_CURLY,
            title,
            Some("Se inserta desde el panel de snippets o el autocompletado."),
        );
        widgets::section_label(ui, &pal, "Nombre");
        ui.add(
            egui::TextEdit::singleline(&mut e.name)
                .hint_text("Mi snippet")
                .margin(widgets::FIELD_MARGIN)
                .desired_width(f32::INFINITY),
        );
        ui.add_space(6.0);
        widgets::section_label(ui, &pal, "Contenido");
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut e.body)
                        .code_editor()
                        .margin(widgets::FIELD_MARGIN)
                        .desired_rows(8)
                        .desired_width(f32::INFINITY),
                );
            });
        ui.label(
            RichText::new("${sel} selección · ${sel:x} selección o x · ${x} relleno · ${} cursor")
                .small()
                .monospace()
                .color(pal.text_dim),
        );
        widgets::modal_footer(ui, |ui| {
            let ok = !e.name.trim().is_empty() && !e.body.is_empty();
            if widgets::primary_button(ui, &pal, "Guardar", ok).clicked() {
                done = Some(true);
            }
            if widgets::secondary_button(ui, &pal, "Cancelar").clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                done = Some(false);
            }
        });
    });
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expande_marcadores() {
        assert_eq!(expand("*${sel:texto}*", "hola"), ("*hola*".into(), 6, 6));
        assert_eq!(expand("*${sel:texto}*", ""), ("*texto*".into(), 1, 6));
        assert_eq!(
            expand("frac(${sel:a}, ${b})", ""),
            ("frac(a, b)".into(), 5, 6)
        );
        assert_eq!(
            expand("frac(${sel:a}, ${b})", "x+1"),
            ("frac(x+1, b)".into(), 10, 11)
        );
        assert_eq!(expand("#v(${}1em)", ""), ("#v(1em)".into(), 3, 3));
        assert_eq!(expand("sin marcas", ""), ("sin marcas".into(), 10, 10));
        // `${sel}` vacío: el cursor queda en su sitio; con selección, al final.
        assert_eq!(
            expand("#text(size: 9pt)[${sel}]", ""),
            ("#text(size: 9pt)[]".into(), 17, 17)
        );
        assert_eq!(expand("*${sel}*", "x"), ("*x*".into(), 3, 3));
        assert_eq!(expand("$${sel:x}$", ""), ("$x$".into(), 1, 2));
        // Offsets en chars, no en bytes.
        assert_eq!(expand("ñ${á}", ""), ("ñá".into(), 1, 2));
    }
}
