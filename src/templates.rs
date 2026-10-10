//! Builders de plantillas: a partir de opciones, en un panel lateral, crean
//! en una carpeta `template.typ` (el formato) y `main.typ` (los datos, que
//! importa la plantilla). v1: solo Artículo (tipos futuros cuelgan de `Kind`).
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
#[derive(Clone, Debug)]
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

/// Nombres de los ficheros que crea el builder.
pub const TEMPLATE_FILE: &str = "template.typ";
pub const MAIN_FILE: &str = "main.typ";

/// Escapa un texto para interpolarlo en un string typst entre comillas.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "")
        .replace('\n', "\\n")
}

fn fmt_pt(s: f32) -> String {
    if s.fract() == 0.0 {
        format!("{}", s as i32)
    } else {
        format!("{s}")
    }
}

const MESES: &str = "(\"enero\", \"febrero\", \"marzo\", \"abril\", \"mayo\", \"junio\",\n  \"julio\", \"agosto\", \"septiembre\", \"octubre\", \"noviembre\", \"diciembre\")";

impl Article {
    /// `template.typ`: la función `articulo` con el formato elegido (página,
    /// texto, portada, resumen, índice). Los datos van en `main.typ`.
    pub fn build_template(&self) -> String {
        let es = self.lang == Lang::Es;
        let mut src = String::from(
            "// Plantilla de artículo generada por el editor. Se aplica desde main.typ\n\
             // con `#show: articulo.with(...)`; el formato se cambia aquí.\n\n",
        );
        if es {
            src.push_str(&format!("#let _meses = {MESES}\n\n"));
        }
        src.push_str(
            "// `fecha`: auto = hoy, none = sin fecha, o un texto.\n\
             #let articulo(\n",
        );
        src.push_str(&format!(
            "  titulo: \"{}\",\n",
            if es { "Título" } else { "Title" }
        ));
        src.push_str(
            "  autor: none,\n  fecha: auto,\n  resumen: none,\n  indice: true,\n  body,\n) = {\n  set document(title: titulo)\n",
        );
        src.push_str(&format!(
            "  set page(paper: \"{}\", margin: {})\n",
            self.paper.code(),
            self.margins.code()
        ));
        src.push_str(&format!(
            "  set text(size: {}pt, lang: \"{}\")\n",
            fmt_pt(self.font_size),
            self.lang.code()
        ));
        if self.justify {
            src.push_str("  set par(justify: true)\n");
        }
        if self.numbering {
            src.push_str("  set heading(numbering: \"1.1\")\n");
        }
        // Fecha de hoy: typst no traduce los meses, en español van a mano.
        src.push_str("\n  let fecha = if fecha == auto {\n    let hoy = datetime.today()\n");
        src.push_str(if es {
            "    [#hoy.day() de #_meses.at(hoy.month() - 1) de #hoy.year()]\n"
        } else {
            "    hoy.display(\"[month repr:long] [day padding:none], [year]\")\n"
        });
        src.push_str("  } else { fecha }\n\n");
        src.push_str(
            "  align(center)[\n\
             \x20   #text(size: 20pt, weight: \"bold\", titulo)\n\
             \x20   #if autor != none [ \\ #text(size: 12pt, autor)]\n\
             \x20   #if fecha != none [ \\ #fecha]\n\
             \x20 ]\n\n",
        );
        src.push_str(&format!(
            "  if resumen != none {{\n    block(inset: (x: 2em))[*{}.* #resumen]\n  }}\n",
            if es { "Resumen" } else { "Abstract" }
        ));
        src.push_str(&format!(
            "  if indice {{\n    outline(title: [{}])\n  }}\n  body\n}}\n",
            if es { "Índice" } else { "Contents" }
        ));
        src
    }

    /// `main.typ`: importa la plantilla y le pasa los datos. Los textos van
    /// como cadenas (`"…"`): `$`, `@`, `]`, `*`… se muestran tal cual. Un
    /// campo en blanco queda en `none` (o en el valor por defecto).
    pub fn build_main(&self) -> String {
        let es = self.lang == Lang::Es;
        let opt = |s: &str| {
            let t = s.trim();
            if t.is_empty() {
                "none".to_string()
            } else {
                format!("\"{}\"", esc(t))
            }
        };
        let mut src = format!("#import \"{TEMPLATE_FILE}\": articulo\n\n#show: articulo.with(\n");
        if !self.title.trim().is_empty() {
            src.push_str(&format!("  titulo: \"{}\",\n", esc(self.title.trim())));
        }
        src.push_str(&format!("  autor: {},\n", opt(&self.author)));
        let fecha = match self.date {
            DateMode::Today => "auto".to_string(),
            DateMode::Custom => opt(&self.custom_date),
            DateMode::None => "none".to_string(),
        };
        src.push_str(&format!("  fecha: {fecha},\n"));
        src.push_str(&format!("  resumen: {},\n", opt(&self.abstract_text)));
        src.push_str(&format!("  indice: {},\n)\n\n", self.toc));
        src.push_str(if es {
            "= Introducción\n"
        } else {
            "= Introduction\n"
        });
        src
    }

    /// Escribe `template.typ` y `main.typ` en `dir`. No sobrescribe: si
    /// alguno existe, no toca nada. Devuelve la ruta de `main.typ`.
    pub fn write_project(&self, dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
        let (tpl, main) = (dir.join(TEMPLATE_FILE), dir.join(MAIN_FILE));
        let taken: Vec<_> = [&tpl, &main]
            .into_iter()
            .filter(|p| p.exists())
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        if !taken.is_empty() {
            return Err(format!(
                "Ya existe {} en {}",
                taken.join(" y "),
                dir.display()
            ));
        }
        let write = |p: &std::path::Path, text: String| {
            std::fs::write(p, text).map_err(|e| format!("No se pudo escribir {}: {e}", p.display()))
        };
        write(&tpl, self.build_template())?;
        if let Err(e) = write(&main, self.build_main()) {
            let _ = std::fs::remove_file(&tpl);
            return Err(e);
        }
        Ok(main)
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
                    // Proyecto vinculado (desde «Nuevo proyecto»): Crear
                    // escribe ahí sin preguntar.
                    let mut unlink = false;
                    if let Some(dir) = &st.template_dir {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{}  {}",
                                    icon::FOLDER,
                                    dir.file_name().unwrap_or_default().to_string_lossy()
                                ))
                                .small()
                                .color(pal.accent),
                            )
                            .on_hover_text(dir.display().to_string());
                            unlink = ui
                                .small_button(icon::X)
                                .on_hover_text("Desvincular: Crear preguntará la carpeta")
                                .clicked();
                        });
                    }
                    if unlink {
                        st.template_dir = None;
                    }
                    ui.separator();
                    let linked = st.template_dir.is_some();
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
                        .on_hover_text(if linked {
                            format!("Crea {TEMPLATE_FILE} y {MAIN_FILE} en el proyecto y abre {MAIN_FILE}")
                        } else {
                            format!("Elige una carpeta: crea {TEMPLATE_FILE} y {MAIN_FILE}, y abre {MAIN_FILE}")
                        })
                        .clicked()
                    {
                        st.actions.push(Action::NewFromTemplate(Box::new(f.clone())));
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
    widgets::combo(
        ui,
        pal,
        RichText::new(current).small(),
        min_w,
        label,
        content,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workdir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("typst-editor-tmpl-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Escribe el proyecto en una carpeta propia y compila `main.typ`.
    fn compiles(a: &Article) -> bool {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = workdir().join(N.fetch_add(1, Ordering::Relaxed).to_string());
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let main = a.write_project(&dir).unwrap();
        let src = std::fs::read_to_string(main).unwrap();
        let out = crate::engine::compile_pdf(&src, &dir, MAIN_FILE);
        let ok = out.pdf.is_some();
        if !ok {
            eprintln!(
                "{:?}",
                out.diags.iter().map(|d| &d.message).collect::<Vec<_>>()
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
        ok
    }

    #[test]
    fn escapa_comillas_y_barras() {
        assert_eq!(esc("di \"hola\" \\ adiós"), "di \\\"hola\\\" \\\\ adiós");
        assert_eq!(esc("a\r\nb"), "a\\nb");
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
            for (justify, numbering, toc) in [
                (true, true, true),
                (false, false, false),
                (true, false, true),
            ] {
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
                    "no compila: {lang:?} {paper:?} {margins:?} {size} {date:?} j={justify} n={numbering} t={toc}\n{}\n{}",
                    a.build_template(),
                    a.build_main()
                );
            }
        }
    }

    #[test]
    fn markup_es_literal_y_compila() {
        let a = Article {
            title: "Costo $5 * _x_ // nota ] #f".into(),
            author: "ana@uni.edu <etq>".into(),
            date: DateMode::Custom,
            custom_date: "9 de octubre".into(),
            abstract_text: "Ver [1] y $5 ] ana@uni.edu\n\nSegundo párrafo".into(),
            ..Article::default()
        };
        let main = a.build_main();
        assert!(main.contains("fecha: \"9 de octubre\""), "{main}");
        assert!(compiles(&a), "no compila:\n{main}");
    }

    #[test]
    fn main_importa_la_plantilla() {
        let main = Article::default().build_main();
        assert!(
            main.starts_with("#import \"template.typ\": articulo\n"),
            "{main}"
        );
        assert!(main.contains("#show: articulo.with("), "{main}");
    }

    #[test]
    fn no_sobrescribe() {
        let dir = workdir().join("ocupado");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(MAIN_FILE), "mío").unwrap();
        let err = Article::default().write_project(&dir).unwrap_err();
        assert!(err.contains(MAIN_FILE), "{err}");
        assert_eq!(std::fs::read_to_string(dir.join(MAIN_FILE)).unwrap(), "mío");
        assert!(!dir.join(TEMPLATE_FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
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
        let main = a.build_main();
        assert!(
            !main.contains("titulo:"),
            "el título vacío usa el de la plantilla:\n{main}"
        );
        assert!(
            main.contains("autor: none") && main.contains("resumen: none"),
            "{main}"
        );
        assert!(a.build_template().contains("titulo: \"Título\""));
        let en = Article {
            lang: Lang::En,
            ..a.clone()
        };
        assert!(en.build_template().contains("titulo: \"Title\""));
        assert!(compiles(&a) && compiles(&en));
    }
}
