//! Widget de edición: `TextEdit` con números de línea y resaltado Typst.
//!
//! Sustituye a `egui_code_editor`: el resaltado sale de `highlight.rs`, y los
//! números se calculan a partir de las filas del galley, así que funcionan
//! también con ajuste de línea (cada línea lógica numera su primera fila).

use crate::highlight;
use crate::theme::Palette;
use egui::text::LayoutJob;
use egui::text_edit::TextEditOutput;
use egui::{Align2, FontId, Rect};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub struct Options {
    pub font_size: f32,
    pub wrap: bool,
    pub rows: usize,
}

/// `LayoutJob` cacheado por (hash del texto, tamaño, tema): el layouter se
/// llama en cada frame y analizar un documento grande no es gratis.
#[derive(Clone, Default)]
struct JobCache {
    key: u64,
    job: Option<Arc<LayoutJob>>,
}

fn cache_key(text: &str, size: f32, dark: bool) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    size.to_bits().hash(&mut h);
    dark.hash(&mut h);
    h.finish()
}

/// Dibuja el editor. `salt` identifica su estado (cursor, scroll); el
/// ScrollArea usa `"{salt}_outer_scroll"` (ver `editor::scroll_into_view`).
pub fn show(
    ui: &mut egui::Ui,
    salt: &str,
    text: &mut String,
    opt: &Options,
    pal: &Palette,
) -> TextEditOutput {
    let font = FontId::monospace(opt.font_size);
    let cache_id = ui.make_persistent_id(format!("{salt}_hl"));
    let digits = text.lines().count().max(1).to_string().len().max(2);
    let char_w = ui.ctx().fonts_mut(|f| f.glyph_width(&font, '0'));
    let gutter = digits as f32 * char_w + 18.0;

    let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
        let src = buf.as_str();
        let key = cache_key(src, opt.font_size, pal.dark);
        let cached: JobCache = ui.data(|d| d.get_temp(cache_id)).unwrap_or_default();
        let job = match cached.job {
            Some(j) if cached.key == key => j,
            _ => {
                let j = Arc::new(highlight::layout_job(src, font.clone(), pal));
                ui.data_mut(|d| {
                    d.insert_temp(
                        cache_id,
                        JobCache {
                            key,
                            job: Some(j.clone()),
                        },
                    )
                });
                j
            }
        };
        let mut job = (*job).clone();
        job.wrap.max_width = if opt.wrap { wrap_width } else { f32::INFINITY };
        ui.fonts_mut(|f| f.layout_job(job))
    };

    let mut output = None;
    egui::ScrollArea::both()
        .id_salt(format!("{salt}_outer_scroll"))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(gutter);
                // Fondo de la línea actual: se reserva antes para quedar detrás del texto.
                let line_bg = ui.painter().add(egui::Shape::Noop);
                let width = if opt.wrap {
                    ui.available_width()
                } else {
                    f32::INFINITY
                };
                let out = egui::TextEdit::multiline(text)
                    .id_salt(salt)
                    .code_editor()
                    .font(font.clone())
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::symmetric(6, 2))
                    .desired_rows(opt.rows)
                    .desired_width(width)
                    .layouter(&mut layouter)
                    .show(ui);
                paint_gutter(ui, &out, gutter, line_bg, &font, pal);
                output = Some(out);
            });
        });
    output.expect("TextEdit dibujado")
}

/// Números de línea (solo filas visibles) y resalte de la línea del cursor.
fn paint_gutter(
    ui: &egui::Ui,
    out: &TextEditOutput,
    gutter: f32,
    line_bg: egui::layers::ShapeIdx,
    font: &FontId,
    pal: &Palette,
) {
    let clip = ui.clip_rect();
    let offset = out.galley_pos.to_vec2();
    let rows = &out.galley.rows;
    // Línea lógica de cada fila (con ajuste, una línea ocupa varias filas).
    let mut lines = Vec::with_capacity(rows.len());
    let mut n = 1;
    for p in rows.iter() {
        lines.push(n);
        if p.ends_with_newline {
            n += 1;
        }
    }
    let cursor_line = out.cursor_range.and_then(|r| {
        let y = out.galley.pos_from_cursor(r.primary).center().y;
        rows.iter()
            .position(|p| p.rect().y_range().contains(y))
            .map(|i| lines[i])
    });
    let x_right = out.galley_pos.x - 8.0;
    let mut current: Option<Rect> = None;
    for (i, p) in rows.iter().enumerate() {
        let r = p.rect().translate(offset);
        let is_cur = Some(lines[i]) == cursor_line;
        if is_cur {
            current = Some(current.map_or(r, |c| c.union(r)));
        }
        let first_row = i == 0 || lines[i] != lines[i - 1];
        if first_row && r.bottom() >= clip.top() && r.top() <= clip.bottom() {
            let color = if is_cur { pal.text_dim } else { pal.text_faint };
            ui.painter().text(
                egui::pos2(x_right, r.top()),
                Align2::RIGHT_TOP,
                lines[i].to_string(),
                font.clone(),
                color,
            );
        }
    }
    if out.response.has_focus()
        && let Some(c) = current
    {
        let x = (out.galley_pos.x - gutter)..=clip.right().max(out.response.rect.right());
        let full = Rect::from_x_y_ranges(x, c.y_range());
        ui.painter().set(
            line_bg,
            egui::epaint::RectShape::filled(full, 0.0, pal.surface_hover.gamma_multiply(0.55)),
        );
    }
}
