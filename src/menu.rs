//! Menú desplegable compuesto, portado de `egui-share-ui`.
//!
//! API declarativa inspirada en Radix/shadcn (`DropdownMenuTrigger`,
//! `Content`, `Group`, `Label`, `Item`) y adaptada a egui inmediato: el
//! builder describe las secciones y `show` las dibuja en un `Area` de primer
//! plano (el equivalente al *portal* de Radix). Devuelve `Option<T>` en vez
//! de ejecutar closures, para que la lógica siga siendo testeable.

use egui::{self, Align2, Id, Pos2, Rect, Sense, Vec2};

use crate::theme;

struct Section<T> {
    label: Option<String>,
    items: Vec<(String, T)>,
}

/// Trigger + secciones. Las secciones se separan con un divisor.
pub struct Menu<T> {
    id: String,
    trigger: String,
    sections: Vec<Section<T>>,
}

impl<T: Clone> Menu<T> {
    pub fn new(id: impl Into<String>, trigger: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            trigger: trigger.into(),
            sections: Vec::new(),
        }
    }

    /// Añade un grupo con etiqueta opcional. Cada par es un item.
    pub fn section<S, I>(mut self, label: Option<&str>, items: I) -> Self
    where
        S: Into<String>,
        I: IntoIterator<Item = (S, T)>,
    {
        self.sections.push(Section {
            label: label.map(str::to_string),
            items: items
                .into_iter()
                .map(|(text, value)| (text.into(), value))
                .collect(),
        });
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> Option<T> {
        let p = theme::palette(ui.ctx());
        let id = Id::new(&self.id);

        let mut open: bool = ui.ctx().data(|data| data.get_temp(id)).unwrap_or(false);
        let response = ui.add(
            egui::Button::new(egui::RichText::new(&self.trigger).color(p.text))
                .fill(p.chrome)
                .stroke(egui::Stroke::new(1.0, p.border))
                .corner_radius(egui::CornerRadius::same(6))
                .min_size(egui::vec2(72.0, 24.0)),
        );
        if response.clicked() {
            open = !open;
            ui.ctx().data_mut(|data| data.insert_temp(id, open));
        }
        if response.clicked_elsewhere() && open {
            open = false;
            ui.ctx().data_mut(|data| data.insert_temp(id, false));
        }
        if !open {
            return None;
        }

        let row_h = 26.0;
        let label_h = 20.0;
        let gap = 8.0;
        let mut height = gap * 2.0;
        for (i, section) in self.sections.iter().enumerate() {
            if i > 0 {
                height += gap;
            }
            if section.label.is_some() {
                height += label_h;
            }
            height += section.items.len() as f32 * row_h;
        }

        let screen = ui.ctx().viewport_rect();
        let mut origin = response.rect.min + Vec2::new(0.0, response.rect.height() + 4.0);
        let width = response.rect.width().max(212.0);
        if origin.x + width > screen.max.x {
            origin.x = screen.max.x - width - 4.0;
        }
        if origin.y + height > screen.max.y {
            origin.y = response.rect.min.y - height - 4.0;
        }

        let size = Vec2::new(width, height);
        let mut chosen = None;
        egui::Area::new(Id::new(format!("{}-popover", self.id)))
            .order(egui::Order::Foreground)
            .fixed_pos(origin)
            .interactable(true)
            .show(ui.ctx(), |ui| {
                // `Painter` usa coordenadas globales: dibujar en `origin`, no
                // en el origen local del Area.
                let rect = Rect::from_min_size(origin, size);
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(10), p.surface);
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(10),
                    egui::Stroke::new(1.0, p.border),
                    egui::StrokeKind::Inside,
                );

                let mut y = rect.min.y + gap;
                for (i, section) in self.sections.iter().enumerate() {
                    if i > 0 {
                        ui.painter().line_segment(
                            [
                                Pos2::new(rect.min.x + 8.0, y),
                                Pos2::new(rect.max.x - 8.0, y),
                            ],
                            egui::Stroke::new(1.0, p.border),
                        );
                        y += gap;
                    }
                    if let Some(label) = &section.label {
                        ui.painter().text(
                            Pos2::new(rect.min.x + 12.0, y + label_h / 2.0),
                            Align2::LEFT_CENTER,
                            label.to_uppercase(),
                            egui::FontId::new(10.5, egui::FontFamily::Proportional),
                            p.text_dim,
                        );
                        y += label_h;
                    }
                    for (label, value) in &section.items {
                        let row = Rect::from_min_size(
                            Pos2::new(rect.min.x + 4.0, y),
                            Vec2::new(width - 8.0, row_h),
                        );
                        let r = ui.allocate_rect(row, Sense::click());
                        if r.hovered() {
                            ui.painter().rect_filled(
                                row,
                                egui::CornerRadius::same(6),
                                p.surface_hover,
                            );
                        }
                        ui.painter().text(
                            Pos2::new(row.min.x + 10.0, row.center().y),
                            Align2::LEFT_CENTER,
                            label.clone(),
                            egui::FontId::new(12.5, egui::FontFamily::Proportional),
                            p.text,
                        );
                        if r.clicked() {
                            chosen = Some(value.clone());
                        }
                        y += row_h;
                    }
                }
            });

        if chosen.is_some() {
            ui.ctx().data_mut(|data| data.insert_temp(id, false));
        }
        chosen
    }
}
