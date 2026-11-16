//! Barra superior (menús + herramientas), barra de estado y diálogos.

use crate::plugins::run::Kind;
use crate::actions::Action;
use crate::app::{Modal, State, StatusKind};
use crate::diagnostics;
use crate::i18n::{tr, trf};
use crate::layout::{self, Panel, Preset, Tab};
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{Button, Margin, RichText};
use egui_dock::DockState;
use egui_phosphor::regular as icon;

fn menu_item(ui: &mut egui::Ui, ic: &str, text: &str, shortcut: &str) -> bool {
    let b = Button::new(format!("{ic}  {text}")).shortcut_text(RichText::new(shortcut).small());
    let clicked = ui.add(b).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

fn menu_check(ui: &mut egui::Ui, on: bool, text: &str, shortcut: &str) -> bool {
    let ic = if on { icon::CHECK_SQUARE } else { icon::SQUARE };
    menu_item(ui, ic, text, shortcut)
}

/// Entradas de exportación, comunes al menú Archivo y al botón de la barra.
fn export_items(ui: &mut egui::Ui, a: &mut Vec<Action>, plugins: &[PluginExport]) {
    if menu_item(ui, icon::FILE_PDF, "PDF…", "Ctrl+E") {
        a.push(Action::Export("pdf".into()))
    }
    if menu_item(ui, icon::FILE_PDF, tr("PDF/A-2b (archivo)…"), "") {
        a.push(Action::Export("pdfa".into()))
    }
    if menu_item(ui, icon::FILE_PNG, "PNG…", "") {
        a.push(Action::Export("png".into()))
    }
    if menu_item(ui, icon::FILE_SVG, "SVG…", "") {
        a.push(Action::Export("svg".into()))
    }
    if !plugins.is_empty() {
        ui.separator();
    }
    for (plugin, id, name) in plugins {
        if menu_item(ui, icon::PUZZLE_PIECE, &format!("{name}…"), "") {
            a.push(Action::PluginRun {
                plugin: plugin.clone(),
                kind: Kind::Export,
                id: id.clone(),
            })
        }
    }
}

/// Exportador de un plugin: (plugin, id, nombre).
type PluginExport = (String, String, String);

pub fn top_bar(st: &mut State, dock: &DockState<Tab>, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    let exports: Vec<PluginExport> = st
        .plugins
        .tools(Kind::Export)
        .map(|(p, t)| (p.id().to_string(), t.id.clone(), t.name.clone()))
        .collect();
    let a = &mut st.actions;
    // Con poco ancho los iconos pasan a menús: primero los paneles, luego los
    // atajos de archivo (que siguen en el menú Archivo).
    let width = ui.available_width();
    let compact = width < 1060.0;
    let tiny = width < 790.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.label(RichText::new("typst").strong().size(15.0).color(pal.accent));
        ui.add_space(8.0);

        ui.menu_button(tr("Archivo"), |ui| {
            ui.set_min_width(240.0);
            if menu_item(ui, icon::FILE_PLUS, tr("Nuevo"), "Ctrl+N") {
                a.push(Action::NewDoc)
            }
            if menu_item(ui, icon::FOLDER_PLUS, tr("Nuevo proyecto…"), tr("Ctrl+Mayús+N")) {
                a.push(Action::NewProject)
            }
            if menu_item(ui, icon::ARTICLE, tr("Nuevo desde plantilla…"), tr("Ctrl+Mayús+T")) {
                a.push(Action::ShowPanel(Panel::Templates))
            }
            if menu_item(ui, icon::FOLDER_OPEN, tr("Abrir…"), "Ctrl+O") {
                a.push(Action::Open)
            }
            if menu_item(ui, icon::FOLDER, tr("Abrir carpeta…"), "") {
                a.push(Action::PickFolder)
            }
            if menu_item(ui, icon::DOWNLOAD_SIMPLE, tr("Importar…"), "") {
                a.push(Action::Import)
            }
            ui.separator();
            if menu_item(ui, icon::FLOPPY_DISK, tr("Guardar"), "Ctrl+S") {
                a.push(Action::Save)
            }
            if menu_item(ui, icon::FLOPPY_DISK_BACK, tr("Guardar como…"), tr("Ctrl+Mayús+S")) {
                a.push(Action::SaveAs)
            }
            ui.separator();
            export_items(ui, a, &exports);
            ui.separator();
            if menu_item(ui, icon::X, tr("Cerrar pestaña"), "Ctrl+W") {
                a.push(Action::CloseCurrent)
            }
        });
        ui.menu_button(tr("Editar"), |ui| {
            ui.set_min_width(240.0);
            if menu_item(ui, icon::MAGNIFYING_GLASS, tr("Buscar"), "Ctrl+F") {
                a.push(Action::Find)
            }
            if menu_item(ui, icon::ARROW_BEND_DOWN_RIGHT, tr("Ir a línea…"), "Ctrl+G") {
                a.push(Action::GotoLinePrompt)
            }
            ui.separator();
            if menu_check(ui, st.prefs.wrap, tr("Ajuste de línea"), "Alt+Z") {
                a.push(Action::ToggleWrap)
            }
            if menu_item(ui, icon::TEXT_AA, tr("Aumentar fuente"), "Ctrl+=") {
                a.push(Action::FontDelta(1.0))
            }
            if menu_item(ui, icon::TEXT_AA, tr("Reducir fuente"), "Ctrl+-") {
                a.push(Action::FontDelta(-1.0))
            }
            if menu_item(ui, icon::TEXT_AA, tr("Fuente por defecto"), "Ctrl+0") {
                a.push(Action::FontReset)
            }
        });
        ui.menu_button(tr("Ver"), |ui| {
            ui.set_min_width(260.0);
            for p in Panel::ALL {
                if menu_check(ui, layout::has_panel(dock, p), p.title(), p.shortcut()) {
                    a.push(Action::TogglePanel(p))
                }
            }
            ui.separator();
            ui.menu_button(trf!("{}  Diseño", icon::LAYOUT), |ui| {
                ui.set_min_width(280.0);
                for preset in Preset::ALL {
                    if menu_item(ui, preset.icon(), preset.label(), "") {
                        a.push(Action::ApplyPreset(preset))
                    }
                }
            });
            ui.separator();
            if menu_check(ui, st.prefs.format_bar, tr("Barra de formato"), "") {
                a.push(Action::ToggleFormatBar)
            }
            if menu_check(ui, st.prefs.auto_compile, tr("Compilar al escribir"), "") {
                a.push(Action::ToggleAuto)
            }
            if menu_check(ui, st.prefs.auto_save, tr("Autoguardar"), "") {
                a.push(Action::ToggleAutoSave)
            }
            let pinned = st.pinned.is_some();
            if (pinned || st.docs.cur().path.is_some())
                && menu_check(ui, pinned, tr("Fijar vista previa"), "")
            {
                a.push(Action::TogglePin)
            }
            if menu_check(ui, pal.dark, tr("Tema oscuro"), "") {
                a.push(Action::ToggleTheme)
            }
            // El nombre del idioma al que se cambia, siempre en ese idioma.
            let other = match crate::i18n::current() {
                crate::i18n::Lang::Es => "English",
                crate::i18n::Lang::En => "Español",
            };
            if menu_item(ui, icon::TRANSLATE, other, "") {
                a.push(Action::ToggleLang)
            }
        });
        ui.menu_button(tr("Ayuda"), |ui| {
            ui.set_min_width(200.0);
            if menu_item(ui, icon::INFO, tr("Acerca de"), "") {
                a.push(Action::About)
            }
        });

        widgets::vsep(ui, &pal);

        if !tiny {
            if widgets::icon_button(ui, icon::FILE_PLUS, tr("Nuevo  (Ctrl+N)")).clicked() {
                a.push(Action::NewDoc)
            }
            if widgets::icon_button(ui, icon::FOLDER_OPEN, tr("Abrir  (Ctrl+O)")).clicked() {
                a.push(Action::Open)
            }
            let modified = st.docs.cur().modified();
            ui.add_enabled_ui(modified || st.docs.cur().path.is_none(), |ui| {
                if widgets::icon_button(ui, icon::FLOPPY_DISK, tr("Guardar  (Ctrl+S)")).clicked() {
                    a.push(Action::Save)
                }
            });

            widgets::vsep(ui, &pal);
        }

        if widgets::accent_button(ui, &pal, &trf!("{}  Compilar", icon::PLAY))
            .on_hover_text("F5")
            .clicked()
        {
            a.push(Action::Compile);
        }
        if !tiny
            && widgets::icon_toggle(
                ui,
                st.prefs.auto_compile,
                icon::LIGHTNING,
                tr("Compilar al escribir"),
            )
            .clicked()
        {
            a.push(Action::ToggleAuto);
        }
        ui.menu_button(RichText::new(trf!("{}  Exportar", icon::EXPORT)), |ui| {
            ui.set_min_width(220.0);
            export_items(ui, a, &exports);
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let theme_icon = if pal.dark { icon::SUN } else { icon::MOON };
            if widgets::icon_button(ui, theme_icon, tr("Cambiar tema")).clicked() {
                a.push(Action::ToggleTheme)
            }
            ui.menu_button(RichText::new(icon::LAYOUT).size(16.0), |ui| {
                ui.set_min_width(280.0);
                widgets::section_label(ui, &pal, tr("Diseño"));
                for preset in Preset::ALL {
                    if menu_item(ui, preset.icon(), preset.label(), "") {
                        a.push(Action::ApplyPreset(preset))
                    }
                }
            })
            .response
            .on_hover_text(tr("Diseño de paneles"));
            widgets::vsep(ui, &pal);
            let panels = [
                Panel::Problems,
                Panel::Preview,
                Panel::Snippets,
                Panel::Templates,
                Panel::Files,
                Panel::Scratch,
                Panel::Plugins,
                Panel::Symbols,
                Panel::Outline,
            ];
            if compact {
                widgets::icon_menu(ui, icon::SQUARES_FOUR, tr("Paneles"), |ui| {
                    ui.set_min_width(240.0);
                    for p in panels {
                        if menu_check(ui, layout::has_panel(dock, p), p.title(), p.shortcut()) {
                            a.push(Action::TogglePanel(p));
                        }
                    }
                });
            } else {
                for p in panels {
                    let on = layout::has_panel(dock, p);
                    if widgets::icon_toggle(ui, on, p.icon(), p.title()).clicked() {
                        a.push(Action::TogglePanel(p));
                    }
                }
            }
            widgets::vsep(ui, &pal);
            if widgets::icon_toggle(ui, st.prefs.format_bar, icon::TEXT_AA, tr("Barra de formato"))
                .clicked()
            {
                a.push(Action::ToggleFormatBar);
            }
        });
    });
    // Fila de formato fija: tipografía y párrafo (ver `formatbar::docked`).
    if st.prefs.format_bar {
        ui.separator();
        crate::formatbar::docked(st, ui);
    }
}

pub fn status_bar(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    let doc = st.docs.cur();
    // Estado de compilación: el del documento principal (el fijado si lo hay).
    let main = st.pinned.filter(|&id| st.docs.get(id).is_some()).unwrap_or(doc.id);
    let built = st.compiler.built.get(&main);
    let (errors, warnings) = built
        .map(|b| diagnostics::count(&b.diagnostics))
        .unwrap_or((0, 0));
    let compiling = st.compiler.in_flight.is_some_and(|k| k.doc == main);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        // Estado de compilación.
        if compiling {
            ui.add(egui::Spinner::new().size(12.0).color(pal.accent));
            ui.label(RichText::new(tr("Compilando…")).small().color(pal.text_dim));
        } else if let Some(b) = built {
            let (ic, color, text) = if errors > 0 {
                (icon::X_CIRCLE, pal.err, tr("Error de compilación").to_string())
            } else {
                let pages = st.preview.page_count();
                let p = if pages == 1 { tr("página") } else { tr("páginas") };
                (
                    icon::CHECK_CIRCLE,
                    pal.ok,
                    trf!("{} {} · {} ms", pages, p, b.ms),
                )
            };
            ui.label(RichText::new(format!("{ic} {text}")).small().color(color));
        } else {
            ui.label(
                RichText::new(format!("{} {}", icon::CIRCLE_DASHED, tr("Sin compilar")))
                    .small()
                    .color(pal.text_faint),
            );
        }
        // Contadores → panel Problemas.
        let counts = RichText::new(format!(
            "{} {errors}   {} {warnings}",
            icon::X_CIRCLE,
            icon::WARNING
        ))
        .small()
        .color(if errors > 0 {
            pal.err
        } else if warnings > 0 {
            pal.warn
        } else {
            pal.text_faint
        });
        if ui
            .add(Button::new(counts).frame_when_inactive(false))
            .on_hover_text(tr("Problemas"))
            .clicked()
        {
            st.actions.push(Action::TogglePanel(Panel::Problems));
        }
        // Mensaje transitorio.
        if let Some((msg, kind)) = st.status.visible() {
            let color = match kind {
                StatusKind::Ok => pal.ok,
                StatusKind::Err => pal.err,
                StatusKind::Info => pal.text_dim,
            };
            // Deja sitio a los controles de la derecha; el texto entero, en el tooltip.
            let width = (ui.available_width() - 470.0).max(0.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 18.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add(egui::Label::new(RichText::new(msg).small().color(color)).truncate())
                        .on_hover_text(msg);
                },
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if let Some(v) = &st.typst_version {
                ui.label(RichText::new(v).small().color(pal.text_faint));
                widgets::vsep(ui, &pal);
            }
            let wrap_txt = if st.prefs.wrap {
                tr("Ajuste: sí")
            } else {
                tr("Ajuste: no")
            };
            if ui
                .add(Button::new(RichText::new(wrap_txt).small()).frame_when_inactive(false))
                .on_hover_text(tr("Ajuste de línea (Alt+Z)"))
                .clicked()
            {
                st.actions.push(Action::ToggleWrap);
            }
            ui.menu_button(
                RichText::new(format!("{} {:.0}", icon::TEXT_AA, st.prefs.font_size)).small(),
                |ui| {
                    if menu_item(ui, icon::PLUS, "Aumentar", "Ctrl+=") {
                        st.actions.push(Action::FontDelta(1.0))
                    }
                    if menu_item(ui, icon::MINUS, "Reducir", "Ctrl+-") {
                        st.actions.push(Action::FontDelta(-1.0))
                    }
                    if menu_item(ui, icon::ARROW_COUNTER_CLOCKWISE, "Restablecer", "Ctrl+0") {
                        st.actions.push(Action::FontReset)
                    }
                },
            );
            widgets::vsep(ui, &pal);
            let (lines, words) = st.stats.get(doc);
            ui.label(
                RichText::new(trf!("{} líneas · {} palabras", lines, words))
                    .small()
                    .color(pal.text_faint),
            );
            if let Some(c) = st.editor.cursor {
                let (l, col) = st.stats.line_col(doc, c);
                let r = ui.add(
                    Button::new(RichText::new(format!("Ln {l}, Col {col}")).small())
                        .frame_when_inactive(false),
                );
                if r.on_hover_text(tr("Ir a línea (Ctrl+G)")).clicked() {
                    st.actions.push(Action::GotoLinePrompt);
                }
            }
            if doc.modified() {
                ui.label(
                    RichText::new(format!("{} sin guardar", icon::DOT_OUTLINE))
                        .small()
                        .color(pal.warn),
                );
            }
        });
    });
}

fn ui_enter(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.key_pressed(egui::Key::Enter))
}

pub fn modals(st: &mut State, ctx: &egui::Context) {
    let pal = theme::palette(ctx);
    let Some(modal) = st.modal.as_mut() else {
        return;
    };
    let mut close = false;
    match modal {
        Modal::ConfirmClose(id) => {
            let id = *id;
            let name = st.docs.get(id).map(|d| d.fname()).unwrap_or_default();
            widgets::modal(ctx, "confirm-close", 400.0, |ui| {
                widgets::modal_header_tinted(
                    ui,
                    &pal,
                    icon::WARNING,
                    pal.warn,
                    &trf!("¿Guardar los cambios de «{}»?", name),
                    Some(tr("Si no los guardas, se perderán.")),
                );
                widgets::modal_footer(ui, |ui| {
                    if widgets::primary_button(ui, &pal, tr("Guardar"), true).clicked() {
                        st.actions.push(Action::SaveAndClose(id));
                        close = true;
                    }
                    if widgets::secondary_button(ui, &pal, tr("No guardar")).clicked() {
                        st.actions.push(Action::Discard(id));
                        close = true;
                    }
                    if widgets::secondary_button(ui, &pal, tr("Cancelar")).clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape))
                    {
                        close = true;
                    }
                });
            });
        }
        Modal::ConfirmDelete(path) => {
            let path = path.clone();
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            widgets::modal(ctx, "confirm-delete", 400.0, |ui| {
                widgets::modal_header_tinted(
                    ui,
                    &pal,
                    icon::TRASH,
                    pal.err,
                    &trf!("¿Eliminar «{}»?", name),
                    Some(tr("Se borra del disco y no se puede deshacer.")),
                );
                widgets::modal_footer(ui, |ui| {
                    if widgets::danger_button(ui, &pal, tr("Eliminar")).clicked() {
                        crate::files::delete_path(st, &path);
                        close = true;
                    }
                    if widgets::secondary_button(ui, &pal, tr("Cancelar")).clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape))
                    {
                        close = true;
                    }
                });
            });
        }
        Modal::GotoLine(input) => {
            widgets::modal(ctx, "goto-line", 320.0, |ui| {
                widgets::modal_header(
                    ui,
                    &pal,
                    icon::ARROW_BEND_DOWN_RIGHT,
                    tr("Ir a línea"),
                    Some(tr("Línea y, opcional, columna: 42 o 42:7.")),
                );
                let r = ui.add(
                    egui::TextEdit::singleline(input)
                        .hint_text(tr("línea[:columna]"))
                        .margin(widgets::FIELD_MARGIN)
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                let go = widgets::modal_footer(ui, |ui| {
                    let go = widgets::primary_button(ui, &pal, tr("Ir"), true).clicked();
                    if widgets::secondary_button(ui, &pal, tr("Cancelar")).clicked() {
                        close = true;
                    }
                    go
                });
                if go || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let mut it = input
                        .split([':', ','])
                        .map(|s| s.trim().parse::<usize>().ok());
                    if let Some(Some(line)) = it.next() {
                        let col = it.next().flatten().unwrap_or(1);
                        st.actions.push(Action::JumpTo {
                            line: line.max(1),
                            col: col.max(1),
                        });
                    }
                    close = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
        }
        Modal::NewProject(np) => {
            close = crate::project::show(ctx, &pal, np, &mut st.actions);
        }
        Modal::Export(d) => {
            let (title, what) = match d.fmt.as_str() {
                "pdf" => (tr("Exportar PDF"), tr("un PDF")),
                "pdfa" => (tr("Exportar PDF/A-2b"), tr("un PDF/A-2b, para archivo")),
                "png" => (tr("Exportar PNG"), tr("una imagen PNG por página")),
                _ => (tr("Exportar SVG"), tr("un SVG por página")),
            };
            let parsed = crate::engine::parse_pages(&d.pages, d.total);
            let png = d.fmt == "png";
            let go = widgets::modal(ctx, "export", 380.0, |ui| {
                widgets::modal_header(
                    ui,
                    &pal,
                    icon::EXPORT,
                    title,
                    Some(&trf!("Se guarda {}.", what)),
                );
                ui.label(RichText::new(tr("Páginas")).small().color(pal.text_dim));
                let r = ui.add(
                    egui::TextEdit::singleline(&mut d.pages)
                        .hint_text(tr("todas · p. ej. 1-3, 5, 8-"))
                        .margin(widgets::FIELD_MARGIN)
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                match &parsed {
                    Err(e) => {
                        ui.label(RichText::new(e).small().color(pal.err));
                    }
                    Ok(v) => {
                        let msg = match (v.len(), d.total) {
                            (0, 0) => tr("Todas las páginas").to_string(),
                            (0, n) => trf!("Todas las páginas ({})", n),
                            (1, _) => tr("1 página").to_string(),
                            (k, _) => trf!("{} páginas", k),
                        };
                        ui.label(RichText::new(msg).small().color(pal.text_dim));
                        if !v.is_empty() && !png && d.fmt != "svg" {
                            ui.label(
                                RichText::new(
                                    tr("Con rango, el PDF sale sin etiquetas de accesibilidad."),
                                )
                                .small()
                                .color(pal.text_faint),
                            );
                        }
                    }
                }
                if png {
                    ui.add_space(4.0);
                    ui.label(RichText::new(tr("Resolución")).small().color(pal.text_dim));
                    ui.horizontal(|ui| {
                        for ppi in [144, 300, 600] {
                            if ui
                                .selectable_label(d.ppi == ppi, format!("{ppi} ppi"))
                                .clicked()
                            {
                                d.ppi = ppi;
                            }
                        }
                    });
                }
                widgets::modal_footer(ui, |ui| {
                    let go =
                        widgets::primary_button(ui, &pal, tr("Exportar…"), parsed.is_ok()).clicked();
                    if widgets::secondary_button(ui, &pal, tr("Cancelar")).clicked() {
                        close = true;
                    }
                    go
                })
            })
            .inner;
            let enter = ui_enter(ctx);
            if let Ok(pages) = &parsed
                && (go || enter)
            {
                st.actions.push(Action::ExportRun {
                    fmt: d.fmt.clone(),
                    ppi: png.then_some(d.ppi),
                    pages: pages.clone(),
                });
                close = true;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        }
        Modal::About => {
            let r = widgets::modal(ctx, "about", 420.0, |ui| {
                widgets::modal_header(
                    ui,
                    &pal,
                    icon::INFO,
                    "Grammata",
                    Some(tr("Editor de Typst en Rust + egui, con vista previa del PDF real.")),
                );
                let v = st
                    .typst_version
                    .as_deref()
                    .unwrap_or(tr("typst no encontrado en el PATH"));
                ui.label(
                    RichText::new(trf!("Compilador: {}", v))
                        .small()
                        .color(pal.text_dim),
                );
                ui.label(
                    RichText::new(tr("Render PDF: hayro · Iconos: Phosphor"))
                        .small()
                        .color(pal.text_dim),
                );
                ui.add_space(10.0);
                widgets::section_label(ui, &pal, tr("Atajos"));
                egui::Grid::new("shortcuts")
                    .num_columns(2)
                    .spacing([16.0, 3.0])
                    .show(ui, |ui| {
                        for (k, d) in SHORTCUTS {
                            ui.label(RichText::new(tr(k)).monospace().small().color(pal.text));
                            ui.label(RichText::new(tr(d)).small().color(pal.text_dim));
                            ui.end_row();
                        }
                    });
                widgets::modal_footer(ui, |ui| {
                    widgets::secondary_button(ui, &pal, tr("Cerrar")).clicked()
                })
            });
            if r.inner || r.should_close() {
                close = true;
            }
        }
        // Lo dibuja `snippets::edit_modal` desde `App::ui`.
        Modal::EditSnippet(_) => {}
    }
    if close {
        st.modal = None;
    }
}

const SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+N / O / S", "nuevo · abrir · guardar"),
    ("Ctrl+Mayús+N", "nuevo proyecto"),
    ("Ctrl+W", "cerrar pestaña"),
    ("Ctrl+Tab / Ctrl+RePág", "cambiar de pestaña"),
    ("Ctrl+1…9", "ir a la pestaña n"),
    ("F5", "compilar"),
    ("Ctrl+E", "exportar PDF"),
    ("Ctrl+B / I / U", "negrita · cursiva · subrayado"),
    ("Ctrl+F · F3", "buscar · siguiente"),
    ("Ctrl+G", "ir a línea"),
    ("Ctrl+Espacio", "autocompletar"),
    ("Ctrl+Mayús+O / V / M", "esquema · vista previa · problemas"),
    (
        "Ctrl+Mayús+Y / I / T / F / B",
        "símbolos · snippets · plantillas · archivos · borrador",
    ),
    ("Ctrl+= / - / 0", "tamaño de fuente"),
    ("Ctrl+rueda", "zoom de la vista previa"),
];

/// Marco de la barra superior.
pub fn top_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::new().fill(pal.chrome).inner_margin(Margin {
        left: 10,
        right: 8,
        top: 5,
        bottom: 5,
    })
}

pub fn status_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::new().fill(pal.chrome).inner_margin(Margin {
        left: 10,
        right: 10,
        top: 3,
        bottom: 3,
    })
}
