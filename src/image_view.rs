//! Visor de imágenes: una pestaña por archivo (png, jpg, gif, webp, svg,
//! pdf). Las renderiza el propio typst (`engine::render_image`) en un hilo
//! aparte, así se ve exactamente lo que dibujaría `#image`.

use crate::i18n::{tr, trf};
use crate::actions::Action;
use crate::app::State;
use crate::engine::{self, ImageRender};
use crate::theme::Palette;
use crate::widgets;
use egui::{Sense, Vec2};
use egui_phosphor::regular as icon;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

pub type ImageId = u64;

/// Formatos que acepta `#image` en typst.
const EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "pdf"];
/// Lado mayor de la textura (px).
const MAX_PX: f32 = 4096.0;
/// 100 % = 1 pt de typst a 96 ppp, como la vista previa.
const PT_TO_PX: f32 = 96.0 / 72.0;
const ZOOM_MIN: f32 = 0.05;
const ZOOM_MAX: f32 = 16.0;

/// ¿Se abre en el visor?
pub fn is_viewable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

pub struct ImageTab {
    pub id: ImageId,
    pub path: PathBuf,
    tex: Option<egui::TextureHandle>,
    size_pt: Vec2,
    px: (f64, f64),
    format: String,
    error: Option<String>,
    loading: bool,
    /// `None` = ajustar a la pestaña.
    zoom: Option<f32>,
}

impl ImageTab {
    pub fn loading(&self) -> bool {
        self.loading || (self.tex.is_none() && self.error.is_none())
    }

    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

type Loaded = (ImageId, Result<ImageRender, String>);

pub struct Images {
    pub list: Vec<ImageTab>,
    next_id: ImageId,
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
    ctx: egui::Context,
}

impl Images {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = channel();
        Self {
            list: Vec::new(),
            next_id: 0,
            tx,
            rx,
            ctx,
        }
    }

    pub fn get(&self, id: ImageId) -> Option<&ImageTab> {
        self.list.iter().find(|t| t.id == id)
    }

    /// Abre `path` (o devuelve la pestaña que ya lo muestra).
    pub fn open(&mut self, path: PathBuf) -> ImageId {
        if let Some(t) = self.list.iter().find(|t| t.path == path) {
            return t.id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.list.push(ImageTab {
            id,
            path,
            tex: None,
            size_pt: Vec2::ZERO,
            px: (0.0, 0.0),
            format: String::new(),
            error: None,
            loading: false,
            zoom: None,
        });
        self.load(id);
        id
    }

    pub fn close(&mut self, id: ImageId) {
        self.list.retain(|t| t.id != id);
    }

    /// (Re)lee la imagen del disco en segundo plano.
    fn load(&mut self, id: ImageId) {
        let Some(t) = self.list.iter_mut().find(|t| t.id == id) else {
            return;
        };
        t.loading = true;
        let (path, tx, ctx) = (t.path.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::Builder::new()
            .name("visor-imagen".into())
            .stack_size(16 << 20)
            .spawn(move || {
                let out = std::panic::catch_unwind(|| engine::render_image(&path, MAX_PX))
                    .unwrap_or_else(|_| Err(tr("fallo interno al leer la imagen").into()));
                let _ = tx.send((id, out));
                ctx.request_repaint();
            })
            .expect("hilo visor-imagen");
    }

    fn poll(&mut self) {
        while let Ok((id, out)) = self.rx.try_recv() {
            let Some(t) = self.list.iter_mut().find(|t| t.id == id) else {
                continue;
            };
            t.loading = false;
            match out {
                Ok(r) => {
                    t.tex = Some(self.ctx.load_texture(
                        format!("imagen-{id}"),
                        r.image,
                        egui::TextureOptions::LINEAR,
                    ));
                    t.size_pt = r.size_pt;
                    t.px = r.px;
                    t.format = r.format;
                    t.error = None;
                }
                Err(e) => {
                    t.tex = None;
                    t.error = Some(e);
                }
            }
        }
    }
}

/// Contenido de la pestaña de una imagen.
pub fn tab(st: &mut State, ui: &mut egui::Ui, id: ImageId) {
    st.images.poll();
    let pal = crate::theme::palette(ui.ctx());
    let mut reload = false;
    let Some(t) = st.images.list.iter_mut().find(|t| t.id == id) else {
        return;
    };
    let avail = ui.available_size();
    let natural = t.size_pt * PT_TO_PX;
    let fit = if natural.x > 0.0 && natural.y > 0.0 {
        ((avail.x - 32.0) / natural.x)
            .min((avail.y - 72.0) / natural.y)
            .clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        1.0
    };
    let zoom = t.zoom.unwrap_or(fit);

    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::icon_button(ui, icon::MAGNIFYING_GLASS_MINUS, tr("Alejar  (Ctrl+rueda)"))
                .clicked()
            {
                t.zoom = Some((zoom / 1.25).max(ZOOM_MIN));
            }
            let label = match t.zoom {
                None => trf!("{}  Ajustar", icon::FRAME_CORNERS),
                Some(z) => format!("{}  {:.0} %", icon::MAGNIFYING_GLASS, z * 100.0),
            };
            ui.menu_button(egui::RichText::new(label).monospace(), |ui| {
                ui.set_min_width(150.0);
                if ui
                    .add(
                        egui::Button::new(trf!("{}  Ajustar", icon::FRAME_CORNERS))
                            .selected(t.zoom.is_none()),
                    )
                    .clicked()
                {
                    t.zoom = None;
                    ui.close();
                }
                ui.separator();
                for p in [25, 50, 100, 200, 400] {
                    let sel = t.zoom.is_some_and(|z| (z * 100.0).round() as i32 == p);
                    if ui
                        .add(egui::Button::new(format!("{p} %")).selected(sel))
                        .clicked()
                    {
                        t.zoom = Some(p as f32 / 100.0);
                        ui.close();
                    }
                }
            });
            if widgets::icon_button(ui, icon::MAGNIFYING_GLASS_PLUS, tr("Acercar  (Ctrl+rueda)"))
                .clicked()
            {
                t.zoom = Some((zoom * 1.25).min(ZOOM_MAX));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, icon::ARROWS_CLOCKWISE, tr("Volver a leer del disco"))
                    .clicked()
                {
                    reload = true;
                }
                if widgets::icon_button(ui, icon::COPY, tr("Copiar ruta")).clicked() {
                    ui.ctx().copy_text(t.path.display().to_string());
                }
                if widgets::accent_button(ui, &pal, &trf!("{}  Insertar", icon::PLUS))
                    .on_hover_text(tr("Inserta #image(\"…\") en el documento actual, en el cursor"))
                    .clicked()
                {
                    st.actions.push(Action::InsertImage(t.path.clone()));
                }
                if t.loading() {
                    ui.add(egui::Spinner::new().size(14.0).color(pal.accent));
                }
            });
        });
        if t.tex.is_some() {
            ui.label(
                egui::RichText::new(info_line(t))
                    .small()
                    .color(pal.text_dim),
            );
        }
    });

    if let Some(e) = &t.error {
        message(ui, &pal, icon::FILE_X, tr("No se pudo mostrar la imagen"), e);
    } else if let Some(tex) = &t.tex {
        let size = natural * zoom;
        let out = egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let w = ui.available_width().max(size.x + 32.0);
                let h = ui.available_height().max(size.y + 32.0);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
                let img = egui::Rect::from_center_size(rect.center(), size);
                ui.painter().rect_filled(img.expand(1.0), 0.0, pal.border);
                ui.painter().image(
                    tex.id(),
                    img,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            });
        // Ctrl+rueda (o pellizco) = zoom.
        let dz = ui.input(|i| i.zoom_delta());
        if ui.rect_contains_pointer(out.inner_rect) && (dz - 1.0).abs() > 1e-4 {
            t.zoom = Some((zoom * dz).clamp(ZOOM_MIN, ZOOM_MAX));
        }
    } else {
        message(ui, &pal, icon::HOURGLASS_MEDIUM, tr("Cargando…"), "");
    }

    if reload {
        st.images.load(id);
    }
}

fn info_line(t: &ImageTab) -> String {
    let (w, h) = t.px;
    format!(
        "{} × {} px  ·  {}  ·  {:.0} × {:.0} pt",
        w.round(),
        h.round(),
        t.format.to_uppercase(),
        t.size_pt.x,
        t.size_pt.y
    )
}

fn message(ui: &mut egui::Ui, pal: &Palette, ic: &str, title: &str, sub: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() * 0.3).min(160.0));
        ui.label(egui::RichText::new(ic).size(36.0).color(pal.text_faint));
        ui.add_space(6.0);
        ui.label(egui::RichText::new(title).strong().color(pal.text));
        if !sub.is_empty() {
            ui.label(egui::RichText::new(sub).small().color(pal.text_dim));
        }
    });
}

/// Ruta para `#image` desde el documento `doc` (si tiene ruta) con raíz
/// `root`: relativa a su carpeta si la imagen cuelga de ella, si no
/// absoluta desde la raíz (`/img/x.png`). `None` = fuera de la raíz.
pub fn image_ref(img: &Path, doc: Option<&Path>, root: &Path) -> Option<String> {
    let join = |p: &Path| {
        p.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    };
    if let Some(dir) = doc.and_then(Path::parent)
        && let Ok(rel) = img.strip_prefix(dir)
    {
        return Some(join(rel));
    }
    img.strip_prefix(root).ok().map(|r| format!("/{}", join(r)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas_para_image() {
        let root = Path::new("/p");
        let doc = Some(Path::new("/p/cap/a.typ"));
        assert_eq!(
            image_ref(Path::new("/p/cap/img/x.png"), doc, root).as_deref(),
            Some("img/x.png")
        );
        assert_eq!(
            image_ref(Path::new("/p/fig/y.svg"), doc, root).as_deref(),
            Some("/fig/y.svg")
        );
        assert_eq!(image_ref(Path::new("/otro/z.png"), doc, root), None);
        assert!(is_viewable(Path::new("a.SVG")));
        assert!(!is_viewable(Path::new("a.typ")));
    }

    #[test]
    fn renderiza_svg_y_png() {
        let d = std::env::temp_dir().join(format!("visor-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let svg = d.join("c \"q\".svg");
        std::fs::write(
            &svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="red"/></svg>"#,
        )
        .unwrap();
        let r = engine::render_image(&svg, 512.0).unwrap();
        assert_eq!(r.format, "svg");
        assert!(r.size_pt.x > r.size_pt.y);
        let px = r.image.pixels[r.image.size[0] * r.image.size[1] / 2];
        assert!(px.r() > 200 && px.g() < 50, "{px:?}");
        assert!(engine::render_image(&d.join("no-existe.png"), 512.0).is_err());
        std::fs::remove_dir_all(&d).ok();
    }
}
