//! Sesión en `~/.config/typst-editor/session.json`: pestañas, preferencias y
//! diseño del dock. Todos los campos tienen valor por defecto, así que las
//! sesiones de versiones anteriores siguen cargando.

use crate::layout::{Panel, Tab};
use egui_dock::DockState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Límite de texto guardado por pestaña sin archivo.
const MAX_UNSAVED_BYTES: usize = 200_000;
pub const MAX_TABS: usize = 16;

#[derive(Serialize, Deserialize, Default)]
pub struct SessionTab {
    pub path: Option<PathBuf>,
    pub text: String,
}

/// Pestaña del dock guardada: los docs se refieren por posición en `tabs`.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum SavedTab {
    Doc(usize),
    Panel(Panel),
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum FitMode {
    #[default]
    Width,
    Page,
    Manual,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub dark: bool,
    pub font_size: f32,
    pub wrap: bool,
    pub auto_compile: bool,
    pub zoom: f32,
    pub fit: FitMode,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: true,
            font_size: 14.0,
            wrap: false,
            auto_compile: true,
            zoom: 1.0,
            fit: FitMode::Width,
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Session {
    pub tabs: Vec<SessionTab>,
    pub current: usize,
    pub prefs: Prefs,
    pub layout: Option<DockState<SavedTab>>,
    /// Últimos símbolos insertados (lo que se inserta, más reciente primero).
    pub recent_symbols: Vec<String>,
    // Campos de la versión anterior (se leen una vez y se migran a `prefs`).
    #[serde(skip_serializing)]
    dark: Option<bool>,
    #[serde(skip_serializing)]
    font_size: Option<f32>,
    #[serde(skip_serializing)]
    auto: Option<bool>,
}

impl Session {
    pub fn new(
        tabs: Vec<SessionTab>,
        current: usize,
        prefs: Prefs,
        layout: DockState<SavedTab>,
        recent_symbols: Vec<String>,
    ) -> Self {
        Self {
            tabs,
            current,
            prefs,
            layout: Some(layout),
            recent_symbols,
            ..Default::default()
        }
    }
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
            PathBuf::from(home).join(".config")
        });
    base.join("typst-editor")
}

pub fn path() -> PathBuf {
    config_dir().join("session.json")
}

pub fn load() -> Option<Session> {
    let data = std::fs::read_to_string(path()).ok()?;
    let mut s: Session = serde_json::from_str(&data).ok()?;
    if let Some(d) = s.dark.take() {
        s.prefs.dark = d;
    }
    if let Some(f) = s.font_size.take() {
        s.prefs.font_size = f;
    }
    if let Some(a) = s.auto.take() {
        s.prefs.auto_compile = a;
    }
    s.prefs.font_size = s.prefs.font_size.clamp(9.0, 28.0);
    s.prefs.zoom = s.prefs.zoom.clamp(0.25, 4.0);
    Some(s)
}

pub fn save(s: &Session) -> std::io::Result<()> {
    let p = path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(s).map_err(std::io::Error::other)?;
    // Escritura atómica: un cierre a medias no deja la sesión corrupta.
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, p)
}

pub fn tab_text(text: &str) -> String {
    if text.len() <= MAX_UNSAVED_BYTES {
        return text.to_string();
    }
    let mut end = MAX_UNSAVED_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Traduce el dock en vivo a uno guardable (docs → posición).
pub fn save_layout(dock: &DockState<Tab>, order: &[u64]) -> DockState<SavedTab> {
    dock.filter_map_tabs(|t| match *t {
        Tab::Doc(id) => order.iter().position(|&o| o == id).map(SavedTab::Doc),
        Tab::Panel(p) => Some(SavedTab::Panel(p)),
    })
}

/// Inversa de `save_layout`; `ids[i]` es el id del doc restaurado en la
/// posición `i` (o `None` si no se pudo restaurar).
pub fn restore_layout(saved: &DockState<SavedTab>, ids: &[Option<u64>]) -> DockState<Tab> {
    saved.filter_map_tabs(|t| match *t {
        SavedTab::Doc(i) => ids.get(i).copied().flatten().map(Tab::Doc),
        SavedTab::Panel(p) => Some(Tab::Panel(p)),
    })
}
