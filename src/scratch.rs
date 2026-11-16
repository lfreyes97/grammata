//! Borrador: buffer temporal con vista previa en vivo para construir
//! ecuaciones (modo ecuación) o probar fragmentos y gráficos con primitivas
//! nativas. Se inserta en el documento con un clic. Persiste en la sesión.

use crate::i18n::{tr, trf};
use crate::actions::Action;
use crate::app::State;
use crate::doc::DocId;
use crate::highlight::EquationAt;
use crate::theme;
use crate::widgets;
use egui::RichText;
use egui::text::{CCursor, CCursorRange};
use egui_phosphor::regular as icon;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// Espera tras editar antes de re-renderizar.
const DEBOUNCE: Duration = Duration::from_millis(300);
/// Resolución del render (px por punto).
const SCALE: f32 = 2.0;

pub struct Scratch {
    pub text: String,
    pub equation: bool,
    /// Ecuación en bloque (`$ x $`) o en línea (`$x$`): vista previa e
    /// inserción usan la misma.
    pub block: bool,
    /// Ecuación del documento que se está editando: Insertar la reemplaza.
    pub editing: Option<Editing>,
    /// Ecuación bajo el cursor del documento, por (doc, revisión, cursor).
    at_cursor: Option<(AtKey, Option<EquationAt>)>,
    tex: Option<egui::TextureHandle>,
    /// Clave renderizada y clave actual (fuente + modo): si difieren, hay
    /// cambios pendientes (con debounce).
    rendered: String,
    current: String,
    last_edit: Instant,
    pub error: Option<String>,
    /// Hilo de render (se crea al primer uso) y clave pedida pendiente.
    worker: Option<Worker>,
    pending: Option<String>,
}

impl Default for Scratch {
    fn default() -> Self {
        Self {
            text: String::new(),
            equation: true,
            block: true,
            editing: None,
            at_cursor: None,
            tex: None,
            rendered: String::new(),
            current: String::new(),
            last_edit: Instant::now(),
            error: None,
            worker: None,
            pending: None,
        }
    }
}

/// Ecuación del documento cargada en el borrador.
#[derive(Clone, Debug, PartialEq)]
pub struct Editing {
    pub doc: DocId,
    /// Inicio en chars del `$…$` y su texto al cargarla: al reemplazar se
    /// busca ese texto (por si el documento cambió entretanto).
    pub start: usize,
    pub original: String,
}

type AtKey = (DocId, u64, usize);

// ---------- Render en segundo plano ----------

struct Job {
    key: String,
    src: String,
    root: PathBuf,
}

struct Done {
    key: String,
    result: Result<egui::ColorImage, String>,
}

/// Compila y renderiza fuera del hilo de la interfaz: un `@preview` que se
/// descarga o un fragmento pesado no congelan la app.
struct Worker {
    tx: Sender<Job>,
    rx: Receiver<Done>,
}

impl Worker {
    fn new(ctx: egui::Context) -> Self {
        let (tx, rx_job) = channel::<Job>();
        let (tx_out, rx) = channel::<Done>();
        std::thread::Builder::new()
            .name("typst-scratch".into())
            // Como `compile.rs`: el compilador evalúa recursivamente.
            .stack_size(64 << 20)
            .spawn(move || {
                while let Ok(mut job) = rx_job.recv() {
                    // Si se encolaron varias, solo cuenta la última.
                    while let Ok(newer) = rx_job.try_recv() {
                        job = newer;
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        crate::engine::render_preview(&job.src, &job.root, SCALE)
                    }))
                    .unwrap_or_else(|_| Err(tr("error interno del compilador (panic)").into()));
                    if tx_out
                        .send(Done {
                            key: job.key,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("hilo typst-scratch");
        Self { tx, rx }
    }
}

impl Scratch {
    pub fn from_session(text: String, equation: bool, block: bool) -> Self {
        Self {
            text,
            equation,
            block,
            ..Default::default()
        }
    }
}

// ---------- Lógica pura (testeable) ----------

/// Interior de la ecuación del borrador, o `None` si es marcado (según el
/// parser de typst):
/// - código al principio de una línea (`#let`, `#set`, `#rect`…): marcado;
/// - solo ecuaciones `$…$`: su interior (varias, unidas con `\`);
/// - texto con ecuaciones sueltas (`$a$ y $b$`): marcado;
/// - sin `$`: el texto es la ecuación.
pub fn equation_inner(text: &str) -> Option<String> {
    use typst_syntax::SyntaxKind as K;
    let t = text.trim();
    let root = typst_syntax::parse(t);
    let kids: Vec<_> = root.children().collect();
    let mut line_start = true;
    for (i, n) in kids.iter().enumerate() {
        if n.kind() == K::Hash
            && line_start
            && kids.get(i + 1).is_some_and(|next| {
                !matches!(
                    next.kind(),
                    K::Parenthesized | K::CodeBlock | K::ContentBlock
                )
            })
        {
            return None;
        }
        line_start = match n.kind() {
            K::Space => n.leaf_text().contains('\n'),
            K::Parbreak => true,
            _ => false,
        };
    }
    if let Some((inner, _, _)) = crate::snippets::strip_equations(t, 0, 0) {
        return Some(inner);
    }
    if kids.iter().any(|n| n.kind() == K::Equation) {
        return None;
    }
    Some(t.to_string())
}

/// `$ x $` o `$x$`.
fn wrap(inner: &str, block: bool) -> String {
    if block {
        format!("$ {inner} $")
    } else {
        format!("${inner}$")
    }
}

/// Fuente completo para la vista previa: página ajustada al contenido. En
/// modo ecuación, la ecuación tal como se insertará (en bloque o en línea).
pub fn preview_source(text: &str, equation: bool, block: bool) -> String {
    let mut src = String::from("#set page(width: auto, height: auto, margin: 10pt)\n");
    match equation.then(|| equation_inner(text)).flatten() {
        Some(inner) => src.push_str(&wrap(&inner, block)),
        None => src.push_str(text),
    }
    src.push('\n');
    src
}

/// Texto a insertar en el documento (`None` si está en blanco).
pub fn insert_source(text: &str, equation: bool, block: bool) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    Some(match equation.then(|| equation_inner(t)).flatten() {
        Some(inner) if inner.trim().is_empty() => return None,
        Some(inner) => wrap(&inner, block),
        None => t.to_string(),
    })
}

// ---------- Panel ----------

pub fn panel(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::section_label(ui, &pal, tr("Borrador"));
        });
    });
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 8.0;
                    // Ecuación bajo el cursor del documento (por revisión).
                    let at = {
                        let d = st.docs.cur();
                        let key = (d.id, d.rev, st.editor.cursor.unwrap_or(usize::MAX));
                        match &st.scratch.at_cursor {
                            Some((k, v)) if *k == key => v.clone(),
                            _ => {
                                let v = st
                                    .editor
                                    .cursor
                                    .and_then(|c| crate::highlight::equation_at(&d.text, c));
                                st.scratch.at_cursor = Some((key, v.clone()));
                                v
                            }
                        }
                    };
                    let root = st.compile_root();
                    let s = &mut st.scratch;
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut s.equation, tr("Como ecuación"))
                            .on_hover_text(tr("La vista previa y la inserción envuelven en $…$"));
                        if s.equation {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.selectable_value(&mut s.block, true, tr("Bloque"))
                                        .on_hover_text(tr("$ x $: centrada, en su propia línea"));
                                    ui.selectable_value(&mut s.block, false, tr("En línea"))
                                        .on_hover_text(tr("$x$: dentro del párrafo"));
                                },
                            );
                        }
                    });
                    // Editar una ecuación del documento: se carga aquí e
                    // Insertar la reemplaza.
                    if let Some(e) = &s.editing {
                        let name = st
                            .docs
                            .get(e.doc)
                            .map(|d| d.fname())
                            .unwrap_or_default();
                        let mut stop = false;
                        egui::Frame::new()
                            .fill(pal.accent_soft)
                            .corner_radius(6)
                            .inner_margin(egui::Margin::symmetric(8, 5))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(trf!("{}  Editando una ecuación de «{}»", icon::PENCIL_SIMPLE, name))
                                        .small()
                                        .color(pal.text),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            stop = ui
                                                .small_button(icon::X)
                                                .on_hover_text(
                                                    tr("Dejar de editar: Insertar volverá a añadir en el cursor"),
                                                )
                                                .clicked();
                                        },
                                    );
                                });
                            });
                        if stop {
                            s.editing = None;
                        }
                    } else if let Some(eq) = &at
                        && ui
                            .button(trf!("{}  Editar la ecuación del cursor", icon::PENCIL_SIMPLE))
                            .on_hover_text(tr("La carga en el borrador; Insertar la reemplaza"))
                            .clicked()
                    {
                        let d = st.docs.cur();
                        s.text = eq.inner.clone();
                        s.equation = true;
                        s.block = eq.block;
                        s.editing = Some(Editing {
                            doc: d.id,
                            start: eq.start,
                            original: d.text.chars().skip(eq.start).take(eq.end - eq.start).collect(),
                        });
                    }
                    // Editor multilínea con resaltado (sin scroll propio: el
                    // scroll es el del panel; un ScrollArea anidado rompería
                    // el layout de lo que viene debajo).
                    let font_size = st.prefs.font_size;
                    let font = egui::FontId::monospace(font_size);
                    let mut layouter =
                        |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
                            let mut job =
                                crate::highlight::layout_job(buf.as_str(), font.clone(), &pal);
                            job.wrap.max_width = wrap_width;
                            ui.fonts_mut(|f| f.layout_job(job))
                        };
                    let out = egui::TextEdit::multiline(&mut s.text)
                        .font(font.clone())
                        .code_editor()
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter)
                        .show(ui);
                    // Biblioteca (en modo ecuación, solo los grupos de
                    // matemáticas) + estructuras de Símbolos en el cursor.
                    let mut picked: Option<&'static str> = None;
                    let eq = s.equation;
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.menu_button(trf!("{}  Añadir snippet", icon::BRACKETS_CURLY), |ui| {
                            for g in crate::snippets::BUILTIN {
                                // En modo ecuación, solo lo que el parser ve
                                // como ecuación (o código que acaba en una).
                                let items: Vec<_> = g
                                    .items
                                    .iter()
                                    .filter(|s| !eq || crate::snippets::mode(s.body).for_equation())
                                    .collect();
                                if items.is_empty() {
                                    continue;
                                }
                                ui.menu_button(format!("{}  {}", g.icon, g.name), |ui| {
                                    for s in items {
                                        if ui.button(s.name).clicked() {
                                            picked = Some(s.body);
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        });
                        if eq {
                            for sym in crate::symbols::STRUCTURES.items {
                                let label = RichText::new(sym.label)
                                    .family(egui::FontFamily::Name("ui-math".into()));
                                if ui.small_button(label).on_hover_text(sym.name).clicked() {
                                    picked = Some(sym.insert);
                                }
                            }
                        }
                    });
                    if let Some(body) = picked {
                        apply_snippet(ui, &out, &mut s.text, body, eq);
                    }
                    // Vista previa en vivo (con debounce).
                    let src = preview_source(&s.text, s.equation, s.block);
                    let key = format!("{src}|{SCALE:.1}");
                    if key != s.current {
                        s.current = key.clone();
                        s.last_edit = Instant::now();
                        // El modo pudo cambiar con texto en blanco: olvida el
                        // error anterior para no mostrarlo caducado.
                        if s.text.trim().is_empty() {
                            s.error = None;
                        }
                    }
                    if s.text.trim().is_empty() {
                        ui.label(
                            RichText::new(tr("Escribe una ecuación o un fragmento para verlo aquí."))
                                .small()
                                .color(pal.text_faint),
                        );
                    } else {
                        // Resultados del hilo (puede llegar uno ya viejo:
                        // se muestra igual y abajo se pide el actual).
                        if let Some(w) = &s.worker {
                            while let Ok(done) = w.rx.try_recv() {
                                match done.result {
                                    Ok(img) => {
                                        s.tex = Some(ui.ctx().load_texture(
                                            "scratch-preview",
                                            img,
                                            egui::TextureOptions::LINEAR,
                                        ));
                                        s.error = None;
                                    }
                                    Err(e) => s.error = Some(e),
                                }
                                if s.pending.as_ref() == Some(&done.key) {
                                    s.pending = None;
                                }
                                s.rendered = done.key;
                            }
                        }
                        if key != s.rendered && s.pending.as_ref() != Some(&key) {
                            if s.last_edit.elapsed() >= DEBOUNCE {
                                let w = s
                                    .worker
                                    .get_or_insert_with(|| Worker::new(ui.ctx().clone()));
                                let job = Job {
                                    key: key.clone(),
                                    src,
                                    root,
                                };
                                if w.tx.send(job).is_ok() {
                                    s.pending = Some(key);
                                }
                            } else {
                                ui.ctx()
                                    .request_repaint_after(DEBOUNCE - s.last_edit.elapsed());
                            }
                        }
                        if let Some(e) = &s.error {
                            ui.label(RichText::new(e).small().color(pal.err));
                        }
                        if let Some(tex) = &s.tex {
                            // Centrada sin pedir altura infinita al scroll.
                            let mut size = tex.size_vec2() / SCALE;
                            let max_w = (ui.available_width() - 8.0).max(40.0);
                            if size.x > max_w {
                                size *= max_w / size.x;
                            }
                            ui.horizontal(|ui| {
                                ui.add_space(((ui.available_width() - size.x) / 2.0).max(0.0));
                                ui.add(
                                    egui::Image::new(tex)
                                        .fit_to_exact_size(size)
                                        .corner_radius(6.0),
                                );
                            });
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        let can = insert_source(&s.text, s.equation, s.block).is_some();
                        let label = if s.editing.is_some() {
                            tr("Reemplazar en el documento")
                        } else {
                            tr("Insertar en el documento")
                        };
                        ui.add_enabled_ui(can, |ui| {
                            let go = widgets::accent_button(
                                ui,
                                &pal,
                                &format!("{}  {label}", icon::ARROW_BEND_DOWN_RIGHT),
                            )
                            .on_hover_text(if s.editing.is_some() {
                                tr("Sustituye la ecuación que se está editando")
                            } else {
                                tr("Inserta en el cursor del editor")
                            })
                            .clicked();
                            if go && let Some(body) = insert_source(&s.text, s.equation, s.block)
                            {
                                st.actions.push(match &s.editing {
                                    Some(e) => Action::ReplaceText {
                                        doc: e.doc,
                                        start: e.start,
                                        expect: e.original.clone(),
                                        text: body,
                                    },
                                    None => Action::InsertSnippet(body),
                                });
                            }
                        });
                        if ui
                            .small_button(tr("Limpiar"))
                            .on_hover_text(tr("Vacía el borrador"))
                            .clicked()
                        {
                            s.text.clear();
                            s.error = None;
                            s.editing = None;
                        }
                    });
                });
        });
}

/// Aplica un snippet de la biblioteca en la selección del borrador (como
/// `App::insert` pero sobre el buffer temporal). En modo ecuación pela los
/// `$…$` que traiga el snippet: el borrador ya envuelve.
fn apply_snippet(
    ui: &egui::Ui,
    out: &egui::text_edit::TextEditOutput,
    text: &mut String,
    body: &str,
    equation: bool,
) {
    let len = text.chars().count();
    let (a, b) = out
        .state
        .cursor
        .char_range()
        .map(|r| (r.primary.index.0.min(len), r.secondary.index.0.min(len)))
        .unwrap_or((len, len));
    let (sa, sb) = (a.min(b), a.max(b));
    let sel: String = text.chars().skip(sa).take(sb - sa).collect();
    let (expanded, na, nb) = crate::snippets::expand(body, &sel);
    // En modo ecuación, si el snippet son solo ecuaciones, va su interior:
    // el borrador ya envuelve.
    let stripped = if equation {
        crate::snippets::strip_equations(&expanded, na, nb)
    } else {
        None
    };
    let (expanded, oa, ob) = stripped.unwrap_or((expanded, na, nb));
    let (ba, bb) = (
        crate::text::char_to_byte(text, sa),
        crate::text::char_to_byte(text, sb),
    );
    text.replace_range(ba..bb, &expanded);
    let mut state = out.state.clone();
    state.cursor.set_char_range(Some(CCursorRange::two(
        CCursor::new(sa + oa),
        CCursor::new(sa + ob),
    )));
    state.store(ui.ctx(), out.response.id);
    out.response.request_focus();
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "#set page(width: auto, height: auto, margin: 10pt)\n";

    #[test]
    fn envuelve_fuente_en_bloque_o_en_linea() {
        assert_eq!(
            preview_source("x^2", true, true),
            format!("{PAGE}$ x^2 $\n")
        );
        assert_eq!(preview_source("x^2", true, false), format!("{PAGE}$x^2$\n"));
        assert_eq!(
            preview_source("#rect(width: 2cm)", false, true),
            format!("{PAGE}#rect(width: 2cm)\n")
        );
    }

    #[test]
    fn inserto_igual_que_la_vista_previa() {
        assert_eq!(
            insert_source("  x^2  ", true, false).as_deref(),
            Some("$x^2$")
        );
        assert_eq!(
            insert_source("  x^2  ", true, true).as_deref(),
            Some("$ x^2 $")
        );
        assert_eq!(insert_source("hola", false, true).as_deref(), Some("hola"));
        assert!(insert_source("  ", true, true).is_none());
        assert!(insert_source("$  $", true, true).is_none());
    }

    #[test]
    fn decide_con_el_parser() {
        // Código al principio de línea: marcado tal cual.
        assert_eq!(equation_inner("#let a = 2\n$x$"), None);
        assert_eq!(equation_inner("  #set text(12pt)"), None);
        // `#(…)` es una expresión dentro de la ecuación, no marcado.
        assert_eq!(equation_inner("#(a + b)").as_deref(), Some("#(a + b)"));
        // Un `#` en mitad de la línea tampoco.
        assert_eq!(
            equation_inner("x = #calc.pow(2, 3)").as_deref(),
            Some("x = #calc.pow(2, 3)")
        );
        // Ecuaciones: su interior; varias, unidas.
        assert_eq!(
            equation_inner("$ sum_(i = 1)^n a_i $").as_deref(),
            Some("sum_(i = 1)^n a_i")
        );
        assert_eq!(equation_inner("$ a $\n$ b $").as_deref(), Some("a \\\nb"));
        // Texto con ecuaciones sueltas: marcado (antes salía `a$ + $b`).
        assert_eq!(equation_inner("$a$ + $b$"), None);
        let calc = "#let a = 2\n$Delta = #a$";
        assert_eq!(preview_source(calc, true, true), format!("{PAGE}{calc}\n"));
        assert_eq!(insert_source(calc, true, true).as_deref(), Some(calc));
    }

    #[test]
    fn renderiza_ecuacion_y_falla_con_error() {
        let dir = std::env::temp_dir();
        let src = preview_source("x^2 + frac(a, b)", true, true);
        let img = crate::engine::render_preview(&src, &dir, 2.0).expect("debería renderizar");
        assert!(!img.pixels.is_empty());
        assert!(img.size[0] > 0 && img.size[1] > 0);
        let bad = preview_source("#funcion_inexistente(1)", false, true);
        let err = crate::engine::render_preview(&bad, &dir, 2.0).expect_err("debería fallar");
        assert!(!err.is_empty(), "el error no debe venir vacío");
    }
}
