//! Barra flotante de formato sobre la selección: negrita, tamaño y familia de
//! fuente, color, alineación, enlace, imagen, listas y encabezados.
//!
//! No es un clon de ninguna barra concreta: pill compacta con desplegables,
//! anclada sobre el final de la selección. Las acciones reutilizan
//! `Action::InsertSnippet` (envuelve la selección vía `${sel}`) y
//! `Action::PrefixLines` (prefijos de línea con toggle).

use crate::actions::Action;
use crate::app::State;
use crate::compile;
use crate::theme;
use crate::widgets;
use egui::{Margin, RichText, Stroke};
use egui_phosphor::regular as icon;

/// Swatches del color pick (hex). El `+` abre el picker libre de egui.
const SWATCHES: &[&str] = &[
    "#ffffff", "#1e1e1e", "#808000", "#ffa500", "#008080", "#0066cc", "#800080", "#e03131",
    "#f06595",
];
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
    pub fonts: Option<Vec<String>>,
    pub font_filter: String,
    /// Fija la barra como fila de la topbar en vez de flotante.
    pub pinned: bool,
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
            fonts: None,
            font_filter: String::new(),
            pinned: false,
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
            *start <= a && a <= end
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

/// "12" en vez de "12.0" para el `size: …pt` y la etiqueta.
fn fmt_pt(s: f32) -> String {
    if s.fract() == 0.0 {
        format!("{}", s as i32)
    } else {
        format!("{s}")
    }
}

// ---------- UI ----------

/// Dibuja la barra si hay selección no vacía con ancla vigente.
/// Se llama tras el dock en `App::ui` (el ancla la guardó `doc_tab`).
pub fn show(st: &mut State, ctx: &egui::Context) {
    st.fmt.poll_fonts();
    // Fija: se dibuja como fila de la topbar (ver `docked`), no flotante.
    if st.fmt.pinned {
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
    // Sobre la selección; si no cabe, debajo (flip).
    let above = anchor.y - 60.0;
    let y = if above < vp.min.y + 28.0 {
        anchor.y + 24.0
    } else {
        above
    };
    let x = (anchor.x - 200.0).clamp(vp.min.x + 8.0, (vp.max.x - 430.0).max(vp.min.x + 8.0));
    let pal0 = theme::palette(ctx);
    egui::Area::new(egui::Id::new("formatbar"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(x, y))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(pal0.surface)
                .stroke(Stroke::new(1.0, pal0.border_strong))
                .corner_radius(egui::CornerRadius::same(14))
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    if let Some(a) = bar_row(st, ui, true) {
                        st.actions.push(a);
                    }
                });
        });
}

/// Fila fija para la topbar: siempre visible cuando está fijada (actúa sobre
/// la selección o, sin ella, sobre el cursor).
pub fn docked(st: &mut State, ui: &mut egui::Ui) {
    st.fmt.poll_fonts();
    if let Some(a) = bar_row(st, ui, false) {
        st.actions.push(a);
    }
}

/// Contenido compartido (flotante y fija). Fija muestra para desfijar al
/// inicio; flotante muestra la chincheta al final.
fn bar_row(st: &mut State, ui: &mut egui::Ui, floating: bool) -> Option<Action> {
    let pal = theme::palette(ui.ctx());
    let mut act: Option<Action> = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if !floating {
            if widgets::icon_button(ui, icon::PUSH_PIN_SLASH, "Desfijar (flotante)").clicked() {
                st.fmt.pinned = false;
            }
            widgets::vsep(ui, &pal);
        }
        if widgets::icon_button(ui, icon::TEXT_BOLDER, "Negrita").clicked() {
            act = Some(Action::InsertSnippet("*${sel}*".into()));
        }
        if widgets::icon_button(ui, icon::TEXT_ITALIC, "Cursiva").clicked() {
            act = Some(Action::InsertSnippet("_${sel}_".into()));
        }
        if widgets::icon_button(ui, icon::TEXT_UNDERLINE, "Subrayado").clicked() {
            act = Some(Action::InsertSnippet("#underline[${sel}]".into()));
        }
        if widgets::icon_button(ui, icon::TEXT_STRIKETHROUGH, "Tachado").clicked() {
            act = Some(Action::InsertSnippet("#strike[${sel}]".into()));
        }
        widgets::vsep(ui, &pal);
        // Tamaño de fuente.
        ui.menu_button(RichText::new(fmt_pt(st.fmt.size_pt)).small(), |ui| {
            ui.set_min_width(120.0);
            for s in SIZES_PT {
                if ui
                    .selectable_label(*s == st.fmt.size_pt, format!("{} pt", fmt_pt(*s)))
                    .clicked()
                {
                    st.fmt.size_pt = *s;
                    act = Some(Action::InsertSnippet(format!(
                        "#text(size: {}pt)[${{sel}}]",
                        fmt_pt(*s)
                    )));
                    ui.close();
                }
            }
        });
        // Familia (precargada en hilo al arrancar; el botón reintenta).
        ui.menu_button(
            RichText::new(if st.fmt.font.is_empty() {
                "Aa"
            } else {
                st.fmt.font.as_str()
            })
            .small(),
            |ui| {
                ui.set_min_width(240.0);
                if st.fmt.fonts.is_none() {
                    ui.label(RichText::new("Cargando fuentes…").small());
                    if ui.button("Reintentar").clicked() {
                        st.fmt.fonts = Some(compile::typst_fonts());
                    }
                    return;
                }
                let fonts = st.fmt.fonts.clone().unwrap_or_default();
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
                                act = Some(Action::InsertSnippet(format!(
                                    "#text(font: \"{f}\")[${{sel}}]"
                                )));
                                ui.close();
                            }
                        }
                    });
            },
        );
        // Color.
        ui.menu_button(
            RichText::new(format!("A {}", st.fmt.color_hex))
                .small()
                .color(hex_to_color(&st.fmt.color_hex).unwrap_or(pal.text)),
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
                                act = Some(Action::InsertSnippet(format!(
                                    "#text(fill: rgb(\"{h}\"))[${{sel}}]"
                                )));
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
                        act = Some(Action::InsertSnippet(format!(
                            "#text(fill: rgb(\"{h}\"))[${{sel}}]"
                        )));
                        ui.close();
                    }
                });
            },
        );
        widgets::vsep(ui, &pal);
        // Alineación.
        ui.menu_button(RichText::new(icon::TEXT_ALIGN_LEFT).size(16.0), |ui| {
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
                    act = Some(Action::InsertSnippet(format!("#align({name})[${{sel}}]")));
                    ui.close();
                }
            }
        });
        // Enlace (despliega campo URL).
        if widgets::icon_button(ui, icon::LINK, "Enlace").clicked() {
            st.fmt.link_open = !st.fmt.link_open;
        }
        // Imagen (diálogo; se inserta tras la selección sin borrarla).
        if widgets::icon_button(ui, icon::IMAGE, "Imagen…").clicked()
            && let Some(body) = pick_image(st)
        {
            // Colapsa al final para no sustituir lo seleccionado.
            if let Some((_, b)) = st.editor.selection {
                st.editor.selection = Some((b, b));
                st.editor.cursor = Some(b);
            }
            act = Some(Action::InsertSnippet(body));
        }
        // Listas.
        ui.menu_button(RichText::new(icon::LIST_BULLETS).size(16.0), |ui| {
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
        // Encabezados.
        ui.menu_button(RichText::new(icon::TEXT_H).size(16.0), |ui| {
            ui.set_min_width(180.0);
            if ui.button("Texto normal").clicked() {
                act = Some(Action::PrefixLines(String::new()));
                ui.close();
            }
            for (label, mark) in [
                ("Título (= )", "= "),
                ("Subtítulo (== )", "== "),
                ("Sección (=== )", "=== "),
            ] {
                if ui.button(label).clicked() {
                    act = Some(Action::PrefixLines(mark.into()));
                    ui.close();
                }
            }
        });
        // Código en línea.
        if widgets::icon_button(ui, icon::CODE_SIMPLE, "Código").clicked() {
            act = Some(Action::InsertSnippet("`${sel:código}`".into()));
        }
        if floating {
            widgets::vsep(ui, &pal);
            if widgets::icon_button(ui, icon::PUSH_PIN, "Fijar en la barra superior").clicked() {
                st.fmt.pinned = true;
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

/// Diálogo de imagen → cuerpo `image(…)` con ruta relativa al documento.
fn pick_image(st: &mut State) -> Option<String> {
    let mut dlg =
        rfd::FileDialog::new().add_filter("Imagen", &["png", "jpg", "jpeg", "svg", "webp", "gif"]);
    if let Some(dir) = st.docs.cur().path.as_deref().and_then(|p| p.parent()) {
        dlg = dlg.set_directory(dir);
    }
    let p = dlg.pick_file()?;
    let s = st
        .docs
        .cur()
        .path
        .as_deref()
        .and_then(|d| d.parent())
        .and_then(|dir| p.strip_prefix(dir).ok())
        .map(|r| r.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned());
    Some(format!("image(\"{s}\", width: 80%)"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
