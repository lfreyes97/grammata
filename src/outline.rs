//! Esquema del documento: encabezados, `#let` y etiquetas.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Heading(usize),
    Let,
    Label,
}

#[derive(Clone)]
pub struct Item {
    pub kind: Kind,
    pub title: String,
    pub line: usize,
    pub char_idx: usize,
}

pub fn parse(text: &str) -> Vec<Item> {
    let mut out = Vec::new();
    let mut char_idx = 0;
    let mut in_raw = false;
    for (ln, line) in text.split_inclusive('\n').enumerate() {
        let line_char_idx = char_idx;
        char_idx += line.chars().count();
        let t = line.trim_end_matches(['\n', '\r']).trim_start();
        // Bloques ``` … ``` : su contenido no cuenta.
        if t.starts_with("```") {
            in_raw = !in_raw;
            continue;
        }
        if in_raw {
            continue;
        }
        let push = |out: &mut Vec<Item>, kind, title: String| {
            out.push(Item { kind, title, line: ln + 1, char_idx: line_char_idx })
        };
        if t.starts_with('=') {
            let lvl = t.chars().take_while(|&c| c == '=').count();
            let rest = &t[lvl..];
            if lvl <= 6 && rest.starts_with([' ', '\t']) {
                let title = rest.trim();
                // Quita una etiqueta final: `= Intro <intro>`.
                let title = match title.rfind(" <") {
                    Some(i) if title.ends_with('>') => title[..i].trim_end(),
                    _ => title,
                };
                if !title.is_empty() {
                    push(&mut out, Kind::Heading(lvl), title.to_string());
                }
            }
        } else if let Some(rest) = t.strip_prefix("#let") {
            if rest.starts_with([' ', '\t']) {
                let name = rest
                    .trim_start()
                    .split([' ', '=', '(', ';'])
                    .next()
                    .unwrap_or("");
                if !name.is_empty() {
                    push(&mut out, Kind::Let, name.to_string());
                }
            }
        } else if t.starts_with('<') && t.ends_with('>') && t.len() > 2 && !t.contains(' ') {
            push(&mut out, Kind::Label, t.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_items() {
        let t = "= Uno <uno>\nxx\n== Dos\n#let f(x) = x\n<lbl>\n```\n= no\n```\n=no\n";
        let o = parse(t);
        let titles: Vec<_> = o.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["Uno", "Dos", "f", "<lbl>"]);
        assert!(o[1].kind == Kind::Heading(2));
        assert_eq!(o[1].char_idx, "= Uno <uno>\nxx\n".chars().count());
    }
}
