//! Piezas de UI reutilizadas por barras y paneles.

use crate::theme::Palette;
use egui::{Button, CornerRadius, Margin, Response, RichText, Stroke, Ui, vec2};

/// Marco de una barra de herramientas de panel (fondo chrome + borde inferior).
pub fn toolbar_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(pal.surface)
        .inner_margin(Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 4,
        })
        .stroke(Stroke::NONE)
}

/// Botón cuadrado solo con icono y tooltip.
pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str) -> Response {
    ui.add(
        Button::new(RichText::new(icon).size(16.0))
            .frame_when_inactive(false)
            .min_size(vec2(26.0, 24.0)),
    )
    .on_hover_text(tip)
}

/// Botón de icono con estado activado/desactivado.
pub fn icon_toggle(ui: &mut Ui, on: bool, icon: &str, tip: &str) -> Response {
    ui.add(
        Button::new(RichText::new(icon).size(16.0))
            .selected(on)
            .frame_when_inactive(false)
            .min_size(vec2(26.0, 24.0)),
    )
    .on_hover_text(tip)
}

/// Botón principal relleno con el color de acento.
pub fn accent_button(ui: &mut Ui, pal: &Palette, text: &str) -> Response {
    ui.add(
        Button::new(RichText::new(text).color(pal.on_accent).strong())
            .fill(pal.accent)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(6))
            .min_size(vec2(0.0, 24.0)),
    )
}

/// Desplegable con valor visible: marco suave, texto y flecha a la derecha.
/// Devuelve la respuesta del botón (para pintar encima, p. ej. el color).
pub fn combo(
    ui: &mut Ui,
    pal: &Palette,
    text: RichText,
    min_w: f32,
    tip: &str,
    content: impl FnOnce(&mut Ui),
) -> Response {
    let button = Button::new(text)
        .right_text(
            RichText::new(egui_phosphor::regular::CARET_DOWN)
                .small()
                .color(pal.text_faint),
        )
        .fill(pal.chrome)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(5))
        .min_size(vec2(min_w, 24.0));
    let (r, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, content);
    r.on_hover_text(tip)
}

/// Menú desplegable con solo icono (sin marco hasta el hover) y tooltip.
pub fn icon_menu(ui: &mut Ui, icon: &str, tip: &str, content: impl FnOnce(&mut Ui)) -> Response {
    let button = Button::new(RichText::new(icon).size(16.0))
        .frame_when_inactive(false)
        .min_size(vec2(26.0, 24.0));
    let (r, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, content);
    r.on_hover_text(tip)
}

/// Separador vertical fino para barras.
pub fn vsep(ui: &mut Ui, pal: &Palette) {
    let h = 16.0;
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, h), egui::Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0, pal.border_strong),
    );
}

/// Etiqueta pequeña en mayúsculas para títulos de sección.
pub fn section_label(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .small()
            .color(pal.text_faint),
    );
}
