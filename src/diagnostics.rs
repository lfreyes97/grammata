//! Diagnósticos de `typst compile --diagnostic-format short`.
//!
//! Formato: `ruta:L:C: error: mensaje`. Las líneas sin prefijo continúan el
//! mensaje anterior (pánicos multilínea, `hint:`).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    /// `None` = el propio documento (`<stdin>`); `Some` = otro archivo
    /// (relativo a la raíz de compilación).
    pub file: Option<String>,
    pub line: Option<usize>,
    pub col: Option<usize>,
    pub message: String,
    pub hints: Vec<String>,
}

impl Diagnostic {
    pub fn location(&self) -> String {
        let file = self.file.as_deref().unwrap_or("");
        match (self.line, self.col) {
            (Some(l), Some(c)) if file.is_empty() => format!("{l}:{c}"),
            (Some(l), Some(c)) => format!("{file}:{l}:{c}"),
            _ => file.to_string(),
        }
    }
}

pub fn count(diags: &[Diagnostic]) -> (usize, usize) {
    let e = diags.iter().filter(|d| d.severity == Severity::Error).count();
    (e, diags.len() - e)
}

pub fn parse(stderr: &str) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    for line in stderr.lines() {
        if let Some(d) = parse_head(line) {
            out.push(d);
        } else if let Some(last) = out.last_mut() {
            let t = line.trim();
            if let Some(h) = t.strip_prefix("hint:") {
                last.hints.push(h.trim().to_string());
            } else if !t.is_empty() {
                last.message.push('\n');
                last.message.push_str(line);
            }
        } else if !line.trim().is_empty() {
            out.push(Diagnostic {
                severity: Severity::Error,
                file: None,
                line: None,
                col: None,
                message: line.trim().to_string(),
                hints: Vec::new(),
            });
        }
    }
    out
}

fn parse_head(line: &str) -> Option<Diagnostic> {
    for (marker, severity) in [
        ("error: ", Severity::Error),
        ("warning: ", Severity::Warning),
    ] {
        // Sin ubicación: `error: mensaje`.
        if let Some(msg) = line.strip_prefix(marker) {
            return Some(Diagnostic {
                severity,
                file: None,
                line: None,
                col: None,
                message: msg.to_string(),
                hints: Vec::new(),
            });
        }
        let sep = format!(": {marker}");
        if let Some(pos) = line.find(&sep) {
            let loc = &line[..pos];
            let msg = &line[pos + sep.len()..];
            // `ruta:L:C` — la ruta puede contener ':' así que se parte por la derecha.
            let mut parts = loc.rsplitn(3, ':');
            let col = parts.next().and_then(|s| s.parse().ok());
            let ln = parts.next().and_then(|s| s.parse().ok());
            let file = parts.next().unwrap_or(loc);
            let is_self = file.ends_with("<stdin>");
            return Some(Diagnostic {
                severity,
                file: (!is_self).then(|| file.to_string()),
                line: ln,
                col,
                message: msg.to_string(),
                hints: Vec::new(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_short_format() {
        let s = "<stdin>:2:6: error: unexpected argument\n\
                 cap/a.typ:1:16: warning: unknown font family: x\n\
                 <stdin>:1:1: error: panicked with: boom\nline2\n  hint: try this\n\
                 error: no location";
        let d = parse(s);
        assert_eq!(d.len(), 4);
        assert_eq!((d[0].line, d[0].col, d[0].file.clone()), (Some(2), Some(6), None));
        assert_eq!(d[1].severity, Severity::Warning);
        assert_eq!(d[1].file.as_deref(), Some("cap/a.typ"));
        assert_eq!(d[2].message, "panicked with: boom\nline2");
        assert_eq!(d[2].hints, vec!["try this"]);
        assert_eq!(d[3].line, None);
        assert_eq!(count(&d), (3, 1));
    }
}
