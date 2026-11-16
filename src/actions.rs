//! Acciones de la app. Menús, barras, atajos y paneles solo *emiten*
//! acciones; `App::run_actions` las ejecuta en un único sitio con acceso a
//! todo el estado (incluido el dock).

use crate::doc::DocId;
use crate::layout::{Panel, Preset};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Action {
    NewDoc,
    /// Documento nuevo sin guardar con ese texto (plantilla de un plugin).
    NewDocWith(String),
    /// Vuelve a leer la carpeta de plugins.
    PluginsReload,
    /// Activa o desactiva el plugin con ese id.
    PluginEnable(String, bool),
    /// Escribe un plugin de serie (`plugins::BUNDLED`) y abre su `plugin.toml`.
    PluginInstall(&'static str),
    /// Permite o retira el permiso de ejecutar los programas del plugin.
    PluginAllow(String, bool),
    /// Elige un archivo que algún plugin sepa importar.
    Import,
    /// Convierte ese archivo a typst con el importador de su extensión y
    /// lo abre como documento nuevo.
    ImportPath(PathBuf),
    /// Ejecuta un exportador o un comando de un plugin sobre el documento.
    PluginRun {
        plugin: String,
        kind: crate::plugins::run::Kind,
        id: String,
    },
    /// Tema `plugin/nombre` de un plugin (`None` = los de serie).
    SetTheme(Option<String>),
    /// Snippet de una biblioteca `snippets.typ`: `preamble` (el `#import`
    /// o la definición) va al principio del documento si hace falta.
    InsertLibSnippet {
        body: String,
        preamble: Option<String>,
    },
    /// Crea (si no existe) una biblioteca `snippets.typ` con ejemplos y la abre.
    CreateSnippetLib(PathBuf),
    /// Reemplaza en `doc` el texto `expect` que empezaba en `start` (chars;
    /// si se movió, la aparición más cercana). El borrador lo usa al editar
    /// una ecuación del documento.
    ReplaceText {
        doc: DocId,
        start: usize,
        expect: String,
        text: String,
    },
    /// Abre el diálogo «Nuevo proyecto».
    NewProject,
    /// Proyecto recién creado en `dir`: lo fija como raíz y abre `main.typ`
    /// o el builder según el tipo.
    OpenProject {
        dir: PathBuf,
        kind: crate::project::Kind,
    },
    /// Builder de plantillas: elige carpeta, escribe `template.typ` y
    /// `main.typ`, y abre este último.
    NewFromTemplate(Box<crate::templates::Options>),
    /// Elige la carpeta raíz del gestor de archivos y lo muestra.
    PickFolder,
    Open,
    OpenPath(PathBuf),
    /// Inserta `#image("…")` con la ruta de esa imagen en el documento actual.
    InsertImage(PathBuf),
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
    /// Abre el diálogo de exportación de `fmt` (`pdf`, `pdfa`, `png`, `svg`).
    Export(String),
    /// Exporta con lo elegido en el diálogo (`pages` vacío = todas).
    ExportRun {
        fmt: String,
        ppi: Option<u32>,
        pages: Vec<usize>,
    },
    Find,
    GotoLinePrompt,
    /// Salta a L:C (desde 1) del documento actual.
    JumpTo {
        line: usize,
        col: usize,
    },
    /// Desplaza la vista previa a la página (0-based) y altura en puntos.
    /// Es el "forward" del synctex: problemas la empuja al saltar
    /// (el esquema solo mueve el cursor del búfer).
    GotoPreviewPage {
        page: usize,
        y: f32,
    },
    /// Forward synctex desde el cursor: lleva la vista previa a la línea
    /// actual (mostrando el panel si está oculto).
    SyncToPdf,
    /// Fija la vista previa en el documento actual (o la desfija).
    TogglePin,
    ToggleAutoSave,
    /// Selecciona el rango de chars `[start, end)` del documento actual.
    Select {
        start: usize,
        end: usize,
    },
    TogglePanel(Panel),
    ShowPanel(Panel),
    ApplyPreset(Preset),
    ToggleTheme,
    /// Alterna entre español e inglés y lo guarda en las preferencias.
    ToggleLang,
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
    /// Inserta una regla `#set …` en su propia línea en el cursor.
    InsertSetRule(String),
    /// Alterna negrita, cursiva… sobre la selección o la palabra del cursor.
    ToggleMark(crate::formatbar::Mark),
    /// Diálogo de snippet nuevo con la selección actual.
    NewSnippet,
    EditSnippet(usize),
    DeleteSnippet(usize),
}
