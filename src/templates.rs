//! Builders de plantillas: a partir de opciones, en un panel lateral, crean
//! en una carpeta `template.typ` (el formato) y `main.typ` (los datos, que
//! importa la plantilla), más `refs.bib` si hay bibliografía. Tipos:
//! artículo, informe, libro, apuntes y presentación (ver `Kind`).
//!
//! Todo lo generado debe compilar: los tests de `codegen` construyen
//! combinaciones de opciones para cada tipo y las pasan por el motor.

mod codegen;

use crate::i18n::{tr, trf};
use crate::actions::Action;
use crate::app::State;
use crate::theme;
use crate::widgets;
use egui::RichText;
use egui_phosphor::regular as icon;

/// Tipos de plantilla.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Kind {
    #[default]
    Article,
    Report,
    Book,
    Notes,
    Slides,
}

/// Partes del formulario que dependen del tipo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Subtitle,
    Institution,
    Course,
    Abstract,
    Solutions,
    /// Papel y márgenes (la presentación es 16:9 fija).
    Page,
    Columns,
    Justify,
    Numbering,
    Header,
    /// Encabezado con el capítulo en curso.
    Chapter,
    Toc,
    Bib,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Article,
        Kind::Report,
        Kind::Book,
        Kind::Notes,
        Kind::Slides,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Article => tr("Artículo"),
            Kind::Report => tr("Informe"),
            Kind::Book => tr("Libro"),
            Kind::Notes => tr("Apuntes"),
            Kind::Slides => tr("Presentación"),
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Kind::Article => tr("Portada en la primera página; resumen e índice opcionales."),
            Kind::Report => {
                tr("Informe o monografía: portada a página completa, resumen, índice y capítulos en página nueva.")
            }
            Kind::Book => {
                tr("Portada, índice en romanos, capítulos en página impar y márgenes a doble cara.")
            }
            Kind::Notes => {
                tr("Apuntes u hoja de ejercicios: cajas de definición, teorema, ejemplo, ejercicio y solución.")
            }
            Kind::Slides => tr("Diapositivas 16:9: «=» abre una sección y «==» una diapositiva."),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Kind::Article => icon::ARTICLE,
            Kind::Report => icon::FILE_TEXT,
            Kind::Book => icon::BOOK_OPEN,
            Kind::Notes => icon::EXAM,
            Kind::Slides => icon::PRESENTATION,
        }
    }

    /// Nombre de la función de la plantilla (`#show: articulo.with(...)`).
    fn func(self) -> &'static str {
        match self {
            Kind::Article => "articulo",
            Kind::Report => "informe",
            Kind::Book => "libro",
            Kind::Notes => "apuntes",
            Kind::Slides => "presentacion",
        }
    }

    pub fn has(self, f: Field) -> bool {
        use Field::*;
        match self {
            Kind::Article => !matches!(f, Subtitle | Course | Solutions | Chapter),
            Kind::Report => !matches!(f, Course | Solutions),
            Kind::Book => !matches!(f, Course | Solutions | Abstract | Columns),
            Kind::Notes => !matches!(f, Subtitle | Abstract | Chapter),
            Kind::Slides => matches!(f, Subtitle | Institution | Toc),
        }
    }

    pub fn font_sizes(self) -> &'static [f32] {
        match self {
            Kind::Slides => &[18.0, 20.0, 24.0],
            _ => &[10.0, 11.0, 12.0],
        }
    }

    fn default_size(self) -> f32 {
        match self {
            Kind::Slides => 20.0,
            _ => 11.0,
        }
    }

    fn default_header(self) -> HeaderMode {
        match self {
            Kind::Report | Kind::Book => HeaderMode::Chapter,
            Kind::Notes => HeaderMode::Title,
            Kind::Article | Kind::Slides => HeaderMode::None,
        }
    }

    fn default_page_num(self) -> PageNum {
        match self {
            Kind::Book => PageNum::Outside,
            Kind::Slides => PageNum::Right,
            _ => PageNum::Center,
        }
    }
}

/// Enumerados del formulario: lista y etiqueta para los desplegables.
pub trait Choice: Copy + PartialEq + 'static {
    const ALL: &'static [Self];
    fn label(self) -> &'static str;
}

macro_rules! choice {
    ($t:ident { $($v:ident => $l:expr),+ $(,)? }) => {
        impl Choice for $t {
            const ALL: &'static [Self] = &[$($t::$v),+];
            fn label(self) -> &'static str {
                match self { $($t::$v => $crate::i18n::tr($l)),+ }
            }
        }
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    Es,
    En,
}
choice!(Lang { Es => "Español", En => "English" });

impl Lang {
    fn code(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
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
choice!(Paper { A4 => "A4", A5 => "A5", Letter => "Carta" });

impl Paper {
    fn code(self) -> &'static str {
        match self {
            Paper::A4 => "a4",
            Paper::A5 => "a5",
            Paper::Letter => "us-letter",
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
choice!(Margins { Estrecho => "Estrechos", Normal => "Normales", Amplio => "Amplios" });

impl Margins {
    /// Margen en cm (en el libro, interior +0,5 y exterior −0,5).
    fn cm(self) -> f32 {
        match self {
            Margins::Estrecho => 2.0,
            Margins::Normal => 2.5,
            Margins::Amplio => 3.0,
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
choice!(DateMode { Today => "Hoy", Custom => "Personalizada", None => "Sin fecha" });

/// Interlineado (el `leading` de typst; el espacio entre párrafos crece con él).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Leading {
    #[default]
    Normal,
    OneHalf,
    Double,
}
choice!(Leading { Normal => "Normal", OneHalf => "1,5", Double => "Doble" });

impl Leading {
    /// `(leading, spacing)` o `None` para dejar los de typst.
    fn code(self) -> Option<(&'static str, &'static str)> {
        match self {
            Leading::Normal => None,
            Leading::OneHalf => Some(("1.1em", "1.6em")),
            Leading::Double => Some(("1.7em", "2.2em")),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HeaderMode {
    #[default]
    None,
    Title,
    Chapter,
    Custom,
}
choice!(HeaderMode {
    None => "Ninguno",
    Title => "Título",
    Chapter => "Capítulo actual",
    Custom => "Texto propio",
});

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PageNum {
    None,
    #[default]
    Center,
    Right,
    Outside,
}
choice!(PageNum {
    None => "Sin número",
    Center => "Centro",
    Right => "Derecha",
    Outside => "Exterior",
});

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PageFmt {
    #[default]
    Plain,
    OfTotal,
    Word,
}
choice!(PageFmt { Plain => "1", OfTotal => "1 / 12", Word => "Página 1" });

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BibStyle {
    #[default]
    None,
    Apa,
    Ieee,
    Chicago,
    Mla,
}
choice!(BibStyle {
    None => "Sin bibliografía",
    Apa => "APA",
    Ieee => "IEEE",
    Chicago => "Chicago (autor-fecha)",
    Mla => "MLA",
});

impl BibStyle {
    fn code(self) -> Option<&'static str> {
        match self {
            BibStyle::None => None,
            BibStyle::Apa => Some("apa"),
            BibStyle::Ieee => Some("ieee"),
            BibStyle::Chicago => Some("chicago-author-date"),
            BibStyle::Mla => Some("mla"),
        }
    }
}

/// Opciones del builder (también estado del formulario). Los campos que el
/// tipo no usa (`Kind::has`) se ignoran al generar.
#[derive(Clone, Debug)]
pub struct Options {
    pub kind: Kind,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub institution: String,
    pub course: String,
    pub date: DateMode,
    pub custom_date: String,
    pub abstract_text: String,
    pub lang: Lang,
    pub paper: Paper,
    pub margins: Margins,
    pub columns: u8,
    /// Familia; vacío = la de typst (Libertinus Serif).
    pub font: String,
    pub font_size: f32,
    pub leading: Leading,
    pub justify: bool,
    pub numbering: bool,
    pub header: HeaderMode,
    pub header_text: String,
    pub page_num: PageNum,
    pub page_fmt: PageFmt,
    pub toc: bool,
    pub bib: BibStyle,
    pub solutions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            kind: Kind::Article,
            title: "Título".to_string(),
            subtitle: String::new(),
            author: "Autor".to_string(),
            institution: String::new(),
            course: String::new(),
            date: DateMode::Today,
            custom_date: String::new(),
            abstract_text: String::new(),
            lang: Lang::Es,
            paper: Paper::A4,
            margins: Margins::Normal,
            columns: 1,
            font: String::new(),
            font_size: 11.0,
            leading: Leading::Normal,
            justify: true,
            numbering: true,
            header: HeaderMode::None,
            header_text: String::new(),
            page_num: PageNum::Center,
            page_fmt: PageFmt::Plain,
            toc: true,
            bib: BibStyle::None,
            solutions: true,
        }
    }
}

impl Options {
    /// Opciones por defecto de un tipo.
    #[cfg(test)]
    pub fn of(kind: Kind) -> Self {
        let mut o = Self::default();
        o.set_kind(kind);
        o
    }

    /// Cambia de tipo conservando los datos; encabezado, pie y tamaño
    /// pasan a los del tipo nuevo.
    pub fn set_kind(&mut self, kind: Kind) {
        self.kind = kind;
        self.header = kind.default_header();
        self.page_num = kind.default_page_num();
        if !kind.font_sizes().contains(&self.font_size) {
            self.font_size = kind.default_size();
        }
    }

    /// Ficheros que escribe `write_project`, en orden.
    pub fn files(&self) -> Vec<&'static str> {
        let mut v = vec![TEMPLATE_FILE, MAIN_FILE];
        if self.uses_bib() {
            v.push(BIB_FILE);
        }
        v
    }

    fn uses_bib(&self) -> bool {
        self.kind.has(Field::Bib) && self.bib.code().is_some()
    }

    /// Escribe la plantilla, `main.typ` y, si hace falta, `refs.bib` en
    /// `dir`. No sobrescribe: si alguno existe, no toca nada. Devuelve la
    /// ruta de `main.typ`.
    pub fn write_project(&self, dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
        let names = self.files();
        let taken: Vec<_> = names
            .iter()
            .filter(|n| dir.join(n).exists())
            .copied()
            .collect();
        if !taken.is_empty() {
            return Err(trf!("Ya existe {} en {}", taken.join(" y "), dir.display()));
        }
        let mut written = Vec::new();
        for name in names {
            let text = match name {
                TEMPLATE_FILE => self.build_template(),
                MAIN_FILE => self.build_main(),
                _ => codegen::SAMPLE_BIB.to_string(),
            };
            let p = dir.join(name);
            if let Err(e) = std::fs::write(&p, text) {
                for w in written {
                    let _ = std::fs::remove_file(w);
                }
                return Err(trf!("No se pudo escribir {}: {}", p.display(), e));
            }
            written.push(p);
        }
        Ok(dir.join(MAIN_FILE))
    }
}

/// Nombres de los ficheros que crea el builder.
pub const TEMPLATE_FILE: &str = "template.typ";
pub const MAIN_FILE: &str = "main.typ";
pub const BIB_FILE: &str = "refs.bib";

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

// ---------- Panel ----------

/// Panel lateral: tipo + formulario + crear. Crear escribe los ficheros en
/// la carpeta del proyecto vinculado (o en la que se elija) y abre `main.typ`.
pub fn panel(st: &mut State, ui: &mut egui::Ui) {
    let pal = theme::palette(ui.ctx());
    widgets::toolbar_frame(&pal).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::section_label(ui, &pal, tr("Plantillas"));
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
                    form(st, ui, &pal);
                });
        });
}

fn form(st: &mut State, ui: &mut egui::Ui, pal: &theme::Palette) {
    // Tipo.
    let kind = st.template.kind;
    let mut kind_pick = None;
    widgets::combo(
        ui,
        pal,
        RichText::new(format!("{}  {}", kind.icon(), kind.name())).small(),
        160.0,
        tr("Tipo de plantilla"),
        |ui| {
            ui.set_min_width(220.0);
            for k in Kind::ALL {
                if ui
                    .selectable_label(k == kind, format!("{}  {}", k.icon(), k.name()))
                    .on_hover_text(k.desc())
                    .clicked()
                {
                    kind_pick = Some(k);
                    ui.close();
                }
            }
        },
    );
    if let Some(k) = kind_pick {
        st.template.set_kind(k);
    }
    let kind = st.template.kind;
    ui.label(RichText::new(kind.desc()).small().color(pal.text_faint));

    // Proyecto vinculado (desde «Nuevo proyecto»): Crear escribe ahí sin
    // preguntar.
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
                .on_hover_text(tr("Desvincular: Crear preguntará la carpeta"))
                .clicked();
        });
    }
    if unlink {
        st.template_dir = None;
    }
    let linked = st.template_dir.is_some();
    let fonts = st.fmt.fonts.as_deref();
    let font_filter = &mut st.template_font_filter;
    let f = &mut st.template;
    let has = |x| kind.has(x);

    // Datos.
    ui.separator();
    widgets::section_label(ui, pal, tr("Datos"));
    field(ui, pal, tr("Título"), &mut f.title, tr("Título del trabajo"));
    if has(Field::Subtitle) {
        field(
            ui,
            pal,
            tr("Subtítulo"),
            &mut f.subtitle,
            tr("Vacío = sin subtítulo"),
        );
    }
    if has(Field::Course) {
        field(ui, pal, tr("Asignatura"), &mut f.course, tr("Cálculo I, tema 3…"));
    }
    let who = if kind == Kind::Notes {
        tr("Profesor")
    } else {
        tr("Autor")
    };
    field(ui, pal, who, &mut f.author, tr("Nombre"));
    if has(Field::Institution) {
        let hint = if kind == Kind::Book {
            tr("Editorial o institución")
        } else {
            tr("Universidad, colegio, iglesia…")
        };
        field(ui, pal, tr("Institución"), &mut f.institution, hint);
    }
    ui.columns(2, |c| {
        pick(&mut c[0], pal, tr("Fecha"), &mut f.date);
        pick(&mut c[1], pal, tr("Idioma"), &mut f.lang);
    });
    if f.date == DateMode::Custom {
        ui.add(
            egui::TextEdit::singleline(&mut f.custom_date)
                .hint_text(tr("9 de octubre de 2026"))
                .desired_width(f32::INFINITY),
        );
    }
    if has(Field::Abstract) {
        ui.label(RichText::new(tr("Resumen")).small().color(pal.text_dim));
        ui.add(
            egui::TextEdit::multiline(&mut f.abstract_text)
                .hint_text(tr("Vacío = sin resumen"))
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
    }

    // Página.
    if has(Field::Page) || has(Field::Columns) {
        ui.separator();
        widgets::section_label(ui, pal, tr("Página"));
        if has(Field::Page) {
            ui.columns(2, |c| {
                pick(&mut c[0], pal, tr("Papel"), &mut f.paper);
                pick(&mut c[1], pal, tr("Márgenes"), &mut f.margins);
            });
        }
        if has(Field::Columns) {
            ui.label(RichText::new(tr("Columnas")).small().color(pal.text_dim));
            ui.horizontal(|ui| {
                for n in [1u8, 2] {
                    ui.selectable_value(&mut f.columns, n, n.to_string());
                }
            });
        }
    }

    // Texto.
    ui.separator();
    widgets::section_label(ui, pal, tr("Texto"));
    ui.label(RichText::new(tr("Fuente")).small().color(pal.text_dim));
    font_combo(ui, pal, fonts, font_filter, &mut f.font);
    ui.columns(2, |c| {
        let size = format!("{} pt", fmt_pt(f.font_size));
        combo_row(&mut c[0], pal, tr("Tamaño"), &size, 110.0, |ui| {
            for &s in kind.font_sizes() {
                if ui
                    .selectable_label(f.font_size == s, format!("{} pt", fmt_pt(s)))
                    .clicked()
                {
                    f.font_size = s;
                    ui.close();
                }
            }
        });
        pick(&mut c[1], pal, tr("Interlineado"), &mut f.leading);
    });
    if has(Field::Justify) || has(Field::Numbering) {
        ui.columns(2, |c| {
            if has(Field::Justify) {
                c[0].checkbox(&mut f.justify, tr("Justificar"));
            }
            if has(Field::Numbering) {
                c[1].checkbox(&mut f.numbering, tr("Numerar apartados"));
            }
        });
    }

    // Encabezado y pie.
    ui.separator();
    widgets::section_label(ui, pal, tr("Encabezado y pie"));
    if has(Field::Header) {
        let modes: Vec<HeaderMode> = HeaderMode::ALL
            .iter()
            .copied()
            .filter(|m| *m != HeaderMode::Chapter || has(Field::Chapter))
            .collect();
        ui.label(RichText::new(tr("Encabezado")).small().color(pal.text_dim));
        pick_from(ui, pal, tr("Encabezado"), &mut f.header, &modes);
        if f.header == HeaderMode::Custom {
            ui.add(
                egui::TextEdit::singleline(&mut f.header_text)
                    .hint_text(tr("Texto del encabezado"))
                    .desired_width(f32::INFINITY),
            );
        }
    }
    ui.columns(2, |c| {
        pick(&mut c[0], pal, tr("Nº de página"), &mut f.page_num);
        if f.page_num != PageNum::None {
            pick(&mut c[1], pal, tr("Formato"), &mut f.page_fmt);
        }
    });

    // Estructura.
    ui.separator();
    widgets::section_label(ui, pal, tr("Estructura"));
    if has(Field::Toc) {
        let label = if kind == Kind::Slides {
            tr("Diapositiva de contenido")
        } else {
            tr("Índice")
        };
        ui.checkbox(&mut f.toc, label);
    }
    if has(Field::Solutions) {
        ui.checkbox(&mut f.solutions, tr("Mostrar soluciones"))
            .on_hover_text(tr("Se puede cambiar luego en main.typ (soluciones: true/false)"));
    }
    if has(Field::Bib) {
        ui.label(RichText::new(tr("Bibliografía")).small().color(pal.text_dim));
        pick_from(
            ui,
            pal,
            tr("Estilo de citas; crea refs.bib con una entrada de ejemplo"),
            &mut f.bib,
            BibStyle::ALL,
        );
    }

    ui.separator();
    let files = f.files().join(", ");
    if widgets::accent_button(ui, pal, &trf!("{}  Crear documento", icon::FILE_PLUS))
        .on_hover_text(if linked {
            trf!("Crea {} en el proyecto y abre {}", files, MAIN_FILE)
        } else {
            trf!("Elige una carpeta: crea {} y abre {}", files, MAIN_FILE)
        })
        .clicked()
    {
        st.actions
            .push(Action::NewFromTemplate(Box::new(st.template.clone())));
    }
}

/// Familia de fuente: lista del motor (precargada por la barra de formato)
/// con filtro; la primera opción deja la de typst.
fn font_combo(
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    fonts: Option<&[String]>,
    filter: &mut String,
    font: &mut String,
) {
    let shown = if font.is_empty() {
        tr("Predeterminada (Libertinus Serif)")
    } else {
        font.as_str()
    };
    widgets::combo(
        ui,
        pal,
        RichText::new(format!("{}  {shown}", icon::TEXT_AA)).small(),
        200.0,
        tr("Familia de fuente"),
        |ui| {
            ui.set_min_width(240.0);
            let Some(fonts) = fonts else {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(tr("Cargando fuentes…")).small());
                });
                return;
            };
            ui.add(
                egui::TextEdit::singleline(filter)
                    .hint_text(tr("Filtrar…"))
                    .desired_width(f32::INFINITY),
            );
            if ui
                .selectable_label(font.is_empty(), tr("Predeterminada (Libertinus Serif)"))
                .clicked()
            {
                font.clear();
                ui.close();
            }
            let q = filter.to_lowercase();
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for name in fonts
                        .iter()
                        .filter(|n| q.is_empty() || n.to_lowercase().contains(&q))
                    {
                        if ui.selectable_label(*name == *font, name).clicked() {
                            *font = name.clone();
                            ui.close();
                        }
                    }
                });
        },
    );
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

/// Desplegable con etiqueta para un `Choice`.
fn pick<T: Choice>(ui: &mut egui::Ui, pal: &theme::Palette, label: &str, value: &mut T) {
    ui.label(RichText::new(label).small().color(pal.text_dim));
    pick_from(ui, pal, label, value, T::ALL);
}

fn pick_from<T: Choice>(
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    tip: &str,
    value: &mut T,
    all: &[T],
) {
    widgets::combo(
        ui,
        pal,
        RichText::new(value.label()).small(),
        110.0,
        tip,
        |ui| {
            ui.set_min_width(140.0);
            for &v in all {
                if ui.selectable_label(*value == v, v.label()).clicked() {
                    *value = v;
                    ui.close();
                }
            }
        },
    );
}

/// Desplegable con etiqueta y contenido libre.
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
