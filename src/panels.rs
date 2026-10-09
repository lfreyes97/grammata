//! Paneles Esquema y Problemas.

use crate::actions::Action;
use crate::app::State;
use crate::diagnostics::Severity;
use crate::outline::{self, Kind};
use crate::theme;
use crate::widgets;
use egui::{RichText, Sense};
use egui_phosphor::regular as icon;

/// Esquema cacheado por (doc, rev), más la última sección mostrada
/// (para llevar el scroll hasta ella solo cuando cambia).
#[derive(Default)]
pub struct OutlineCache {
    key: Option<(u64, u64)>,
    items: Vec<outline::Item>,
    last_active: Option<usize>,
}

/// Esquema: clic mueve el cursor en el búfer (sin tocar la vista previa:
/// la sincronización con el PDF va con el texto, no con el esquema).
pub fn outline(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    let doc = st.docs.cur();
    let key = (doc.id, doc.rev);
    if st.outline.key.map(|(id, _)| id) != Some(key.0) {
        // Otro documento: se vuelve a centrar su sección (no en cada
        // pulsación: al escribir dentro de una sección no se toca el scroll).
        st.outline.last_active = None;
    }
    if st.outline.key != Some(key) {
        st.outline.items = outline::parse(&doc.text);
        st.outline.key = Some(key);
    }
    let items = &st.outline.items;
    let cursor = st.editor.cursor.unwrap_or(0);
    // Sección actual: último encabezado antes del cursor.
    let active = items
        .iter()
        .rposition(|i| matches!(i.kind, Kind::Heading(_)) && i.char_idx <= cursor);
    // Solo se lleva el scroll hasta la sección activa cuando esta cambia
    // (moverse por el editor la sigue; hojear el esquema a mano no se pelea).
    let want_scroll = st.outline.last_active != active;

    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::section_label(ui, &pal, &doc.fname());
        });
    });
    if items.is_empty() {
        egui::Frame::new().inner_margin(14).show(ui, |ui| {
            ui.label(RichText::new("Sin encabezados todavía.").color(pal.text_dim));
            ui.label(
                RichText::new("Escribe `= Encabezado 1` o `== Encabezado 2`.")
                    .small()
                    .color(pal.text_faint),
            );
        });
        return;
    }
    let mut jump = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(6, 6))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for (i, it) in items.iter().enumerate() {
                        let (ic, indent, color) = match it.kind {
                            Kind::Heading(l) => (
                                match l {
                                    1 => icon::TEXT_H_ONE,
                                    2 => icon::TEXT_H_TWO,
                                    3 => icon::TEXT_H_THREE,
                                    4 => icon::TEXT_H_FOUR,
                                    5 => icon::TEXT_H_FIVE,
                                    _ => icon::TEXT_H_SIX,
                                },
                                (l - 1) as f32 * 14.0,
                                pal.text,
                            ),
                            Kind::Let => (icon::FUNCTION, 0.0, pal.text_dim),
                            Kind::Label => (icon::TAG, 0.0, pal.text_dim),
                        };
                        let selected = active == Some(i);
                        let text = RichText::new(format!("{ic}  {}", it.title))
                            .color(if selected { pal.accent } else { color });
                        ui.horizontal(|ui| {
                            ui.add_space(indent);
                            let r = ui
                                .add(
                                    egui::Button::new((text, egui::Atom::grow()))
                                        .selected(selected)
                                        .frame_when_inactive(false)
                                        .min_size(egui::vec2(ui.available_width(), 24.0)),
                                )
                                .on_hover_text(format!("Línea {}", it.line));
                            if selected && want_scroll {
                                r.scroll_to_me(Some(egui::Align::Center));
                            }
                            if r.clicked() {
                                jump = Some(it.char_idx);
                            }
                        });
                    }
                });
        });
    if want_scroll {
        st.outline.last_active = active;
    }
    if let Some(c) = jump {
        // Solo el búfer: cursor, foco y scroll del editor. El PDF no se mueve.
        st.actions.push(Action::Select { start: c, end: c });
    }
}

pub fn problems(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    let doc = st.docs.cur();
    let built = st.compiler.built.get(&doc.id);
    let diags = built.map(|b| b.diagnostics.as_slice()).unwrap_or(&[]);
    let stale = built.is_some_and(|b| b.key.rev != doc.rev);
    let (errors, warnings) = crate::diagnostics::count(diags);

    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} {errors}", icon::X_CIRCLE)).color(if errors > 0 { pal.err } else { pal.text_faint }))
                .on_hover_text("Errores");
            ui.label(RichText::new(format!("{} {warnings}", icon::WARNING)).color(if warnings > 0 { pal.warn } else { pal.text_faint }))
                .on_hover_text("Advertencias");
            if stale {
                ui.label(RichText::new("(anterior)").small().color(pal.text_faint))
                    .on_hover_text("Diagnósticos de la compilación anterior; el texto ha cambiado desde entonces");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!diags.is_empty(), |ui| {
                    if widgets::icon_button(ui, icon::COPY, "Copiar todo").clicked() {
                        let txt: Vec<String> = diags
                            .iter()
                            .map(|d| format!("{}: {}", d.location(), d.message))
                            .collect();
                        ui.ctx().copy_text(txt.join("\n"));
                    }
                });
            });
        });
    });

    if diags.is_empty() {
        egui::Frame::new().inner_margin(14).show(ui, |ui| {
            let msg = if built.is_some() {
                "Sin problemas."
            } else {
                "Aún no se ha compilado."
            };
            ui.label(RichText::new(format!("{}  {msg}", icon::CHECK_CIRCLE)).color(pal.text_dim));
        });
        return;
    }

    let mut act = None;
    let mut go = None;
    // Owned (Arc) para no prestar `st` dentro de los closures (abajo &mut).
    let root = doc.root_dir();
    let sync = built.map(|b| b.sync.clone()).unwrap_or_default();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(6, 6))
                .show(ui, |ui| {
                    for d in diags {
                        let (ic, color) = match d.severity {
                            Severity::Error => (icon::X_CIRCLE, pal.err),
                            Severity::Warning => (icon::WARNING, pal.warn),
                        };
                        let bg = ui.painter().add(egui::Shape::Noop);
                        let r = egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(8, 5))
                            .corner_radius(6)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal_top(|ui| {
                                    ui.label(RichText::new(ic).color(color));
                                    ui.vertical(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&d.message).color(pal.text),
                                            )
                                            .wrap(),
                                        );
                                        for h in &d.hints {
                                            ui.label(
                                                RichText::new(format!("{}  {h}", icon::LIGHTBULB))
                                                    .small()
                                                    .color(pal.text_dim),
                                            );
                                        }
                                        let loc = d.location();
                                        if !loc.is_empty() {
                                            ui.label(
                                                RichText::new(loc)
                                                    .small()
                                                    .monospace()
                                                    .color(pal.text_faint),
                                            );
                                        }
                                    });
                                });
                            })
                            .response
                            .interact(Sense::click());
                        if r.hovered() {
                            ui.painter().set(
                                bg,
                                egui::epaint::RectShape::filled(r.rect, 6.0, pal.surface_hover),
                            );
                        }
                        let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
                        if r.clicked()
                            && let (Some(line), Some(col)) = (d.line, d.col)
                        {
                            act = Some(match &d.file {
                                None => Action::JumpTo { line, col },
                                Some(f) => Action::OpenAt {
                                    path: root.join(f),
                                    line,
                                    col,
                                },
                            });
                            // Forward synctex: la preview acompaña al salto.
                            go = crate::engine::page_for_line(&sync, d.file.as_deref(), line)
                                .map(|(page, y)| Action::GotoPreviewPage { page, y });
                        }
                    }
                });
        });
    if let Some(a) = act {
        st.actions.push(a);
    }
    if let Some(a) = go {
        st.actions.push(a);
    }
}
