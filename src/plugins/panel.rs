//! Panel «Plugins»: lista, activar/desactivar, clave de licencia, temas y
//! las plantillas con su formulario.

use egui::RichText;
use egui_phosphor::regular as icon;

use super::{Field, FieldKind, Plugin, Template};
use crate::actions::Action;
use crate::app::State;
use crate::{theme, widgets};

pub fn panel(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    let mut acts: Vec<Action> = Vec::new();
    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::section_label(ui, &pal, "Plugins");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, icon::ARROWS_CLOCKWISE, "Volver a leer la carpeta de plugins")
                    .clicked()
                {
                    acts.push(Action::PluginsReload);
                }
                widgets::icon_menu(ui, icon::PLUS, "Añadir un plugin de serie", |ui| {
                    for b in super::BUNDLED {
                        let have = st.plugins.list.iter().any(|p| p.id() == b.id);
                        if ui
                            .add_enabled(!have, egui::Button::new(b.name))
                            .on_hover_text(b.about)
                            .clicked()
                        {
                            acts.push(Action::PluginInstall(b.id));
                            ui.close();
                        }
                    }
                });
                let dir = st.plugins.dir.display().to_string();
                if widgets::icon_button(ui, icon::COPY, &format!("Copiar la ruta de la carpeta\n{dir}"))
                    .clicked()
                {
                    ui.ctx().copy_text(dir);
                }
            });
        });
    });
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 6.0;
                    body(st, ui, &pal, &mut acts);
                });
        });
    st.actions.extend(acts);
}

fn body(st: &mut State, ui: &mut egui::Ui, pal: &theme::Palette, acts: &mut Vec<Action>) {
    let State {
        plugins,
        prefs,
        fmt,
        ..
    } = st;
    if plugins.list.is_empty() {
        note(
            ui,
            pal,
            &format!(
                "Un plugin es una carpeta con un plugin.toml dentro de {}. Puede traer un \
                 paquete typst con su formulario, fuentes, snippets, temas y programas \
                 que importan, exportan o transforman el texto.",
                plugins.dir.display()
            ),
        );
        for b in super::BUNDLED {
            if ui
                .button(format!("{}  {}", icon::PLUS, b.name))
                .on_hover_text(b.about)
                .clicked()
            {
                acts.push(Action::PluginInstall(b.id));
            }
        }
        return;
    }
    if prefs.theme.is_some()
        && ui
            .small_button(format!("{}  Volver al tema de serie", icon::PALETTE))
            .clicked()
    {
        acts.push(Action::SetTheme(None));
    }
    let super::Plugins {
        list,
        saved,
        open,
        forms,
        font_filter,
        ..
    } = &mut *plugins;
    let mut key_changed = false;
    for p in list.iter() {
        let title = format!("{}  {}", icon::PUZZLE_PIECE, p.manifest.name);
        let color = if p.active() { pal.text } else { pal.text_faint };
        egui::CollapsingHeader::new(RichText::new(title).color(color))
            .id_salt(("plugin", p.id()))
            .default_open(true)
            .show(ui, |ui| {
                header(ui, pal, p, acts);
                if p.manifest.needs_key && !p.broken {
                    ui.label(RichText::new("Clave de licencia").small().color(pal.text_dim));
                    let key = saved.keys.entry(p.id().to_string()).or_default();
                    let r = ui.add(
                        egui::TextEdit::singleline(key)
                            .password(true)
                            .hint_text("Pega aquí tu clave")
                            .desired_width(f32::INFINITY),
                    );
                    key_changed |= r.lost_focus();
                }
                if !p.active() {
                    return;
                }
                contents(ui, pal, p);
                programs(ui, pal, p, acts);
                if !p.themes.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Temas").small().color(pal.text_dim));
                        for (name, _) in &p.themes {
                            let key = format!("{}/{name}", p.id());
                            let on = prefs.theme.as_deref() == Some(key.as_str());
                            if ui.selectable_label(on, name).clicked() {
                                acts.push(Action::SetTheme((!on).then_some(key)));
                            }
                        }
                    });
                }
                let mut group = "";
                for t in &p.manifest.templates {
                    if t.group != group {
                        group = &t.group;
                        ui.label(RichText::new(group).small().color(pal.text_dim));
                    }
                    let id = (p.id().to_string(), t.id.clone());
                    let on = open.as_ref() == Some(&id);
                    let caret = if on { icon::CARET_DOWN } else { icon::CARET_RIGHT };
                    let r = ui
                        .add_sized(
                            [ui.available_width(), 24.0],
                            egui::Button::new((
                                RichText::new(format!("{caret}  {}", t.name)).color(pal.text),
                                egui::Atom::grow(),
                            ))
                            .selected(on)
                            .frame_when_inactive(false)
                            .truncate(),
                        )
                        .on_hover_text(&t.description);
                    if r.clicked() {
                        *open = (!on).then(|| id.clone());
                    }
                    if on {
                        let values = forms.entry(id).or_insert_with(|| t.initial());
                        form(ui, pal, p, t, values, fmt.fonts.as_deref(), font_filter, acts);
                    }
                }
            });
    }
    if key_changed {
        saved.keys.retain(|_, k| !k.trim().is_empty());
        plugins.save();
    }
}

fn note(ui: &mut egui::Ui, pal: &theme::Palette, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).small().color(pal.text_faint)).wrap());
}

/// Activar, versión, descripción, licencia y errores de carga.
fn header(ui: &mut egui::Ui, pal: &theme::Palette, p: &Plugin, acts: &mut Vec<Action>) {
    let m = &p.manifest;
    ui.horizontal(|ui| {
        let mut on = p.enabled;
        if ui
            .add_enabled(!p.broken, egui::Checkbox::new(&mut on, "Activo"))
            .changed()
        {
            acts.push(Action::PluginEnable(p.id().to_string(), on));
        }
        if !m.version.is_empty() {
            ui.label(RichText::new(format!("v{}", m.version)).small().color(pal.text_faint));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, icon::PENCIL_SIMPLE, "Abrir plugin.toml").clicked() {
                acts.push(Action::OpenPath(p.dir.join(super::MANIFEST)));
            }
        });
    });
    if !m.description.is_empty() {
        ui.add(egui::Label::new(RichText::new(&m.description).color(pal.text_dim)).wrap());
    }
    let mut about = Vec::new();
    if !m.authors.is_empty() {
        about.push(m.authors.join(", "));
    }
    if !m.license.is_empty() {
        about.push(format!("Licencia: {}", m.license));
    }
    if !about.is_empty() {
        note(ui, pal, &about.join(" · "));
    }
    if !m.homepage.is_empty() {
        ui.hyperlink_to(RichText::new(&m.homepage).small(), &m.homepage);
    }
    for e in &p.errors {
        ui.add(
            egui::Label::new(
                RichText::new(format!("{}  {e}", icon::WARNING_CIRCLE))
                    .small()
                    .color(pal.err),
            )
            .wrap(),
        );
    }
}

/// Resumen de lo que aporta (el paquete, con su `#import` para copiar).
fn contents(ui: &mut egui::Ui, pal: &theme::Palette, p: &Plugin) {
    if let Some(pkg) = &p.package {
        let import = format!("#import \"{}\": *", pkg.spec());
        if ui
            .add(
                egui::Button::new(
                    RichText::new(format!("{}  {}", icon::PACKAGE, pkg.spec()))
                        .small()
                        .monospace()
                        .color(pal.text_dim),
                )
                .frame_when_inactive(false),
            )
            .on_hover_text(format!("Copiar: {import}"))
            .clicked()
        {
            ui.ctx().copy_text(import);
        }
    }
    let mut parts = Vec::new();
    let snippets: usize = p.snippets.iter().map(|l| l.items.len()).sum();
    if snippets > 0 {
        parts.push(format!("{snippets} snippets (en su panel)"));
    }
    if !p.font_dirs.is_empty() {
        parts.push("fuentes".to_string());
    }
    if !parts.is_empty() {
        note(ui, pal, &format!("Aporta: {}", parts.join(", ")));
    }
}

/// Programas externos: permiso, qué importa y exporta, y sus comandos.
fn programs(ui: &mut egui::Ui, pal: &theme::Palette, p: &Plugin, acts: &mut Vec<Action>) {
    use super::run::Kind;
    if !p.has_programs() {
        return;
    }
    let m = &p.manifest;
    let lines = m.programs().join("\n");
    if !p.allowed {
        ui.add(
            egui::Label::new(
                RichText::new(format!(
                    "{}  Este plugin ejecuta programas en tu equipo. Permítelo solo si \
                     confías en él.",
                    icon::WARNING
                ))
                .small()
                .color(pal.warn),
            )
            .wrap(),
        );
        ui.label(RichText::new(&lines).small().monospace().color(pal.text_dim));
        if ui.button("Permitir sus programas").clicked() {
            acts.push(Action::PluginAllow(p.id().to_string(), true));
        }
        return;
    }
    let names = |kind| {
        m.tools(kind)
            .iter()
            .map(|t| format!("{} (.{})", t.name, t.extensions.join(", .")))
            .collect::<Vec<_>>()
            .join(", ")
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}  Ejecuta programas", icon::TERMINAL)).small().color(pal.text_dim))
            .on_hover_text(lines);
        if ui.small_button("Retirar permiso").clicked() {
            acts.push(Action::PluginAllow(p.id().to_string(), false));
        }
    });
    if !m.imports.is_empty() {
        ui.horizontal_wrapped(|ui| {
            note(ui, pal, &format!("Importa: {}", names(Kind::Import)));
            if ui.small_button("Importar…").clicked() {
                acts.push(Action::Import);
            }
        });
    }
    if !m.exports.is_empty() {
        note(ui, pal, &format!("Exporta (menú Exportar): {}", names(Kind::Export)));
    }
    for t in &m.commands {
        if ui
            .button(format!("{}  {}", icon::PLAY, t.name))
            .on_hover_text(&t.description)
            .clicked()
        {
            acts.push(Action::PluginRun {
                plugin: p.id().to_string(),
                kind: Kind::Command,
                id: t.id.clone(),
            });
        }
    }
}

/// Formulario de una plantilla y sus dos salidas.
#[allow(clippy::too_many_arguments)]
fn form(
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    p: &Plugin,
    t: &Template,
    values: &mut std::collections::HashMap<String, String>,
    fonts: Option<&[String]>,
    font_filter: &mut String,
    acts: &mut Vec<Action>,
) {
    egui::Frame::new()
        .fill(pal.chrome)
        .stroke(egui::Stroke::new(1.0, pal.border))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !t.description.is_empty() {
                note(ui, pal, &t.description);
            }
            for f in &t.fields {
                let v = values.entry(f.key.clone()).or_insert_with(|| f.initial());
                ui.push_id((&t.id, &f.key), |ui| field(ui, pal, f, v, fonts, font_filter));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if widgets::accent_button(ui, pal, &format!("{}  Nuevo documento", icon::FILE_PLUS))
                    .clicked()
                {
                    acts.push(Action::NewDocWith(t.render(values, p.package.as_ref())));
                }
                if ui
                    .button("Insertar")
                    .on_hover_text("Inserta el código en el cursor del documento actual")
                    .clicked()
                {
                    acts.push(Action::InsertSnippet(t.render(values, p.package.as_ref())));
                }
                if widgets::icon_button(ui, icon::ARROW_COUNTER_CLOCKWISE, "Valores por defecto")
                    .clicked()
                {
                    *values = t.initial();
                }
            });
        });
}

fn field(
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    f: &Field,
    v: &mut String,
    fonts: Option<&[String]>,
    font_filter: &mut String,
) {
    let label = if f.label.is_empty() { &f.key } else { &f.label };
    if f.kind == FieldKind::Bool {
        let mut on = v == "true";
        if ui.checkbox(&mut on, label).changed() {
            *v = on.to_string();
        }
        return;
    }
    ui.label(RichText::new(label).small().color(pal.text_dim));
    match f.kind {
        FieldKind::Content | FieldKind::String | FieldKind::Code => {
            let edit = if f.multiline {
                egui::TextEdit::multiline(v).desired_rows(3)
            } else {
                egui::TextEdit::singleline(v)
            };
            let edit = if f.kind == FieldKind::Code {
                edit.font(egui::TextStyle::Monospace)
            } else {
                edit
            };
            ui.add(edit.hint_text(&f.hint).desired_width(f32::INFINITY));
        }
        FieldKind::Color => {
            let mut rgb = parse_hex(v).unwrap_or([0, 0, 0]);
            ui.horizontal(|ui| {
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    *v = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
                }
                ui.label(RichText::new(v.as_str()).monospace().small().color(pal.text_faint));
            });
        }
        FieldKind::Number => {
            let mut n: f64 = v.trim().parse().unwrap_or(0.0);
            let range = f.min.unwrap_or(f64::MIN)..=f.max.unwrap_or(f64::MAX);
            let drag = egui::DragValue::new(&mut n).range(range).suffix(&f.unit);
            if ui.add(drag).changed() {
                *v = n.to_string();
            }
        }
        FieldKind::Choice => {
            let shown = f
                .options
                .iter()
                .find(|o| o.value() == v)
                .map_or(v.as_str(), |o| o.label())
                .to_string();
            widgets::combo(ui, pal, RichText::new(shown).small(), 200.0, label, |ui| {
                for o in &f.options {
                    if ui.selectable_label(o.value() == v, o.label()).clicked() {
                        *v = o.value().to_string();
                        ui.close();
                    }
                }
            });
        }
        FieldKind::Font => font_combo(ui, pal, fonts, font_filter, v),
        FieldKind::Bool => {}
    }
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#').filter(|h| h.len() == 6)?;
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

/// Familias del motor (incluidas las del plugin), con filtro.
fn font_combo(
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    fonts: Option<&[String]>,
    filter: &mut String,
    font: &mut String,
) {
    let shown = if font.is_empty() { "Elegir…" } else { font.as_str() };
    let text = RichText::new(format!("{}  {shown}", icon::TEXT_AA)).small();
    widgets::combo(ui, pal, text, 200.0, "Familia de fuente", |ui| {
        ui.set_min_width(240.0);
        let Some(fonts) = fonts else {
            ui.label(RichText::new("Cargando fuentes…").small());
            return;
        };
        ui.add(
            egui::TextEdit::singleline(filter)
                .hint_text("Filtrar…")
                .desired_width(f32::INFINITY),
        );
        let q = filter.to_lowercase();
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for name in fonts
                .iter()
                .filter(|n| q.is_empty() || n.to_lowercase().contains(&q))
            {
                if ui.selectable_label(*name == *font, name).clicked() {
                    *font = name.clone();
                    ui.close();
                }
            }
        });
    });
}
