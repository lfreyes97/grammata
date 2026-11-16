//! Traducción de la interfaz. El español es el idioma de origen: el texto en
//! español es la clave y `en.rs` guarda su traducción al inglés. Sin entrada
//! (o con el idioma en español) se muestra el original.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

mod en;

static ENGLISH: AtomicBool = AtomicBool::new(false);
static TABLE: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| en::ENTRIES.iter().copied().collect());

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        match code.get(..2)?.to_ascii_lowercase().as_str() {
            "es" => Some(Lang::Es),
            "en" => Some(Lang::En),
            _ => None,
        }
    }
}

/// Idioma del sistema (`LC_ALL`, `LC_MESSAGES`, `LANG`); inglés si no es español.
pub fn detect() -> Lang {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .and_then(|v| Lang::from_code(&v))
        .unwrap_or(Lang::En)
}

/// Aplica la preferencia guardada (`None` = el idioma del sistema).
pub fn init(pref: Option<&str>) {
    set(pref.and_then(Lang::from_code).unwrap_or_else(detect));
}

pub fn set(lang: Lang) {
    ENGLISH.store(lang == Lang::En, Ordering::Relaxed);
}

pub fn current() -> Lang {
    if ENGLISH.load(Ordering::Relaxed) {
        Lang::En
    } else {
        Lang::Es
    }
}

pub fn tr(es: &'static str) -> &'static str {
    if current() == Lang::Es {
        return es;
    }
    TABLE.get(es).copied().unwrap_or(es)
}

/// Traduce la plantilla y sustituye cada `{}` por el siguiente argumento.
pub fn fmt(es: &'static str, args: &[&dyn Display]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut rest = tr(es);
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        if let Some(a) = args.next() {
            out.push_str(&a.to_string());
        }
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

/// `trf!("Exportado {}", ruta)` traduce la plantilla y rellena los `{}`.
macro_rules! trf {
    ($s:literal) => {
        $crate::i18n::tr($s)
    };
    ($s:literal, $($a:expr),+ $(,)?) => {
        $crate::i18n::fmt($s, &[$(&$a as &dyn ::std::fmt::Display),+])
    };
}
pub(crate) use trf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_claves_repetidas_y_con_los_mismos_huecos() {
        assert_eq!(TABLE.len(), en::ENTRIES.len(), "clave repetida");
        for (es, en) in en::ENTRIES {
            assert_eq!(
                es.matches("{}").count(),
                en.matches("{}").count(),
                "huecos distintos en «{es}»"
            );
        }
    }

    #[test]
    fn rellena_los_huecos_en_orden() {
        set(Lang::Es);
        assert_eq!(fmt("{} de {}", &[&1, &"a"]), "1 de a");
    }

    #[test]
    fn detecta_el_codigo_del_idioma() {
        assert!(Lang::from_code("es_MX.UTF-8") == Some(Lang::Es));
        assert!(Lang::from_code("en_US") == Some(Lang::En));
        assert!(Lang::from_code("fr_FR").is_none());
    }

    /// Cada `tr("…")` y `trf!("…")` del código tiene su traducción.
    #[test]
    fn todo_texto_traducible_tiene_entrada() {
        fn literal_tras(src: &str, marca: &str) -> Vec<String> {
            let mut out = vec![];
            let mut resto = src;
            while let Some(i) = resto.find(marca) {
                let antes = resto[..i].chars().next_back();
                resto = &resto[i + marca.len()..];
                if antes.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                    continue;
                }
                let mut lit = String::new();
                let mut chars = resto.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => match chars.next() {
                            Some('n') => lit.push('\n'),
                            Some(o) => lit.push(o),
                            None => break,
                        },
                        '"' => break,
                        _ => lit.push(c),
                    }
                }
                out.push(lit);
            }
            out
        }
        let mut faltan = vec![];
        let mut dirs = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !matches!(p.file_name().unwrap().to_str(), Some("en.rs" | "ui_tests.rs")) {
                    let src = std::fs::read_to_string(&p).unwrap();
                    let src = src.split("\n#[cfg(test)]").next().unwrap();
                    for marca in ["tr(\"", "trf!(\""] {
                        for lit in literal_tras(src, marca) {
                            if !TABLE.contains_key(lit.as_str()) {
                                faltan.push(format!("{}: {lit}", p.display()));
                            }
                        }
                    }
                }
            }
        }
        assert!(faltan.is_empty(), "sin traducción:\n{}", faltan.join("\n"));
    }
}
