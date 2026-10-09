//! Gestor de archivos: árbol de una carpeta raíz elegible (persistida en
//! `Prefs::files_root`), con abrir, crear, renombrar y eliminar.
//!
//! La raíz se elige con `Archivo → Abrir carpeta…` o desde el propio panel;
//! el árbol se reescanea solo (cada 2 s) y tras cada operación.

use crate::actions::Action;
use crate::app::{State, StatusKind};
use crate::doc::Docs;
use crate::theme;
use crate::widgets;
use egui::RichText;
use egui_phosphor::regular as icon;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Profundidad máxima del árbol y tope de entradas (carpetas enormes no
/// deben congelar la UI).
const MAX_DEPTH: usize = 8;
const MAX_ENTRIES: usize = 5000;
const RESCAN_EVERY: Duration = Duration::from_secs(2);

/// Extensiones que se abren en el editor al clicar (el resto informa).
const OPENABLE: &[&str] = &[
    "typ", "md", "markdown", "txt", "bib", "tex", "csv", "json", "toml", "yaml", "yml",
];
/// Extensiones de imagen (icono distinto; se insertan con `#image`).
const IMAGES: &[&str] = &["png", "jpg", "jpeg", "svg", "webp", "gif"];

/// Una fila del árbol.
#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
}

/// Estado transitorio del panel (la raíz vive en `Prefs::files_root`).
#[derive(Default)]
pub struct FileMgr {
    entries: Vec<Entry>,
    scanned_root: Option<PathBuf>,
    scan_error: Option<String>,
    last_scan: Option<Instant>,
    expanded: HashSet<PathBuf>,
    selected: Option<PathBuf>,
    creating_dir: Option<bool>,
    new_name: String,
    create_focus: bool,
    renaming: Option<PathBuf>,
    rename_text: String,
    rename_focus: bool,
}

// ---------- Lógica pura (testeable) ----------

/// Nombre válido para crear/renombrar: sin separadores ni valores vacíos.
pub fn valid_name(name: &str) -> bool {
    let t = name.trim();
    !t.is_empty() && t != "." && t != ".." && !t.contains(['/', '\\', '\0'])
}

/// ¿Se abre en el editor al clicar?
pub fn openable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| OPENABLE.contains(&e.to_ascii_lowercase().as_str()))
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGES.contains(&e.to_ascii_lowercase().as_str()))
}

/// Escanea `root` en profundidad (directorios primero, alfabético
/// insensible a mayúsculas). Ocultos y enlaces se saltan.
pub fn scan(root: &Path) -> std::io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    walk(root, 0, &mut out)?;
    Ok(out)
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<Entry>) -> std::io::Result<()> {
    if depth > MAX_DEPTH || out.len() >= MAX_ENTRIES {
        return Ok(());
    }
    let mut dirs: Vec<(PathBuf, String)> = Vec::new();
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let ft = e.file_type()?;
        // Sin seguir enlaces (bucles) ni nada raro.
        if ft.is_symlink() {
            continue;
        } else if ft.is_dir() {
            dirs.push((e.path(), name));
        } else if ft.is_file() {
            files.push((e.path(), name));
        }
    }
    dirs.sort_by_key(|a| a.1.to_lowercase());
    files.sort_by_key(|a| a.1.to_lowercase());
    for (path, name) in dirs {
        out.push(Entry {
            path: path.clone(),
            name,
            is_dir: true,
            depth,
        });
        // Un subdirectorio ilegible no tumba todo el árbol.
        let _ = walk(&path, depth + 1, out);
    }
    for (path, name) in files {
        out.push(Entry {
            path,
            name,
            is_dir: false,
            depth,
        });
    }
    Ok(())
}

/// Tras renombrar `old → new` (archivo o directorio): actualiza los docs
/// abiertos que apuntaban dentro. Devuelve cuántos tocó.
pub fn docs_rename(docs: &mut Docs, old: &Path, new: &Path) -> usize {
    let mut n = 0;
    for d in &mut docs.list {
        let swap = if d.path.as_deref() == Some(old) {
            Some(new.to_path_buf())
        } else {
            d.path
                .as_deref()
                .and_then(|p| p.strip_prefix(old).ok())
                .map(|p| new.join(p))
        };
        if let Some(p) = swap {
            d.path = Some(p);
            d.touch();
            n += 1;
        }
    }
    n
}

/// Tras eliminar `gone` (archivo o directorio): los docs abiertos que
/// apuntaban dentro se quedan sin archivo (conservan el texto para
/// guardarlos en otro sitio con `Guardar como…`).
pub fn docs_detach(docs: &mut Docs, gone: &Path) -> usize {
    let mut n = 0;
    for d in &mut docs.list {
        let inside = d.path.as_deref() == Some(gone)
            || d.path
                .as_deref()
                .is_some_and(|p| p.strip_prefix(gone).is_ok());
        if inside {
            d.path = None;
            // Ya no hay copia en disco: cerrar debe pedir confirmación.
            d.saved.clear();
            n += 1;
        }
    }
    n
}

/// Elimina `path` del disco y desengancha los docs abiertos afectados.
pub fn delete_path(st: &mut State, path: &Path) {
    let r = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match r {
        Ok(()) => {
            let n = docs_detach(&mut st.docs, path);
            st.files.expanded.remove(path);
            if st.files.selected.as_deref() == Some(path) {
                st.files.selected = None;
            }
            st.files.refresh();
            st.needs_persist = true;
            let msg = if n > 0 {
                format!("Eliminado {} ({n} pestaña(s) sin archivo)", path.display())
            } else {
                format!("Eliminado {}", path.display())
            };
            st.set_status(msg, StatusKind::Info);
        }
        Err(e) => st.set_status(format!("No se pudo eliminar: {e}"), StatusKind::Err),
    }
}

// ---------- Panel ----------

impl FileMgr {
    fn refresh(&mut self) {
        self.last_scan = None;
    }

    /// ¿Se muestra la entrada? Toda carpeta antecesora (hasta la raíz) debe
    /// estar desplegada.
    fn visible(&self, it: &Entry, root: &Path) -> bool {
        let mut a = it.path.parent();
        while let Some(p) = a {
            if p == root {
                break;
            }
            if !self.expanded.contains(p) {
                return false;
            }
            a = p.parent();
        }
        true
    }
}

/// Elige la carpeta raíz del gestor.
pub fn pick_root(st: &mut State) {
    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
        st.prefs.files_root = Some(dir);
        st.files.scanned_root = None;
        st.files.selected = None;
        st.needs_persist = true;
    }
}

pub fn panel(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    let root = st.prefs.files_root.clone();
    let Some(root) = root else {
        empty_state(st, ui, &pal);
        return;
    };
    if !root.is_dir() {
        empty_state(st, ui, &pal);
        st.set_status(
            format!("La carpeta {} ya no existe", root.display()),
            StatusKind::Err,
        );
        return;
    }

    // Reescaneo periódico (la raíz también la tocan otros programas).
    let new_root = st.files.scanned_root.as_deref() != Some(root.as_path());
    if new_root {
        // Al fijar raíz: los ancestros del documento actual quedan a la vista.
        let mut a = st.docs.cur().path.as_deref().and_then(|p| p.parent());
        while let Some(p) = a {
            if p == root.as_path() {
                break;
            }
            st.files.expanded.insert(p.to_path_buf());
            a = p.parent();
        }
    }
    let stale = new_root || st.files.last_scan.is_none_or(|t| t.elapsed() > RESCAN_EVERY);
    if stale {
        match scan(&root) {
            Ok(entries) => {
                st.files.entries = entries;
                st.files.scan_error = None;
            }
            Err(e) => {
                st.files.entries.clear();
                st.files.scan_error = Some(e.to_string());
            }
        }
        st.files.scanned_root = Some(root.clone());
        st.files.last_scan = Some(Instant::now());
    }

    // Barra: actualizar, cambiar de carpeta, crear.
    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::section_label(
                ui,
                &pal,
                &root
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| root.display().to_string()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, icon::ARROWS_CLOCKWISE, "Actualizar").clicked() {
                    st.files.refresh();
                }
                if widgets::icon_button(ui, icon::FOLDER_OPEN, "Abrir otra carpeta…").clicked() {
                    pick_root(st);
                    return;
                }
                if widgets::icon_button(ui, icon::FILE_TEXT, "Usar la carpeta del documento actual")
                    .clicked()
                {
                    use_doc_dir(st);
                    return;
                }
                if widgets::icon_button(ui, icon::FOLDER_PLUS, "Nueva carpeta").clicked() {
                    st.files.creating_dir = Some(true);
                    st.files.new_name = "nueva-carpeta".to_string();
                    st.files.create_focus = true;
                    st.files.renaming = None;
                }
                if widgets::icon_button(ui, icon::FILE_PLUS, "Nuevo archivo").clicked() {
                    st.files.creating_dir = Some(false);
                    st.files.new_name = "nuevo-archivo.typ".to_string();
                    st.files.create_focus = true;
                    st.files.renaming = None;
                }
            });
        });
    });

    // Fila de creación (bajo la barra, como la de búsqueda).
    if st.files.creating_dir.is_some() {
        create_row(st, ui, &pal, &root);
    }

    if let Some(e) = &st.files.scan_error {
        ui.label(RichText::new(format!("No se pudo leer: {e}")).color(pal.err));
    }
    // Migas de pan de la selección (o la raíz): clic selecciona y despliega.
    breadcrumbs(st, ui, &pal, &root);
    let mut open: Option<PathBuf> = None;
    let mut toggle: Option<PathBuf> = None;
    let mut start_rename: Option<(PathBuf, String)> = None;
    let mut ask_delete: Option<PathBuf> = None;
    let mut create_here: Option<(bool, PathBuf)> = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(6, 6))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let current = st.docs.cur().path.clone();
                    for it in st.files.entries.clone() {
                        // Oculta lo que cuelga de una carpeta plegada.
                        if !st.files.visible(&it, &root) {
                            continue;
                        }
                        row(
                            st,
                            ui,
                            &pal,
                            &it,
                            current.as_deref(),
                            &mut open,
                            &mut toggle,
                            &mut start_rename,
                            &mut ask_delete,
                            &mut create_here,
                        );
                    }
                });
        });
    if let Some(p) = toggle {
        toggle_dir(st, &p);
    }
    if let Some((is_dir, dir)) = create_here {
        // Crear dentro de una carpeta concreta (desde su menú).
        st.files.selected = Some(dir);
        st.files.creating_dir = Some(is_dir);
        st.files.new_name = if is_dir {
            "nueva-carpeta".to_string()
        } else {
            "nuevo-archivo.typ".to_string()
        };
        st.files.create_focus = true;
        st.files.renaming = None;
    }
    if let Some((p, name)) = start_rename {
        st.files.renaming = Some(p);
        st.files.rename_text = name;
        st.files.rename_focus = true;
        st.files.creating_dir = None;
    }
    if let Some(p) = ask_delete {
        st.modal = Some(crate::app::Modal::ConfirmDelete(p));
    }
    if let Some(p) = open {
        st.actions.push(Action::OpenPath(p));
    }
}

/// Sin raíz: qué hacer.
fn empty_state(st: &mut State, ui: &mut egui::Ui, pal: &theme::Palette) {
    widgets::toolbar_frame(pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::section_label(ui, pal, "Archivos");
        });
    });
    egui::Frame::new().inner_margin(14).show(ui, |ui| {
        ui.label(RichText::new("Sin carpeta todavía.").color(pal.text_dim));
        ui.label(
            RichText::new("El gestor muestra el árbol de una carpeta para abrir y organizar tus `.typ`.")
                .small()
                .color(pal.text_faint),
        );
        ui.add_space(8.0);
        if widgets::accent_button(ui, pal, &format!("{}  Abrir carpeta…", icon::FOLDER_OPEN))
            .clicked()
        {
            pick_root(st);
        }
        if ui
            .button(format!("{}  Usar la del documento", icon::FILE_TEXT))
            .on_hover_text("Fija la carpeta del documento actual")
            .clicked()
        {
            use_doc_dir(st);
        }
    });
}

/// Fija como raíz la carpeta del documento actual (si tiene archivo).
fn use_doc_dir(st: &mut State) {
    let dir = st.docs.cur().root_dir();
    if st.docs.cur().path.is_none() {
        st.set_status(
            "Guarda el documento primero para usar su carpeta",
            StatusKind::Info,
        );
        return;
    }
    st.prefs.files_root = Some(dir);
    st.files.scanned_root = None;
    st.files.selected = None;
    st.needs_persist = true;
}

/// Migas de pan desde la raíz hasta la selección: clic selecciona la
/// carpeta y la despliega.
fn breadcrumbs(st: &mut State, ui: &mut egui::Ui, pal: &theme::Palette, root: &Path) {
    let target = st
        .files
        .selected
        .clone()
        .unwrap_or_else(|| root.to_path_buf());
    let mut chain = vec![root.to_path_buf()];
    if let Ok(rel) = target.strip_prefix(root) {
        let mut p = root.to_path_buf();
        for c in rel.components() {
            p.push(c);
            chain.push(p.clone());
        }
    }
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(10, 2))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, p) in chain.iter().enumerate() {
                    if i > 0 {
                        ui.label(RichText::new("›").small().color(pal.text_faint));
                    }
                    let name = p
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    let last = i + 1 == chain.len();
                    let b = egui::Button::new(
                        RichText::new(name).small().color(if last {
                            pal.text
                        } else {
                            pal.text_dim
                        }),
                    )
                    .selected(last)
                    .frame_when_inactive(false);
                    if ui.add(b).on_hover_text(p.display().to_string()).clicked() {
                        st.files.selected = Some(p.clone());
                        if p.is_dir() {
                            st.files.expanded.insert(p.clone());
                        }
                    }
                }
            });
        });
}
/// Una fila del árbol (o la edición de su nombre al renombrar).
#[allow(clippy::too_many_arguments)]
fn row(
    st: &mut State,
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    it: &Entry,
    current: Option<&Path>,
    open: &mut Option<PathBuf>,
    toggle: &mut Option<PathBuf>,
    start_rename: &mut Option<(PathBuf, String)>,
    ask_delete: &mut Option<PathBuf>,
    create_here: &mut Option<(bool, PathBuf)>,
) {
    let selected = st.files.selected.as_deref() == Some(it.path.as_path());
    let is_current = current == Some(it.path.as_path());
    let open_modified = !it.is_dir
        && st
            .docs
            .list
            .iter()
            .any(|d| d.path.as_deref() == Some(it.path.as_path()) && d.modified());
    if st.files.renaming.as_deref() == Some(it.path.as_path()) {
        // Edición en línea del nombre.
        ui.horizontal(|ui| {
            ui.add_space(it.depth as f32 * 14.0);
            let r = ui.add(
                egui::TextEdit::singleline(&mut st.files.rename_text)
                    .desired_width(f32::INFINITY),
            );
            if st.files.rename_focus {
                r.request_focus();
                st.files.rename_focus = false;
            }
            // `lost_focus`: egui suelta el foco del singleline al pulsar Enter/Esc.
            let live = r.has_focus() || r.lost_focus();
            let enter = live && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = live && ui.input(|i| i.key_pressed(egui::Key::Escape));
            if enter {
                commit_rename(st);
            } else if esc {
                st.files.renaming = None;
            }
        });
        return;
    }
    let (ic, color) = if it.is_dir {
        let open = st.files.expanded.contains(&it.path);
        (
            if open { icon::FOLDER_OPEN } else { icon::FOLDER },
            pal.text_dim,
        )
    } else if is_image(&it.path) {
        (icon::IMAGE, pal.text_dim)
    } else if it.path.extension().and_then(|e| e.to_str()) == Some("pdf") {
        (icon::FILE_PDF, pal.text_dim)
    } else if openable(&it.path) {
        (icon::FILE_TEXT, pal.text)
    } else {
        (icon::FILE, pal.text_faint)
    };
    let name_color = if is_current { pal.accent } else { color };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        // Guías de jerarquía en el sangrado (una línea por nivel).
        let indent_w = it.depth as f32 * 14.0;
        if indent_w > 0.0 {
            let (space, _) =
                ui.allocate_exact_size(egui::vec2(indent_w, 24.0), egui::Sense::hover());
            let painter = ui.painter();
            for k in 0..it.depth {
                painter.vline(
                    space.left() + 7.0 + k as f32 * 14.0,
                    space.y_range(),
                    egui::Stroke::new(1.0, pal.border),
                );
            }
        }
        if it.is_dir {
            let caret = if st.files.expanded.contains(&it.path) {
                icon::CARET_DOWN
            } else {
                icon::CARET_RIGHT
            };
            ui.label(RichText::new(caret).small().color(pal.text_faint));
        }
        let mut label = format!("{ic}  {}", it.name);
        if open_modified {
            label.push_str(&format!("  {}", icon::DOT_OUTLINE));
        }
        let mut text = RichText::new(label).color(name_color);
        if it.is_dir {
            text = text.strong();
        }
        let r = ui
            .add(
                egui::Button::new((text, egui::Atom::grow()))
                    .selected(selected)
                    .frame_when_inactive(false)
                    .min_size(egui::vec2(ui.available_width(), 24.0)),
            )
            .on_hover_text(it.path.display().to_string());
        if r.clicked() {
            st.files.selected = Some(it.path.clone());
            if it.is_dir {
                *toggle = Some(it.path.clone());
            } else if openable(&it.path) {
                *open = Some(it.path.clone());
            } else {
                st.set_status(
                    "Solo los archivos de texto se abren en el editor",
                    StatusKind::Info,
                );
            }
        }
        // Menú contextual (clic derecho): primero selecciona la fila.
        r.context_menu(|ui| {
            st.files.selected = Some(it.path.clone());
            ui.set_min_width(200.0);
            if it.is_dir {
                if ui.button("Nuevo archivo aquí").clicked() {
                    *create_here = Some((false, it.path.clone()));
                    ui.close();
                }
                if ui.button("Nueva carpeta aquí").clicked() {
                    *create_here = Some((true, it.path.clone()));
                    ui.close();
                }
                ui.separator();
            } else if openable(&it.path) && ui.button("Abrir").clicked() {
                *open = Some(it.path.clone());
                ui.close();
            }
            if ui.button("Renombrar  (F2)").clicked() {
                *start_rename = Some((it.path.clone(), it.name.clone()));
                ui.close();
            }
            if ui.button("Eliminar  (Supr)").clicked() {
                *ask_delete = Some(it.path.clone());
                ui.close();
            }
        });
    });
    // Atajos sobre la selección: F2 renombra, Supr pide confirmación.
    // Solo si ningún widget tiene el teclado (editor, campos de texto…).
    if selected
        && ui.ctx().memory(|m| m.focused().is_none())
        && ui.input(|i| {
            i.key_pressed(egui::Key::F2) || i.key_pressed(egui::Key::Delete)
        })
    {
        if ui.input(|i| i.key_pressed(egui::Key::F2)) {
            *start_rename = Some((it.path.clone(), it.name.clone()));
        } else {
            *ask_delete = Some(it.path.clone());
        }
    }
}

fn toggle_dir(st: &mut State, dir: &Path) {
    if !st.files.expanded.remove(dir) {
        st.files.expanded.insert(dir.to_path_buf());
    }
}

/// Fila de creación bajo la barra: nombre + Crear/Cancelar.
fn create_row(st: &mut State, ui: &mut egui::Ui, pal: &theme::Palette, root: &Path) {
    let is_dir = st.files.creating_dir == Some(true);
    let parent: PathBuf = st
        .files
        .selected
        .as_deref()
        .filter(|p| p.is_dir())
        .unwrap_or(root)
        .to_path_buf();
    widgets::toolbar_frame(pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(if is_dir {
                icon::FOLDER_PLUS
            } else {
                icon::FILE_PLUS
            }));
            let r = ui.add(
                egui::TextEdit::singleline(&mut st.files.new_name)
                    .hint_text(if is_dir { "nombre-carpeta" } else { "nombre-archivo.typ" })
                    .desired_width(220.0),
            );
            if st.files.create_focus {
                r.request_focus();
                st.files.create_focus = false;
            }
            let live = r.has_focus() || r.lost_focus();
            let go = ui.small_button("Crear").clicked()
                || (live && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if go {
                commit_create(st, &parent);
            }
            if ui.small_button("✕").clicked()
                || (live && ui.input(|i| i.key_pressed(egui::Key::Escape)))
            {
                st.files.creating_dir = None;
            }
        });
    });
}

fn commit_create(st: &mut State, parent: &Path) {
    let is_dir = st.files.creating_dir == Some(true);
    let mut name = st.files.new_name.trim().to_string();
    if !is_dir && !name.contains('.') {
        name.push_str(".typ");
    }
    if !valid_name(&name) {
        st.set_status("Nombre no válido", StatusKind::Err);
        return;
    }
    let dest = parent.join(&name);
    if dest.exists() {
        st.set_status("Ya existe algo con ese nombre", StatusKind::Err);
        return;
    }
    let r = if is_dir {
        std::fs::create_dir(&dest).map(|_| ())
    } else {
        std::fs::write(&dest, "").map(|_| ())
    };
    match r {
        Ok(()) => {
            st.files.expanded.insert(parent.to_path_buf());
            st.files.selected = Some(dest.clone());
            st.files.creating_dir = None;
            st.files.refresh();
            st.set_status(format!("Creado {}", dest.display()), StatusKind::Ok);
            if !is_dir && openable(&dest) {
                st.actions.push(Action::OpenPath(dest));
            }
        }
        Err(e) => st.set_status(format!("No se pudo crear: {e}"), StatusKind::Err),
    }
}

fn commit_rename(st: &mut State) {
    let Some(old) = st.files.renaming.clone() else {
        return;
    };
    let name = st.files.rename_text.trim().to_string();
    if !valid_name(&name) {
        st.set_status("Nombre no válido", StatusKind::Err);
        return;
    }
    let Some(parent) = old.parent().map(Path::to_path_buf) else {
        return;
    };
    let dest = parent.join(&name);
    if dest == old {
        st.files.renaming = None;
        return;
    }
    if dest.exists() {
        st.set_status("Ya existe algo con ese nombre", StatusKind::Err);
        return;
    }
    match std::fs::rename(&old, &dest) {
        Ok(()) => {
            let n = docs_rename(&mut st.docs, &old, &dest);
            if st.files.expanded.remove(&old) {
                st.files.expanded.insert(dest.clone());
            }
            st.files.selected = Some(dest.clone());
            st.files.renaming = None;
            st.files.refresh();
            st.needs_persist = true;
            st.set_status(
                if n > 0 {
                    format!("Renombrado ({n} pestaña(s) actualizada(s))")
                } else {
                    "Renombrado".to_string()
                },
                StatusKind::Ok,
            );
        }
        Err(e) => st.set_status(format!("No se pudo renombrar: {e}"), StatusKind::Err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("typst-editor-fm-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a").join("b")).unwrap();
        std::fs::write(d.join("z.typ"), "= Z\n").unwrap();
        std::fs::write(d.join("a").join("m.md"), "# M\n").unwrap();
        std::fs::write(d.join("a").join("b").join("hondo.typ"), "= H\n").unwrap();
        std::fs::write(d.join(".oculto"), "x").unwrap();
        d
    }

    #[test]
    fn escanea_ordenado_sin_ocultos() {
        let d = sandbox("scan");
        let out = scan(&d).unwrap();
        let names: Vec<_> = out
            .iter()
            .map(|e| format!("{}{}", if e.is_dir { "d:" } else { "f:" }, e.name))
            .collect();
        assert_eq!(names, ["d:a", "d:b", "f:hondo.typ", "f:m.md", "f:z.typ"]);
        assert_eq!(out[1].depth, 1);
    }

    #[test]
    fn nombres_validos() {
        assert!(valid_name("notas.typ"));
        assert!(valid_name("con espacios .md"));
        assert!(!valid_name(""));
        assert!(!valid_name("  "));
        assert!(!valid_name("."));
        assert!(!valid_name(".."));
        assert!(!valid_name("a/b"));
        assert!(!valid_name("a\\b"));
    }

    #[test]
    fn abre_solo_texto() {
        assert!(openable(Path::new("a.TYP")));
        assert!(openable(Path::new("x.md")));
        assert!(!openable(Path::new("foto.png")));
        assert!(!openable(Path::new("sin-extension")));
    }

    #[test]
    fn renombrar_actualiza_docs() {
        let d = sandbox("rename");
        let mut docs = Docs::new();
        let old = d.join("a");
        let f = d.join("z.typ");
        docs.push(Some(f.clone()), "= Z\n".into());
        docs.push(Some(old.join("m.md")), "# M\n".into());
        // Archivo.
        let new_f = d.join("zz.typ");
        assert_eq!(docs_rename(&mut docs, &f, &new_f), 1);
        assert_eq!(docs.list[0].path.as_deref(), Some(new_f.as_path()));
        // Directorio: arrastra lo de dentro.
        let new_a = d.join("b2");
        assert_eq!(docs_rename(&mut docs, &old, &new_a), 1);
        assert_eq!(
            docs.list[1].path.as_deref(),
            Some(new_a.join("m.md").as_path())
        );
    }

    #[test]
    fn eliminar_desengancha_docs() {
        let d = sandbox("detach");
        let mut docs = Docs::new();
        docs.push(Some(d.join("a").join("m.md")), "# M\n".into());
        docs.push(Some(d.join("z.typ")), "= Z\n".into());
        assert_eq!(docs_detach(&mut docs, &d.join("a")), 1);
        assert!(docs.list[0].path.is_none());
        assert!(docs.list[1].path.is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn crear_y_borrar_en_disco() {
        let d = sandbox("io");
        let f = d.join("nuevo.typ");
        std::fs::write(&f, "").unwrap();
        assert!(f.is_file());
        std::fs::remove_file(&f).unwrap();
        assert!(!f.exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
