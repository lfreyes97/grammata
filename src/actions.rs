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
    OpenAt {
        path: PathBuf,
        line: usize,
        col: usize,
    },
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
    JumpTo {
        line: usize,
        col: usize,
    },
    /// Desplaza la vista previa a la página (0-based) y altura en puntos.
    /// Es el "forward" del synctex: outline y problemas la empujan al saltar.
    GotoPreviewPage {
        page: usize,
        y: f32,
    },
    /// Forward synctex desde el cursor: lleva la vista previa a la sección
    /// de la línea actual (mostrando el panel si está oculto).
    SyncToPdf,
    /// Selecciona el rango de chars `[start, end)` del documento actual.
    Select {
        start: usize,
        end: usize,
    },
    TogglePanel(Panel),
    ShowPanel(Panel),
    ApplyPreset(Preset),
    ToggleTheme,
    FontDelta(f32),
    FontReset,
    ToggleWrap,
    ToggleAuto,
    ToggleFormatBar,
    About,
    /// Inserta un símbolo o estructura matemática (envuelve en `$…$` si el
    /// cursor no está en una ecuación).
    InsertMath(String),
    /// Inserta un snippet (`${}` cursor, `${sel}` selección).
    InsertSnippet(String),
    /// Alterna un prefijo de línea (`- `, `= `…) en cada línea tocada por la
    /// selección (o la línea del cursor): lo pone si falta, lo quita si todas
    /// lo llevan. Los rivales del mismo grupo se sustituyen (`== ` → `= `).
    PrefixLines(String),
    /// Diálogo de snippet nuevo con la selección actual.
    NewSnippet,
    EditSnippet(usize),
    DeleteSnippet(usize),
}
