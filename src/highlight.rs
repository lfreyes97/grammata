//! Resaltado con el analizador oficial de Typst (`typst-syntax`).
//!
//! A diferencia del resaltado por palabras clave entiende el marcado
//! (encabezados, `*negrita*`, `_cursiva_`, ecuaciones, etiquetas…) y el código
//! dentro de `#…` igual que el compilador.

use crate::theme::Palette;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Stroke};
use typst_syntax::{LinkedNode, Side, SyntaxKind, Tag};

/// Estilo acumulado de las etiquetas del nodo y sus ancestros.
#[derive(Clone, Copy, PartialEq)]
struct Style {
    color: Color32,
    italics: bool,
    underline: Option<Color32>,
}

/// Construye el `LayoutJob` coloreado del texto completo.
pub fn layout_job(text: &str, font: FontId, pal: &Palette) -> LayoutJob {
    let root = typst_syntax::parse(text);
    let mut job = LayoutJob::default();
    let base = Style {
        color: pal.text,
        italics: false,
        underline: None,
    };
    // Tramos contiguos con el mismo estilo se fusionan en una sección.
    let mut run: Option<(usize, usize, Style)> = None;
    walk(
        &LinkedNode::new(&root),
        base,
        pal,
        &mut |start, end, st| match &mut run {
            Some((_, e, s)) if *e == start && *s == st => *e = end,
            _ => {
                if let Some((a, b, s)) = run.replace((start, end, st)) {
                    push(&mut job, text, a, b, s, &font);
                }
            }
        },
    );
    if let Some((a, b, s)) = run {
        push(&mut job, text, a, b, s, &font);
    }
    // Lo que el árbol no cubra (no debería pasar) se añade sin estilo.
    let covered: usize = job
        .sections
        .last()
        .map(|s| usize::from(s.byte_range.end))
        .unwrap_or(0);
    if covered < text.len() {
        push(&mut job, text, covered, text.len(), base, &font);
    }
    job
}

fn push(job: &mut LayoutJob, text: &str, a: usize, b: usize, s: Style, font: &FontId) {
    let mut f = TextFormat::simple(font.clone(), s.color);
    f.italics = s.italics;
    if let Some(c) = s.underline {
        f.underline = Stroke::new(1.0, c);
    }
    job.append(&text[a..b], 0.0, f);
}

fn walk(
    node: &LinkedNode,
    inherited: Style,
    pal: &Palette,
    out: &mut impl FnMut(usize, usize, Style),
) {
    let mut st = inherited;
    if let Some(tag) = typst_syntax::highlight(node) {
        apply(&mut st, tag, pal);
    }
    if node.children().len() == 0 {
        let r = node.range();
        if !r.is_empty() {
            out(r.start, r.end, st);
        }
        return;
    }
    for child in node.children() {
        walk(&child, st, pal, out);
    }
}

fn apply(st: &mut Style, tag: Tag, pal: &Palette) {
    let s = &pal.syntax;
    match tag {
        Tag::Comment => st.color = s.comment,
        Tag::Punctuation => st.color = s.punctuation,
        Tag::Escape => st.color = s.escape,
        Tag::Strong => st.color = s.strong,
        Tag::Emph => st.italics = true,
        Tag::Link => {
            st.color = s.link;
            st.underline = Some(s.link);
        }
        Tag::Raw => st.color = s.raw,
        Tag::Label | Tag::Ref => st.color = s.label,
        Tag::Heading => st.color = s.heading,
        Tag::ListMarker | Tag::ListTerm => st.color = s.list,
        Tag::MathDelimiter | Tag::MathOperator | Tag::MathGroupingParens => st.color = s.math,
        Tag::Keyword => st.color = s.keyword,
        Tag::Operator => st.color = s.operator,
        Tag::Number => st.color = s.number,
        Tag::String => st.color = s.string,
        Tag::Function => st.color = s.function,
        Tag::Interpolated => st.color = s.interpolated,
        Tag::Error => st.underline = Some(pal.err),
    }
}

/// ¿Está el índice de char `cursor` dentro de una ecuación `$ … $`?
pub fn in_math(text: &str, cursor: usize) -> bool {
    let byte = crate::text::char_to_byte(text, cursor);
    let root = typst_syntax::parse(text);
    let root = LinkedNode::new(&root);
    let Some(leaf) = root.leaf_at(byte, Side::Before) else {
        return false;
    };
    // Justo tras un `$`: dentro si es el de apertura, fuera si es el de cierre.
    if leaf.kind() == SyntaxKind::Dollar {
        return leaf
            .parent()
            .is_some_and(|p| p.offset() == leaf.offset() && p.range().end > leaf.range().end);
    }
    let mut node = leaf.parent().cloned();
    while let Some(n) = node {
        if n.kind() == SyntaxKind::Equation {
            return true;
        }
        node = n.parent().cloned();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_modo_matematico() {
        let t = "a $x + y$ b $c";
        let at = |needle: &str| t.find(needle).map(|b| t[..b].chars().count()).unwrap();
        assert!(!in_math(t, at("a")));
        assert!(in_math(t, at("x")));
        assert!(in_math(t, at("$ b")), "antes del `$` de cierre");
        assert!(!in_math(t, at(" b")), "después del `$` de cierre");
        assert!(in_math(t, at("c")), "justo tras el `$` de apertura");
        assert!(in_math(t, t.chars().count()));
        assert!(!in_math("\\$ x", 3));
        assert!(!in_math("// $ comentario\nz", 17));
        assert!(!in_math("`$` z", 4));
        assert!(!in_math("#let s = \"$\"\nz", 14));
        assert!(in_math("$ frac(a, b) $", 9));
    }

    #[test]
    fn cubre_todo_el_texto() {
        let pal = crate::theme::DARK;
        for t in [
            "",
            "= Título\n*negrita* y _cursiva_ $x^2$ #let a = 1 // c",
            "#f(\"sin cerrar",
            "ñ é 🎉 $α$",
        ] {
            let job = layout_job(t, FontId::monospace(13.0), &pal);
            assert_eq!(job.text, t);
            let mut end = 0;
            for s in &job.sections {
                assert_eq!(usize::from(s.byte_range.start), end);
                end = usize::from(s.byte_range.end);
            }
            assert_eq!(end, t.len());
        }
    }

    #[test]
    fn colorea_marcado() {
        let pal = crate::theme::DARK;
        let t = "= Título\n#let x = 1";
        let job = layout_job(t, FontId::monospace(13.0), &pal);
        let color_at = |needle: &str| {
            let b = t.find(needle).unwrap();
            job.sections
                .iter()
                .find(|s| {
                    (usize::from(s.byte_range.start)..usize::from(s.byte_range.end)).contains(&b)
                })
                .unwrap()
                .format
                .color
        };
        assert_eq!(color_at("Título"), pal.syntax.heading);
        assert_eq!(color_at("let"), pal.syntax.keyword);
        assert_eq!(color_at("1"), pal.syntax.number);
    }
}
