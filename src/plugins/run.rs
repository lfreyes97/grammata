//! Protocolo de los plugins con programa externo (versión 1).
//!
//! Un plugin declara herramientas (`[[import]]`, `[[export]]`,
//! `[[command]]`) con `run = ["programa", "arg"…]`. Cada uso lanza el
//! programa una vez, le pasa la entrada por stdin y lee el resultado de
//! stdout. Si sale con código distinto de 0, stderr es el mensaje de error.
//!
//! - `protocol = "text"` (por defecto): stdin y stdout son el texto tal
//!   cual. Sirve para programas que ya existen (pandoc, un formateador…).
//! - `protocol = "json"`: stdin es un objeto `Request` y stdout un objeto
//!   `Response`. Es el que recibe la clave de licencia.
//!
//! En los argumentos, `{input}` es la ruta del archivo que se importa y
//! `{output}` la del que se exporta (para formatos binarios: con
//! `{output}` el programa escribe el archivo y stdout se ignora). El
//! programa recibe además `GRAMMATA_PLUGIN_DIR`, `GRAMMATA_KIND`,
//! `GRAMMATA_ID` y, si hay, `GRAMMATA_FILE`.
//!
//! El programa es de su autor: se comunica por tuberías, no se enlaza con
//! el editor, y solo se ejecuta si el usuario lo permitió en el panel.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Versión del protocolo JSON (`Request::grammata`).
pub const VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Import,
    Export,
    Command,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Import => "import",
            Kind::Export => "export",
            Kind::Command => "command",
        }
    }
}

/// De dónde sale el texto de un `[[command]]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Input {
    /// La selección; sin selección, el documento entero.
    #[default]
    Selection,
    Document,
}

/// Qué se hace con el resultado de un `[[command]]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Output {
    /// Sustituye el texto de entrada.
    #[default]
    Replace,
    /// Lo abre como documento nuevo.
    New,
}

/// Herramienta de un plugin: un importador, un exportador o un comando.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Extensiones sin punto: las que acepta (`import`) o la que escribe
    /// (`export`, la primera).
    pub extensions: Vec<String>,
    /// Programa y argumentos. `./x` es relativo a la carpeta del plugin;
    /// lo demás se busca en el PATH.
    pub run: Vec<String>,
    pub protocol: Protocol,
    /// Segundos antes de cancelarlo.
    pub timeout: Option<u64>,
    pub input: Input,
    pub output: Output,
}

impl Tool {
    /// Usa `{output}`: escribe él el archivo exportado.
    pub fn writes_file(&self) -> bool {
        self.run.iter().any(|a| a.contains("{output}"))
    }

    /// Usa `{input}`: lee él el archivo importado (puede ser binario).
    pub fn reads_file(&self) -> bool {
        self.run.iter().any(|a| a.contains("{input}"))
    }

    pub fn command_line(&self) -> String {
        self.run.join(" ")
    }
}

/// Lo que recibe por stdin un programa con `protocol = "json"`.
#[derive(Serialize)]
struct Request<'a> {
    /// Versión del protocolo.
    grammata: u32,
    /// Versión del editor.
    editor: &'static str,
    kind: Kind,
    /// `id` de la herramienta en el manifiesto.
    id: &'a str,
    text: &'a str,
    /// Archivo de origen (importar) o del documento (exportar, comando).
    file: Option<&'a Path>,
    /// Destino de una exportación.
    output: Option<&'a Path>,
    plugin_dir: &'a Path,
    /// Clave que el usuario escribió en el panel, para que el plugin la
    /// compruebe.
    license_key: Option<&'a str>,
}

/// Lo que devuelve por stdout: `text`, o `error` si no pudo.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Response {
    text: String,
    /// Aviso para la barra de estado.
    message: Option<String>,
    error: Option<String>,
}

/// Una ejecución, con todo lo que necesita el hilo que la lanza.
pub struct Job {
    pub plugin_dir: PathBuf,
    pub tool: Tool,
    pub kind: Kind,
    pub text: String,
    pub file: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub key: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct Done {
    pub text: String,
    pub message: Option<String>,
}

const DEFAULT_TIMEOUT: u64 = 60;

impl Job {
    fn program(&self) -> Result<PathBuf, String> {
        let name = self.tool.run.first().filter(|n| !n.is_empty()).ok_or("falta `run`")?;
        match name.strip_prefix("./") {
            Some(rel) => super::inside(&self.plugin_dir, rel),
            None => Ok(PathBuf::from(name)),
        }
    }

    fn args(&self) -> Vec<String> {
        let path = |p: &Option<PathBuf>| p.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        self.tool.run[1..]
            .iter()
            .map(|a| {
                a.replace("{input}", &path(&self.file))
                    .replace("{output}", &path(&self.output))
            })
            .collect()
    }

    /// Lanza el programa y espera el resultado (bloquea: va en un hilo).
    pub fn run(&self) -> Result<Done, String> {
        let program = self.program()?;
        let stdin = match self.tool.protocol {
            Protocol::Text => self.text.clone(),
            Protocol::Json => serde_json::to_string(&Request {
                grammata: VERSION,
                editor: env!("CARGO_PKG_VERSION"),
                kind: self.kind,
                id: &self.tool.id,
                text: &self.text,
                file: self.file.as_deref(),
                output: self.output.as_deref(),
                plugin_dir: &self.plugin_dir,
                license_key: self.key.as_deref(),
            })
            .map_err(|e| e.to_string())?,
        };
        // Junto al archivo, para que el programa resuelva rutas relativas.
        let cwd = [&self.file, &self.output]
            .into_iter()
            .flatten()
            .filter_map(|p| p.parent())
            .find(|d| d.is_dir())
            .unwrap_or(&self.plugin_dir);
        let mut cmd = Command::new(&program);
        cmd.args(self.args())
            .current_dir(cwd)
            .env("GRAMMATA_PLUGIN_DIR", &self.plugin_dir)
            .env("GRAMMATA_KIND", self.kind.name())
            .env("GRAMMATA_ID", &self.tool.id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(f) = self.file.as_ref().or(self.output.as_ref()) {
            cmd.env("GRAMMATA_FILE", f);
        }
        let shown = program.display().to_string();
        let mut child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => format!("no se encontró el programa «{shown}»: ¿está instalado?"),
            _ => format!("no se pudo ejecutar «{shown}»: {e}"),
        })?;

        // Las tres tuberías a la vez: si no, un programa que escribe mucho
        // antes de leer se queda bloqueado.
        let mut input = child.stdin.take().expect("stdin");
        let feed = std::thread::spawn(move || {
            let _ = input.write_all(stdin.as_bytes());
        });
        fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = pipe.read_to_end(&mut buf);
                buf
            })
        }
        let out = drain(child.stdout.take().expect("stdout"));
        let err = drain(child.stderr.take().expect("stderr"));

        let limit = Duration::from_secs(self.tool.timeout.unwrap_or(DEFAULT_TIMEOUT));
        let start = Instant::now();
        let status = loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(s) => break s,
                None if start.elapsed() > limit => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("«{shown}» no terminó en {} s", limit.as_secs()));
                }
                None => std::thread::sleep(Duration::from_millis(15)),
            }
        };
        let _ = feed.join();
        let stdout = String::from_utf8_lossy(&out.join().unwrap_or_default()).into_owned();
        let stderr = String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned();
        if !status.success() {
            let msg = stderr.trim();
            return Err(if msg.is_empty() {
                format!("«{shown}» falló ({status})")
            } else {
                msg.to_string()
            });
        }
        match self.tool.protocol {
            Protocol::Text => Ok(Done {
                text: stdout,
                message: None,
            }),
            Protocol::Json => {
                let r: Response = serde_json::from_str(&stdout)
                    .map_err(|e| format!("respuesta JSON no válida de «{shown}»: {e}"))?;
                match r.error {
                    Some(e) => Err(e),
                    None => Ok(Done {
                        text: r.text,
                        message: r.message,
                    }),
                }
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn job(run: &[&str], protocol: Protocol, text: &str) -> Job {
        Job {
            plugin_dir: std::env::temp_dir(),
            tool: Tool {
                id: "t".into(),
                run: run.iter().map(|s| s.to_string()).collect(),
                protocol,
                ..Default::default()
            },
            kind: Kind::Command,
            text: text.into(),
            file: None,
            output: None,
            key: None,
        }
    }

    #[test]
    fn texto_por_stdin_y_stdout() {
        let j = job(&["tr", "a-z", "A-Z"], Protocol::Text, "hola ñ\n");
        assert_eq!(j.run().unwrap().text, "HOLA ñ\n");
        // Entradas grandes no bloquean las tuberías.
        let big = "x".repeat(2_000_000);
        assert_eq!(job(&["cat"], Protocol::Text, &big).run().unwrap().text.len(), big.len());
    }

    #[test]
    fn errores_y_tiempo_agotado() {
        let j = job(&["sh", "-c", "echo mal >&2; exit 3"], Protocol::Text, "");
        assert_eq!(j.run().unwrap_err(), "mal");
        let e = job(&["grammata-no-existe"], Protocol::Text, "").run().unwrap_err();
        assert!(e.contains("no se encontró"), "{e}");
        let mut j = job(&["sleep", "5"], Protocol::Text, "");
        j.tool.timeout = Some(0);
        assert!(j.run().unwrap_err().contains("no terminó"));
        // Un programa `./` no puede salir de la carpeta del plugin.
        assert!(job(&["./../x"], Protocol::Text, "").run().is_err());
    }

    #[test]
    fn json_lleva_la_peticion_y_la_clave() {
        let dir = std::env::temp_dir().join(format!("grammata-run-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = r#"cat > "$GRAMMATA_PLUGIN_DIR/req.json"; echo '{"text":"ok","message":"hecho"}'"#;
        let mut j = job(&["sh", "-c", script], Protocol::Json, "hola");
        j.plugin_dir = dir.clone();
        j.kind = Kind::Import;
        j.file = Some(dir.join("a.md"));
        j.key = Some("ABC".into());
        assert_eq!(
            j.run().unwrap(),
            Done {
                text: "ok".into(),
                message: Some("hecho".into())
            }
        );
        let req: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("req.json")).unwrap()).unwrap();
        assert_eq!(req["grammata"], 1);
        assert_eq!(req["kind"], "import");
        assert_eq!(req["id"], "t");
        assert_eq!(req["text"], "hola");
        assert_eq!(req["license_key"], "ABC");
        assert!(req["file"].as_str().unwrap().ends_with("a.md"));

        let j = job(&["sh", "-c", r#"cat >/dev/null; echo '{"error":"sin licencia"}'"#], Protocol::Json, "");
        assert_eq!(j.run().unwrap_err(), "sin licencia");
        let j = job(&["sh", "-c", "cat >/dev/null; echo nada"], Protocol::Json, "");
        assert!(j.run().unwrap_err().contains("JSON"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rutas_en_los_argumentos() {
        let dir = std::env::temp_dir().join(format!("grammata-run-args-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut j = job(&["sh", "-c", "cat > \"$0\"", "{output}"], Protocol::Text, "contenido");
        j.kind = Kind::Export;
        j.output = Some(dir.join("salida.md"));
        assert!(j.tool.writes_file() && !j.tool.reads_file());
        j.run().unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("salida.md")).unwrap(), "contenido");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
