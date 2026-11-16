//! Panel de símbolos matemáticos.
//!
//! Los nombres de `CATEGORIES` están validados contra typst 0.15 (cada
//! `sym.<nombre>` existe y produce ese carácter): ver la prueba
//! `plantillas_y_simbolos_compilan` en `ui_tests.rs`.

use crate::i18n::{tr, trf};
use crate::actions::Action;
use crate::app::State;
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{FontFamily, RichText};

pub struct Sym {
    /// Lo que se ve en el botón.
    pub label: &'static str,
    /// Lo que se inserta (plantilla: `${}` = cursor, `${sel}` = selección).
    pub insert: &'static str,
    /// Nombre typst o descripción (tooltip y búsqueda).
    pub name: &'static str,
}

pub struct Category {
    pub name: &'static str,
    pub items: &'static [Sym],
}

const fn s(name: &'static str, glyph: &'static str) -> Sym {
    Sym {
        label: glyph,
        insert: name,
        name,
    }
}

const fn t(label: &'static str, insert: &'static str, name: &'static str) -> Sym {
    Sym {
        label,
        insert,
        name,
    }
}

pub const STRUCTURES: Category = Category {
    name: "Estructuras",
    items: &[
        t("x²", "${sel:x}^(${n})", "superíndice  x^(n)"),
        t("xᵢ", "${sel:x}_(${i})", "subíndice  x_(i)"),
        t("a⁄b", "frac(${sel:a}, ${b})", "fracción  frac(a, b)"),
        t("√x", "sqrt(${sel:x})", "raíz cuadrada  sqrt(x)"),
        t("ⁿ√x", "root(${n}, ${sel:x})", "raíz n-ésima  root(n, x)"),
        t("∑ⁿ", "sum_(${i=1})^n ", "sumatorio  sum_(i=1)^n"),
        t("∫ᵇₐ", "integral_${a}^b ", "integral definida  integral_a^b"),
        t("lim", "lim_(${x -> 0}) ", "límite  lim_(x -> 0)"),
        t("|x|", "abs(${sel:x})", "valor absoluto  abs(x)"),
        t("‖x‖", "norm(${sel:x})", "norma  norm(x)"),
        t("⌊x⌋", "floor(${sel:x})", "parte entera  floor(x)"),
        t("⌈x⌉", "ceil(${sel:x})", "techo  ceil(x)"),
        t("x̂", "hat(${sel:x})", "sombrero  hat(x)"),
        t("x̄", "overline(${sel:x})", "barra  overline(x)"),
        t("x→", "arrow(${sel:x})", "vector  arrow(x)"),
        t("ẋ", "dot(${sel:x})", "punto (derivada)  dot(x)"),
        t(
            "(ⁿₖ)",
            "binom(${n}, k)",
            "coeficiente binomial  binom(n, k)",
        ),
        t("(…)", "lr(( ${sel:x} ))", "paréntesis ajustables  lr(( ))"),
        t("[⋱]", "mat(${a}, b; c, d)", "matriz  mat(a, b; c, d)"),
        t("(⋮)", "vec(${x}, y, z)", "vector columna  vec(x, y, z)"),
        t(
            "{⋮",
            "cases(${x} &\"si\" x > 0, 0 &\"si no\")",
            "casos  cases(..)",
        ),
        t("“t”", "\"${sel:texto}\"", "texto dentro de la ecuación"),
    ],
};

pub const CATEGORIES: &[Category] = &[
    Category {
        name: "Griegas",
        items: &[
            s("alpha", "α"),
            s("beta", "β"),
            s("gamma", "γ"),
            s("delta", "δ"),
            s("epsilon", "ε"),
            s("epsilon.alt", "ϵ"),
            s("zeta", "ζ"),
            s("eta", "η"),
            s("theta", "θ"),
            s("theta.alt", "ϑ"),
            s("iota", "ι"),
            s("kappa", "κ"),
            s("lambda", "λ"),
            s("mu", "μ"),
            s("nu", "ν"),
            s("xi", "ξ"),
            s("pi", "π"),
            s("rho", "ρ"),
            s("sigma", "σ"),
            s("tau", "τ"),
            s("upsilon", "υ"),
            s("phi", "φ"),
            s("phi.alt", "ϕ"),
            s("chi", "χ"),
            s("psi", "ψ"),
            s("omega", "ω"),
            s("Gamma", "Γ"),
            s("Delta", "Δ"),
            s("Theta", "Θ"),
            s("Lambda", "Λ"),
            s("Xi", "Ξ"),
            s("Pi", "Π"),
            s("Sigma", "Σ"),
            s("Upsilon", "Υ"),
            s("Phi", "Φ"),
            s("Psi", "Ψ"),
            s("Omega", "Ω"),
        ],
    },
    Category {
        name: "Operadores",
        items: &[
            s("plus.minus", "±"),
            s("minus.plus", "∓"),
            s("times", "×"),
            s("div", "÷"),
            s("dot.op", "⋅"),
            s("ast.op", "∗"),
            s("compose", "∘"),
            s("sum", "∑"),
            s("product", "∏"),
            s("product.co", "∐"),
            s("integral", "∫"),
            s("integral.double", "∬"),
            s("integral.triple", "∭"),
            s("integral.cont", "∮"),
            s("partial", "∂"),
            s("nabla", "∇"),
            s("infinity", "∞"),
            s("plus.o", "⊕"),
            s("times.o", "⊗"),
            s("union", "∪"),
            s("inter", "∩"),
            s("union.big", "⋃"),
            s("inter.big", "⋂"),
        ],
    },
    Category {
        name: "Relaciones",
        items: &[
            s("eq.not", "≠"),
            s("lt.eq", "≤"),
            s("gt.eq", "≥"),
            s("lt.double", "≪"),
            s("gt.double", "≫"),
            s("approx", "≈"),
            s("tilde.op", "∼"),
            s("tilde.equiv", "≅"),
            s("equiv", "≡"),
            s("prop", "∝"),
            s("prec", "≺"),
            s("succ", "≻"),
            s("perp", "⟂"),
            s("parallel", "∥"),
            s("divides", "∣"),
            s("colon.eq", "≔"),
            s("eq.def", "≝"),
        ],
    },
    Category {
        name: "Conjuntos y lógica",
        items: &[
            s("in", "∈"),
            s("in.not", "∉"),
            s("subset", "⊂"),
            s("subset.eq", "⊆"),
            s("supset", "⊃"),
            s("supset.eq", "⊇"),
            s("emptyset", "∅"),
            s("without", "∖"),
            s("complement", "∁"),
            s("forall", "∀"),
            s("exists", "∃"),
            s("exists.not", "∄"),
            s("and", "∧"),
            s("or", "∨"),
            s("not", "¬"),
            s("tack.r", "⊢"),
            s("top", "⊤"),
            s("bot", "⊥"),
            s("NN", "ℕ"),
            s("ZZ", "ℤ"),
            s("QQ", "ℚ"),
            s("RR", "ℝ"),
            s("CC", "ℂ"),
            s("aleph", "א"),
        ],
    },
    Category {
        name: "Flechas",
        items: &[
            s("arrow.r", "→"),
            s("arrow.l", "←"),
            s("arrow.l.r", "↔"),
            s("arrow.t", "↑"),
            s("arrow.b", "↓"),
            s("arrow.r.double", "⇒"),
            s("arrow.l.double", "⇐"),
            s("arrow.l.r.double", "⇔"),
            s("arrow.r.long", "⟶"),
            s("arrow.r.long.double", "⟹"),
            s("arrow.r.bar", "↦"),
            s("arrow.r.hook", "↪"),
            s("arrow.r.squiggly", "⇝"),
            s("arrow.tr", "↗"),
            s("arrow.br", "↘"),
            s("harpoon.rt", "⇀"),
            s("arrows.rl", "⇄"),
        ],
    },
    Category {
        name: "Otros",
        items: &[
            s("dots.h", "…"),
            s("dots.c", "⋯"),
            s("dots.v", "⋮"),
            s("dots.down", "⋱"),
            s("degree", "°"),
            s("prime", "′"),
            s("angle", "∠"),
            s("triangle.stroked.t", "△"),
            s("square.stroked", "□"),
            s("star.op", "⋆"),
            s("dagger", "†"),
            s("ell", "ℓ"),
            s("planck", "ħ"),
            s("Re", "ℜ"),
            s("Im", "ℑ"),
            s("qed", "∎"),
            s("checkmark", "✓"),
            s("crossmark", "✗"),
        ],
    },
];

pub fn all() -> impl Iterator<Item = &'static Sym> {
    STRUCTURES
        .items
        .iter()
        .chain(CATEGORIES.iter().flat_map(|c| c.items.iter()))
}

fn find(insert: &str) -> Option<&'static Sym> {
    all().find(|s| s.insert == insert)
}

pub use crate::highlight::in_math;

pub fn panel(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut st.symbol_query)
                    .id_salt("symbol-query")
                    .hint_text(trf!("{}  Buscar símbolo (alpha, flecha…)", egui_phosphor::regular::MAGNIFYING_GLASS))
                    .desired_width(f32::INFINITY),
            );
        });
    });
    let in_math = {
        let doc = st.docs.cur();
        in_math(&doc.text, st.editor.cursor.unwrap_or(0))
    };
    let mut pick: Option<&'static str> = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    let q = st.symbol_query.trim().to_lowercase();
                    if !q.is_empty() {
                        let hits: Vec<&Sym> = all()
                            .filter(|s| (s.name.to_lowercase().contains(&q) || tr(s.name).to_lowercase().contains(&q)) || s.label == q)
                            .collect();
                        if hits.is_empty() {
                            ui.label(RichText::new(tr("Sin resultados.")).color(pal.text_dim));
                        }
                        grid(ui, &pal, hits.into_iter(), &mut pick);
                    } else {
                        let recent: Vec<&Sym> =
                            st.recent_symbols.iter().filter_map(|r| find(r)).collect();
                        if !recent.is_empty() {
                            widgets::section_label(ui, &pal, tr("Recientes"));
                            grid(ui, &pal, recent.into_iter(), &mut pick);
                            ui.add_space(6.0);
                        }
                        for cat in std::iter::once(&STRUCTURES).chain(CATEGORIES.iter()) {
                            egui::CollapsingHeader::new(RichText::new(tr(cat.name)).color(pal.text))
                                .default_open(true)
                                .show(ui, |ui| grid(ui, &pal, cat.items.iter(), &mut pick));
                        }
                    }
                    ui.add_space(8.0);
                    let hint = if in_math {
                        tr("El cursor está en una ecuación: se inserta el nombre.")
                    } else {
                        tr("Fuera de una ecuación se inserta como $…$.")
                    };
                    ui.label(RichText::new(hint).small().color(pal.text_faint));
                });
        });
    if let Some(insert) = pick {
        st.actions.push(Action::InsertMath(insert.to_string()));
    }
}

fn grid(
    ui: &mut egui::Ui,
    pal: &Palette,
    items: impl Iterator<Item = &'static Sym>,
    pick: &mut Option<&'static str>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
        for s in items {
            // Símbolos sueltos con la fuente matemática (como en el PDF); las
            // estructuras combinan caracteres que esa fuente coloca raros.
            let family = FontFamily::Name(
                if s.insert == s.name {
                    "math"
                } else {
                    "ui-math"
                }
                .into(),
            );
            let label = RichText::new(s.label)
                .family(family)
                .size(if s.insert == s.name { 17.0 } else { 14.0 })
                .color(pal.text);
            let r = ui
                .add(
                    egui::Button::new(label)
                        .fill(pal.surface_hover)
                        .min_size(egui::vec2(36.0, 32.0)),
                )
                .on_hover_ui(|ui| {
                    ui.label(RichText::new(tr(s.name)).monospace().color(pal.text));
                    if s.insert != s.name {
                        ui.label(
                            RichText::new(crate::snippets::expand(s.insert, "").0)
                                .monospace()
                                .small()
                                .color(pal.text_dim),
                        );
                    }
                });
            if r.clicked() {
                *pick = Some(s.insert);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_duplicados() {
        let mut seen = std::collections::HashSet::new();
        for s in all() {
            assert!(seen.insert(s.insert), "duplicado: {}", s.insert);
        }
    }
}
