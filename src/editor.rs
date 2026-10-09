//! Pestaña de documento: editor de código, autocompletado y barra de búsqueda.

use crate::app::State;
use crate::doc::DocId;
use crate::syntax;
use crate::text;
use crate::theme::{self, Palette};
use crate::widgets;
use egui::text::{CCursor, CCursorRange};
use egui::{Key, Modifiers, RichText};
use egui_phosphor::regular as icon;

/// Estado del editor compartido entre pestañas (solo el doc actual tiene cursor).
#[derive(Default)]
pub struct EditorUi {
    /// Cursor (índice de char) en el doc actual.
    pub cursor: Option<usize>,
    /// Selección `[inicio, fin)` en chars del doc actual (vacía = cursor).
    pub selection: Option<(usize, usize)>,
    /// Selección pendiente de aplicar en un doc.
    pub goto: Option<Goto>,
    /// Doc cuyo editor tenía el foco en el frame anterior.
    pub focused: Option<DocId>,
    pub ac: Autocomplete,
    pub find: Find,
    /// F3 / Mayús+F3 pendiente (se resuelve con acceso al doc).
    pub pending_step: Option<i32>,
    /// Ancla de la barra de formato: (doc, posición en pantalla del final de
    /// la selección). La escribe `doc_tab`; la lee `formatbar::show`.
    pub sel_anchor: Option<(DocId, egui::Pos2)>,
}

#[derive(Clone, Copy)]
pub struct Goto {
    pub doc: DocId,
    pub start: usize,
    pub end: usize,
    /// Dar el foco al editor (no se hace desde la búsqueda: lo perdería la caja).
    pub focus: bool,
}

#[derive(Default)]
pub struct Autocomplete {
    pub selected: usize,
    /// Ctrl+Espacio: muestra la lista aunque no haya `#prefijo`.
    pub forced: bool,
    /// Esc sobre un `#` concreto: no reabrir hasta salir de él.
    dismissed: Option<(DocId, usize)>,
}

#[derive(Default)]
pub struct Find {
    pub open: bool,
    pub query: String,
    pub match_case: bool,
    pub want_focus: bool,
    /// Coincidencias cacheadas: (doc, rev, query, mayúsculas) → rangos.
    cache_key: Option<(DocId, u64, String, bool)>,
    matches: Vec<(usize, usize)>,
    /// Índice de la coincidencia seleccionada.
    pub current: Option<usize>,
}

impl Find {
    fn refresh(&mut self, doc: DocId, rev: u64, text: &str) {
        let key = (doc, rev, self.query.clone(), self.match_case);
        if self.cache_key.as_ref() != Some(&key) {
            self.matches = text::find_all(text, &self.query, self.match_case);
            self.cache_key = Some(key);
            self.current = None;
        }
    }
}

/// `#prefijo` justo antes del cursor: (byte del `#`, prefijo).
fn hash_prefix(text: &str, cursor: usize) -> Option<(usize, String)> {
    let end = text::char_to_byte(text, cursor);
    let head = &text[..end];
    let start = head.len().saturating_sub(32);
    let start = (start..head.len()).find(|&b| head.is_char_boundary(b)).unwrap_or(head.len());
    let h = start + head[start..].rfind('#')?;
    let between = &head[h + 1..];
    if between.len() > 24 || between.chars().any(|c| !(c.is_alphanumeric() || c == '-' || c == '_')) {
        return None;
    }
    Some((h, between.to_string()))
}

fn insert_completion(st: &mut State, di: usize, hash_byte: usize, c: &AcItem) {
    let cursor = st.editor.cursor.unwrap_or(0);
    let doc = &mut st.docs.list[di];
    let cb = text::char_to_byte(&doc.text, cursor);
    if hash_byte > cb || !doc.text.is_char_boundary(hash_byte) {
        return;
    }
    let raw = completion_replacement(&c.snippet);
    // Los snippets con marcadores `${…}` se expanden (selección actual como
    // `${sel}`); si no, el hueco `..` marca dónde queda el cursor.
    let (replacement, sel) = if raw.contains("${") {
        let sel_text = match st.editor.selection {
            Some((a, b)) if a < b => doc.text.chars().skip(a).take(b - a).collect(),
            _ => String::new(),
        };
        let (text, sa, sb) = crate::snippets::expand(&raw, &sel_text);
        (text, Some((sa, sb)))
    } else {
        (raw, None)
    };
    doc.text.replace_range(hash_byte..cb, &replacement);
    doc.touch();
    let start = doc.text[..hash_byte].chars().count();
    let (sel_start, sel_end) = match sel {
        Some((sa, sb)) => (start + sa, start + sb),
        None => match replacement.find("..") {
            Some(b) => {
                let s = start + replacement[..b].chars().count();
                (s, s + 2)
            }
            None => {
                let e = start + replacement.chars().count();
                (e, e)
            }
        },
    };
    st.editor.goto = Some(Goto { doc: doc.id, start: sel_start, end: sel_end, focus: true });
    st.editor.ac.forced = false;
    st.needs_persist = true;
}

/// Cuerpo listo para sustituir el `#prefijo`: `#` delante solo si empieza
/// por nombre (función/variable). El resto (`*negrita*`, `#link`,
/// `/ término`, `= Tít`, …) va tal cual.
fn completion_replacement(snippet: &str) -> String {
    if snippet.starts_with(|ch: char| ch.is_alphanumeric() || ch == '_') {
        format!("#{snippet}")
    } else {
        snippet.to_string()
    }
}

/// Ítem del popup: de serie (`detail` fija) o propio (`detail` marcada).
struct AcItem {
    name: String,
    snippet: String,
    detail: String,
}

/// De serie por prefijo/subcadena (como `syntax::matching`) + propios por
/// subcadena en nombre o cuerpo. Los propios van tras los de serie.
fn ac_items(st: &State, prefix: &str) -> Vec<AcItem> {
    let p = prefix.to_lowercase();
    let mut out: Vec<AcItem> = syntax::matching(prefix)
        .iter()
        .map(|c| AcItem { name: c.name.to_string(), snippet: c.snippet.to_string(), detail: c.detail.to_string() })
        .collect();
    if !p.is_empty() {
        out.extend(
            st.user_snippets
                .iter()
                .filter(|s| s.name.to_lowercase().contains(&p) || s.body.to_lowercase().contains(&p))
                .map(|s| AcItem {
                    name: s.name.clone(),
                    snippet: s.body.clone(),
                    detail: "snippet propio".to_string(),
                }),
        );
    } else {
        // Popup forzado (Ctrl+Espacio): todos los propios al final.
        out.extend(st.user_snippets.iter().map(|s| AcItem {
            name: s.name.clone(),
            snippet: s.body.clone(),
            detail: "snippet propio".to_string(),
        }));
    }
    out
}

pub fn doc_tab(st: &mut State, ui: &mut egui::Ui, id: DocId) {
    let Some(di) = st.docs.index_of(id) else {
        ui.label("Documento cerrado.");
        return;
    };
    let pal = theme::palette(ui.ctx());
    let is_current = di == st.docs.current;
    let had_focus = st.editor.focused == Some(id);

    // ---- Autocompletado: teclas ANTES de que el editor las vea ----
    let prefix = if is_current && had_focus {
        st.editor.cursor.and_then(|c| hash_prefix(&st.docs.list[di].text, c))
    } else {
        None
    };
    if let (Some((h, _)), Some(d)) = (&prefix, st.editor.ac.dismissed) {
        if d != (id, *h) {
            st.editor.ac.dismissed = None;
        }
    } else if prefix.is_none() {
        st.editor.ac.dismissed = None;
    }
    let dismissed = st.editor.ac.dismissed.is_some();
    let items: Vec<AcItem> = match &prefix {
        Some((_, p)) if !dismissed => ac_items(st, p),
        _ if st.editor.ac.forced && had_focus => ac_items(st, ""),
        _ => Vec::new(),
    };
    let popup = !items.is_empty();
    if popup {
        let n = items.len();
        let sel = &mut st.editor.ac.selected;
        *sel = (*sel).min(n - 1);
        let (esc, down, up, accept) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Tab) || i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if down {
            *sel = (*sel + 1) % n;
        }
        if up {
            *sel = (*sel + n - 1) % n;
        }
        if esc {
            st.editor.ac.forced = false;
            if let Some((h, _)) = prefix {
                st.editor.ac.dismissed = Some((id, h));
            }
        } else if accept {
            let c = &items[*sel];
            let h = match prefix {
                Some((h, _)) => h,
                None => text::char_to_byte(&st.docs.list[di].text, st.editor.cursor.unwrap_or(0)),
            };
            insert_completion(st, di, h, c);
        }
    } else {
        st.editor.ac.selected = 0;
    }

    // ---- Editor ----
    let font = st.prefs.font_size;
    let wrap = st.prefs.wrap;
    let row_h = ui.ctx().fonts_mut(|f| f.row_height(&egui::FontId::monospace(font)));
    let rows = ((ui.available_height() / row_h).floor() as usize).max(4);
    let view = ui.max_rect();
    let doc = &mut st.docs.list[di];
    let opt = crate::code_view::Options { font_size: font, wrap, rows };
    let out = crate::code_view::show(ui, &format!("editor-{id}"), &mut doc.text, &opt, &pal);
    if out.response.changed() {
        doc.touch();
        st.needs_persist = true;
    }
    let has_focus = out.response.has_focus();
    if has_focus {
        st.editor.focused = Some(id);
    } else if st.editor.focused == Some(id) && !popup {
        st.editor.focused = None;
        st.editor.ac.forced = false;
    }
    if is_current && let Some(r) = out.cursor_range {
        let (a, b) = (r.primary.index.0, r.secondary.index.0);
        st.editor.cursor = Some(a);
        st.editor.selection = Some((a.min(b), a.max(b)));
        // Ancla de la barra de formato (coords de pantalla del cursor activo).
        if a != b {
            let p = out.galley.pos_from_cursor(r.primary).translate(out.galley_pos.to_vec2());
            st.editor.sel_anchor = Some((st.docs.list[di].id, p.center()));
        } else {
            st.editor.sel_anchor = None;
        }
    }
    if let Some(g) = st.editor.goto.filter(|g| g.doc == id) {
        st.editor.goto = None;
        let len = st.docs.list[di].text.chars().count();
        let (a, b) = (g.start.min(len), g.end.min(len));
        let mut state = out.state.clone();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
        state.store(ui.ctx(), out.response.id);
        if g.focus {
            out.response.request_focus();
            st.editor.focused = Some(id);
        }
        st.editor.cursor = Some(b);
        st.editor.selection = Some((a, b));
        let r = out.galley.pos_from_cursor(CCursor::new(b)).translate(out.galley_pos.to_vec2());
        scroll_into_view(ui, id, view, r, row_h);
    }

    // ---- Coincidencias de búsqueda resaltadas ----
    if is_current && st.editor.find.open && !st.editor.find.matches.is_empty() {
        paint_matches(ui, &out, view, &st.editor.find, &pal);
    }

    // ---- Popup de autocompletado junto al cursor ----
    if popup && is_current
        && let Some(c) = st.editor.cursor {
            let at = out.galley.pos_from_cursor(CCursor::new(c)).translate(out.galley_pos.to_vec2());
            let mut pick = None;
            completion_popup(ui, &pal, at.left_bottom(), &items, &mut st.editor.ac.selected, &mut pick);
            if let Some(i) = pick {
                let h = prefix
                    .map(|(h, _)| h)
                    .unwrap_or_else(|| text::char_to_byte(&st.docs.list[di].text, c));
                insert_completion(st, di, h, &items[i]);
            }
        }
}

/// Lleva `target` (px de pantalla) al centro del editor si no está a la vista.
/// Se toca el estado del ScrollArea de `code_view` directamente: un
/// `scroll_to_rect` pedido desde fuera de su closure no le llega.
fn scroll_into_view(ui: &egui::Ui, id: DocId, view: egui::Rect, target: egui::Rect, row_h: f32) {
    if view.shrink2(egui::vec2(0.0, row_h * 2.0)).contains(target.center()) {
        return;
    }
    let sid = ui.make_persistent_id(format!("editor-{id}_outer_scroll"));
    if let Some(mut s) = egui::scroll_area::State::load(ui.ctx(), sid) {
        s.offset.y = (s.offset.y + target.center().y - view.center().y).max(0.0);
        s.store(ui.ctx(), sid);
        ui.ctx().request_repaint();
    }
}

/// Resalta las coincidencias visibles (la actual, más intensa). Solo recorre
/// las que caen dentro de la vista.
fn paint_matches(ui: &egui::Ui, out: &egui::text_edit::TextEditOutput, view: egui::Rect, f: &Find, pal: &Palette) {
    let g = &out.galley;
    let to_local = |p: egui::Pos2| p - out.galley_pos.to_vec2();
    let first = g.cursor_from_pos(to_local(view.left_top()).to_vec2()).index.0;
    let last = g.cursor_from_pos(to_local(view.right_bottom()).to_vec2()).index.0;
    let lo = f.matches.partition_point(|m| m.1 < first);
    let hi = f.matches.partition_point(|m| m.0 <= last);
    let painter = ui.painter().with_clip_rect(view.intersect(out.text_clip_rect));
    let soft = pal.warn.gamma_multiply(0.28);
    let strong = pal.warn.gamma_multiply(0.6);
    for (i, &(a, b)) in f.matches.iter().enumerate().take(hi).skip(lo) {
        let ra = g.pos_from_cursor(CCursor::new(a));
        let rb = g.pos_from_cursor(CCursor::new(b));
        // Coincidencias partidas por el ajuste de línea: solo el primer tramo.
        let right = if (rb.top() - ra.top()).abs() < 1.0 { rb.left() } else { ra.left() + 8.0 };
        let r = egui::Rect::from_min_max(egui::pos2(ra.left(), ra.top()), egui::pos2(right, ra.bottom()))
            .translate(out.galley_pos.to_vec2());
        let fill = if f.current == Some(i) { strong } else { soft };
        painter.rect_filled(r.expand2(egui::vec2(1.0, 0.0)), 2.0, fill);
    }
}

fn completion_popup(
    ui: &egui::Ui,
    pal: &Palette,
    at: egui::Pos2,
    items: &[AcItem],
    selected: &mut usize,
    pick: &mut Option<usize>,
) {
    egui::Area::new(egui::Id::new("ac-popup"))
        .fixed_pos(at + egui::vec2(0.0, 4.0))
        .order(egui::Order::Foreground)
        .constrain(true)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).inner_margin(4).show(ui, |ui| {
                ui.set_width(330.0);
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    for (i, c) in items.iter().enumerate() {
                        let sel = i == *selected;
                        let r = ui
                            .horizontal(|ui| {
                                let name = RichText::new(format!("#{}", c.name)).monospace().color(if sel { pal.accent } else { pal.text });
                                let resp = ui.add(egui::Button::new(name).selected(sel).frame_when_inactive(false).min_size(egui::vec2(140.0, 22.0)));
                                ui.label(RichText::new(&c.detail).small().color(pal.text_faint));
                                resp
                            })
                            .inner;
                        if sel {
                            r.scroll_to_me(None);
                        }
                        if r.clicked() {
                            *pick = Some(i);
                        }
                        if r.hovered() {
                            *selected = i;
                        }
                    }
                });
                ui.separator();
                // Vista previa con marcadores resueltos (como en el panel).
                let preview: String = items
                    .get(*selected)
                    .map(|c| crate::snippets::expand(&c.snippet, "…").0)
                    .unwrap_or_default();
                let preview: String = preview.lines().next().unwrap_or("").chars().take(60).collect();
                ui.label(RichText::new(preview).monospace().small().color(pal.text_dim));
                ui.label(RichText::new("Tab/Enter inserta · Esc cierra").small().color(pal.text_faint));
            });
        });
}

/// Barra de búsqueda sobre el dock (Ctrl+F).
pub fn find_bar(st: &mut State, ui: &mut egui::Ui) {
    if !st.editor.find.open {
        return;
    }
    let pal = theme::palette(ui.ctx());
    let doc = st.docs.cur();
    let (id, rev) = (doc.id, doc.rev);
    st.editor.find.refresh(id, rev, &doc.text);
    let mut step: Option<i32> = None;

    widgets::toolbar_frame(&pal).fill(pal.chrome).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::MAGNIFYING_GLASS).color(pal.text_faint));
            let f = &mut st.editor.find;
            let resp = ui.add(
                egui::TextEdit::singleline(&mut f.query)
                    .hint_text("Buscar en el documento")
                    .desired_width(260.0),
            );
            if f.want_focus {
                resp.request_focus();
                f.want_focus = false;
            }
            if resp.has_focus() || resp.lost_focus() {
                let (enter, shift_enter, esc) = ui.input_mut(|i| {
                    (
                        i.consume_key(Modifiers::NONE, Key::Enter),
                        i.consume_key(Modifiers::SHIFT, Key::Enter),
                        i.consume_key(Modifiers::NONE, Key::Escape),
                    )
                });
                // `lost_focus` + Enter: egui suelta el foco del singleline al pulsar Enter.
                if enter || (resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)) {
                    step = Some(1);
                    resp.request_focus();
                }
                if shift_enter {
                    step = Some(-1);
                    resp.request_focus();
                }
                if esc {
                    f.open = false;
                    let sel = f.current.and_then(|i| f.matches.get(i).copied());
                    let (a, b) = sel.unwrap_or_else(|| {
                        let c = st.editor.cursor.unwrap_or(0);
                        (c, c)
                    });
                    st.editor.goto = Some(Goto { doc: id, start: a, end: b, focus: true });
                }
            }
            if widgets::icon_toggle(ui, f.match_case, icon::TEXT_AA, "Distinguir mayúsculas").clicked() {
                f.match_case = !f.match_case;
            }
            let n = f.matches.len();
            let label = match (n, f.current) {
                (0, _) if !f.query.is_empty() => "sin resultados".to_string(),
                (0, _) => String::new(),
                (n, Some(c)) => format!("{} de {}{}", c + 1, n, if n >= 10_000 { "+" } else { "" }),
                (n, None) => format!("{n} resultados"),
            };
            ui.label(RichText::new(label).small().color(if n == 0 && !f.query.is_empty() { pal.err } else { pal.text_dim }));
            ui.add_enabled_ui(n > 0, |ui| {
                if widgets::icon_button(ui, icon::ARROW_UP, "Anterior  (Mayús+Enter)").clicked() {
                    step = Some(-1);
                }
                if widgets::icon_button(ui, icon::ARROW_DOWN, "Siguiente  (Enter / F3)").clicked() {
                    step = Some(1);
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, icon::X, "Cerrar  (Esc)").clicked() {
                    st.editor.find.open = false;
                }
            });
        });
    });
    if let Some(dir) = step {
        find_step(st, dir);
    }
}

/// Selecciona la coincidencia siguiente (`dir` = 1) o anterior (-1) al cursor.
pub fn find_step(st: &mut State, dir: i32) {
    let doc = st.docs.cur();
    let (id, rev) = (doc.id, doc.rev);
    st.editor.find.refresh(id, rev, &doc.text);
    let f = &mut st.editor.find;
    if f.matches.is_empty() {
        return;
    }
    let n = f.matches.len();
    let i = match f.current {
        Some(c) => (c as i64 + dir as i64).rem_euclid(n as i64) as usize,
        None => {
            let cur = st.editor.cursor.unwrap_or(0);
            if dir > 0 {
                f.matches.iter().position(|m| m.0 >= cur).unwrap_or(0)
            } else {
                f.matches.iter().rposition(|m| m.1 < cur).unwrap_or(n - 1)
            }
        }
    };
    f.current = Some(i);
    let (a, b) = f.matches[i];
    st.editor.goto = Some(Goto { doc: id, start: a, end: b, focus: false });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn almohadilla_solo_ante_nombre() {
        assert_eq!(completion_replacement("table(x)"), "#table(x)");
        assert_eq!(completion_replacement("let x = 1"), "#let x = 1");
        assert_eq!(completion_replacement("*${sel}*"), "*${sel}*");
        assert_eq!(completion_replacement("#link(..)[..]"), "#link(..)[..]");
        assert_eq!(completion_replacement("/ término"), "/ término");
        assert_eq!(completion_replacement("= Tít"), "= Tít");
        assert_eq!(completion_replacement("`${sel}`"), "`${sel}`");
    }
}
