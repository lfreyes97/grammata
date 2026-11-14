//! Plugins: carpetas en `~/.config/grammata/plugins/<id>/` con un
//! `plugin.toml` que declara lo que aportan, sin código que se ejecute
//! dentro del editor:
//!
//! - un **paquete typst** servido desde la carpeta (no hay que instalarlo),
//! - **fuentes**, bibliotecas de **snippets** (`snippets.typ`) y **temas**,
//! - **plantillas** con formulario: los campos rellenan los `{{clave}}` del
//!   cuerpo y el resultado se abre como documento o se inserta. Es el
//!   «frontend» del paquete.
//!
//! Además puede declarar **programas externos** (`[[import]]`, `[[export]]`,
//! `[[command]]`): ver el protocolo en `plugins/run.rs`. Esos sí ejecutan
//! código, en un proceso aparte, y el usuario tiene que permitirlo.
//!
//! El formato está descrito en `docs/plugins.md`. La licencia del plugin es
//! cosa suya (ni los datos ni un programa aparte son obra derivada del
//! editor): el editor guarda la clave que escribe el usuario y se la pasa
//! a los programas con `protocol = "json"`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::PackageDir;
use crate::snippet_lib::Library;
use crate::theme::Palette;

#[path = "plugins/panel.rs"]
mod panel;
#[path = "plugins/run.rs"]
pub mod run;
pub use panel::panel;
use run::{Job, Kind, Tool};

pub const MANIFEST: &str = "plugin.toml";

// ---------- Manifiesto ----------

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Manifest {
    /// Identificador estable; por defecto, el nombre de la carpeta.
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    /// Texto libre o identificador SPDX (`MIT`, `Comercial`…).
    pub license: String,
    /// El plugin es de pago: el panel pide una clave de licencia.
    pub needs_key: bool,
    pub homepage: String,
    pub package: Option<PackageDecl>,
    /// Carpetas de fuentes (relativas al plugin).
    pub fonts: Vec<String>,
    /// Bibliotecas de snippets en typst (ver `snippet_lib`).
    pub snippets: Vec<String>,
    /// Temas del editor en TOML (ver `theme::parse_theme`).
    pub themes: Vec<String>,
    #[serde(rename = "template")]
    pub templates: Vec<Template>,
    /// Programas que convierten un archivo a typst.
    #[serde(rename = "import")]
    pub imports: Vec<Tool>,
    /// Programas que convierten el documento a otro formato.
    #[serde(rename = "export")]
    pub exports: Vec<Tool>,
    /// Programas que transforman el texto del editor.
    #[serde(rename = "command")]
    pub commands: Vec<Tool>,
}

impl Manifest {
    pub fn tools(&self, kind: Kind) -> &[Tool] {
        match kind {
            Kind::Import => &self.imports,
            Kind::Export => &self.exports,
            Kind::Command => &self.commands,
        }
    }

    /// Las líneas de comando que ejecuta: lo que el usuario autoriza. Si
    /// el manifiesto las cambia, hay que volver a permitirlas.
    pub fn programs(&self) -> Vec<String> {
        [Kind::Import, Kind::Export, Kind::Command]
            .into_iter()
            .flat_map(|k| self.tools(k))
            .map(Tool::command_line)
            .collect()
    }
}

/// Paquete typst que trae el plugin: la carpeta con su `typst.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PackageDecl {
    pub path: String,
    pub namespace: String,
}

impl Default for PackageDecl {
    fn default() -> Self {
        Self {
            path: ".".into(),
            namespace: "local".into(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub group: String,
    pub description: String,
    /// Código typst con marcadores `{{clave}}` (y `{{package}}`).
    pub body: String,
    #[serde(rename = "field")]
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Field {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: FieldKind,
    pub default: Scalar,
    pub hint: String,
    /// Varias líneas (`content`, `string`, `code`).
    pub multiline: bool,
    /// Vacío = se quitan las líneas del cuerpo que lo usan.
    pub optional: bool,
    /// Sufijo de `number` (`pt`, `%`, `em`…).
    pub unit: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Opciones de `choice`.
    pub options: Vec<Choice>,
}

/// Cómo se edita un campo y cómo se escribe en typst.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    /// Marcado: `[…]`.
    #[default]
    Content,
    /// Cadena: `"…"`.
    String,
    /// Código typst tal cual.
    Code,
    /// `rgb("#rrggbb")`.
    Color,
    Number,
    Bool,
    /// Una de `options` (su valor es código typst).
    Choice,
    /// Familia de fuente instalada: `"…"`.
    Font,
}

/// Valor por defecto: cadena, número o booleano de TOML.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Text(String),
    Bool(bool),
    Int(i64),
    Float(f64),
}

impl Default for Scalar {
    fn default() -> Self {
        Scalar::Text(String::new())
    }
}

impl Scalar {
    fn text(&self) -> String {
        match self {
            Scalar::Text(s) => s.clone(),
            Scalar::Bool(b) => b.to_string(),
            Scalar::Int(n) => n.to_string(),
            Scalar::Float(n) => n.to_string(),
        }
    }
}

/// Opción de un `choice`: `"valor"` o `{ value = "…", label = "…" }`.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Choice {
    Plain(String),
    Labeled { value: String, label: String },
}

impl Choice {
    pub fn value(&self) -> &str {
        match self {
            Choice::Plain(v) | Choice::Labeled { value: v, .. } => v,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Choice::Plain(v) | Choice::Labeled { label: v, .. } => v,
        }
    }
}

impl Field {
    /// Valor inicial del formulario.
    pub fn initial(&self) -> String {
        let d = self.default.text();
        match self.kind {
            FieldKind::Choice if d.is_empty() => self
                .options
                .first()
                .map(|o| o.value().to_string())
                .unwrap_or_default(),
            FieldKind::Bool if d.is_empty() => "false".into(),
            FieldKind::Color if d.is_empty() => "#000000".into(),
            FieldKind::Number if d.is_empty() => "0".into(),
            _ => d,
        }
    }

    /// El valor del formulario escrito como expresión typst.
    fn typst(&self, value: &str) -> String {
        match self.kind {
            FieldKind::Content => format!("[{value}]"),
            FieldKind::String | FieldKind::Font => {
                let esc = value
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n");
                format!("\"{esc}\"")
            }
            FieldKind::Color => format!("rgb(\"{value}\")"),
            FieldKind::Number => format!("{}{}", value.trim(), self.unit),
            FieldKind::Code | FieldKind::Bool | FieldKind::Choice => value.to_string(),
        }
    }
}

impl Template {
    pub fn initial(&self) -> HashMap<String, String> {
        self.fields
            .iter()
            .map(|f| (f.key.clone(), f.initial()))
            .collect()
    }

    /// Rellena el cuerpo: cada `{{clave}}` por su valor en typst y
    /// `{{package}}` por `@namespace/nombre:versión`. Las líneas que usan
    /// un campo opcional vacío se quitan.
    pub fn render(&self, values: &HashMap<String, String>, package: Option<&PackageDir>) -> String {
        let value = |f: &Field| values.get(&f.key).cloned().unwrap_or_else(|| f.initial());
        let mark = |key: &str| format!("{{{{{key}}}}}");
        let mut out = String::new();
        for line in self.body.trim_matches('\n').lines() {
            let dropped = self.fields.iter().any(|f| {
                f.optional && value(f).trim().is_empty() && line.contains(&mark(&f.key))
            });
            if dropped {
                continue;
            }
            let mut line = line.to_string();
            for f in &self.fields {
                let m = mark(&f.key);
                if line.contains(&m) {
                    line = line.replace(&m, &f.typst(&value(f)));
                }
            }
            if let Some(p) = package {
                line = line.replace("{{package}}", &p.spec());
            }
            out += &line;
            out.push('\n');
        }
        out
    }
}

// ---------- Plugin cargado ----------

pub struct Plugin {
    pub dir: PathBuf,
    pub manifest: Manifest,
    /// El manifiesto no se pudo leer: no aporta nada.
    pub broken: bool,
    pub enabled: bool,
    /// El usuario permitió sus programas (los de ahora).
    pub allowed: bool,
    /// Problemas al cargar (archivos que faltan, temas mal escritos…).
    pub errors: Vec<String>,
    pub package: Option<PackageDir>,
    pub font_dirs: Vec<PathBuf>,
    pub snippets: Vec<Library>,
    pub themes: Vec<(String, Palette)>,
}

/// Ruta relativa del manifiesto dentro de la carpeta del plugin (sin `..`
/// ni rutas absolutas).
fn inside(dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let ok = Path::new(rel)
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if ok && !rel.is_empty() {
        Ok(dir.join(rel))
    } else {
        Err(format!("ruta fuera del plugin: {rel:?}"))
    }
}

/// Nombre y versión del `typst.toml` de un paquete.
fn package_meta(dir: &Path) -> Result<(String, String), String> {
    let path = dir.join("typst.toml");
    let src = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let t: toml::Table = src.parse().map_err(|e: toml::de::Error| format!("typst.toml: {}", e.message()))?;
    let get = |k: &str| {
        t.get("package")
            .and_then(|p| p.get(k))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or(format!("typst.toml: falta `package.{k}`"))
    };
    Ok((get("name")?, get("version")?))
}

impl Plugin {
    fn load(dir: PathBuf, enabled: bool) -> Self {
        let folder = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut p = Plugin {
            dir,
            manifest: Manifest::default(),
            broken: false,
            enabled,
            allowed: false,
            errors: Vec::new(),
            package: None,
            font_dirs: Vec::new(),
            snippets: Vec::new(),
            themes: Vec::new(),
        };
        let parsed = std::fs::read_to_string(p.dir.join(MANIFEST))
            .map_err(|e| e.to_string())
            .and_then(|src| toml::from_str::<Manifest>(&src).map_err(|e| e.message().to_string()));
        match parsed {
            Ok(m) => p.manifest = m,
            Err(e) => {
                p.broken = true;
                p.errors.push(format!("{MANIFEST}: {e}"));
            }
        }
        if p.manifest.id.is_empty() {
            p.manifest.id = folder.clone();
        }
        if p.manifest.name.is_empty() {
            p.manifest.name = folder;
        }
        p.resolve();
        p
    }

    /// Comprueba y carga lo que declara el manifiesto.
    fn resolve(&mut self) {
        let m = self.manifest.clone();
        if let Some(decl) = &m.package {
            let found = inside(&self.dir, &decl.path).and_then(|dir| {
                package_meta(&dir).map(|(name, version)| PackageDir {
                    namespace: decl.namespace.clone(),
                    name,
                    version,
                    dir,
                })
            });
            match found {
                Ok(p) => self.package = Some(p),
                Err(e) => self.errors.push(e),
            }
        }
        for rel in &m.fonts {
            match inside(&self.dir, rel) {
                Ok(d) if d.is_dir() => self.font_dirs.push(d),
                Ok(d) => self.errors.push(format!("no existe la carpeta de fuentes {}", d.display())),
                Err(e) => self.errors.push(e),
            }
        }
        for rel in &m.snippets {
            match inside(&self.dir, rel) {
                Ok(f) if f.is_file() => self.snippets.push(Library::new(f)),
                Ok(f) => self.errors.push(format!("no existe {}", f.display())),
                Err(e) => self.errors.push(e),
            }
        }
        for rel in &m.themes {
            let theme = inside(&self.dir, rel).and_then(|f| {
                std::fs::read_to_string(&f)
                    .map_err(|e| e.to_string())
                    .and_then(|src| crate::theme::parse_theme(&src))
                    .map_err(|e| format!("{rel}: {e}"))
            });
            match theme {
                Ok(t) => self.themes.push(t),
                Err(e) => self.errors.push(e),
            }
        }
        for kind in [Kind::Import, Kind::Export, Kind::Command] {
            for t in m.tools(kind) {
                if t.id.is_empty() || t.run.is_empty() {
                    self.errors
                        .push(format!("herramienta {:?}: faltan `id` o `run`", t.name));
                }
                if kind != Kind::Command && t.extensions.is_empty() {
                    self.errors
                        .push(format!("herramienta {}: faltan las `extensions`", t.id));
                }
            }
        }
        for t in &m.templates {
            if t.id.is_empty() || t.body.trim().is_empty() {
                self.errors
                    .push(format!("plantilla {:?}: faltan `id` o `body`", t.name));
            }
            for f in &t.fields {
                if f.kind == FieldKind::Choice && f.options.is_empty() {
                    self.errors
                        .push(format!("plantilla {}: el campo {} no tiene `options`", t.id, f.key));
                }
            }
        }
    }

    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    /// Aporta su contenido: activado y con manifiesto válido.
    pub fn active(&self) -> bool {
        self.enabled && !self.broken
    }

    /// Declara programas externos.
    pub fn has_programs(&self) -> bool {
        !self.manifest.programs().is_empty()
    }

    /// Sus programas se pueden ejecutar.
    pub fn runnable(&self) -> bool {
        self.active() && self.allowed
    }
}

// ---------- Estado guardado ----------

/// Lo que el usuario decide sobre sus plugins (`plugins.json`).
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub disabled: BTreeSet<String>,
    /// Claves de licencia por plugin (texto plano, como el resto de la
    /// configuración).
    pub keys: BTreeMap<String, String>,
    /// Plugins con permiso para ejecutar programas, con las líneas de
    /// comando que se autorizaron.
    pub allowed: BTreeMap<String, Vec<String>>,
}

pub fn plugins_dir() -> PathBuf {
    crate::session::config_dir().join("plugins")
}

/// Escribe el archivo legible solo por el usuario (guarda claves).
fn write_private(path: &Path, text: &str) {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        opts.mode(0o600);
        // Si ya existía con otros permisos, también se corrigen.
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    if let Ok(mut f) = opts.open(path) {
        let _ = f.write_all(text.as_bytes());
    }
}

fn saved_path() -> PathBuf {
    crate::session::config_dir().join("plugins.json")
}

// ---------- Conjunto ----------

pub struct Plugins {
    pub dir: PathBuf,
    pub list: Vec<Plugin>,
    pub saved: Saved,
    /// Plantilla abierta en el panel: (plugin, plantilla).
    pub open: Option<(String, String)>,
    /// Valores de los formularios por (plugin, plantilla).
    pub forms: HashMap<(String, String), HashMap<String, String>>,
    pub font_filter: String,
    /// Los tests no escriben `plugins.json`.
    persist: bool,
}

impl Plugins {
    /// Los plugins del usuario, ya publicados en el motor.
    pub fn load() -> Self {
        let saved = std::fs::read_to_string(saved_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut p = Self::at(plugins_dir(), saved);
        p.persist = true;
        p
    }

    pub fn at(dir: PathBuf, saved: Saved) -> Self {
        let mut p = Self {
            dir,
            list: Vec::new(),
            saved,
            open: None,
            forms: HashMap::new(),
            font_filter: String::new(),
            persist: false,
        };
        p.reload();
        p
    }

    /// Relee la carpeta (una subcarpeta con `plugin.toml` = un plugin).
    pub fn reload(&mut self) {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|d| d.join(MANIFEST).is_file())
            .collect();
        dirs.sort();
        self.list.clear();
        for dir in dirs {
            let mut p = Plugin::load(dir, true);
            p.enabled = !self.saved.disabled.contains(p.id());
            p.allowed = self.saved.allowed.get(p.id()) == Some(&p.manifest.programs());
            if self.list.iter().any(|o| o.id() == p.id()) {
                p.broken = true;
                p.errors.push(format!("id repetido: {}", p.id()));
            }
            self.list.push(p);
        }
        self.publish();
    }

    /// Pasa al motor los paquetes y fuentes de los plugins activos.
    pub fn publish(&self) {
        let active = || self.list.iter().filter(|p| p.active());
        crate::engine::set_extras(
            active().filter_map(|p| p.package.clone()).collect(),
            active().flat_map(|p| p.font_dirs.clone()).collect(),
        );
    }

    pub fn set_enabled(&mut self, id: &str, on: bool) {
        if on {
            self.saved.disabled.remove(id);
        } else {
            self.saved.disabled.insert(id.to_string());
        }
        for p in self.list.iter_mut().filter(|p| p.id() == id) {
            p.enabled = on;
        }
        self.save();
        self.publish();
    }

    /// Permite (o retira el permiso a) los programas del plugin.
    pub fn set_allowed(&mut self, id: &str, on: bool) {
        for p in self.list.iter_mut().filter(|p| p.id() == id) {
            p.allowed = on;
            if on {
                self.saved.allowed.insert(id.to_string(), p.manifest.programs());
            }
        }
        if !on {
            self.saved.allowed.remove(id);
        }
        self.save();
    }

    /// Herramientas de ese tipo de los plugins que pueden ejecutar.
    pub fn tools(&self, kind: Kind) -> impl Iterator<Item = (&Plugin, &Tool)> {
        self.list
            .iter()
            .filter(|p| p.runnable())
            .flat_map(move |p| p.manifest.tools(kind).iter().map(move |t| (p, t)))
    }

    /// El importador de un archivo, por su extensión.
    pub fn importer(&self, path: &Path) -> Option<(&Plugin, &Tool)> {
        let ext = path.extension()?.to_string_lossy().to_lowercase();
        self.tools(Kind::Import)
            .find(|(_, t)| t.extensions.iter().any(|e| e.to_lowercase() == ext))
    }

    /// Prepara la ejecución de una herramienta.
    pub fn job(
        &self,
        plugin: &str,
        kind: Kind,
        id: &str,
        text: String,
        file: Option<PathBuf>,
        output: Option<PathBuf>,
    ) -> Option<Job> {
        let (p, tool) = self
            .tools(kind)
            .find(|(p, t)| p.id() == plugin && t.id == id)?;
        Some(Job {
            plugin_dir: p.dir.clone(),
            tool: tool.clone(),
            kind,
            text,
            file,
            output,
            key: self.saved.keys.get(plugin).cloned(),
        })
    }

    pub fn save(&self) {
        if !self.persist {
            return;
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.saved) {
            let _ = std::fs::create_dir_all(crate::session::config_dir());
            write_private(&saved_path(), &json);
        }
    }

    /// Tema `plugin/nombre` de un plugin activo.
    pub fn theme(&self, key: &str) -> Option<Palette> {
        let (id, name) = key.split_once('/')?;
        self.list
            .iter()
            .filter(|p| p.active() && p.id() == id)
            .flat_map(|p| &p.themes)
            .find(|(n, _)| n == name)
            .map(|(_, pal)| *pal)
    }

    /// Recarga las bibliotecas de snippets que cambiaron en disco.
    pub fn refresh_snippets(&mut self) {
        for lib in self.list.iter_mut().flat_map(|p| &mut p.snippets) {
            lib.refresh();
        }
    }

    /// Escribe uno de los plugins de serie (si no existe) y devuelve su
    /// manifiesto. Sus programas quedan permitidos: lo pidió el usuario.
    pub fn install(&mut self, id: &str) -> std::io::Result<PathBuf> {
        let files = BUNDLED
            .iter()
            .find(|b| b.id == id)
            .map(|b| b.files)
            .ok_or(std::io::ErrorKind::NotFound)?;
        let dir = self.dir.join(id);
        let manifest = dir.join(MANIFEST);
        if manifest.exists() {
            // No se autoriza lo que no es el plugin de serie.
            let bundled = files.iter().find(|(rel, _)| *rel == MANIFEST).map(|(_, t)| *t);
            if std::fs::read_to_string(&manifest).ok().as_deref() != bundled {
                return Err(std::io::Error::other(format!(
                    "ya hay otro plugin en {}: bórralo para instalar el de serie",
                    dir.display()
                )));
            }
        } else {
            for (rel, text) in files {
                let path = dir.join(rel);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, text)?;
            }
        }
        self.reload();
        self.set_allowed(id, true);
        Ok(dir.join(MANIFEST))
    }
}

/// Plugin que viene con el editor y se instala desde el panel.
pub struct Bundled {
    pub id: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    files: &'static [(&'static str, &'static str)],
}

pub const BUNDLED: &[Bundled] = &[
    Bundled {
        id: "pandoc",
        name: "Pandoc: importar y exportar Markdown",
        about: "Ejecuta el programa pandoc, que tiene que estar instalado",
        files: &[(MANIFEST, include_str!("plugins/pandoc/plugin.toml"))],
    },
    Bundled {
        id: "ejemplo",
        name: "Ejemplo: paquete con formulario, snippets y tema",
        about: "Para copiarlo y hacer el tuyo",
        files: &[
            (MANIFEST, include_str!("plugins/ejemplo/plugin.toml")),
            ("typst.toml", include_str!("plugins/ejemplo/typst.toml")),
            ("lib.typ", include_str!("plugins/ejemplo/lib.typ")),
            ("snippets.typ", include_str!("plugins/ejemplo/snippets.typ")),
            ("temas/sepia.toml", include_str!("plugins/ejemplo/temas/sepia.toml")),
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("grammata-plug-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// `Plugins` publica en el motor (estado global): los tests que crean
    /// uno van de uno en uno.
    static ENGINE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn engine() -> std::sync::MutexGuard<'static, ()> {
        ENGINE.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn example(name: &str) -> (PathBuf, Plugins) {
        let d = tmp(name);
        let mut p = Plugins::at(d.clone(), Saved::default());
        p.install("ejemplo").unwrap();
        (d, p)
    }

    #[test]
    fn el_ejemplo_carga_sin_errores() {
        let _engine = engine();
        let (d, p) = example("carga");
        assert_eq!(p.list.len(), 1);
        let e = &p.list[0];
        assert_eq!(e.errors, Vec::<String>::new());
        assert_eq!(e.id(), "ejemplo");
        assert_eq!(e.package.as_ref().unwrap().spec(), "@local/tarjetas:0.1.0");
        assert!(!e.snippets[0].items.is_empty());
        assert!(p.theme("ejemplo/Sepia").is_some_and(|t| !t.dark));
        assert!(p.theme("ejemplo/Otro").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn rellena_marcadores_y_quita_opcionales() {
        let t: Template = toml::from_str(
            r##"
id = "t"
body = """
#import "{{package}}": caja
#caja(
  {{texto}},
  autor: {{autor}},
  fondo: {{fondo}}, ancho: {{ancho}}, forma: {{forma}},
  marco: {{marco}},
)
"""
[[field]]
key = "texto"
default = "Hola"
[[field]]
key = "autor"
type = "string"
optional = true
[[field]]
key = "fondo"
type = "color"
default = "#112233"
[[field]]
key = "ancho"
type = "number"
default = 80
unit = "%"
[[field]]
key = "forma"
type = "choice"
options = ["rect", { value = "circle", label = "Círculo" }]
[[field]]
key = "marco"
type = "bool"
default = true
"##,
        )
        .unwrap();
        let pkg = PackageDir {
            namespace: "local".into(),
            name: "x".into(),
            version: "1.0.0".into(),
            dir: PathBuf::new(),
        };
        let mut v = t.initial();
        assert_eq!(
            t.render(&v, Some(&pkg)),
            "#import \"@local/x:1.0.0\": caja\n#caja(\n  [Hola],\n  \
             fondo: rgb(\"#112233\"), ancho: 80%, forma: rect,\n  marco: true,\n)\n"
        );
        v.insert("autor".into(), "Ana \"la\" Ñ".into());
        v.insert("forma".into(), "circle".into());
        let out = t.render(&v, Some(&pkg));
        assert!(out.contains("  autor: \"Ana \\\"la\\\" Ñ\",\n"), "{out}");
        assert!(out.contains("forma: circle"));
    }

    #[test]
    fn rutas_fuera_del_plugin_y_manifiesto_roto() {
        let _engine = engine();
        let d = tmp("rotos");
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::write(d.join("a").join(MANIFEST), "snippets = [\"../x.typ\", \"/etc/passwd\"]").unwrap();
        std::fs::create_dir_all(d.join("b")).unwrap();
        std::fs::write(d.join("b").join(MANIFEST), "nombre = 1").unwrap();
        std::fs::create_dir_all(d.join("sin-manifiesto")).unwrap();
        let p = Plugins::at(d.clone(), Saved::default());
        assert_eq!(p.list.len(), 2);
        assert_eq!(p.list[0].errors.len(), 2);
        assert!(p.list[0].snippets.is_empty() && p.list[0].active());
        assert!(p.list[1].broken && !p.list[1].active());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn desactivar_se_recuerda() {
        let _engine = engine();
        let (d, mut p) = example("desactivar");
        p.set_enabled("ejemplo", false);
        assert!(!p.list[0].active() && p.theme("ejemplo/Sepia").is_none());
        p.reload();
        assert!(!p.list[0].enabled);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// La plantilla del ejemplo compila contra el paquete del plugin, que
    /// no está instalado en el sistema.
    #[test]
    fn la_plantilla_del_ejemplo_compila() {
        let _engine = engine();
        let (d, p) = example("compila");
        let e = &p.list[0];
        let root = tmp("compila-doc");
        for t in &e.manifest.templates {
            let doc = t.render(&t.initial(), e.package.as_ref());
            let out = crate::engine::compile_pdf(&doc, &root, "main.typ");
            assert!(out.pdf.is_some(), "{doc}\n{:?}", out.diags);
        }
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn los_programas_piden_permiso() {
        let _engine = engine();
        let d = tmp("permiso");
        std::fs::create_dir_all(d.join("x")).unwrap();
        let manifest = |cmd: &str| {
            format!("[[import]]\nid = \"m\"\nextensions = [\"MD\"]\nrun = [\"{cmd}\"]\n")
        };
        std::fs::write(d.join("x").join(MANIFEST), manifest("cat")).unwrap();
        let mut p = Plugins::at(d.clone(), Saved::default());
        let md = Path::new("/a/nota.md");
        assert!(p.list[0].errors.is_empty() && p.list[0].has_programs());
        assert!(p.importer(md).is_none(), "sin permiso no se ofrece");
        p.set_allowed("x", true);
        assert!(p.importer(md).is_some() && p.importer(Path::new("/a/nota.txt")).is_none());
        let job = p.job("x", Kind::Import, "m", "hola".into(), None, None).unwrap();
        #[cfg(unix)]
        assert_eq!(job.run().unwrap().text, "hola");
        // Si el manifiesto cambia el programa, el permiso ya no vale.
        std::fs::write(d.join("x").join(MANIFEST), manifest("rm")).unwrap();
        p.reload();
        assert!(!p.list[0].allowed && p.importer(md).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Markdown → typst con el plugin de pandoc: lo importado compila.
    #[test]
    fn pandoc_importa_y_exporta_markdown() {
        if std::process::Command::new("pandoc").arg("--version").output().is_err() {
            eprintln!("sin pandoc: prueba omitida");
            return;
        }
        let _engine = engine();
        let d = tmp("pandoc");
        let mut p = Plugins::at(d.clone(), Saved::default());
        p.install("pandoc").unwrap();
        assert_eq!(p.list[0].errors, Vec::<String>::new());
        let md = "# Hola\n\nUn *texto* con **negrita** y [enlace](https://typst.app).\n\n\
                  - uno\n- dos\n\n> cita\n\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n$$x^2$$\n";
        let (plugin, tool) = p.importer(Path::new("nota.md")).unwrap();
        let (plugin, tool) = (plugin.id().to_string(), tool.id.clone());
        let typ = p
            .job(&plugin, Kind::Import, &tool, md.into(), None, None)
            .unwrap()
            .run()
            .unwrap()
            .text;
        assert!(typ.starts_with("= Hola\n") && typ.contains("#strong[negrita]"), "{typ}");
        let out = crate::engine::compile_pdf(&typ, &d, "main.typ");
        assert!(out.pdf.is_some(), "{typ}\n{:?}", out.diags);
        let back = p
            .job(&plugin, Kind::Export, "markdown", "= Hola\n\nUn *texto* y $x^2$.\n".into(), None, None)
            .unwrap()
            .run()
            .unwrap()
            .text;
        assert_eq!(back, "# Hola\n\nUn **texto** y $x^{2}$.\n");
        let _ = std::fs::remove_dir_all(&d);
    }
}
