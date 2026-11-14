//! Barras de formato, en dos niveles:
//! - Flotante sobre la selección: formato en línea rápido (negrita, cursiva,
//!   subrayado, tachado, enlace, código) más comentar e ir al PDF. La
//!   chincheta la manda a la fila fija (`Prefs::inline_pinned`).
//! - Fila fija bajo la topbar (se muestra/oculta y se recuerda en `Prefs`):
//!   estilo de texto, familia, tamaño, color, alineación, listas, imagen,
//!   tabla, ecuación, nota/referencia y página.
//!
//! Las acciones reutilizan
//! `Action::InsertSnippet` (envuelve la selección vía `${sel}`) y
//! `Action::PrefixLines` (prefijos de línea con toggle).

use crate::actions::Action;
use crate::app::{State, StatusKind};
use crate::theme;
use crate::widgets;
use egui::{Margin, RichText, Stroke};
use egui_phosphor::regular as icon;

/// Swatches del color pick (hex). El `+` abre el picker libre de egui.
const SWATCHES: &[&str] = &[
    "#ffffff", "#1e1e1e", "#808000", "#ffa500", "#008080", "#0066cc", "#800080", "#e03131",
    "#f06595",
];
/// Encabezados por nivel: icono Phosphor y prefijo de línea.
/// Quitar el encabezado se hace pulsando de nuevo el nivel activo (toggle).
/// (Los iconos son `const`, así que la tabla se construye donde se usa.)
const HEADING_MARKS: [&str; 4] = ["= ", "== ", "=== ", "==== "];
const SIZES_PT: &[f32] = &[8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 18.0, 24.0, 32.0];

/// Estado persistente de la barra (últimos valores usados + cachés).
/// `fonts_rx` trae las fuentes de un hilo (precarga al arrancar, sin
/// bloquear); `Clone` no se deriva por el canal.
pub struct FormatBar {
    pub size_pt: f32,
    pub color_hex: String,
    pub custom: egui::Color32,
    pub font: String,
    pub url: String,
    pub link_open: bool,
    /// Tipografía/alineación como `#set` en el cursor (desde ahí en adelante)
    /// en vez de envolver la selección.
    pub set_mode: bool,
    pub fonts: Option<Vec<String>>,
    pub font_filter: String,
    fonts_rx: Option<std::sync::mpsc::Receiver<Vec<String>>>,
}

impl Default for FormatBar {
    fn default() -> Self {
        Self {
            size_pt: 12.0,
            color_hex: "#e03131".to_string(),
            custom: egui::Color32::from_rgb(0xE0, 0x31, 0x31),
            font: String::new(),
            url: String::new(),
            link_open: false,
            set_mode: false,
            fonts: None,
            font_filter: String::new(),
            fonts_rx: None,
        }
    }
}

impl FormatBar {
    /// Estado inicial con precarga de fuentes en segundo plano.
    pub fn preloaded(ctx: &egui::Context) -> Self {
        let mut f = Self::default();
        f.preload(ctx);
        f
    }

    /// Calienta las fuentes del motor en segundo plano (una vez).
    pub fn preload(&mut self, ctx: &egui::Context) {
        if self.fonts.is_some() || self.fonts_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("typst-fonts".into())
            .spawn(move || {
                let _ = tx.send(crate::engine::font_families());
                ctx.request_repaint();
            })
            .expect("hilo typst-fonts");
        self.fonts_rx = Some(rx);
    }

    /// Vuelve a pedir la lista (cambiaron las fuentes de los plugins).
    pub fn reload_fonts(&mut self, ctx: &egui::Context) {
        self.fonts = None;
        self.fonts_rx = None;
        self.preload(ctx);
    }

    /// Recoge las fuentes si ya llegaron (no bloquea).
    fn poll_fonts(&mut self) {
        if self.fonts.is_none()
            && let Some(rx) = &self.fonts_rx
            && let Ok(list) = rx.try_recv()
        {
            self.fonts = Some(list);
            self.fonts_rx = None;
        }
    }
}

// ---------- Lógica pura (testeable) ----------

/// Prefijos rivales: dentro de un grupo solo vive uno por línea.
/// `""` = quitar encabezado (las listas se quitan con su propio toggle).
fn rivals_of(prefix: &str) -> &'static [&'static str] {
    if prefix.is_empty() || prefix.starts_with('=') {
        &["= ", "== ", "=== ", "==== "]
    } else if prefix == "- " || prefix == "+ " || prefix == "/ " {
        &["- ", "+ ", "/ "]
    } else {
        &[]
    }
}

/// Longitud del indent (espacios/tabs) al inicio de la línea.
fn indent_len(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// Alterna `prefix` en cada línea tocada por la selección `[a, b)` (chars).
/// Si todas lo llevan, lo quita; si no, lo pone (sustituyendo rivales).
/// Con selección colapsada opera sobre la línea del cursor.
/// Devuelve (texto nuevo, inicio, fin) de la selección resultante, que cubre
/// el bloque modificado.
pub fn toggle_prefix(text: &str, a: usize, b: usize, prefix: &str) -> (String, usize, usize) {
    let n = text.chars().count();
    let (a, b) = (a.min(n), b.min(n));
    let (a, b) = (a.min(b), a.max(b));
    let rivals = rivals_of(prefix);
    // Líneas con su offset en chars.
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut off = 0;
    for line in text.split_inclusive('\n') {
        lines.push((off, line.strip_suffix('\n').unwrap_or(line)));
        off += line.chars().count();
    }
    if lines.is_empty() {
        return (text.to_string(), a, b);
    }
    // Líneas tocadas por [a, b); con cursor colapsado, su línea.
    let mut touched = Vec::new();
    for (i, (start, line)) in lines.iter().enumerate() {
        let end = start + line.chars().count() + usize::from(i + 1 < lines.len());
        let hit = if a < b {
            *start < b && end > a
        } else {
            // El cursor pertenece a una sola línea: en columna 0 es la suya,
            // no el final (tras el \n) de la anterior.
            *start <= a && (a < end || i + 1 == lines.len())
        };
        if hit {
            touched.push(i);
        }
    }
    if touched.is_empty() {
        return (text.to_string(), a, b);
    }
    // ¿Lo llevan todas (tras el indent)?
    let has = |line: &str| {
        let ind = indent_len(line);
        let rest = &line[ind..];
        !prefix.is_empty() && rest.starts_with(prefix)
    };
    let all_have = touched.iter().all(|&i| has(lines[i].1));
    let trailing_nl = text.ends_with('\n');
    let last_touched = *touched.last().unwrap();
    let last_line = lines.len() - 1;
    let mut out = String::new();
    let mut new_start = 0;
    let mut new_end = 0;
    for (i, (start, line)) in lines.iter().enumerate() {
        if *start == lines[touched[0]].0 {
            new_start = out.chars().count();
        }
        let ind = indent_len(line);
        let (head, rest) = line.split_at(ind);
        let mut rest = rest.to_string();
        if touched.contains(&i) {
            if all_have {
                // Quitar (el que lleve: prefix o rival).
                for r in std::iter::once(prefix)
                    .chain(rivals.iter().copied())
                    .filter(|r| !r.is_empty())
                {
                    if let Some(stripped) = rest.strip_prefix(r) {
                        rest = stripped.to_string();
                        break;
                    }
                }
            } else {
                // Quitar rival y poner el pedido ("" = solo quitar).
                for r in rivals {
                    if let Some(stripped) = rest.strip_prefix(r) {
                        rest = stripped.to_string();
                        break;
                    }
                }
                if !prefix.is_empty() {
                    // No duplicar si ya lo lleva (líneas mixtas).
                    if !rest.starts_with(prefix) {
                        rest = format!("{prefix}{rest}");
                    }
                }
            }
        }
        out.push_str(head);
        out.push_str(&rest);
        if i + 1 < lines.len() {
            out.push('\n');
        }
        if *start == lines[last_touched].0 {
            new_end = out.chars().count();
        }
    }
    if trailing_nl {
        out.push('\n');
        if last_touched == last_line {
            new_end = out.chars().count();
        }
    }
    (out, new_start, new_end)
}

/// Marca de formato en línea que se alterna sobre la selección.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
}

impl Mark {
    /// (abre, cierra) en la forma corta y en la de función. La corta no vale
    /// pegada a una letra (`a*b*c`): ahí typst exige `#strong[…]`.
    fn forms(self) -> [(&'static str, &'static str); 2] {
        match self {
            Mark::Bold => [("*", "*"), ("#strong[", "]")],
            Mark::Italic => [("_", "_"), ("#emph[", "]")],
            Mark::Underline => [("#underline[", "]"); 2],
            Mark::Strike => [("#strike[", "]"); 2],
            Mark::Code => [("`", "`"); 2],
        }
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

/// Alterna la marca sobre `[a, b)` (chars): la quita si la selección ya está
/// envuelta (por fuera o por dentro), si no la pone. Con el cursor colapsado
/// opera sobre la palabra que lo toca; sin palabra deja la marca vacía con el
/// cursor dentro. Devuelve (texto nuevo, inicio, fin) del texto marcado.
pub fn toggle_wrap(text: &str, a: usize, b: usize, mark: Mark) -> (String, usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let (a, b) = (a.min(b).min(n), a.max(b).min(n));
    let [short, long] = mark.forms();
    let is = |at: usize, pat: &str| {
        let p: Vec<char> = pat.chars().collect();
        at + p.len() <= n && chars[at..at + p.len()] == p[..]
    };
    let len = |s: &str| s.chars().count();
    let splice = |from: usize, to: usize, with: &str| -> String {
        let mut o: String = chars[..from].iter().collect();
        o.push_str(with);
        o.extend(&chars[to..]);
        o
    };
    // Colapsado: la palabra bajo el cursor.
    let (mut a, mut b) = (a, b);
    if a == b {
        while a > 0 && is_word(chars[a - 1]) {
            a -= 1;
        }
        while b < n && is_word(chars[b]) {
            b += 1;
        }
    }
    // ¿Ya envuelta? Por fuera (`*[x]*`) o por dentro (`[*x*]`).
    for (o, c) in [short, long] {
        let (lo, lc) = (len(o), len(c));
        if a >= lo && is(a - lo, o) && is(b, c) {
            let inner: String = chars[a..b].iter().collect();
            return (splice(a - lo, b + lc, &inner), a - lo, b - lo);
        }
        if b - a >= lo + lc && is(a, o) && is(b - lc, c) {
            let inner: String = chars[a + lo..b - lc].iter().collect();
            return (splice(a, b, &inner), a, b - lo - lc);
        }
    }
    let glued = (a > 0 && is_word(chars[a - 1])) || (b < n && is_word(chars[b]));
    let (o, c) = if glued { long } else { short };
    let inner: String = chars[a..b].iter().collect();
    let wrapped = format!("{o}{inner}{c}");
    (splice(a, b, &wrapped), a + len(o), b + len(o))
}

/// Inserta la regla `#set …` en su propia línea en el char `pos`: tal cual si
/// el cursor está al principio de línea (solo espacios antes), y si no, en una
/// línea aparte a continuación del cursor. Devuelve (texto, cursor tras la regla).
pub fn insert_set_line(text: &str, pos: usize, rule: &str) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let pos = pos.min(chars.len());
    let line_start = chars[..pos].iter().rposition(|&c| c == '\n').map_or(0, |i| i + 1);
    let at_start = chars[line_start..pos].iter().all(|c| c.is_whitespace());
    let ins = if at_start {
        format!("{rule}\n")
    } else {
        format!("\n{rule}\n")
    };
    let mut out: String = chars[..pos].iter().collect();
    out.push_str(&ins);
    out.extend(&chars[pos..]);
    (out, pos + ins.chars().count())
}

/// Nivel de encabezado (nº de `=`) de la línea que contiene el char `cursor`;
/// 0 si no es encabezado.
fn heading_level(text: &str, cursor: usize) -> usize {
    let b = crate::text::char_to_byte(text, cursor);
    let start = text[..b].rfind('\n').map_or(0, |i| i + 1);
    let line = text[start..].lines().next().unwrap_or("");
    let rest = &line[indent_len(line)..];
    let n = rest.chars().take_while(|&c| c == '=').count();
    if n > 0 && rest[n..].starts_with(' ') {
        n
    } else {
        0
    }
}

fn hex_to_color(hex: &str) -> Option<egui::Color32> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(egui::Color32::from_rgb(
        (v >> 16) as u8,
        (v >> 8) as u8,
        v as u8,
    ))
}

fn color_to_hex(c: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Recorta a `n` chars con «…» (nombres largos de fuente en la barra).
fn ellipsize(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

/// "12" en vez de "12.0" para el `size: …pt` y la etiqueta.
fn fmt_pt(s: f32) -> String {
    if s.fract() == 0.0 {
        format!("{}", s as i32)
    } else {
        format!("{s}")
    }
}

// ---------- UI ----------

/// Barra flotante: formato en línea rápido sobre una selección no vacía con
/// ancla vigente.
/// Se llama tras el dock en `App::ui` (el ancla la guardó `doc_tab`).
pub fn show(st: &mut State, ctx: &egui::Context) {
    if st.prefs.inline_pinned {
        return;
    }
    let Some((a, b)) = st.editor.selection else {
        return;
    };
    if a >= b {
        return;
    }
    let Some((doc, anchor)) = st.editor.sel_anchor else {
        return;
    };
    if doc != st.docs.cur_id() || st.modal.is_some() {
        return;
    }
    let vp = ctx.viewport_rect();
    // Centrada sobre la selección, creciendo hacia arriba (la fila del enlace
    // no tapa el texto); si no cabe arriba, debajo. `constrain` la mantiene
    // dentro de la ventana sin suponer su tamaño.
    let (pivot, y) = if anchor.top() - 60.0 < vp.min.y + 28.0 {
        (egui::Align2::CENTER_TOP, anchor.bottom() + 8.0)
    } else {
        (egui::Align2::CENTER_BOTTOM, anchor.top() - 8.0)
    };
    let pal0 = theme::palette(ctx);
    egui::Area::new(egui::Id::new("formatbar"))
        .order(egui::Order::Foreground)
        .pivot(pivot)
        .fixed_pos(egui::pos2(anchor.center().x, y))
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(pal0.surface)
                .stroke(Stroke::new(1.0, pal0.border_strong))
                .corner_radius(egui::CornerRadius::same(14))
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    if let Some(a) = inline_row(st, ui, true) {
                        st.actions.push(a);
                    }
                });
        });
}

/// Fila fija bajo la topbar: tipografía y párrafo. Actúa sobre la selección
/// o, sin ella, sobre el cursor.
pub fn docked(st: &mut State, ui: &mut egui::Ui) {
    st.fmt.poll_fonts();
    let pal = theme::palette(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        if st.prefs.inline_pinned {
            if let Some(a) = inline_row(st, ui, false) {
                st.actions.push(a);
            }
            widgets::vsep(ui, &pal);
        }
        if let Some(a) = block_row(st, ui) {
            st.actions.push(a);
        }
    });
}

/// Formato en línea: negrita, cursiva, subrayado, tachado, enlace, código,
/// comentar e ir al PDF. Flotante termina con la chincheta (fijar en la fila
/// de formato); fijada empieza con desfijar. La URL abre una segunda fila.
fn inline_row(st: &mut State, ui: &mut egui::Ui, floating: bool) -> Option<Action> {
    let pal = theme::palette(ui.ctx());
    let mut act: Option<Action> = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if !floating {
            if widgets::icon_button(ui, icon::PUSH_PIN_SLASH, "Desfijar (flotante)").clicked() {
                st.prefs.inline_pinned = false;
                st.needs_persist = true;
            }
            widgets::vsep(ui, &pal);
        }
        if widgets::icon_button(ui, icon::TEXT_BOLDER, "Negrita  (Ctrl+B)").clicked() {
            act = Some(Action::ToggleMark(Mark::Bold));
        }
        if widgets::icon_button(ui, icon::TEXT_ITALIC, "Cursiva  (Ctrl+I)").clicked() {
            act = Some(Action::ToggleMark(Mark::Italic));
        }
        if widgets::icon_button(ui, icon::TEXT_UNDERLINE, "Subrayado  (Ctrl+U)").clicked() {
            act = Some(Action::ToggleMark(Mark::Underline));
        }
        if widgets::icon_button(ui, icon::TEXT_STRIKETHROUGH, "Tachado").clicked() {
            act = Some(Action::ToggleMark(Mark::Strike));
        }
        widgets::vsep(ui, &pal);
        // Enlace (despliega campo URL).
        if widgets::icon_button(ui, icon::LINK, "Enlace").clicked() {
            st.fmt.link_open = !st.fmt.link_open;
        }
        // Código en línea.
        if widgets::icon_button(ui, icon::CODE_SIMPLE, "Código").clicked() {
            act = Some(Action::ToggleMark(Mark::Code));
        }
        widgets::vsep(ui, &pal);
        if widgets::icon_button(ui, "//", "Comentar línea/selección  (Ctrl+/)").clicked() {
            act = Some(Action::PrefixLines("// ".into()));
        }
        if widgets::icon_button(ui, icon::FILE_PDF, "Ir al PDF").clicked() {
            act = Some(Action::SyncToPdf);
        }
        if floating {
            widgets::vsep(ui, &pal);
            if widgets::icon_button(ui, icon::PUSH_PIN, "Fijar en la barra de formato").clicked() {
                st.prefs.inline_pinned = true;
                // Fijar en una fila oculta la haría desaparecer.
                st.prefs.format_bar = true;
                st.needs_persist = true;
            }
        }
    });
    // Segunda fila: URL del enlace.
    if st.fmt.link_open {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::LINK).small());
            let r = ui.add(
                egui::TextEdit::singleline(&mut st.fmt.url)
                    .hint_text("https://…")
                    .desired_width(220.0),
            );
            let go = ui.small_button("Aplicar").clicked()
                || (r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if go && !st.fmt.url.trim().is_empty() {
                let u = st.fmt.url.trim().replace('\\', "\\\\").replace('"', "\\\"");
                act = Some(Action::InsertSnippet(format!(
                    "#link(\"{u}\")[${{sel:enlace}}]"
                )));
                st.fmt.link_open = false;
            }
            if ui.small_button("✕").clicked()
                || (r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)))
            {
                st.fmt.link_open = false;
            }
        });
    }
    act
}

/// Tipografía, texto e inserción (fija): encabezado, familia,
/// tamaño, color, alineación, listas, imagen, tabla, ecuación, nota y
/// referencia, y página.
fn block_row(st: &mut State, ui: &mut egui::Ui) -> Option<Action> {
    let pal = theme::palette(ui.ctx());
    let mut act: Option<Action> = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // Encabezados H1–H4 en desplegable, cada fila con su icono.
        let level = {
            let doc = st.docs.cur();
            heading_level(&doc.text, st.editor.cursor.unwrap_or(0))
        };
        let cur_icon = match level {
            1 => icon::TEXT_H_ONE,
            2 => icon::TEXT_H_TWO,
            3 => icon::TEXT_H_THREE,
            4 => icon::TEXT_H_FOUR,
            _ => icon::TEXT_H,
        };
        let cur_label = match level {
            0 => "Encabezado".to_string(),
            1..=4 => format!("Encabezado {level}"),
            // Nivel 5+: typst lo acepta, se muestra sin entrada propia.
            _ => format!("Encabezado {level}"),
        };
        widgets::combo(
            ui,
            &pal,
            RichText::new(format!("{cur_icon}  {cur_label}")).small(),
            128.0,
            "Encabezado",
            |ui| {
                ui.set_min_width(180.0);
                for (i, mark) in HEADING_MARKS.iter().enumerate() {
                    let n = i + 1;
                    let ic = match n {
                        1 => icon::TEXT_H_ONE,
                        2 => icon::TEXT_H_TWO,
                        3 => icon::TEXT_H_THREE,
                        _ => icon::TEXT_H_FOUR,
                    };
                    let text = format!("{ic}  Encabezado {n}  ({})", mark.trim_end());
                    if ui.selectable_label(level == n, text).clicked() {
                        act = Some(Action::PrefixLines(mark.to_string()));
                        ui.close();
                    }
                }
            },
        );
        widgets::vsep(ui, &pal);
        // Ámbito de tipografía y alineación: selección o `#set` desde el cursor.
        if widgets::icon_toggle(
            ui,
            st.fmt.set_mode,
            icon::TEXT_AA,
            "Tipografía como #set desde el cursor (no solo la selección)",
        )
        .clicked()
        {
            st.fmt.set_mode = !st.fmt.set_mode;
        }
        let set_mode = st.fmt.set_mode;
        let apply = move |inline: String, rule: String| {
            if set_mode {
                Action::InsertSetRule(rule)
            } else {
                Action::InsertSnippet(inline)
            }
        };
        // Tamaño de fuente.
        let size = RichText::new(format!("{} pt", fmt_pt(st.fmt.size_pt))).small();
        widgets::combo(ui, &pal, size, 64.0, "Tamaño", |ui| {
            ui.set_min_width(120.0);
            for s in SIZES_PT {
                if ui
                    .selectable_label(*s == st.fmt.size_pt, format!("{} pt", fmt_pt(*s)))
                    .clicked()
                {
                    st.fmt.size_pt = *s;
                    act = Some(apply(
                        format!("#text(size: {}pt)[${{sel}}]", fmt_pt(*s)),
                        format!("#set text(size: {}pt)", fmt_pt(*s)),
                    ));
                    ui.close();
                }
            }
        });
        // Familia (precargada en un hilo al arrancar).
        let family = if st.fmt.font.is_empty() {
            "Fuente".to_string()
        } else {
            ellipsize(&st.fmt.font, 16)
        };
        widgets::combo(
            ui,
            &pal,
            RichText::new(format!("{}  {family}", icon::TEXT_AA)).small(),
            132.0,
            "Familia de fuente",
            |ui| {
                ui.set_min_width(240.0);
                let Some(fonts) = &st.fmt.fonts else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Cargando fuentes…").small());
                    });
                    return;
                };
                if fonts.is_empty() {
                    ui.label(RichText::new("Sin fuentes del sistema").small());
                    return;
                }
                ui.add(
                    egui::TextEdit::singleline(&mut st.fmt.font_filter)
                        .hint_text("Filtrar…")
                        .desired_width(f32::INFINITY),
                );
                let q = st.fmt.font_filter.to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for f in fonts
                            .iter()
                            .filter(|f| q.is_empty() || f.to_lowercase().contains(&q))
                        {
                            if ui.selectable_label(*f == st.fmt.font, f).clicked() {
                                st.fmt.font = f.clone();
                                let f = f.replace('\\', "\\\\").replace('"', "\\\"");
                                act = Some(apply(
                                    format!("#text(font: \"{f}\")[${{sel}}]"),
                                    format!("#set text(font: \"{f}\")"),
                                ));
                                ui.close();
                            }
                        }
                    });
            },
        );
        // Color: «A» con una barra del último color usado debajo.
        let current_color = hex_to_color(&st.fmt.color_hex).unwrap_or(pal.text);
        let r = widgets::combo(
            ui,
            &pal,
            RichText::new("A").strong(),
            44.0,
            "Color del texto",
            |ui| {
                ui.set_min_width(150.0);
                egui::Grid::new("fmt-colors")
                    .num_columns(3)
                    .spacing([4.0, 4.0])
                    .show(ui, |ui| {
                        for (i, h) in SWATCHES.iter().enumerate() {
                            let c = hex_to_color(h).unwrap_or(pal.text);
                            if ui
                                .add(
                                    egui::Button::new("")
                                        .fill(c)
                                        .min_size(egui::vec2(26.0, 22.0))
                                        .corner_radius(egui::CornerRadius::same(5)),
                                )
                                .on_hover_text(*h)
                                .clicked()
                            {
                                st.fmt.color_hex = h.to_string();
                                act = Some(apply(
                                    format!("#text(fill: rgb(\"{h}\"))[${{sel}}]"),
                                    format!("#set text(fill: rgb(\"{h}\"))"),
                                ));
                                ui.close();
                            }
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.color_edit_button_srgba(&mut st.fmt.custom);
                    if ui.small_button("Aplicar").clicked() {
                        let h = color_to_hex(st.fmt.custom);
                        st.fmt.color_hex = h.clone();
                        st.fmt.custom = hex_to_color(&h).unwrap_or(st.fmt.custom);
                        act = Some(apply(
                            format!("#text(fill: rgb(\"{h}\"))[${{sel}}]"),
                            format!("#set text(fill: rgb(\"{h}\"))"),
                        ));
                        ui.close();
                    }
                });
            },
        );
        let pad = ui.spacing().button_padding.x;
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(r.rect.left() + pad, r.rect.bottom() - 6.0),
                egui::vec2(11.0, 3.0),
            ),
            egui::CornerRadius::same(1),
            current_color,
        );
        widgets::vsep(ui, &pal);
        // Alineación.
        widgets::icon_menu(ui, icon::TEXT_ALIGN_LEFT, "Alineación", |ui| {
            ui.set_min_width(160.0);
            for (ic, name) in [
                (icon::TEXT_ALIGN_LEFT, "left"),
                (icon::TEXT_ALIGN_CENTER, "center"),
                (icon::TEXT_ALIGN_RIGHT, "right"),
                (icon::TEXT_ALIGN_JUSTIFY, "justify"),
            ] {
                if ui
                    .add(egui::Button::new(format!("{ic}  {name}")).frame_when_inactive(false))
                    .clicked()
                {
                    let rule = if name == "justify" {
                        "#set par(justify: true)".to_string()
                    } else {
                        format!("#set align({name})")
                    };
                    act = Some(apply(format!("#align({name})[${{sel}}]"), rule));
                    ui.close();
                }
            }
        });
        // Listas.
        widgets::icon_menu(ui, icon::LIST_BULLETS, "Listas", |ui| {
            ui.set_min_width(180.0);
            if ui.button("Viñetas (- )").clicked() {
                act = Some(Action::PrefixLines("- ".into()));
                ui.close();
            }
            if ui.button("Numerada (+ )").clicked() {
                act = Some(Action::PrefixLines("+ ".into()));
                ui.close();
            }
            if ui.button("Término (/ …:)").clicked() {
                act = Some(Action::InsertSnippet("/ ${sel:término}: definición".into()));
                ui.close();
            }
        });
        // Imagen (diálogo; se inserta tras la selección sin borrarla).
        if widgets::icon_button(ui, icon::IMAGE, "Imagen…").clicked()
            && let Some(body) = pick_image(st)
        {
            collapse_to_end(st);
            act = Some(Action::InsertSnippet(body));
        }
        widgets::vsep(ui, &pal);
        // Tabla: N columnas × 2 filas; la selección va a la primera celda.
        widgets::icon_menu(ui, icon::TABLE, "Tabla", |ui| {
            ui.set_min_width(160.0);
            for n in 2..=5 {
                if ui.button(format!("{n} columnas")).clicked() {
                    act = Some(Action::InsertSnippet(table_snippet(n)));
                    ui.close();
                }
            }
        });
        // Ecuación (envuelve la selección).
        widgets::icon_menu(ui, icon::SIGMA, "Ecuación", |ui| {
            ui.set_min_width(180.0);
            if ui.button("En línea   $x$").clicked() {
                act = Some(Action::InsertSnippet("$${sel:x}$".into()));
                ui.close();
            }
            if ui.button("En bloque   $ x $").clicked() {
                act = Some(Action::InsertSnippet("$ ${sel:x} $".into()));
                ui.close();
            }
        });
        // Nota al pie y referencias a etiquetas del documento.
        widgets::icon_menu(ui, icon::AT, "Nota y referencia", |ui| {
            ui.set_min_width(200.0);
            if ui.button("Nota al pie").clicked() {
                collapse_to_end(st);
                act = Some(Action::InsertSnippet("#footnote[${nota}]".into()));
                ui.close();
            }
            if ui.button("Etiqueta  <…>").clicked() {
                collapse_to_end(st);
                act = Some(Action::InsertSnippet(" <${etiqueta}>".into()));
                ui.close();
            }
            ui.separator();
            let labels = doc_labels(&st.docs.cur().text);
            if labels.is_empty() {
                ui.label(RichText::new("Sin etiquetas que referenciar").small());
            }
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for l in labels {
                        if ui.button(format!("@{l}")).clicked() {
                            collapse_to_end(st);
                            act = Some(Action::InsertSnippet(format!("@{l}")));
                            ui.close();
                        }
                    }
                });
        });
        // Página.
        widgets::icon_menu(ui, icon::FILE_TEXT, "Página", |ui| {
            ui.set_min_width(200.0);
            for (label, body) in [
                ("Salto de página", "#pagebreak()\n"),
                ("Salto de columna", "#colbreak()\n"),
                ("Dos columnas", "#columns(2)[${sel}]"),
                ("Espacio vertical", "#v(${1em})"),
            ] {
                if ui.button(label).clicked() {
                    act = Some(Action::InsertSnippet(body.into()));
                    ui.close();
                }
            }
        });
    });
    act
}

/// Colapsa la selección a su final: lo que se inserta va detrás, sin
/// sustituir el texto seleccionado (imagen, nota, referencia).
fn collapse_to_end(st: &mut State) {
    if let Some((_, b)) = st.editor.selection {
        st.editor.selection = Some((b, b));
        st.editor.cursor = Some(b);
    }
}

/// `#table` de `n` columnas y dos filas; la selección (o el cursor) en la
/// primera celda.
fn table_snippet(n: usize) -> String {
    let row = |first: &str| {
        let mut cells = vec![first.to_string()];
        cells.extend(std::iter::repeat_n("[]".to_string(), n - 1));
        format!("  {},\n", cells.join(", "))
    };
    format!(
        "#table(\n  columns: {n},\n{}{})\n",
        row("[${sel}]"),
        row("[]")
    )
}

/// Etiquetas `<nombre>` del documento, sin repetir y en orden de aparición.
fn doc_labels(text: &str) -> Vec<String> {
    let valid = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.');
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let name: String = rest.chars().take_while(|&c| valid(c)).collect();
        if !name.is_empty() && rest[name.len()..].starts_with('>') && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Diálogo de imagen → cuerpo `image(…)` con ruta relativa a la raíz de
/// compilación.
fn pick_image(st: &mut State) -> Option<String> {
    let root = st.compile_root();
    let p = rfd::FileDialog::new()
        .add_filter("Imagen", &["png", "jpg", "jpeg", "svg", "webp", "gif"])
        .set_directory(&root)
        .pick_file()?;
    // Typst solo lee dentro de la raíz efectiva de compilación.
    let Some(path) = image_path(&root, &p) else {
        st.set_status(
            "La imagen debe estar dentro de la carpeta raíz",
            StatusKind::Err,
        );
        return None;
    };
    Some(format!("#image(\"{path}\", width: 80%)"))
}

/// Ruta de `img` relativa a `root`, con `/` y escapada para un string typst.
/// `None` si queda fuera de la raíz (typst no podría leerla).
fn image_path(root: &std::path::Path, img: &std::path::Path) -> Option<String> {
    let rel = img.strip_prefix(root).ok()?;
    let joined = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    Some(joined.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_en_el_cursor() {
        assert_eq!(
            insert_set_line("uno\ndos", 4, "#set text(size: 9pt)"),
            ("uno\n#set text(size: 9pt)\ndos".into(), 25)
        );
        // A mitad de línea: línea aparte tras el cursor.
        assert_eq!(
            insert_set_line("ab", 1, "#set x"),
            ("a\n#set x\nb".into(), 9)
        );
        assert_eq!(insert_set_line("", 0, "#set x"), ("#set x\n".into(), 7));
    }

    #[test]
    fn marcas_alternan() {
        use Mark::*;
        // Selección: pone y quita (devuelve el texto marcado seleccionado).
        assert_eq!(toggle_wrap("hola mundo", 0, 4, Bold), ("*hola* mundo".into(), 1, 5));
        assert_eq!(toggle_wrap("*hola* mundo", 1, 5, Bold), ("hola mundo".into(), 0, 4));
        // Selección que incluye las marcas.
        assert_eq!(toggle_wrap("*hola*", 0, 6, Bold), ("hola".into(), 0, 4));
        // Cursor en una palabra.
        assert_eq!(toggle_wrap("hola mundo", 7, 7, Italic), ("hola _mundo_".into(), 6, 11));
        // Sin palabra: marca vacía con el cursor dentro.
        assert_eq!(toggle_wrap("a ", 2, 2, Bold), ("a **".into(), 3, 3));
        // Pegada a una letra: forma de función, y se quita igual.
        assert_eq!(toggle_wrap("abcd", 1, 3, Bold), ("a#strong[bc]d".into(), 9, 11));
        assert_eq!(toggle_wrap("a#strong[bc]d", 9, 11, Bold), ("abcd".into(), 1, 3));
        // Funciones y código.
        assert_eq!(toggle_wrap("x y", 0, 1, Underline), ("#underline[x] y".into(), 11, 12));
        assert_eq!(toggle_wrap("x", 0, 1, Code), ("`x`".into(), 1, 2));
        // Offsets en chars.
        assert_eq!(toggle_wrap("ñu", 0, 2, Italic), ("_ñu_".into(), 1, 3));
    }

    #[test]
    fn encabezado_toggle() {
        // Añade.
        assert_eq!(
            toggle_prefix("Hola\n", 0, 4, "= "),
            ("= Hola\n".into(), 0, 7)
        );
        // Quita (todas lo llevan).
        assert_eq!(
            toggle_prefix("= Hola\n", 0, 6, "= "),
            ("Hola\n".into(), 0, 5)
        );
        // Rival: == → =.
        assert_eq!(
            toggle_prefix("== Hola\n", 0, 7, "= "),
            ("= Hola\n".into(), 0, 7)
        );
        // Normal quita encabezado pero no lista.
        assert_eq!(
            toggle_prefix("- item\n", 0, 6, ""),
            ("- item\n".into(), 0, 7)
        );
        assert_eq!(toggle_prefix("== T\n", 0, 4, ""), ("T\n".into(), 0, 2));
    }

    #[test]
    fn lista_multilinea_e_indent() {
        let (t, a, b) = toggle_prefix("a\nb\n", 0, 3, "- ");
        assert_eq!((t.as_str(), a, b), ("- a\n- b\n", 0, 8));
        // Mixto: añade donde falta sin duplicar.
        let (t, _, _) = toggle_prefix("- a\nb\n", 0, 5, "- ");
        assert_eq!(t, "- a\n- b\n");
        // Indent preservado.
        let (t, _, _) = toggle_prefix("  x\n", 0, 3, "- ");
        assert_eq!(t, "  - x\n");
        let (t, _, _) = toggle_prefix("  - x\n", 0, 5, "- ");
        assert_eq!(t, "  x\n");
        // Colapsada: línea del cursor.
        let (t, a, b) = toggle_prefix("uno\ndos\n", 5, 5, "+ ");
        assert_eq!((t.as_str(), a, b), ("uno\n+ dos\n", 4, 10));
        // Colapsada en columna 0: solo esa línea, no también la anterior.
        let (t, _, _) = toggle_prefix("uno\ndos\n", 4, 4, "- ");
        assert_eq!(t, "uno\n- dos\n");
        let (t, _, _) = toggle_prefix("uno\n\ntres", 4, 4, "- ");
        assert_eq!(t, "uno\n- \ntres");
        let (t, _, _) = toggle_prefix("uno\ndos", 7, 7, "- ");
        assert_eq!(t, "uno\n- dos");
    }

    #[test]
    fn ruta_de_imagen() {
        use std::path::Path;
        let root = Path::new("/doc");
        assert_eq!(
            image_path(root, Path::new("/doc/img/foto.png")).as_deref(),
            Some("img/foto.png")
        );
        assert_eq!(
            image_path(root, Path::new("/doc/a\"b.png")).as_deref(),
            Some("a\\\"b.png")
        );
        assert_eq!(image_path(root, Path::new("/otra/foto.png")), None);
    }

    #[test]
    fn comentar_y_descomentar() {
        let (t, _, _) = toggle_prefix("a\n  b\nc\n", 0, 5, "// ");
        assert_eq!(t, "// a\n  // b\nc\n");
        let (t, _, _) = toggle_prefix(&t, 0, 11, "// ");
        assert_eq!(t, "a\n  b\nc\n");
        // Un encabezado comentado sigue siendo encabezado al descomentar.
        let (t, _, _) = toggle_prefix("= T\n", 1, 1, "// ");
        assert_eq!(t, "// = T\n");
    }

    #[test]
    fn nivel_de_encabezado() {
        let t = "= Uno\ntexto\n  == Dos\n=sin espacio\n";
        assert_eq!(heading_level(t, 0), 1);
        assert_eq!(heading_level(t, 5), 1);
        assert_eq!(heading_level(t, 6), 0);
        assert_eq!(heading_level(t, 14), 2);
        assert_eq!(heading_level(t, 22), 0);
        assert_eq!(heading_level("", 0), 0);
    }

    #[test]
    fn tabla_y_etiquetas() {
        assert_eq!(
            table_snippet(3),
            "#table(\n  columns: 3,\n  [${sel}], [], [],\n  [], [], [],\n)\n"
        );
        let t = "= Intro <intro>\n$ x < y > z $ <eq:1>\nVer @intro <intro>.";
        assert_eq!(doc_labels(t), ["intro", "eq:1"]);
    }

    #[test]
    fn insertables_compilan() {
        let dir = std::env::temp_dir();
        let mut bodies: Vec<String> = (2..=5).map(table_snippet).collect();
        bodies.extend(
            [
                "$${sel:x}$",
                "$ ${sel:x} $",
                "#footnote[${nota}]",
                // @ a un encabezado exige numeración (regla de typst).
                "#set heading(numbering: \"1.\")\n= T <intro>\nVer @intro.",
                "Texto <${etiqueta}>",
                "#pagebreak()\n",
                "#colbreak()\n",
                "#columns(2)[${sel}]",
                "#v(${1em})",
            ]
            .map(String::from),
        );
        for body in bodies {
            for sel in ["", "x"] {
                let (src, _, _) = crate::snippets::expand(&body, sel);
                let out = crate::engine::compile_pdf(&src, &dir, "main.typ");
                assert!(out.pdf.is_some(), "no compila:\n{src}\n{:?}", out.diags);
            }
        }
    }

    #[test]
    fn hex_color() {
        assert_eq!(
            hex_to_color("#ff0000"),
            Some(egui::Color32::from_rgb(255, 0, 0))
        );
        assert!(hex_to_color("rojo").is_none());
        assert_eq!(
            color_to_hex(egui::Color32::from_rgb(0, 102, 204)),
            "#0066cc"
        );
    }
}
