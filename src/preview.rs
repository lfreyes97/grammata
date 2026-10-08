//! Vista previa del PDF real que produce typst.
//!
//! Solo se rasterizan las páginas visibles (más una pantalla de margen) y a
//! la resolución exacta del zoom × densidad de pantalla. Mientras llega el
//! render nuevo se dibuja la textura anterior escalada, así que recompilar o
//! hacer zoom no parpadea.

use crate::actions::Action;
use crate::compile::Key;
use crate::layout::Panel;
use crate::pdf_render::{self, Event, Job, PdfRenderer};
use crate::session::{FitMode, Prefs};
use crate::theme::{self, Palette};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Vec2, pos2, vec2};
use egui_phosphor::regular as icon;
use std::collections::HashMap;
use std::sync::Arc;

/// Puntos PDF → px lógicos al 100 % (tamaño real a 96 dpi).
const PT_TO_PX: f32 = 96.0 / 72.0;
const GAP: f32 = 20.0;
const MARGIN: f32 = 28.0;
pub const ZOOM_MIN: f32 = 0.1;
pub const ZOOM_MAX: f32 = 6.0;
/// Texturas que se conservan aunque estén lejos del viewport.
const KEEP_TEXTURES: usize = 10;

struct PageTex {
    tex: egui::TextureHandle,
    scale: f32,
    generation: u64,
}

/// Lo que la vista previa necesita saber del resto de la app.
pub struct Info {
    pub compiling: bool,
    pub auto: bool,
    pub errors: usize,
    /// El PDF mostrado es de una versión anterior del texto.
    pub stale: bool,
    pub has_build: bool,
}

pub struct Preview {
    renderer: PdfRenderer,
    pub loaded: Option<Key>,
    generation: u64,
    /// Tamaño de cada página en puntos (de la última generación cargada).
    sizes: Vec<Vec2>,
    textures: HashMap<usize, PageTex>,
    pub current_page: usize,
    scroll_to: Option<usize>,
    load_error: Option<String>,
    /// Zoom efectivo del último frame (en modo ajustar lo calcula el ancho).
    pub effective_zoom: f32,
    last_content_h: f32,
    last_offset: Vec2,
}

impl Preview {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            renderer: PdfRenderer::new(ctx),
            loaded: None,
            generation: 0,
            sizes: Vec::new(),
            textures: HashMap::new(),
            current_page: 0,
            scroll_to: None,
            load_error: None,
            effective_zoom: 1.0,
            last_content_h: 0.0,
            last_offset: Vec2::ZERO,
        }
    }

    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }

    /// Páginas con textura de la generación actual (para las pruebas).
    #[cfg(test)]
    pub fn fresh_pages(&self) -> usize {
        self.textures.values().filter(|t| t.generation == self.generation).count()
    }

    /// Carga un PDF. Si es otro documento, vuelve arriba y descarta texturas.
    pub fn load(&mut self, key: Key, data: Arc<Vec<u8>>) {
        if self.loaded.map(|k| k.doc) != Some(key.doc) {
            self.reset_view();
        }
        self.generation = self.renderer.load(data);
        self.loaded = Some(key);
        self.load_error = None;
    }

    pub fn clear(&mut self) {
        self.loaded = None;
        self.reset_view();
    }

    fn reset_view(&mut self) {
        self.sizes.clear();
        self.textures.clear();
        self.current_page = 0;
        self.scroll_to = Some(0);
    }

    pub fn goto_page(&mut self, p: usize) {
        if !self.sizes.is_empty() {
            self.scroll_to = Some(p.min(self.sizes.len() - 1));
        }
    }

    /// Recoge páginas y metadatos del hilo de render.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Some(ev) = self.renderer.try_recv() {
            match ev {
                Event::Loaded { generation, sizes } if generation == self.generation => {
                    self.textures.retain(|&i, _| i < sizes.len());
                    self.current_page = self.current_page.min(sizes.len().saturating_sub(1));
                    self.sizes = sizes;
                }
                Event::Failed { generation, error } if generation == self.generation => {
                    self.load_error = Some(error);
                }
                Event::Page { generation, job, image } if generation == self.generation => {
                    match self.textures.get_mut(&job.page) {
                        Some(t) => {
                            t.tex.set(image, egui::TextureOptions::LINEAR);
                            t.scale = job.scale;
                            t.generation = generation;
                        }
                        None => {
                            let tex = ctx.load_texture(
                                format!("pdf-page-{}", job.page),
                                image,
                                egui::TextureOptions::LINEAR,
                            );
                            self.textures.insert(job.page, PageTex { tex, scale: job.scale, generation });
                        }
                    }
                }
                _ => {} // de una generación anterior
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, prefs: &mut Prefs, info: &Info, actions: &mut Vec<Action>) {
        let pal = theme::palette(ui.ctx());
        self.toolbar(ui, prefs, info, &pal, actions);

        let canvas = egui::Frame::new().fill(pal.canvas);
        canvas.show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            if self.sizes.is_empty() {
                self.empty_state(ui, info, &pal, actions);
                return;
            }
            self.pages(ui, prefs, &pal);
            if info.errors > 0 && info.stale {
                error_banner(ui, &pal, info.errors, actions);
            }
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, prefs: &mut Prefs, info: &Info, pal: &Palette, actions: &mut Vec<Action>) {
        crate::widgets::toolbar_frame(pal).show(ui, |ui| {
            ui.horizontal(|ui| {
                let z = self.effective_zoom;
                if crate::widgets::icon_button(ui, icon::MAGNIFYING_GLASS_MINUS, "Alejar  (Ctrl+rueda)").clicked() {
                    prefs.fit = FitMode::Manual;
                    prefs.zoom = step_zoom(z, -1);
                }
                let label = match prefs.fit {
                    FitMode::Width => "Ancho".to_string(),
                    FitMode::Page => "Página".to_string(),
                    FitMode::Manual => format!("{:.0} %", z * 100.0),
                };
                ui.menu_button(egui::RichText::new(label).monospace(), |ui| {
                    ui.set_min_width(170.0);
                    if ui.add(egui::Button::new(format!("{}  Ajustar al ancho", icon::ARROWS_OUT_LINE_HORIZONTAL)).selected(prefs.fit == FitMode::Width)).clicked() {
                        prefs.fit = FitMode::Width;
                        ui.close();
                    }
                    if ui.add(egui::Button::new(format!("{}  Página completa", icon::FRAME_CORNERS)).selected(prefs.fit == FitMode::Page)).clicked() {
                        prefs.fit = FitMode::Page;
                        ui.close();
                    }
                    ui.separator();
                    for p in [50, 75, 100, 125, 150, 200, 300] {
                        let sel = prefs.fit == FitMode::Manual && (prefs.zoom * 100.0).round() as i32 == p;
                        if ui.add(egui::Button::new(format!("{p} %")).selected(sel)).clicked() {
                            prefs.fit = FitMode::Manual;
                            prefs.zoom = p as f32 / 100.0;
                            ui.close();
                        }
                    }
                });
                if crate::widgets::icon_button(ui, icon::MAGNIFYING_GLASS_PLUS, "Acercar  (Ctrl+rueda)").clicked() {
                    prefs.fit = FitMode::Manual;
                    prefs.zoom = step_zoom(z, 1);
                }

                crate::widgets::vsep(ui, pal);

                let n = self.sizes.len();
                ui.add_enabled_ui(n > 0, |ui| {
                    if crate::widgets::icon_button(ui, icon::CARET_UP, "Página anterior  (RePág)").clicked() {
                        self.goto_page(self.current_page.saturating_sub(1));
                    }
                    let txt = if n == 0 { "– / –".to_string() } else { format!("{} / {}", self.current_page + 1, n) };
                    ui.label(egui::RichText::new(txt).monospace().color(pal.text_dim));
                    if crate::widgets::icon_button(ui, icon::CARET_DOWN, "Página siguiente  (AvPág)").clicked() {
                        self.goto_page(self.current_page + 1);
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::widgets::icon_button(ui, icon::ARROWS_CLOCKWISE, "Compilar  (F5)").clicked() {
                        actions.push(Action::Compile);
                    }
                    if info.compiling {
                        ui.add(egui::Spinner::new().size(14.0).color(pal.accent));
                    } else if info.errors > 0 {
                        let r = ui.add(
                            egui::Label::new(egui::RichText::new(format!("{} {}", icon::WARNING_CIRCLE, info.errors)).color(pal.err))
                                .sense(Sense::click()),
                        );
                        if r.on_hover_text("Errores de compilación — ver Problemas").clicked() {
                            actions.push(Action::ShowPanel(Panel::Problems));
                        }
                    } else if info.stale && !info.auto {
                        ui.label(egui::RichText::new("desactualizado").small().color(pal.warn))
                            .on_hover_text("Autocompilar está desactivado: pulsa F5");
                    }
                });
            });
        });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui, info: &Info, pal: &Palette, actions: &mut Vec<Action>) {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.3).min(160.0));
            let (ic, title, sub) = if info.compiling {
                (icon::HOURGLASS_MEDIUM, "Compilando…", "")
            } else if let Some(e) = &self.load_error {
                (icon::FILE_X, "No se pudo mostrar el PDF", e.as_str())
            } else if info.errors > 0 {
                (icon::WARNING_CIRCLE, "La compilación falló", "Revisa el panel Problemas para ver qué falla.")
            } else if !info.has_build {
                (icon::FILE_PDF, "Sin vista previa todavía", "Compila el documento para verlo aquí.")
            } else {
                (icon::FILE_DASHED, "El documento no tiene páginas", "")
            };
            ui.label(egui::RichText::new(ic).size(44.0).color(pal.text_faint));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(title).size(15.0).color(pal.text));
            if !sub.is_empty() {
                ui.label(egui::RichText::new(sub).color(pal.text_dim));
            }
            ui.add_space(10.0);
            if info.errors > 0 && !info.compiling {
                if ui.button(format!("{}  Ver problemas", icon::LIST_MAGNIFYING_GLASS)).clicked() {
                    actions.push(Action::ShowPanel(Panel::Problems));
                }
            } else if !info.compiling && ui.button(format!("{}  Compilar  (F5)", icon::PLAY)).clicked() {
                actions.push(Action::Compile);
            }
        });
    }

    fn pages(&mut self, ui: &mut egui::Ui, prefs: &mut Prefs, pal: &Palette) {
        let avail = ui.available_size();
        let max_w = self.sizes.iter().map(|s| s.x).fold(1.0, f32::max) * PT_TO_PX;
        let max_h = self.sizes.iter().map(|s| s.y).fold(1.0, f32::max) * PT_TO_PX;
        let scrollbar = ui.spacing().scroll.bar_width + ui.spacing().scroll.bar_outer_margin;
        let fit_w = ((avail.x - 2.0 * MARGIN - scrollbar) / max_w).max(ZOOM_MIN);
        let zoom = match prefs.fit {
            FitMode::Width => fit_w,
            FitMode::Page => fit_w.min((avail.y - 2.0 * MARGIN) / max_h),
            FitMode::Manual => prefs.zoom,
        }
        .clamp(ZOOM_MIN, ZOOM_MAX);

        // Geometría en coordenadas de contenido.
        let mut tops = Vec::with_capacity(self.sizes.len());
        let mut y = MARGIN;
        for s in &self.sizes {
            tops.push(y);
            y += s.y * PT_TO_PX * zoom + GAP;
        }
        let content_h = y - GAP + MARGIN;
        let content_w = (max_w * zoom + 2.0 * MARGIN).max(avail.x - scrollbar);

        let mut area = egui::ScrollArea::both().id_salt("pdf-canvas").auto_shrink([false, false]);
        if let Some(p) = self.scroll_to.take() {
            let top = tops.get(p).copied().unwrap_or(0.0);
            area = area.vertical_scroll_offset((top - MARGIN * 0.5).max(0.0));
        } else if self.last_content_h > 0.0 && (content_h - self.last_content_h).abs() > 0.5 && (zoom - self.effective_zoom).abs() > 1e-4 {
            // Zoom: conserva la posición relativa del scroll.
            let k = content_h / self.last_content_h;
            area = area.scroll_offset(vec2(self.last_offset.x * k, self.last_offset.y * k));
        }
        self.effective_zoom = zoom;

        let ppp = ui.ctx().pixels_per_point();
        let out = area.show_viewport(ui, |ui, viewport| {
            let origin = ui.max_rect().min;
            ui.allocate_rect(Rect::from_min_size(origin, vec2(content_w, content_h)), Sense::hover());
            let painter = ui.painter();
            let prefetch = viewport.expand2(vec2(0.0, viewport.height()));
            let probe = viewport.top() + viewport.height() * 0.3;
            let mut visible = Vec::new();
            let mut near = Vec::new();
            let mut keep = Vec::new();
            for (i, s) in self.sizes.iter().enumerate() {
                let size = *s * PT_TO_PX * zoom;
                let local = Rect::from_min_size(pos2((content_w - size.x) * 0.5, tops[i]), size);
                if local.top() <= probe {
                    self.current_page = i;
                }
                if !local.intersects(prefetch) {
                    continue;
                }
                keep.push(i);
                let rect = local.translate(origin.to_vec2());
                let want = desired_scale(*s, zoom, ppp);
                let tex = self.textures.get(&i);
                let fresh = tex.is_some_and(|t| t.generation == self.generation && t.scale == want);
                if !fresh {
                    let job = Job { page: i, scale: want };
                    if local.intersects(viewport) { visible.push(job) } else { near.push(job) }
                }
                if !local.intersects(viewport) {
                    continue;
                }
                page_shadow(painter, rect, pal);
                match tex {
                    Some(t) => {
                        painter.image(t.tex.id(), rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        painter.rect_filled(rect, CornerRadius::same(1), Color32::WHITE);
                        painter.text(rect.center(), Align2::CENTER_CENTER, icon::HOURGLASS_MEDIUM, FontId::proportional(22.0), Color32::from_gray(190));
                    }
                }
                if ui.rect_contains_pointer(rect) {
                    page_badge(painter, rect, i + 1, pal);
                }
            }
            visible.extend(near);
            self.renderer.set_jobs(self.generation, visible);
            self.evict(&keep);
        });
        self.last_content_h = content_h;
        self.last_offset = out.state.offset;

        // Ctrl+rueda (o pellizco) sobre el lienzo = zoom.
        let hovered = ui.rect_contains_pointer(out.inner_rect);
        let dz = ui.input(|i| i.zoom_delta());
        if hovered && (dz - 1.0).abs() > 1e-4 {
            prefs.fit = FitMode::Manual;
            prefs.zoom = (zoom * dz).clamp(ZOOM_MIN, ZOOM_MAX);
        }
        // RePág/AvPág: con el puntero encima y sin un campo de texto con foco.
        if hovered && ui.ctx().memory(|m| m.focused().is_none()) {
            let (up, down) = ui.input(|i| (i.key_pressed(egui::Key::PageUp), i.key_pressed(egui::Key::PageDown)));
            if up {
                self.goto_page(self.current_page.saturating_sub(1));
            }
            if down {
                self.goto_page(self.current_page + 1);
            }
        }
    }

    /// Libera texturas lejos de la vista cuando hay muchas.
    fn evict(&mut self, keep: &[usize]) {
        if self.textures.len() <= KEEP_TEXTURES {
            return;
        }
        let drop: Vec<usize> = self.textures.keys().copied().filter(|i| !keep.contains(i)).collect();
        for i in drop {
            self.textures.remove(&i);
            self.renderer.forget(i);
        }
    }
}

/// Escala de render (px de pantalla por punto PDF), cuantizada a pasos de
/// 2^(1/8) para no re-renderizar en cada frame de un zoom continuo.
fn desired_scale(page_pt: Vec2, zoom: f32, ppp: f32) -> f32 {
    let raw = zoom * PT_TO_PX * ppp;
    let q = 2f32.powf((raw.log2() * 8.0).round() / 8.0);
    let cap = pdf_render::MAX_SIDE / page_pt.x.max(page_pt.y).max(1.0);
    q.min(cap)
}

fn step_zoom(z: f32, dir: i32) -> f32 {
    const STEPS: [f32; 13] = [0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 2.0, 3.0, 4.0];
    let next = if dir > 0 {
        STEPS.iter().copied().find(|&s| s > z + 0.01)
    } else {
        STEPS.iter().rev().copied().find(|&s| s < z - 0.01)
    };
    next.unwrap_or(z).clamp(ZOOM_MIN, ZOOM_MAX)
}

fn page_shadow(painter: &egui::Painter, rect: Rect, pal: &Palette) {
    let shadow = egui::Shadow {
        offset: [0, 2],
        blur: 10,
        spread: 0,
        color: Color32::from_black_alpha(if pal.dark { 120 } else { 45 }),
    };
    painter.add(shadow.as_shape(rect, CornerRadius::same(1)));
}

fn page_badge(painter: &egui::Painter, rect: Rect, n: usize, pal: &Palette) {
    let galley = painter.layout_no_wrap(n.to_string(), FontId::proportional(11.5), Color32::WHITE);
    let pad = vec2(8.0, 3.0);
    let size = galley.size() + pad * 2.0;
    let r = Rect::from_min_size(rect.right_bottom() - size - vec2(10.0, 10.0), size);
    painter.rect_filled(r, CornerRadius::same(10), pal.chrome.gamma_multiply(0.85));
    painter.galley(r.min + pad, galley, Color32::WHITE);
}

fn error_banner(ui: &mut egui::Ui, pal: &Palette, errors: usize, actions: &mut Vec<Action>) {
    let rect = ui.max_rect();
    let text = format!(
        "{}  {} {} — mostrando la última versión válida",
        icon::WARNING_CIRCLE,
        errors,
        if errors == 1 { "error" } else { "errores" }
    );
    egui::Area::new(ui.id().with("err-banner"))
        .fixed_pos(rect.center_top() + vec2(0.0, 12.0))
        .pivot(Align2::CENTER_TOP)
        .order(egui::Order::Middle)
        .show(ui.ctx(), |ui| {
            let r = egui::Frame::new()
                .fill(pal.surface)
                .stroke(egui::Stroke::new(1.0, pal.err))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(12, 6))
                .shadow(ui.visuals().popup_shadow)
                .show(ui, |ui| ui.label(egui::RichText::new(text).color(pal.err)))
                .response
                .interact(Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Ver problemas");
            if r.clicked() {
                actions.push(Action::ShowPanel(Panel::Problems));
            }
        });
}
