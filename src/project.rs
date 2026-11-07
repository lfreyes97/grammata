//! Nuevo proyecto: diálogo con nombre, ubicación y tipo. Crea la carpeta, la
//! fija como raíz del gestor de archivos y, según el tipo, abre `main.typ`
//! (vacío) o el builder de plantillas apuntando a esa carpeta.

use crate::actions::Action;
use crate::theme;
use crate::widgets;
use egui::{CornerRadius, Margin, RichText, Sense, Stroke};
use egui_phosphor::regular as icon;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Carpeta con un `main.typ` vacío.
    Empty,
    /// Carpeta vacía + builder de plantillas con ese tipo (escribe
    /// `template.typ` y `main.typ` al pulsar Crear en el panel).
    Template(crate::templates::Kind),
}

impl Kind {
    /// Icono, nombre y descripción corta de la tarjeta.
    fn card(self) -> (&'static str, &'static str, &'static str) {
        use crate::templates::Kind as T;
        let desc = match self {
            Kind::Empty => "Solo main.typ, para empezar desde cero.",
            Kind::Template(T::Article) => "Portada, resumen e índice en la primera página.",
            Kind::Template(T::Report) => "Portada a página completa y capítulos.",
            Kind::Template(T::Book) => "Capítulos en página impar, doble cara.",
            Kind::Template(T::Notes) => "Apuntes o ejercicios con cajas y soluciones.",
            Kind::Template(T::Slides) => "Diapositivas 16:9.",
        };
        match self {
            Kind::Empty => (icon::FILE, "Vacío", desc),
            Kind::Template(k) => (k.icon(), k.name(), desc),
        }
    }
}

/// Estado del diálogo.
#[derive(Clone, Debug)]
pub struct NewProject {
    pub name: String,
    pub parent: PathBuf,
    pub kind: Kind,
    pub error: Option<String>,
    focus: bool,
}

impl NewProject {
    /// Ubicación por defecto: junto a la raíz actual del gestor; si no hay,
    /// la carpeta personal.
    pub fn new(files_root: Option<&Path>) -> Self {
        let parent = files_root
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            name: String::new(),
            parent,
            kind: Kind::Empty,
            error: None,
            focus: true,
        }
    }

    pub fn target(&self) -> PathBuf {
        self.parent.join(self.name.trim())
    }

    /// Crea la carpeta (y `main.typ` si es vacío). No reutiliza carpetas
    /// existentes: así nunca pisa nada. Devuelve la carpeta creada.
    pub fn create(&self) -> Result<PathBuf, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Escribe un nombre".into());
        }
        if !crate::files::valid_name(name) {
            return Err("Nombre no válido (sin / ni \\)".into());
        }
        if !self.parent.is_dir() {
            return Err(format!("No existe la carpeta {}", self.parent.display()));
        }
        let dir = self.target();
        if dir.exists() {
            return Err(format!("Ya existe {}", dir.display()));
        }
        std::fs::create_dir(&dir).map_err(|e| format!("No se pudo crear la carpeta: {e}"))?;
        if self.kind == Kind::Empty {
            let main = dir.join(crate::templates::MAIN_FILE);
            if let Err(e) = std::fs::write(&main, "") {
                let _ = std::fs::remove_dir(&dir);
                return Err(format!("No se pudo crear {}: {e}", main.display()));
            }
        }
        Ok(dir)
    }
}

/// Muestra el modal. Devuelve `true` si hay que cerrarlo.
pub fn show(
    ctx: &egui::Context,
    pal: &theme::Palette,
    np: &mut NewProject,
    actions: &mut Vec<Action>,
) -> bool {
    widgets::modal(ctx, "new-project", 440.0, |ui| dialog(ui, pal, np, actions)).inner
}

fn dialog(
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    np: &mut NewProject,
    actions: &mut Vec<Action>,
) -> bool {
    let mut close = false;
    widgets::modal_header(
        ui,
        pal,
        icon::FOLDER_PLUS,
        "Nuevo proyecto",
        Some("Una carpeta propia para el documento y sus recursos."),
    );

    // Nombre.
    widgets::section_label(ui, pal, "Nombre");
    let r = ui.add(
        egui::TextEdit::singleline(&mut np.name)
            .hint_text("mi-articulo")
            .margin(widgets::FIELD_MARGIN)
            .desired_width(f32::INFINITY),
    );
    if np.focus {
        r.request_focus();
        np.focus = false;
    }
    if r.changed() {
        np.error = None;
    }
    ui.add_space(8.0);

    // Ubicación: ruta en un campo de solo lectura + botón.
    widgets::section_label(ui, pal, "Ubicación");
    ui.horizontal(|ui| {
        let pick = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let pick =
                widgets::secondary_button(ui, pal, &format!("{}  Cambiar…", icon::FOLDER_OPEN))
                    .on_hover_text("Elegir la carpeta donde se crea el proyecto");
            egui::Frame::new()
                .fill(pal.chrome)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(8, 7))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    // La fila va de derecha a izquierda; la ruta, no.
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(np.parent.display().to_string())
                                    .monospace()
                                    .small()
                                    .color(pal.text_dim),
                            )
                            .truncate(),
                        )
                        .on_hover_text(np.parent.display().to_string());
                    });
                });
            pick
        });
        if pick.inner.clicked()
            && let Some(d) = rfd::FileDialog::new()
                .set_directory(&np.parent)
                .pick_folder()
        {
            np.parent = d;
            np.error = None;
        }
    });
    ui.add_space(8.0);

    // Tipo: vacío o una de las plantillas, en tarjetas de dos en dos.
    widgets::section_label(ui, pal, "Tipo");
    let kinds: Vec<Kind> = std::iter::once(Kind::Empty)
        .chain(crate::templates::Kind::ALL.map(Kind::Template))
        .collect();
    for pair in kinds.chunks(2) {
        ui.columns(2, |c| {
            for (ui, &k) in c.iter_mut().zip(pair) {
                kind_card(ui, pal, np, k);
            }
        });
    }

    // Destino o error.
    ui.add_space(6.0);
    if let Some(e) = &np.error {
        ui.label(
            RichText::new(format!("{}  {e}", icon::WARNING_CIRCLE))
                .small()
                .color(pal.err),
        );
    } else if !np.name.trim().is_empty() {
        ui.add(
            egui::Label::new(
                RichText::new(format!("Se creará {}", np.target().display()))
                    .small()
                    .color(pal.text_faint),
            )
            .truncate(),
        );
    } else {
        ui.label(RichText::new(" ").small());
    }

    let can = !np.name.trim().is_empty();
    widgets::modal_footer(ui, |ui| {
        let enter = can && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if widgets::primary_button(ui, pal, "Crear", can).clicked() || enter {
            match np.create() {
                Ok(dir) => {
                    actions.push(Action::OpenProject { dir, kind: np.kind });
                    close = true;
                }
                Err(e) => np.error = Some(e),
            }
        }
        if widgets::secondary_button(ui, pal, "Cancelar").clicked()
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            close = true;
        }
    });
    close
}

/// Tarjeta seleccionable de tipo de proyecto.
fn kind_card(ui: &mut egui::Ui, pal: &theme::Palette, np: &mut NewProject, kind: Kind) {
    let (ic, title, desc) = kind.card();
    let on = np.kind == kind;
    let mut p = egui::Frame::new()
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
        .begin(ui);
    {
        let ui = &mut p.content_ui;
        ui.set_width(ui.available_width());
        // Misma altura en cada fila aunque la descripción ocupe menos.
        ui.set_min_height(56.0);
        ui.horizontal(|ui| {
            let c = if on { pal.accent } else { pal.text_dim };
            ui.label(RichText::new(ic).size(20.0).color(c));
            ui.label(RichText::new(title).strong().color(pal.text));
            if on {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(icon::CHECK_CIRCLE).color(pal.accent));
                });
            }
        });
        ui.add_space(2.0);
        // `ui.columns` justifica: sin un layout propio, la etiqueta estira
        // el espacio entre letras.
        ui.vertical(|ui| {
            ui.add(egui::Label::new(RichText::new(desc).size(12.5).color(pal.text_dim)).wrap());
        });
    }
    let rect = p.allocate_space(ui).rect;
    let r = ui
        .interact(rect, ui.id().with(("kind", title)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    p.frame = p
        .frame
        .fill(if on {
            pal.accent_soft
        } else if r.hovered() {
            pal.surface_hover
        } else {
            pal.chrome
        })
        .stroke(Stroke::new(
            if on { 1.5 } else { 1.0 },
            if on { pal.accent } else { pal.border },
        ));
    p.paint(ui);
    if r.clicked() {
        np.kind = kind;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("grammata-np-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn np(parent: &Path, name: &str, kind: Kind) -> NewProject {
        NewProject {
            name: name.into(),
            parent: parent.to_path_buf(),
            kind,
            ..NewProject::new(None)
        }
    }

    #[test]
    fn vacio_crea_carpeta_y_main() {
        let d = sandbox("vacio");
        let dir = np(&d, "  tesis ", Kind::Empty).create().unwrap();
        assert_eq!(dir, d.join("tesis"));
        assert_eq!(std::fs::read_to_string(dir.join("main.typ")).unwrap(), "");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn articulo_crea_carpeta_vacia() {
        let d = sandbox("art");
        let dir = np(&d, "paper", Kind::Template(crate::templates::Kind::Article))
            .create()
            .unwrap();
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn rechaza_existente_y_nombres_raros() {
        let d = sandbox("mal");
        std::fs::create_dir(d.join("ya")).unwrap();
        std::fs::write(d.join("ya").join("main.typ"), "mío").unwrap();
        assert!(
            np(&d, "ya", Kind::Empty)
                .create()
                .unwrap_err()
                .contains("Ya existe")
        );
        assert_eq!(
            std::fs::read_to_string(d.join("ya").join("main.typ")).unwrap(),
            "mío"
        );
        assert!(np(&d, "  ", Kind::Empty).create().is_err());
        assert!(np(&d, "a/b", Kind::Empty).create().is_err());
        assert!(np(&d.join("no-existe"), "x", Kind::Empty).create().is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
