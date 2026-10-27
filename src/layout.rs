//! Pestañas del dock, paneles y diseños predefinidos.

use crate::doc::DocId;
use egui_dock::{DockState, Node, NodeIndex, NodePath, Split, SurfaceIndex};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Tab {
    Doc(DocId),
    Panel(Panel),
    /// Visor de imagen (no se guarda en la sesión).
    Image(crate::image_view::ImageId),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Panel {
    Preview,
    Outline,
    Problems,
    Symbols,
    Snippets,
    Templates,
    Files,
    Scratch,
}

impl Panel {
    pub const ALL: [Panel; 8] = [
        Panel::Outline,
        Panel::Symbols,
        Panel::Snippets,
        Panel::Templates,
        Panel::Files,
        Panel::Scratch,
        Panel::Preview,
        Panel::Problems,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Panel::Preview => "Vista previa",
            Panel::Outline => "Esquema",
            Panel::Problems => "Problemas",
            Panel::Symbols => "Símbolos",
            Panel::Snippets => "Snippets",
            Panel::Templates => "Plantillas",
            Panel::Files => "Archivos",
            Panel::Scratch => "Borrador",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Panel::Preview => "Ctrl+Mayús+V",
            Panel::Outline => "Ctrl+Mayús+O",
            Panel::Problems => "Ctrl+Mayús+M",
            Panel::Symbols => "Ctrl+Mayús+Y",
            Panel::Snippets => "Ctrl+Mayús+I",
            Panel::Templates => "Ctrl+Mayús+T",
            Panel::Files => "Ctrl+Mayús+F",
            Panel::Scratch => "Ctrl+Mayús+B",
        }
    }

    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as i;
        match self {
            Panel::Preview => i::FILE_PDF,
            Panel::Outline => i::LIST_BULLETS,
            Panel::Problems => i::WARNING_CIRCLE,
            Panel::Symbols => i::SIGMA,
            Panel::Snippets => i::BRACKETS_CURLY,
            Panel::Templates => i::ARTICLE,
            Panel::Files => i::FOLDER,
            Panel::Scratch => i::PENCIL_LINE,
        }
    }

    /// Paneles de la columna lateral: se apilan como pestañas en un mismo nodo.
    fn is_side(self) -> bool {
        matches!(
            self,
            Panel::Outline
                | Panel::Symbols
                | Panel::Snippets
                | Panel::Templates
                | Panel::Files
                | Panel::Scratch
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    /// Esquema | editor | vista previa.
    Classic,
    /// Editor | vista previa.
    SideBySide,
    /// Editor estrecho | vista previa ancha.
    PreviewFocus,
    /// Solo el editor.
    Writing,
}

impl Preset {
    pub const ALL: [Preset; 4] = [
        Preset::Classic,
        Preset::SideBySide,
        Preset::PreviewFocus,
        Preset::Writing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Classic => "Clásico (esquema · editor · vista previa)",
            Preset::SideBySide => "Editor y vista previa",
            Preset::PreviewFocus => "Vista previa amplia",
            Preset::Writing => "Solo escritura",
        }
    }

    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as i;
        match self {
            Preset::Classic => i::SQUARE_SPLIT_HORIZONTAL,
            Preset::SideBySide => i::COLUMNS,
            Preset::PreviewFocus => i::FILE_PDF,
            Preset::Writing => i::PENCIL_SIMPLE_LINE,
        }
    }

    pub fn build(self, docs: Vec<Tab>) -> DockState<Tab> {
        let mut d = DockState::new(docs);
        let s = d.main_surface_mut();
        match self {
            Preset::Classic => {
                let [ed, _] =
                    s.split_right(NodeIndex::root(), 0.55, vec![Tab::Panel(Panel::Preview)]);
                let side = vec![
                    Tab::Panel(Panel::Outline),
                    Tab::Panel(Panel::Symbols),
                    Tab::Panel(Panel::Snippets),
                ];
                s.split_left(ed, 0.46, side);
            }
            Preset::SideBySide => {
                s.split_right(NodeIndex::root(), 0.5, vec![Tab::Panel(Panel::Preview)]);
            }
            Preset::PreviewFocus => {
                s.split_right(NodeIndex::root(), 0.36, vec![Tab::Panel(Panel::Preview)]);
            }
            Preset::Writing => {}
        }
        d
    }
}

pub fn has_panel(dock: &DockState<Tab>, p: Panel) -> bool {
    dock.find_tab(&Tab::Panel(p)).is_some()
}

/// Nodo donde viven los documentos (el del doc `id`, o el primero que tenga
/// alguno). Cae en la raíz de la superficie principal.
fn docs_node(dock: &DockState<Tab>, id: DocId) -> NodePath {
    if let Some(p) = dock.find_tab(&Tab::Doc(id)) {
        return NodePath {
            surface: p.surface,
            node: p.node,
        };
    }
    for (path, tab) in dock.iter_all_tabs() {
        if matches!(tab, Tab::Doc(_)) {
            return NodePath {
                surface: path.surface,
                node: path.node,
            };
        }
    }
    NodePath {
        surface: SurfaceIndex::main(),
        node: NodeIndex::root(),
    }
}

/// Muestra u oculta un panel. Al mostrarlo lo coloca en su sitio natural
/// respecto a los documentos.
pub fn toggle_panel(dock: &mut DockState<Tab>, p: Panel, cur_doc: DocId) {
    let tab = Tab::Panel(p);
    if let Some(path) = dock.find_tab(&tab) {
        dock.remove_tab(path);
        return;
    }
    show_panel(dock, p, cur_doc);
}

pub fn show_panel(dock: &mut DockState<Tab>, p: Panel, cur_doc: DocId) {
    let tab = Tab::Panel(p);
    if let Some(path) = dock.find_tab(&tab) {
        let _ = dock.set_active_tab(path);
        return;
    }
    // Los laterales se apilan junto a otro lateral si ya hay uno abierto.
    if p.is_side() {
        let sibling = Panel::ALL
            .into_iter()
            .filter(|q| q.is_side())
            .find_map(|q| dock.find_tab(&Tab::Panel(q)));
        if let Some(path) = sibling
            && let Node::Leaf(leaf) = &mut dock[path.surface][path.node]
        {
            leaf.append_tab(tab);
            let _ = leaf.set_active_tab(leaf.len() - 1);
            return;
        }
    }
    let at = docs_node(dock, cur_doc);
    let empty = dock.iter_all_tabs().next().is_none();
    if empty {
        dock.main_surface_mut().push_to_first_leaf(tab);
        return;
    }
    // `fraction` es siempre la parte del hijo izquierdo/superior (para Left
    // es el panel nuevo, aunque la doc de egui_dock diga lo contrario).
    let (split, frac) = match p {
        Panel::Preview => (Split::Right, 0.55),
        Panel::Outline | Panel::Symbols | Panel::Snippets | Panel::Templates | Panel::Files | Panel::Scratch => {
            (Split::Left, 0.3)
        }
        Panel::Problems => (Split::Below, 0.74),
    };
    dock.split(at, split, frac, Node::leaf(tab));
}

/// Añade una pestaña de documento junto a los demás documentos.
pub fn add_doc(dock: &mut DockState<Tab>, id: DocId, near: DocId) {
    add_tab(dock, Tab::Doc(id), near);
}

/// Añade `tab` (documento o imagen) junto a los documentos.
pub fn add_tab(dock: &mut DockState<Tab>, tab: Tab, near: DocId) {
    let at = docs_node(dock, near);
    if dock.iter_all_tabs().next().is_none() {
        dock.main_surface_mut().push_to_first_leaf(tab);
        return;
    }
    // Si el nodo de docs ya no es una hoja (raro), cae a la raíz.
    match &mut dock[at.surface][at.node] {
        Node::Leaf(leaf) => {
            leaf.append_tab(tab);
            let _ = leaf.set_active_tab(leaf.len() - 1);
        }
        _ => dock.main_surface_mut().push_to_first_leaf(tab),
    }
}

/// Activa la pestaña del doc y enfoca su nodo (para que el dock lo trate
/// como el documento actual).
pub fn focus_doc(dock: &mut DockState<Tab>, id: DocId) {
    focus_tab(dock, Tab::Doc(id));
}

pub fn focus_tab(dock: &mut DockState<Tab>, tab: Tab) {
    if let Some(path) = dock.find_tab(&tab) {
        let _ = dock.set_active_tab(path);
        dock.set_focused_node_and_surface(NodePath {
            surface: path.surface,
            node: path.node,
        });
    }
}
