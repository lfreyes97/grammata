//! Paleta, estilo de egui/dock/editor y fuentes.
//!
//! Todos los colores salen de `Palette`: los paneles piden `palette(ctx)` y
//! no definen colores propios.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, vec2};

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    /// Fondo de la ventana / barras.
    pub chrome: Color32,
    /// Fondo de paneles y pestañas.
    pub surface: Color32,
    pub surface_hover: Color32,
    pub surface_active: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    /// Fondo del lienzo de la vista previa.
    pub canvas: Color32,
    pub syntax: SyntaxColors,
}

/// Colores del resaltado (ver `highlight.rs`).
#[derive(Clone, Copy)]
pub struct SyntaxColors {
    pub comment: Color32,
    pub punctuation: Color32,
    pub escape: Color32,
    pub strong: Color32,
    pub link: Color32,
    pub raw: Color32,
    pub label: Color32,
    pub heading: Color32,
    pub list: Color32,
    pub math: Color32,
    pub keyword: Color32,
    pub operator: Color32,
    pub number: Color32,
    pub string: Color32,
    pub function: Color32,
    pub interpolated: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const DARK: Palette = Palette {
    dark: true,
    chrome: hex(0x15171c),
    surface: hex(0x1b1e24),
    surface_hover: hex(0x252932),
    surface_active: hex(0x2d323c),
    border: hex(0x272b33),
    border_strong: hex(0x383e4a),
    text: hex(0xe4e7ec),
    text_dim: hex(0xa0a7b4),
    text_faint: hex(0x6b7280),
    accent: hex(0x3fb5c6),
    accent_soft: Color32::from_rgba_premultiplied(25, 72, 79, 110),
    on_accent: hex(0x0c1a1d),
    ok: hex(0x4cc38a),
    warn: hex(0xe5b84b),
    err: hex(0xef6b6b),
    canvas: hex(0x0f1115),
    syntax: SyntaxColors {
        comment: hex(0x6b7280),
        punctuation: hex(0x8b93a1),
        escape: hex(0xc3a6ff),
        strong: hex(0xffffff),
        link: hex(0x7cc4ff),
        raw: hex(0x93d18b),
        label: hex(0xc3a6ff),
        heading: hex(0x3fb5c6),
        list: hex(0xe5b84b),
        math: hex(0x5fd3c4),
        keyword: hex(0xf07fa6),
        operator: hex(0xa0a7b4),
        number: hex(0xe5b84b),
        string: hex(0x93d18b),
        function: hex(0x7cc4ff),
        interpolated: hex(0xf2a46b),
    },
};

pub const LIGHT: Palette = Palette {
    dark: false,
    chrome: hex(0xeef0f3),
    surface: hex(0xfbfbfc),
    surface_hover: hex(0xeceef2),
    surface_active: hex(0xe2e5ea),
    border: hex(0xdfe2e7),
    border_strong: hex(0xc9ced6),
    text: hex(0x1d2128),
    text_dim: hex(0x575e6c),
    text_faint: hex(0x8b92a0),
    accent: hex(0x1d8a99),
    accent_soft: Color32::from_rgba_premultiplied(26, 124, 138, 46),
    on_accent: hex(0xffffff),
    ok: hex(0x1f9d63),
    warn: hex(0xb07d12),
    err: hex(0xd64545),
    canvas: hex(0xdfe2e7),
    syntax: SyntaxColors {
        comment: hex(0x8b92a0),
        punctuation: hex(0x6b7280),
        escape: hex(0x7148c8),
        strong: hex(0x000000),
        link: hex(0x1f63c9),
        raw: hex(0x2a7f3a),
        label: hex(0x7148c8),
        heading: hex(0x1d8a99),
        list: hex(0x9a5a00),
        math: hex(0x0f7f73),
        keyword: hex(0xb8265f),
        operator: hex(0x575e6c),
        number: hex(0x9a5a00),
        string: hex(0x2a7f3a),
        function: hex(0x1f63c9),
        interpolated: hex(0xb4561b),
    },
};

pub fn palette_for(dark: bool) -> Palette {
    if dark { DARK } else { LIGHT }
}

pub fn palette(ctx: &egui::Context) -> Palette {
    palette_for(ctx.global_style().visuals.dark_mode)
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = palette_for(dark);
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.global_style_mut(|style| {
        let r6 = CornerRadius::same(6);
        let r4 = CornerRadius::same(4);
        let v = &mut style.visuals;
        *v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.panel_fill = p.chrome;
        v.window_fill = p.surface;
        v.window_stroke = Stroke::new(1.0, p.border_strong);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.window_shadow = egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 110 } else { 40 }),
        };
        v.popup_shadow = egui::Shadow {
            offset: [0, 4],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 90 } else { 30 }),
        };
        v.faint_bg_color = p.surface_hover;
        v.extreme_bg_color = if dark { hex(0x111317) } else { hex(0xffffff) };
        v.code_bg_color = p.surface_hover;
        v.hyperlink_color = p.accent;
        v.warn_fg_color = p.warn;
        v.error_fg_color = p.err;
        v.selection.bg_fill = if dark { hex(0x264a52) } else { hex(0xc8e7ec) };
        v.selection.stroke = Stroke::new(1.0, p.accent);
        v.indent_has_left_vline = false;
        v.collapsing_header_frame = false;
        v.slider_trailing_fill = true;

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = p.surface;
        w.noninteractive.weak_bg_fill = p.surface;
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, p.text_dim);
        w.noninteractive.corner_radius = r6;

        w.inactive.bg_fill = p.surface_hover;
        w.inactive.weak_bg_fill = Color32::TRANSPARENT;
        w.inactive.bg_stroke = Stroke::NONE;
        w.inactive.fg_stroke = Stroke::new(1.0, p.text);
        w.inactive.corner_radius = r6;

        w.hovered.bg_fill = p.surface_active;
        w.hovered.weak_bg_fill = p.surface_hover;
        w.hovered.bg_stroke = Stroke::NONE;
        w.hovered.fg_stroke = Stroke::new(1.0, p.text);
        w.hovered.corner_radius = r6;
        w.hovered.expansion = 0.0;

        w.active.bg_fill = p.surface_active;
        w.active.weak_bg_fill = p.surface_active;
        w.active.bg_stroke = Stroke::new(1.0, p.accent);
        w.active.fg_stroke = Stroke::new(1.0, p.text);
        w.active.corner_radius = r6;
        w.active.expansion = 0.0;

        w.open.bg_fill = p.surface_active;
        w.open.weak_bg_fill = p.surface_active;
        w.open.bg_stroke = Stroke::NONE;
        w.open.fg_stroke = Stroke::new(1.0, p.text);
        w.open.corner_radius = r6;
        let _ = r4;

        let s = &mut style.spacing;
        s.item_spacing = vec2(6.0, 5.0);
        s.button_padding = vec2(7.0, 3.0);
        s.interact_size = vec2(24.0, 24.0);
        s.menu_margin = Margin::same(6);
        s.window_margin = Margin::same(14);
        s.icon_spacing = 6.0;
        s.indent = 14.0;
        s.combo_height = 300.0;
        s.scroll = egui::style::ScrollStyle::thin();

        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(17.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(13.5, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(11.5, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}

/// Estilo del dock a juego con la paleta.
pub fn dock_style(style: &egui::Style, p: &Palette) -> egui_dock::Style {
    use egui_dock::TabInteractionStyle;
    let mut s = egui_dock::Style::from_egui(style);
    let r = CornerRadius {
        nw: 6,
        ne: 6,
        sw: 0,
        se: 0,
    };
    let tab = |bg: Color32, text: Color32, outline: Color32| TabInteractionStyle {
        bg_fill: bg,
        text_color: text,
        corner_radius: r,
        outline_color: outline,
    };
    s.main_surface_border_stroke = Stroke::NONE;
    s.main_surface_border_rounding = CornerRadius::ZERO;
    s.dock_area_padding = None;

    s.tab_bar.height = 30.0;
    s.tab_bar.bg_fill = p.chrome;
    s.tab_bar.hline_color = p.border;
    s.tab_bar.corner_radius = CornerRadius::ZERO;
    s.tab_bar.fill_tab_bar = false;

    s.tab.tab_body.bg_fill = p.surface;
    s.tab.tab_body.stroke = Stroke::new(1.0, p.border);
    s.tab.tab_body.corner_radius = CornerRadius::ZERO;
    s.tab.tab_body.inner_margin = Margin::ZERO;
    s.tab.hline_below_active_tab_name = true;
    s.tab.minimum_width = None;
    s.tab.spacing = 4.0;

    s.tab.active = tab(p.surface, p.text, Color32::TRANSPARENT);
    s.tab.focused = tab(p.surface, p.text, Color32::TRANSPARENT);
    s.tab.hovered = tab(p.surface_hover, p.text, Color32::TRANSPARENT);
    s.tab.inactive = tab(Color32::TRANSPARENT, p.text_faint, Color32::TRANSPARENT);
    s.tab.active_with_kb_focus = tab(p.surface, p.text, p.accent);
    s.tab.focused_with_kb_focus = tab(p.surface, p.text, p.accent);
    s.tab.inactive_with_kb_focus = tab(Color32::TRANSPARENT, p.text_dim, p.accent);

    s.separator.width = 1.0;
    s.separator.extra = 175.0;
    s.separator.extra_interact_width = 4.0;
    s.separator.color_idle = p.border;
    s.separator.color_hovered = p.accent;
    s.separator.color_dragged = p.accent;

    s.buttons.close_tab_color = p.text_faint;
    s.buttons.close_tab_active_color = p.text;
    s.buttons.close_tab_bg_fill = p.surface_active;

    s.overlay.selection_color = p.accent_soft;
    s.overlay.button_color = p.surface_hover;
    s.overlay.button_border_stroke = Stroke::new(1.0, p.border_strong);
    s
}

/// Fuentes: Inter (o la sans del sistema) + monoespaciada del sistema vía
/// `fc-match`, con los iconos Phosphor por delante. Si `fc-match` no existe
/// se quedan las fuentes de egui.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    let ui_font = system_font("Inter:style=Regular", Some("Inter"))
        .or_else(|| system_font("sans-serif:style=Regular", None));
    let mono_font = system_font("monospace:style=Regular", None);
    // Fuente matemática para el panel de símbolos (se ven como en el PDF).
    let math_font = [
        "STIX Two Math",
        "Latin Modern Math",
        "New Computer Modern Math",
        "Noto Sans Math",
    ]
    .iter()
    .find_map(|f| system_font(&format!("{f}:style=Regular"), Some("Math")));

    for (name, data, family) in [
        ("ui", ui_font, FontFamily::Proportional),
        ("mono", mono_font, FontFamily::Monospace),
    ] {
        if let Some(fd) = data {
            fonts.font_data.insert(name.into(), fd.into());
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.into());
        }
    }
    // Familia "math": fuente matemática si la hay, y detrás la de la UI.
    let mut math = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(fd) = math_font {
        fonts.font_data.insert("math".into(), fd.into());
        math.insert(0, "math".into());
    }
    // "ui-math": la de la UI primero y la matemática para lo que le falte.
    let mut ui_math = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if math.first().is_some_and(|f| f == "math") {
        ui_math.push("math".into());
    }
    fonts.families.insert(FontFamily::Name("math".into()), math);
    fonts
        .families
        .insert(FontFamily::Name("ui-math".into()), ui_math);
    // Inter (y Adwaita Sans) tienen glifos en el área de uso privado, la misma
    // que usa Phosphor: si va detrás, Inter tapa los iconos. Va delante, pero
    // sin sus letras (ver `icons_only`).
    let icons = icons_only(egui_phosphor::Variant::Regular.font_bytes().to_vec());
    fonts
        .font_data
        .insert("phosphor".into(), egui::FontData::from_owned(icons).into());
    // También en la monoespaciada: etiquetas como «⌕ 100 %» van en `.monospace()`.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            list.retain(|n| n != "phosphor");
            list.insert(0, "phosphor".into());
        }
    }
    ctx.set_fonts(fonts);
}

/// Phosphor asigna también `\0`, espacio, `-` y `a`–`z` (para sus ligaduras)
/// a glifos vacíos; con la fuente la primera de la familia el texto
/// desaparecería. Se parchean los segmentos de esos códigos en el `cmap`
/// (formato 4) para que apunten al glifo 0, «no existe», y egui siga con la
/// fuente siguiente. Los segmentos se convierten en 0..k para seguir ordenados.
fn icons_only(mut font: Vec<u8>) -> Vec<u8> {
    fn patch(f: &mut [u8]) -> Option<()> {
        let u16_at =
            |f: &[u8], o: usize| Some(u16::from_be_bytes([*f.get(o)?, *f.get(o + 1)?]) as usize);
        let u32_at = |f: &[u8], o: usize| {
            Some(u32::from_be_bytes(f.get(o..o + 4)?.try_into().ok()?) as usize)
        };
        let tables = u16_at(f, 4)?;
        let rec = (0..tables)
            .map(|i| 12 + 16 * i)
            .find(|&r| f.get(r..r + 4) == Some(b"cmap"))?;
        let cmap = u32_at(f, rec + 8)?;
        for i in 0..u16_at(f, cmap + 2)? {
            let st = cmap + u32_at(f, cmap + 4 + 8 * i + 4)?;
            if u16_at(f, st)? != 4 {
                continue;
            }
            let seg = u16_at(f, st + 6)? / 2;
            let ends = st + 14;
            let (starts, deltas, ranges) =
                (ends + 2 * seg + 2, ends + 4 * seg + 2, ends + 6 * seg + 2);
            for s in 0..seg {
                if u16_at(f, ends + 2 * s)? >= 0xE000 {
                    break;
                }
                let v = s as u16;
                for (base, val) in [
                    (ends, v),
                    (starts, v),
                    (deltas, v.wrapping_neg()),
                    (ranges, 0),
                ] {
                    f.get_mut(base + 2 * s..base + 2 * s + 2)?
                        .copy_from_slice(&val.to_be_bytes());
                }
            }
        }
        Some(())
    }
    let _ = patch(&mut font);
    font
}

fn system_font(pattern: &str, want_family: Option<&str>) -> Option<egui::FontData> {
    let out = std::process::Command::new("fc-match")
        .args(["-f", "%{family}|%{file}|%{index}", pattern])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let mut it = s.split('|');
    let family = it.next()?;
    let file = it.next()?;
    let index: u32 = it.next()?.trim().parse().unwrap_or(0);
    if want_family.is_some_and(|w| !family.contains(w)) {
        return None;
    }
    let bytes = std::fs::read(file).ok()?;
    let mut fd = egui::FontData::from_owned(bytes);
    fd.index = index;
    Some(fd)
}
