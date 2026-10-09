//! Borrador: buffer temporal con vista previa en vivo para construir
//! ecuaciones (modo ecuación) o probar fragmentos y gráficos con primitivas
//! nativas. Se inserta en el documento con un clic. Persiste en la sesión.

use crate::actions::Action;
use crate::app::State;
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

/// Estructuras rápidas para el constructor de ecuaciones: etiqueta e
/// inserto (se coloca en el cursor del borrador).
const QUICK: &[(&str, &str)] = &[
    ("frac", "frac(a, b)"),
    ("∑", "sum_(i=1)^n "),
    ("mat", "mat(a, b; c, d)"),
    ("√", "sqrt(x)"),
    ("casos", "cases(1 &\"si\" x > 0, 0 &\"si no\")"),
];

pub struct Scratch {
    pub text: String,
    pub equation: bool,
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
                    .unwrap_or_else(|_| Err("error interno del compilador (panic)".into()));
                    if tx_out.send(Done { key: job.key, result }).is_err() {
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
    pub fn from_session(text: String, equation: bool) -> Self {
        Self {
            text,
            equation,
            ..Default::default()
        }
    }
}

// ---------- Lógica pura (testeable) ----------

/// Fuente completo para la vista previa: página ajustada al contenido.
/// En modo ecuación envuelve en `$…$`, salvo que el texto parezca marcado
/// (una línea que empieza por `#código`: `#let`, `#set`…), que va tal cual.
pub fn preview_source(text: &str, equation: bool) -> String {
    let mut src = String::from("#set page(width: auto, height: auto, margin: 10pt)\n");
    if equation && !looks_like_markup(text) {
        src.push_str("$ ");
        src.push_str(strip_math(text));
        src.push_str(" $");
    } else {
        src.push_str(text);
    }
    src.push('\n');
    src
}

/// ¿Parece marcado en vez de mate? Una línea que empieza por `#` + letra
/// (`#let`, `#set`, `#rect`…) es código de bloque: envolverla en `$…$`
/// rompería (p. ej. snippets de cálculo en modo ecuación).
fn looks_like_markup(text: &str) -> bool {
    text.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with('#') && t[1..].starts_with(|c: char| c.is_alphabetic())
    })
}

/// Texto a insertar en el documento (`None` si está en blanco). En modo
/// ecuación se pela un nivel de `$…$` y se envuelve, salvo marcado
/// (igual que la vista previa).
pub fn insert_source(text: &str, equation: bool) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    Some(if equation && !looks_like_markup(t) {
        let inner = strip_math(t);
        if inner.is_empty() {
            return None;
        }
        format!("${inner}$")
    } else {
        t.to_string()
    })
}

/// Quita un nivel de `$…$` exterior (con espacios). Si no hay, devuelve el
/// texto tal cual (puede quedar vacío si solo había dólares).
fn strip_math(s: &str) -> &str {
    let t = s.trim();
    t.strip_prefix('$')
        .and_then(|r| r.strip_suffix('$'))
        .map(str::trim)
        .unwrap_or(t)
}

/// Inserta `snippet` en `cursor` (índices de char). Devuelve el texto nuevo
/// y el cursor tras lo insertado.
pub fn splice_at(text: &str, cursor: usize, snippet: &str) -> (String, usize) {
    let n = text.chars().count();
    let c = cursor.min(n);
    let mut out = String::new();
    out.extend(text.chars().take(c));
    out.push_str(snippet);
    out.extend(text.chars().skip(c));
    (out, c + snippet.chars().count())
}

// ---------- Panel ----------

pub fn panel(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::section_label(ui, &pal, "Borrador");
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
                    let s = &mut st.scratch;
                    ui.checkbox(&mut s.equation, "Como ecuación")
                        .on_hover_text("La vista previa y la inserción envuelven en $…$");
                    // Editor multilínea con resaltado (sin scroll propio: el
                    // scroll es el del panel; un ScrollArea anidado rompería
                    // el layout de lo que viene debajo).
                    let font_size = st.prefs.font_size;
                    let font = egui::FontId::monospace(font_size);
                    let mut layouter = |ui: &egui::Ui,
                                        buf: &dyn egui::TextBuffer,
                                        wrap_width: f32| {
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
                    // Inserción rápida en el cursor del borrador + biblioteca.
                    let mut picked: Option<&'static str> = None;
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.menu_button(format!("{}  Añadir snippet", icon::BRACKETS_CURLY), |ui| {
                            for g in crate::snippets::BUILTIN {
                                ui.menu_button(
                                    format!("{}  {}", g.icon, g.name),
                                    |ui| {
                                        for s in g.items {
                                            if ui.button(s.name).clicked() {
                                                picked = Some(s.body);
                                                ui.close();
                                            }
                                        }
                                    },
                                );
                            }
                        });
                        for (label, snippet) in QUICK {
                            if ui
                                .small_button(RichText::new(*label).monospace())
                                .on_hover_text(*snippet)
                                .clicked()
                            {
                                insert_at_cursor(ui, &out, &mut s.text, snippet);
                            }
                        }
                    });
                    if let Some(body) = picked {
                        let eq = s.equation;
                        apply_snippet(ui, &out, &mut s.text, body, eq);
                    }
                    // Vista previa en vivo (con debounce).
                    let root = st.docs.cur().root_dir();
                    let src = preview_source(&s.text, s.equation);
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
                            RichText::new("Escribe una ecuación o un fragmento para verlo aquí.")
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
                                ui.add_space(
                                    ((ui.available_width() - size.x) / 2.0).max(0.0),
                                );
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
                        let can = insert_source(&s.text, s.equation).is_some();
                        ui.add_enabled_ui(can, |ui| {
                            let go = widgets::accent_button(
                                ui,
                                &pal,
                                &format!("{}  Insertar en el documento", icon::ARROW_BEND_DOWN_RIGHT),
                            )
                            .on_hover_text("Inserta en el cursor del editor")
                            .clicked();
                            if go && let Some(body) = insert_source(&s.text, s.equation) {
                                st.actions.push(Action::InsertSnippet(body));
                            }
                        });
                        if ui
                            .small_button("Limpiar")
                            .on_hover_text("Vacía el borrador")
                            .clicked()
                        {
                            s.text.clear();
                            s.error = None;
                        }
                    });
                });
        });
}

/// Inserta `snippet` en el cursor del editor del borrador (o al final).
fn insert_at_cursor(
    ui: &egui::Ui,
    out: &egui::text_edit::TextEditOutput,
    text: &mut String,
    snippet: &str,
) {
    let len = text.chars().count();
    // `state.cursor` y no `cursor_range`: este solo se rellena con foco, y
    // el clic en el botón ya lo quitó.
    let c = out
        .state
        .cursor
        .char_range()
        .map(|r| r.primary.index.0)
        .unwrap_or(len)
        .min(len);
    let (t, nc) = splice_at(text, c, snippet);
    *text = t;
    let mut state = out.state.clone();
    state
        .cursor
        .set_char_range(Some(CCursorRange::two(CCursor::new(nc), CCursor::new(nc))));
    state.store(ui.ctx(), out.response.id);
    out.response.request_focus();
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
    // En modo ecuación pela los `$…$` del snippet y el cursor va al final
    // (los marcadores ya se resolvieron al pelar).
    let (expanded, (oa, ob)) = if equation {
        let e = strip_math(&expanded).to_string();
        let n = e.chars().count();
        (e, (n, n))
    } else {
        (expanded, (na, nb))
    };
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

    #[test]
    fn envuelve_fuente() {
        assert_eq!(
            preview_source("x^2", true),
            "#set page(width: auto, height: auto, margin: 10pt)\n$ x^2 $\n"
        );
        assert_eq!(
            preview_source("#rect(width: 2cm)", false),
            "#set page(width: auto, height: auto, margin: 10pt)\n#rect(width: 2cm)\n"
        );
    }

    #[test]
    fn inserto() {
        assert_eq!(insert_source("  x^2  ", true).as_deref(), Some("$x^2$"));
        assert_eq!(
            insert_source("hola", false).as_deref(),
            Some("hola")
        );
        assert!(insert_source("  ", true).is_none());
    }

    #[test]
    fn marcado_no_se_envuelve() {
        assert!(looks_like_markup("#let a = 2\n$x$"));
        assert!(looks_like_markup("  #set text(12pt)"));
        assert!(!looks_like_markup("$ x^2 $\n$ y $"));
        assert!(!looks_like_markup("#(a + b)"));
        let calc = "#let a = 2\n$Delta = #a$";
        assert_eq!(
            preview_source(calc, true),
            "#set page(width: auto, height: auto, margin: 10pt)\n#let a = 2\n$Delta = #a$\n"
        );
        assert_eq!(insert_source(calc, true).as_deref(), Some(calc));
    }

    #[test]
    fn pela_dolares_una_vez() {
        // Snippets con sus propios `$…$` en modo ecuación: sin duplicar.
        assert_eq!(
            insert_source("$ sum_(i = 1)^n a_i $", true).as_deref(),
            Some("$sum_(i = 1)^n a_i$")
        );
        assert_eq!(
            preview_source("$ sum_(i = 1)^n a_i $", true),
            "#set page(width: auto, height: auto, margin: 10pt)\n$ sum_(i = 1)^n a_i $\n"
        );
        assert_eq!(strip_math("  $$x$$  "), "$x$");
        assert_eq!(strip_math("x^2"), "x^2");
        assert!(insert_source("$  $", true).is_none());
    }

    #[test]
    fn empalma_en_cursor() {
        assert_eq!(splice_at("ab", 1, "X"), ("aXb".to_string(), 2));
        assert_eq!(splice_at("ab", 0, "X"), ("Xab".to_string(), 1));
        assert_eq!(splice_at("ab", 99, "X"), ("abX".to_string(), 3));
        assert_eq!(splice_at("", 0, "frac(a, b)"), ("frac(a, b)".to_string(), 10));
    }

    #[test]
    fn renderiza_ecuacion_y_falla_con_error() {
        let dir = std::env::temp_dir();
        let src = preview_source("x^2 + frac(a, b)", true);
        let img = crate::engine::render_preview(&src, &dir, 2.0).expect("debería renderizar");
        assert!(!img.pixels.is_empty());
        assert!(img.size[0] > 0 && img.size[1] > 0);
        let bad = preview_source("#funcion_inexistente(1)", false);
        let err = crate::engine::render_preview(&bad, &dir, 2.0).expect_err("debería fallar");
        assert!(!err.is_empty(), "el error no debe venir vacío");
    }
}
