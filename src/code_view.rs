//! Widget de edición: `TextEdit` con números de línea y resaltado Typst.
//!
//! Sustituye a `egui_code_editor`: el resaltado sale de `highlight.rs`, y los
//! números se calculan a partir de las filas del galley, así que funcionan
//! también con ajuste de línea (cada línea lógica numera su primera fila).

use crate::highlight;
use crate::theme::Palette;
use egui::epaint::text::{ByteIndex, Galley, LayoutSection};
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

/// Dibuja el editor. `salt` identifica su estado (cursor, scroll).
/// Devuelve la salida del `TextEdit` y el id del ScrollArea exterior (para
/// llevar el scroll hasta el cursor desde fuera, sin replicar hashes).
pub fn show(
    ui: &mut egui::Ui,
    salt: &str,
    text: &mut String,
    opt: &Options,
    pal: &Palette,
) -> (TextEditOutput, egui::Id) {
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
        if !opt.wrap {
            job.wrap.max_width = f32::INFINITY;
            return ui.fonts_mut(|f| f.layout_job(job));
        }
        job.wrap.max_width = wrap_width;
        let ppp = ui.ctx().pixels_per_point();
        ui.fonts_mut(|f| layout_hanging(job, |j| f.layout_job(j), ppp))
    };

    let mut output = None;
    let area = egui::ScrollArea::both()
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
    (output.expect("TextEdit dibujado"), area.id)
}

/// Con ajuste de línea, las filas de continuación arrancan bajo el texto de
/// su línea (tras la sangría y la viñeta de lista), no en el margen. Cada
/// línea lógica se maqueta aparte (egui cachea cada una) con el ancho menos
/// la sangría, y sus filas de continuación se desplazan esa medida.
fn layout_hanging(
    job: LayoutJob,
    mut layout: impl FnMut(LayoutJob) -> Arc<Galley>,
    pixels_per_point: f32,
) -> Arc<Galley> {
    let width = job.wrap.max_width;
    if job.text.is_empty() {
        return layout(job);
    }
    let mut galleys = Vec::new();
    for mut para in paragraphs(&job) {
        let k = hang_chars(&para.text);
        let g = layout(para.clone());
        let indent = match g.rows.as_slice() {
            [first, _, ..] if k > 0 => first.glyphs.get(k).map_or(0.0, |gl| gl.pos.x),
            _ => 0.0,
        };
        if indent <= 0.0 || indent > width * 0.6 {
            galleys.push(g);
            continue;
        }
        para.wrap.max_width = width - indent;
        let mut g = (*layout(para)).clone();
        for row in g.rows.iter_mut().skip(1) {
            row.pos.x += indent;
        }
        galleys.push(Arc::new(g));
    }
    Arc::new(Galley::concat(Arc::new(job), &galleys, pixels_per_point))
}

/// Parte `job` en líneas lógicas como egui (`\n` fuera; el final se queda
/// en la última), con las secciones de estilo recortadas a cada una.
fn paragraphs(job: &LayoutJob) -> Vec<LayoutJob> {
    let text = &job.text;
    let mut out = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = text[start..].find('\n').map_or(text.len(), |i| start + i);
        if end == text.len() - 1 && text.ends_with('\n') {
            end += 1;
        }
        let sections = job
            .sections
            .iter()
            .filter_map(|s| {
                let (a, b) = (
                    usize::from(s.byte_range.start),
                    usize::from(s.byte_range.end),
                );
                let (a, b) = (a.max(start), b.min(end));
                (a < b || (a == b && start == end)).then(|| LayoutSection {
                    leading_space: 0.0,
                    byte_range: ByteIndex(a - start)..ByteIndex(b - start),
                    format: s.format.clone(),
                })
            })
            .collect();
        out.push(LayoutJob {
            text: text[start..end].to_owned(),
            sections,
            first_row_min_height: 0.0,
            ..job.clone()
        });
        start = end + 1;
    }
    // Texto acabado en `\n`… seguido de nada: egui lo deja en el último
    // párrafo; aquí ya lo incluye el `end += 1`.
    out
}

/// Caracteres de sangría colgante de una línea: espacios iniciales más la
/// viñeta de lista (`- `, `+ `, `12. `), si la hay.
fn hang_chars(line: &str) -> usize {
    let ws = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
    let rest = &line[ws..];
    let marker = if rest.starts_with("- ") || rest.starts_with("+ ") {
        2
    } else {
        let d = rest.chars().take_while(char::is_ascii_digit).count();
        if d > 0 && rest[d..].starts_with(". ") {
            d + 2
        } else {
            0
        }
    };
    ws + marker
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `show` devuelve el id del ScrollArea que localiza su estado: con él
    /// `editor::scroll_into_view` desplaza sin replicar hashes (una
    /// divergencia movía el cursor sin desplazar el editor, en silencio).
    #[test]
    fn show_devuelve_id_del_scroll() {
        let mut h = egui_kittest::Harness::new_ui(|ui| {
            let mut text = String::from("hola\n");
            let opt = Options {
                font_size: 14.0,
                wrap: false,
                rows: 4,
            };
            let pal = crate::theme::palette_for(false);
            let (_out, sid) = show(ui, "editor-test", &mut text, &opt, &pal);
            assert!(
                egui::scroll_area::State::load(ui.ctx(), sid).is_some(),
                "el id devuelto no localiza el estado del ScrollArea"
            );
        });
        h.run_steps(2);
    }

    /// Las continuaciones de `  - …` arrancan bajo el texto de la viñeta,
    /// nada se sale del ancho y el texto (y sus saltos) queda intacto.
    #[test]
    fn sangria_colgante() {
        assert_eq!(hang_chars("  - uno"), 4);
        assert_eq!(hang_chars("12. dos"), 4);
        assert_eq!(hang_chars("#set"), 0);
        let mut h = egui_kittest::Harness::new_ui(|ui| {
            let src = "= Título\n\n  - una viñeta con bastante texto para que se parta en varias filas seguro\nfin\n";
            let pal = crate::theme::palette_for(false);
            let mut job = highlight::layout_job(src, FontId::monospace(14.0), &pal);
            job.wrap.max_width = 220.0;
            let ppp = ui.ctx().pixels_per_point();
            let g = ui.fonts_mut(|f| layout_hanging(job, |j| f.layout_job(j), ppp));
            assert_eq!(g.text(), src);
            let lines = g.rows.iter().filter(|r| r.ends_with_newline).count();
            assert_eq!(lines, src.matches('\n').count());
            // Filas de la viñeta: la 3.ª línea lógica (índices tras 2 saltos).
            let mut n = 0;
            let mut vin = Vec::new();
            for r in &g.rows {
                if n == 2 {
                    vin.push(r);
                }
                if r.ends_with_newline {
                    n += 1;
                }
            }
            assert!(vin.len() > 1, "la viñeta no se partió");
            let indent = vin[0].glyphs[4].pos.x;
            assert!(indent > 0.0);
            for r in &vin[1..] {
                assert!((r.pos.x - indent).abs() < 1.0, "{} vs {indent}", r.pos.x);
            }
            for r in &g.rows {
                assert!(r.rect().right() <= 220.0 + 1.0, "{:?}", r.rect());
            }
        });
        h.run_steps(1);
    }
}
