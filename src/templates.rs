//! Builders de plantillas: generan un documento typst completo a partir de
//! opciones, en un panel lateral. v1: solo Artículo (tipos futuros cuelgan
//! de `Kind`).
//!
//! Todo lo generado debe compilar: los tests construyen la matriz de
//! opciones y la pasan por el motor embebido.

use crate::actions::Action;
use crate::app::State;
use crate::theme;
use crate::widgets;
use egui::RichText;
use egui_phosphor::regular as icon;

/// Tipos de plantilla (v1: solo artículo).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Article,
}

impl Kind {
    pub const ALL: [Kind; 1] = [Kind::Article];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Article => "Artículo",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Kind::Article => "Título, autor, fecha, resumen e índice opcionales.",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    Es,
    En,
}

impl Lang {
    fn code(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Lang::Es => "Español",
            Lang::En => "English",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Paper {
    #[default]
    A4,
    A5,
    Letter,
}

impl Paper {
    fn code(self) -> &'static str {
        match self {
            Paper::A4 => "a4",
            Paper::A5 => "a5",
            Paper::Letter => "us-letter",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::A5 => "A5",
            Paper::Letter => "Carta",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Margins {
    Estrecho,
    #[default]
    Normal,
    Amplio,
}

impl Margins {
    pub const ALL: [Margins; 3] = [Margins::Estrecho, Margins::Normal, Margins::Amplio];

    fn code(self) -> &'static str {
        match self {
            Margins::Estrecho => "2cm",
            Margins::Normal => "2.5cm",
            Margins::Amplio => "3cm",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Margins::Estrecho => "Estrechos",
            Margins::Normal => "Normales",
            Margins::Amplio => "Amplios",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DateMode {
    #[default]
    Today,
    Custom,
    None,
}

impl DateMode {
    pub const ALL: [DateMode; 3] = [DateMode::Today, DateMode::Custom, DateMode::None];

    fn label(self) -> &'static str {
        match self {
            DateMode::Today => "Hoy",
            DateMode::Custom => "Personalizada",
            DateMode::None => "Sin fecha",
        }
    }
}

/// Opciones del builder de artículo (también estado del formulario).
#[derive(Clone)]
pub struct Article {
    pub title: String,
    pub author: String,
    pub date: DateMode,
    pub custom_date: String,
    pub lang: Lang,
    pub paper: Paper,
    pub margins: Margins,
    pub font_size: f32,
    pub justify: bool,
    pub numbering: bool,
    pub toc: bool,
    pub abstract_text: String,
}

impl Default for Article {
    fn default() -> Self {
        Self {
            title: "Título".to_string(),
            author: "Autor".to_string(),
            date: DateMode::Today,
            custom_date: String::new(),
            lang: Lang::Es,
            paper: Paper::A4,
            margins: Margins::Normal,
            font_size: 11.0,
            justify: true,
            numbering: true,
            toc: true,
            abstract_text: String::new(),
        }
    }
}

/// Escapa un texto para interpolarlo en un string typst entre comillas.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn fmt_pt(s: f32) -> String {
    if s.fract() == 0.0 {
        format!("{}", s as i32)
    } else {
        format!("{s}")
    }
}

impl Article {
    /// Genera el fuente `.typ` completo. Siempre compila (ver tests).
    pub fn build(&self) -> String {
        let es = self.lang == Lang::Es;
        let mut src = String::new();
        src.push_str(&format!(
            "#set page(paper: \"{}\", margin: {})\n",
            self.paper.code(),
            self.margins.code()
        ));
        src.push_str(&format!(
            "#set text(size: {}pt, lang: \"{}\")\n",
            fmt_pt(self.font_size),
            self.lang.code()
        ));
        if self.justify {
            src.push_str("#set par(justify: true)\n");
        }
        if self.numbering {
            src.push_str("#set heading(numbering: \"1.1\")\n");
        }
        src.push('\n');
        // Portada centrada (título con suplente si se vacía; autor y fecha
        // se omiten en blanco para no dejar huecos raros).
        let title = if self.title.trim().is_empty() {
            "Título"
        } else {
            self.title.trim()
        };
        // Los textos van como cadenas (`"…"`), no como markup: `$`, `@`,
        // `//`, `*`… se muestran tal cual. Una línea en blanco entre piezas
        // para que no se junten en un solo párrafo.
        let mut cover = vec![format!(
            "  #text(size: 20pt, weight: \"bold\", \"{}\")",
            esc(title)
        )];
        if !self.author.trim().is_empty() {
            cover.push(format!(
                "  #text(size: 12pt, \"{}\")",
                esc(self.author.trim())
            ));
        }
        match self.date {
            DateMode::Today => cover.push("  #datetime.today().display()".into()),
            DateMode::Custom if !self.custom_date.trim().is_empty() => {
                cover.push(format!("  #\"{}\"", esc(self.custom_date.trim())));
            }
            _ => {}
        }
        src.push_str("#align(center)[\n");
        src.push_str(&cover.join("\n\n"));
        src.push_str("\n]\n");
        // Resumen (vacío = se omite).
        if !self.abstract_text.trim().is_empty() {
            let tag = if es { "Resumen" } else { "Abstract" };
            src.push_str(&format!(
                "\n#block(inset: (x: 2em))[\n  *{tag}.* {}\n]\n",
                self.abstract_text.trim()
            ));
        }
        // Índice (título explícito por idioma).
        if self.toc {
            let t = if es { "Índice" } else { "Contents" };
            src.push_str(&format!("\n#outline(title: [{t}])\n"));
        }
        // Primera sección para empezar a escribir.
        src.push_str(if es {
            "\n= Introducción\n"
        } else {
            "\n= Introduction\n"
        });
        src
    }
}

// ---------- Panel ----------

/// Panel lateral: tipo + formulario + crear. El documento se abre en una
/// pestaña nueva sin título (se guarda con Ctrl+S como cualquier otro).
pub fn panel(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::section_label(ui, &pal, "Plantillas");
        });
    });
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 8.0;
                    // Tipo (v1: uno solo, listo para crecer).
                    widgets::combo(
                        ui,
                        &pal,
                        RichText::new(format!("{}  {}", icon::ARTICLE, Kind::Article.name()))
                            .small(),
                        160.0,
                        "Tipo de plantilla",
                        |ui| {
                            ui.set_min_width(220.0);
                            for k in Kind::ALL {
                                ui.selectable_label(true, format!("{}  {}", icon::ARTICLE, k.name()))
                                    .on_hover_text(k.desc());
                            }
                        },
                    );
                    ui.label(
                        RichText::new(Kind::Article.desc())
                            .small()
                            .color(pal.text_faint),
                    );
                    ui.separator();
                    let f = &mut st.template;
                    field(ui, &pal, "Título", &mut f.title, "Título del trabajo");
                    field(ui, &pal, "Autor", &mut f.author, "Nombre del autor");
                    // Fecha: modo + campo si es personalizada.
                    ui.label(RichText::new("Fecha").small().color(pal.text_dim));
                    let mut date_pick: Option<DateMode> = None;
                    widgets::combo(
                        ui,
                        &pal,
                        RichText::new(f.date.label()).small(),
                        160.0,
                        "Modo de fecha",
                        |ui| {
                            ui.set_min_width(160.0);
                            for m in DateMode::ALL {
                                if ui.selectable_label(f.date == m, m.label()).clicked() {
                                    date_pick = Some(m);
                                    ui.close();
                                }
                            }
                        },
                    );
                    if let Some(m) = date_pick {
                        f.date = m;
                    }
                    if f.date == DateMode::Custom {
                        ui.add(
                            egui::TextEdit::singleline(&mut f.custom_date)
                                .hint_text("9 de octubre de 2026")
                                .desired_width(f32::INFINITY),
                        );
                    }
                    // Página: idioma, papel, márgenes, fuente.
                    ui.columns(2, |c| {
                        combo_row(&mut c[0], &pal, "Idioma", f.lang.label(), 110.0, |ui| {
                            for l in [Lang::Es, Lang::En] {
                                if ui.selectable_label(f.lang == l, l.label()).clicked() {
                                    f.lang = l;
                                    ui.close();
                                }
                            }
                        });
                        combo_row(&mut c[1], &pal, "Papel", f.paper.label(), 110.0, |ui| {
                            for p in [Paper::A4, Paper::A5, Paper::Letter] {
                                if ui.selectable_label(f.paper == p, p.label()).clicked() {
                                    f.paper = p;
                                    ui.close();
                                }
                            }
                        });
                    });
                    ui.columns(2, |c| {
                        combo_row(&mut c[0], &pal, "Márgenes", f.margins.label(), 110.0, |ui| {
                            for m in Margins::ALL {
                                if ui.selectable_label(f.margins == m, m.label()).clicked() {
                                    f.margins = m;
                                    ui.close();
                                }
                            }
                        });
                        let size = format!("{} pt", fmt_pt(f.font_size));
                        combo_row(&mut c[1], &pal, "Fuente", &size, 110.0, |ui| {
                            for s in [10.0, 11.0, 12.0] {
                                if ui
                                    .selectable_label(f.font_size == s, format!("{} pt", fmt_pt(s)))
                                    .clicked()
                                {
                                    f.font_size = s;
                                    ui.close();
                                }
                            }
                        });
                    });
                    // Interruptores.
                    ui.columns(2, |c| {
                        c[0].checkbox(&mut f.justify, "Justificar");
                        c[1].checkbox(&mut f.numbering, "Numerar apartados");
                    });
                    ui.checkbox(&mut f.toc, "Índice al inicio");
                    // Resumen (vacío = no se incluye).
                    ui.label(RichText::new("Resumen").small().color(pal.text_dim));
                    ui.add(
                        egui::TextEdit::multiline(&mut f.abstract_text)
                            .hint_text("Vacío = sin resumen")
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    ui.separator();
                    if widgets::accent_button(ui, &pal, &format!("{}  Crear documento", icon::FILE_PLUS))
                        .on_hover_text("Abre el artículo en una pestaña nueva")
                        .clicked()
                    {
                        st.actions.push(Action::NewFromTemplate { text: f.build() });
                    }
                });
        });
}

/// Campo de texto con etiqueta.
fn field(ui: &mut egui::Ui, pal: &theme::Palette, label: &str, value: &mut String, hint: &str) {
    ui.label(RichText::new(label).small().color(pal.text_dim));
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    );
}

/// Desplegable con etiqueta en una columna.
fn combo_row(
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    label: &str,
    current: &str,
    min_w: f32,
    content: impl FnOnce(&mut egui::Ui),
) {
    ui.label(RichText::new(label).small().color(pal.text_dim));
    widgets::combo(ui, pal, RichText::new(current).small(), min_w, label, content);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workdir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "typst-editor-tmpl-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn compiles(a: &Article) -> bool {
        let src = a.build();
        let out = crate::engine::compile_pdf(&src, &workdir(), "main.typ");
        out.pdf.is_some()
    }

    #[test]
    fn escapa_comillas_y_barras() {
        assert_eq!(esc("di \"hola\" \\ adiós"), "di \\\"hola\\\" \\\\ adiós");
    }

    #[test]
    fn articulo_por_defecto_compila() {
        assert!(compiles(&Article::default()));
    }

    #[test]
    fn matriz_de_opciones_compila() {
        let mut cases = Vec::new();
        for lang in [Lang::Es, Lang::En] {
            for paper in [Paper::A4, Paper::A5, Paper::Letter] {
                for margins in Margins::ALL {
                    for size in [10.0, 11.0, 12.0] {
                        for date in DateMode::ALL {
                            cases.push((lang, paper, margins, size, date));
                        }
                    }
                }
            }
        }
        assert_eq!(cases.len(), 2 * 3 * 3 * 3 * 3);
        for (lang, paper, margins, size, date) in cases {
            for (justify, numbering, toc) in
                [(true, true, true), (false, false, false), (true, false, true)]
            {
                let a = Article {
                    title: "El \"niño\" y la \\barra\\".into(),
                    author: "Ana \"La\" Autora".into(),
                    date,
                    custom_date: "1º de enero de \"2026\"".into(),
                    lang,
                    paper,
                    margins,
                    font_size: size,
                    justify,
                    numbering,
                    toc,
                    abstract_text: "Resumen con *énfasis* y números 123.".into(),
                };
                assert!(
                    compiles(&a),
                    "no compila: {lang:?} {paper:?} {margins:?} {size} {date:?} j={justify} n={numbering} t={toc}\n{}",
                    a.build()
                );
            }
        }
    }

    #[test]
    fn markup_en_portada_es_literal_y_compila() {
        let a = Article {
            title: "Costo $5 * _x_ // nota ] #f".into(),
            author: "ana@uni.edu <etq>".into(),
            date: DateMode::Custom,
            custom_date: "9 de octubre".into(),
            ..Article::default()
        };
        let src = a.build();
        assert!(src.contains("#\"9 de octubre\""), "fecha sin corchetes:\n{src}");
        assert!(compiles(&a), "no compila:\n{src}");
    }

    #[test]
    fn portada_en_lineas_separadas() {
        let a = Article {
            author: "Ana".into(),
            date: DateMode::Today,
            ..Article::default()
        };
        let src = a.build();
        let body = &src[src.find("#align(center)[").unwrap()..];
        let body = &body[..body.find("\n]").unwrap()];
        assert_eq!(body.matches("\n\n").count(), 2, "título, autor y fecha separados:\n{src}");
    }

    #[test]
    fn vacios_se_omiten_y_compila() {
        let a = Article {
            title: "   ".into(),
            author: String::new(),
            date: DateMode::None,
            custom_date: String::new(),
            abstract_text: "  ".into(),
            toc: false,
            ..Article::default()
        };
        let src = a.build();
        assert!(src.contains("Título"), "el título vacío lleva suplente:\n{src}");
        assert!(compiles(&a));
    }
}
