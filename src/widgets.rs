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

/// Como `section_label`, pero en `width` como máximo: si no cabe se corta con «…»
/// y el texto completo queda en el tooltip.
pub fn section_label_fit(ui: &mut Ui, pal: &Palette, text: &str, width: f32) {
    ui.allocate_ui_with_layout(
        vec2(width.max(0.0), 24.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(text.to_uppercase())
                        .small()
                        .color(pal.text_faint),
                )
                .truncate(),
            )
            .on_hover_text(text);
        },
    );
}

// ---------- Diálogos modales (estilo común) ----------

/// Modal con el marco común (márgenes amplios, esquinas suaves) y ancho fijo.
pub fn modal<R>(
    ctx: &egui::Context,
    id: &str,
    width: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::ModalResponse<R> {
    let pal = crate::theme::palette(ctx);
    let frame = egui::Frame::popup(&ctx.global_style())
        .inner_margin(Margin::same(20))
        .corner_radius(CornerRadius::same(10));
    egui::Modal::new(egui::Id::new(id))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 6.0;
            // Campos de texto con borde visible también sin foco.
            let w = &mut ui.visuals_mut().widgets;
            w.inactive.bg_stroke = Stroke::new(1.0, pal.border_strong);
            w.hovered.bg_stroke = Stroke::new(1.0, pal.text_faint);
            add_contents(ui)
        })
}

/// Cabecera de modal: icono de acento, título y, opcional, una línea de ayuda.
pub fn modal_header(ui: &mut Ui, pal: &Palette, icon: &str, title: &str, subtitle: Option<&str>) {
    modal_header_tinted(ui, pal, icon, pal.accent, title, subtitle);
}

/// Como `modal_header`, con el color del icono a elección (p. ej. aviso).
pub fn modal_header_tinted(
    ui: &mut Ui,
    pal: &Palette,
    icon: &str,
    tint: egui::Color32,
    title: &str,
    subtitle: Option<&str>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(22.0).color(tint));
        ui.vertical(|ui| {
            // Aire entre título y ayuda: con 17 pt el texto pequeño quedaba
            // pegado a los descendentes del título.
            ui.spacing_mut().item_spacing.y = 5.0;
            ui.label(RichText::new(title).size(17.0).strong().color(pal.text));
            if let Some(s) = subtitle {
                ui.add(egui::Label::new(RichText::new(s).size(13.0).color(pal.text_dim)).wrap());
            }
        });
    });
    ui.add_space(10.0);
}

/// Pie de modal: separador y botones alineados a la derecha. `add` los
/// recibe de derecha a izquierda: primero el principal.
pub fn modal_footer<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // Margen interior también en los textos largos («No guardar»), y
        // algo de separación entre botones.
        ui.spacing_mut().button_padding = vec2(14.0, 5.0);
        ui.spacing_mut().item_spacing.x = 8.0;
        add(ui)
    })
    .inner
}

const DIALOG_BUTTON: egui::Vec2 = vec2(84.0, 30.0);

/// Botón principal de diálogo (relleno de acento).
pub fn primary_button(ui: &mut Ui, pal: &Palette, text: &str, enabled: bool) -> Response {
    filled_button(ui, pal, text, pal.accent, enabled)
}

/// Botón de acción destructiva (relleno de error).
pub fn danger_button(ui: &mut Ui, pal: &Palette, text: &str) -> Response {
    filled_button(ui, pal, text, pal.err, true)
}

fn filled_button(
    ui: &mut Ui,
    pal: &Palette,
    text: &str,
    fill: egui::Color32,
    enabled: bool,
) -> Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(pal.on_accent).strong())
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(6))
            .min_size(DIALOG_BUTTON),
    )
}

/// Botón secundario con marco visible (los de serie no lo tienen en reposo).
pub fn secondary_button(ui: &mut Ui, pal: &Palette, text: &str) -> Response {
    ui.add(
        Button::new(RichText::new(text).color(pal.text))
            .fill(pal.chrome)
            .stroke(Stroke::new(1.0, pal.border_strong))
            .corner_radius(CornerRadius::same(6))
            .min_size(DIALOG_BUTTON),
    )
}

/// Margen interior común de los campos de texto de los diálogos.
pub const FIELD_MARGIN: Margin = Margin::symmetric(8, 6);
