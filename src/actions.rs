//! Acciones de la app. Menús, barras, atajos y paneles solo *emiten*
//! acciones; `App::run_actions` las ejecuta en un único sitio con acceso a
//! todo el estado (incluido el dock).

use crate::doc::DocId;
use crate::layout::{Panel, Preset};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Action {
    NewDoc,
    Open,
    OpenPath(PathBuf),
    /// Abre (o enfoca) un archivo y salta a L:C — errores en `#include`.
    OpenAt { path: PathBuf, line: usize, col: usize },
    Save,
    SaveAs,
    /// Cierra pidiendo confirmación si hay cambios sin guardar.
    Close(DocId),
    CloseCurrent,
    /// Cierra sin preguntar.
    Discard(DocId),
    SaveAndClose(DocId),
    NextDoc,
    PrevDoc,
    SwitchTo(usize),
    Compile,
    ExportPdf,
    ExportPng(u32),
    ExportSvg,
    Find,
    GotoLinePrompt,
    /// Salta a L:C (desde 1) del documento actual.
    JumpTo { line: usize, col: usize },
    /// Selecciona el rango de chars `[start, end)` del documento actual.
    Select { start: usize, end: usize },
    TogglePanel(Panel),
    ShowPanel(Panel),
    ApplyPreset(Preset),
    ToggleTheme,
    FontDelta(f32),
    FontReset,
    ToggleWrap,
    ToggleAuto,
    About,
    /// Inserta un símbolo o estructura matemática (envuelve en `$…$` si el
    /// cursor no está en una ecuación).
    InsertMath(String),
    /// Inserta un snippet (`${}` cursor, `${sel}` selección).
    InsertSnippet(String),
    /// Diálogo de snippet nuevo con la selección actual.
    NewSnippet,
    EditSnippet(usize),
    DeleteSnippet(usize),
}
