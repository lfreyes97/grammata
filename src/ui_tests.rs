//! Pruebas de la UI con egui_kittest (teclado/ratón simulados, render wgpu).
//!
//! Necesitan `typst` en el PATH y GPU, así que van con `#[ignore]`:
//!   cargo test -- --ignored
//! Con `SHOT_DIR=/ruta` guardan capturas PNG de cada paso.

use super::*;
use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;

fn harness() -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1360.0, 840.0))
        .wgpu()
        .build_eframe(|cc| App::new(cc))
}

/// Ejecuta frames (con tiempo real, por los hilos de compilación/render)
/// hasta que se cumple `cond`.
fn wait(h: &mut Harness<'_, App>, what: &str, mut cond: impl FnMut(&App) -> bool) {
    for _ in 0..600 {
        h.step();
        if cond(h.state()) {
            h.run_steps(3);
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("tiempo agotado esperando: {what}");
}

fn shot(h: &mut Harness<'_, App>, name: &str) {
    let Ok(dir) = std::env::var("SHOT_DIR") else {
        return;
    };
    h.run_steps(2);
    let img = h.render().expect("render wgpu");
    img.save(Path::new(&dir).join(format!("{name}.png")))
        .expect("guardar captura");
}

fn cur_text(h: &Harness<'_, App>) -> String {
    h.state().st.docs.cur().text.clone()
}

fn put_cursor_at_end(h: &mut Harness<'_, App>) {
    let st = &mut h.state_mut().st;
    let doc = st.docs.cur();
    let n = doc.text.chars().count();
    st.editor.goto = Some(Goto {
        doc: doc.id,
        start: n,
        end: n,
        focus: true,
    });
    h.run_steps(3);
}

fn rendered_pages(app: &App) -> bool {
    app.st.preview.page_count() > 0 && app.st.compiler.in_flight.is_none()
}

#[test]
#[ignore = "necesita GPU"]
fn recorrido_de_la_ui() {
    let tmp = std::env::temp_dir().join(format!("grammata-test-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // SAFETY: único test que toca el entorno; se fija antes de crear la app.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };

    let mut h = harness();
    wait(&mut h, "primera compilación", rendered_pages);
    wait(&mut h, "render de páginas", |a| {
        a.st.preview.fresh_pages() >= 1
    });
    shot(&mut h, "01-inicio");
    // Ajuste de línea con números de línea (antes eran incompatibles).
    h.state_mut().st.actions.push(Action::ToggleWrap);
    put_cursor_at_end(&mut h);
    shot(&mut h, "01b-ajuste");
    h.state_mut().st.actions.push(Action::ToggleWrap);
    h.run_steps(2);

    // Autocompletado: `#ta` + Tab.
    put_cursor_at_end(&mut h);
    h.key_press(Key::Enter);
    h.event(Event::Text("#ta".into()));
    h.run_steps(4);
    shot(&mut h, "02-autocompletar");
    h.key_press(Key::Tab);
    h.run_steps(3);
    assert!(
        cur_text(&h).ends_with("#table(\n  \n)"),
        "no se insertó el snippet:\n{}",
        cur_text(&h)
    );

    // Error de compilación → banner + panel Problemas.
    h.key_press(Key::Enter);
    h.event(Event::Text("#funcion_inexistente(1)".into()));
    wait(&mut h, "diagnóstico", |a| {
        let d = a.st.docs.cur();
        a.st.compiler
            .built
            .get(&d.id)
            .is_some_and(|b| b.key.rev == d.rev && !b.diagnostics.is_empty())
    });
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Problems));
    h.run_steps(4);
    assert!(layout::has_panel(&h.state().dock, Panel::Problems));
    shot(&mut h, "03-errores");

    // Clic en el problema → salta a la línea del error.
    let diag_line = {
        let a = h.state();
        a.st.compiler.built[&a.st.docs.cur_id()].diagnostics[0]
            .line
            .unwrap()
    };
    h.state_mut().st.actions.push(Action::JumpTo {
        line: diag_line,
        col: 1,
    });
    h.run_steps(4);
    let (l, _) = {
        let a = h.state();
        text::char_to_line_col(&a.st.docs.cur().text, a.st.editor.cursor.unwrap())
    };
    assert_eq!(l, diag_line);

    // Buscar (Ctrl+F) con coincidencias resaltadas.
    h.key_press_modifiers(Modifiers::CTRL, Key::F);
    h.run_steps(2);
    h.event(Event::Text("Formato".into()));
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(4);
    assert!(
        h.state().st.editor.find.current.is_some(),
        "Enter no seleccionó coincidencia"
    );
    shot(&mut h, "04-buscar");
    h.key_press(Key::Escape);
    h.run_steps(3);
    assert!(!h.state().st.editor.find.open);
    assert_eq!(
        h.state().st.editor.focused,
        Some(h.state().st.docs.cur_id()),
        "Esc debería devolver el foco al editor"
    );

    // Tema claro + documento de varias páginas abierto desde disco.
    let multi = tmp.join("largo.typ");
    std::fs::write(&multi, "#set page(paper: \"a5\")\n= Largo\n#for i in range(1, 7) [\n  == Sección #i\n  #lorem(220)\n]\n").unwrap();
    h.state_mut().st.actions.push(Action::ToggleTheme);
    h.state_mut()
        .st
        .actions
        .push(Action::OpenPath(multi.clone()));
    wait(&mut h, "doc multipágina", |a| {
        a.st.docs.cur().path.as_deref() == Some(multi.as_path()) && a.st.preview.page_count() >= 3
    });
    wait(&mut h, "render de páginas", |a| {
        a.st.preview.fresh_pages() >= 1
    });
    shot(&mut h, "05-claro-multipagina");

    // Zoom manual al 150 %.
    {
        let p = &mut h.state_mut().st.prefs;
        p.fit = crate::session::FitMode::Manual;
        p.zoom = 1.5;
    }
    for _ in 0..30 {
        h.step();
        std::thread::sleep(Duration::from_millis(15));
    }
    shot(&mut h, "06-zoom");

    // Cerrar con cambios → confirmación.
    put_cursor_at_end(&mut h);
    h.event(Event::Text("x".into()));
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::CTRL, Key::W);
    h.run_steps(3);
    assert!(matches!(h.state().st.modal, Some(Modal::ConfirmClose(_))));
    shot(&mut h, "07-confirmar-cierre");
    h.state_mut().st.modal = None;

    // Diseño «solo escritura» y vuelta al clásico.
    h.state_mut()
        .st
        .actions
        .push(Action::ApplyPreset(Preset::Writing));
    h.run_steps(3);
    assert!(!layout::has_panel(&h.state().dock, Panel::Preview));
    shot(&mut h, "08-escritura");
    h.state_mut()
        .st
        .actions
        .push(Action::ApplyPreset(Preset::Classic));
    h.run_steps(3);
    assert!(layout::has_panel(&h.state().dock, Panel::Preview));

    // Panel de símbolos: fuera de una ecuación envuelve en `$…$`, dentro
    // inserta el nombre separado del identificador vecino.
    h.state_mut().st.actions.push(Action::NewDoc);
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Symbols));
    h.run_steps(3);
    h.state_mut()
        .st
        .actions
        .push(Action::InsertMath("alpha".into()));
    h.run_steps(3);
    assert_eq!(cur_text(&h), "$alpha$");
    // El cursor quedó dentro de la ecuación (antes del `$` final).
    h.state_mut()
        .st
        .actions
        .push(Action::InsertMath("arrow.r".into()));
    h.run_steps(3);
    assert_eq!(cur_text(&h), "$alpha arrow.r$");
    // Estructura sin selección: inserta relleno válido y lo deja seleccionado.
    h.state_mut()
        .st
        .actions
        .push(Action::InsertMath("frac(${sel:a}, ${b})".into()));
    h.run_steps(3);
    assert_eq!(cur_text(&h), "$alpha arrow.r frac(a, b)$");
    assert_eq!(
        h.state().st.editor.selection,
        Some((20, 21)),
        "debería quedar seleccionada la «a»"
    );
    shot(&mut h, "09-simbolos");

    // Con selección, la estructura la envuelve: selecciona «alpha» → abs(alpha).
    {
        let st = &mut h.state_mut().st;
        let id = st.docs.cur_id();
        st.editor.goto = Some(Goto {
            doc: id,
            start: 1,
            end: 6,
            focus: true,
        });
    }
    h.run_steps(3);
    h.state_mut()
        .st
        .actions
        .push(Action::InsertMath("abs(${sel:x})".into()));
    h.run_steps(3);
    assert_eq!(cur_text(&h), "$abs(alpha) arrow.r frac(a, b)$");

    // Snippet de texto que envuelve la selección.
    h.state_mut().st.actions.push(Action::NewDoc);
    h.run_steps(2);
    put_cursor_at_end(&mut h);
    h.event(Event::Text("hola mundo".into()));
    h.run_steps(2);
    {
        let st = &mut h.state_mut().st;
        let id = st.docs.cur_id();
        st.editor.goto = Some(Goto {
            doc: id,
            start: 5,
            end: 10,
            focus: true,
        });
    }
    h.run_steps(3);
    h.state_mut()
        .st
        .actions
        .push(Action::InsertSnippet("*${sel:texto}*".into()));
    h.run_steps(3);
    assert_eq!(cur_text(&h), "hola *mundo*");

    // Guardar la selección como snippet propio.
    {
        let st = &mut h.state_mut().st;
        let id = st.docs.cur_id();
        st.editor.goto = Some(Goto {
            doc: id,
            start: 5,
            end: 12,
            focus: true,
        });
    }
    h.run_steps(3);
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Snippets));
    h.state_mut().st.actions.push(Action::NewSnippet);
    h.run_steps(3);
    match &mut h.state_mut().st.modal {
        Some(Modal::EditSnippet(e)) => {
            assert_eq!(e.body, "*mundo*");
            e.name = "Mundo en negrita".into();
        }
        _ => panic!("no se abrió el diálogo de snippet"),
    }
    shot(&mut h, "10-nuevo-snippet");
    h.state_mut().finish_snippet_edit();
    h.run_steps(3);
    assert_eq!(
        snippets::load_user().len(),
        1,
        "snippets.json no se escribió"
    );
    shot(&mut h, "11-snippets");

    // La sesión se guarda y se puede volver a leer.
    h.state_mut().persist();
    let s = session::load().expect("sesión legible");
    assert_eq!(s.tabs.len(), h.state().st.docs.list.len());
    assert!(s.layout.is_some());
    assert_eq!(s.recent_symbols.len(), 4);
    assert_eq!(s.recent_symbols[0], "abs(${sel:x})");
}

/// El esquema navega al búfer: clic en una entrada → cursor en su línea
/// (sin mover la vista previa).
#[test]
#[ignore = "necesita GPU"]
fn esquema_navega_al_buffer() {
    use egui_kittest::kittest::Queryable;
    let tmp = std::env::temp_dir().join(format!("grammata-test-outline-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // SAFETY: único test que toca el entorno; se fija antes de crear la app.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };

    let mut h = harness();
    h.run_steps(5);
    assert!(layout::has_panel(&h.state().dock, Panel::Outline));
    // El documento de ejemplo trae "= Hola Typst" y "== Formato".
    h.get_by_label_contains("Formato").click();
    h.run_steps(5);
    let pages_before = h.state().st.preview.page_count();
    let (l, _) = {
        let a = h.state();
        let c =
            a.st.editor
                .cursor
                .expect("el clic en el esquema no movió el cursor");
        text::char_to_line_col(&a.st.docs.cur().text, c)
    };
    let esperado = {
        let a = h.state();
        a.st.docs
            .cur()
            .text
            .lines()
            .position(|l| l.starts_with("== Formato"))
            .unwrap()
            + 1
    };
    assert_eq!(
        l, esperado,
        "el clic en el esquema no llevó al cursor a su línea"
    );
    assert_eq!(
        h.state().st.preview.page_count(),
        pages_before,
        "el esquema no debería cambiar la vista previa"
    );
}

/// Gestor de archivos: plegar oculta hijos, desplegar y clicar abre en el editor.
#[test]
#[ignore = "necesita GPU"]
fn archivos_pliegan_y_abren() {
    use egui_kittest::kittest::Queryable;
    let tmp = std::env::temp_dir().join(format!("grammata-test-fm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("proyecto");
    std::fs::create_dir_all(root.join("capitulos")).unwrap();
    std::fs::write(root.join("main.typ"), "= Principal\n").unwrap();
    std::fs::write(root.join("capitulos").join("uno.typ"), "= Uno\n").unwrap();
    std::fs::create_dir_all(tmp.join("cfg")).unwrap();
    // SAFETY: único test que toca el entorno; se fija antes de crear la app.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let mut h = harness();
    h.run_steps(5);
    h.state_mut().st.prefs.files_root = Some(root.clone());
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Files));
    h.run_steps(5);
    // El árbol se escanea en otro hilo.
    std::thread::sleep(std::time::Duration::from_millis(200));
    h.run_steps(5);
    // "capitulos" plegada esconde a "uno.typ": se despliega y se abre.
    assert!(
        h.query_by_label_contains("uno.typ").is_none(),
        "plegada debería ocultar a sus hijos"
    );
    h.get_by_label_contains("capitulos").click();
    h.run_steps(5);
    h.get_by_label_contains("uno.typ").click();
    h.run_steps(5);
    let opened = h.state().st.docs.cur().path.clone();
    assert!(
        opened.as_deref().is_some_and(|p| p.ends_with("uno.typ")),
        "el clic no abrió uno.typ: {opened:?}"
    );
    assert_eq!(cur_text(&h), "= Uno\n");
}

/// Nuevo proyecto: vacío abre `main.typ`; artículo vincula el builder, que
/// escribe en la carpeta sin preguntar.
#[test]
#[ignore = "necesita GPU"]
fn nuevo_proyecto_vacio_y_articulo() {
    use egui_kittest::kittest::Queryable;
    let tmp = std::env::temp_dir().join(format!("grammata-test-np-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("cfg")).unwrap();
    // SAFETY: único test que toca el entorno; se fija antes de crear la app.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let mut h = harness();
    h.run_steps(5);
    let crear = |h: &mut Harness<'_, App>, name: &str, kind: crate::project::Kind| {
        h.state_mut().st.actions.push(Action::NewProject);
        h.run_steps(3);
        if let Some(Modal::NewProject(np)) = &mut h.state_mut().st.modal {
            np.name = name.into();
            np.parent = tmp.clone();
            np.kind = kind;
        } else {
            panic!("no se abrió el diálogo");
        }
        h.run_steps(2);
        shot(h, &format!("nuevo-proyecto-{name}"));
        h.get_by_label("Crear").click();
        h.run_steps(5);
        assert!(h.state().st.modal.is_none(), "el diálogo sigue abierto");
    };
    crear(&mut h, "vacio", crate::project::Kind::Empty);
    let dir = std::fs::canonicalize(tmp.join("vacio")).unwrap();
    assert_eq!(
        h.state().st.prefs.files_root.as_deref(),
        Some(tmp.join("vacio").as_path())
    );
    assert_eq!(
        h.state().st.docs.cur().path.as_deref(),
        Some(dir.join("main.typ").as_path())
    );

    crear(
        &mut h,
        "art",
        crate::project::Kind::Template(crate::templates::Kind::Article),
    );
    assert_eq!(
        h.state().st.template_dir.as_deref(),
        Some(tmp.join("art").as_path())
    );
    // El formulario es largo: el botón queda bajo el pliegue.
    h.get_by_label_contains("Crear documento").scroll_to_me();
    h.run_steps(3);
    shot(&mut h, "plantillas-panel");
    h.get_by_label_contains("Crear documento").click();
    h.run_steps(5);
    let art = std::fs::canonicalize(tmp.join("art")).unwrap();
    assert!(art.join("template.typ").is_file(), "falta template.typ");
    assert_eq!(
        h.state().st.docs.cur().path.as_deref(),
        Some(art.join("main.typ").as_path())
    );
    assert!(cur_text(&h).starts_with("#import \"template.typ\""));
    assert!(
        h.state().st.template_dir.is_none(),
        "el builder sigue vinculado"
    );
}

/// Borrador: un snippet de la biblioteca se inserta en su buffer.
#[test]
#[ignore = "necesita GPU"]
fn borrador_inserta_snippet() {
    use egui_kittest::kittest::Queryable;
    let tmp = std::env::temp_dir().join(format!("grammata-test-ss-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("cfg")).unwrap();
    // SAFETY: único test que toca el entorno; se fija antes de crear la app.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let mut h = harness();
    h.run_steps(5);
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Scratch));
    h.run_steps(5);
    // Estructura de Símbolos (botón «a⁄b»): llega sin `$`.
    h.get_by_label("a⁄b").click();
    h.run_steps(5);
    let t = h.state().st.scratch.text.clone();
    assert_eq!(t, "frac(a, b)", "la estructura no llegó al borrador");
    shot(&mut h, "borrador-estructuras");
    // En modo ecuación el menú solo ofrece grupos de matemáticas.
    h.get_by_label_contains("Añadir snippet").click();
    h.run_steps(5);
    assert!(h.query_by_label_contains("Listas").is_none());
    // Ni, dentro de un grupo, lo que no es ecuación («Numerar ecuaciones»
    // es un `#set`).
    h.get_by_label_contains("Ecuaciones").hover();
    h.run_steps(3);
    assert!(h.query_by_label_contains("Numerar ecuaciones").is_none());
    assert!(h.query_by_label_contains("Ecuaciones alineadas").is_some());
    // Un snippet con sus propios `$…$` no se duplica en modo ecuación.
    h.get_by_label_contains("Álgebra y cálculo").click();
    h.run_steps(5);
    h.get_by_label_contains("Sumatorio").click();
    h.run_steps(5);
    let t = h.state().st.scratch.text.clone();
    assert!(
        t.contains("sum_") && !t.contains('$'),
        "doble $$ en el borrador: {t:?}"
    );
}

/// Borrador: un snippet de cálculo se previsualiza sin error en modo ecuación.
#[test]
#[ignore = "necesita GPU"]
fn borrador_snippet_calc() {
    use egui_kittest::kittest::Queryable;
    let tmp = std::env::temp_dir().join(format!("grammata-test-cc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("cfg")).unwrap();
    // SAFETY: único test que toca el entorno; se fija antes de crear la app.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let mut h = harness();
    h.run_steps(5);
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Scratch));
    h.run_steps(5);
    h.get_by_label_contains("Añadir snippet").click();
    h.run_steps(5);
    h.get_by_label_contains("Calcular con typst").click();
    h.run_steps(5);
    h.get_by_label_contains("Calcular discriminante").click();
    h.run_steps(4);
    std::thread::sleep(std::time::Duration::from_millis(600));
    h.run_steps(6);
    let st = &h.state().st.scratch;
    assert!(st.text.contains("calc.pow"), "no llegó: {:?}", st.text);
    assert!(st.error.is_none(), "error en preview: {:?}", st.error);
}

/// Cada símbolo, estructura y snippet de serie compila con el motor embebido.
#[test]
#[ignore = "compila ~100 documentos (lento)"]
fn plantillas_y_simbolos_compilan() {
    let dir = std::env::temp_dir().join(format!("grammata-snip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Recursos a los que apuntan algunos snippets.
    let out = crate::engine::export_pages(
        "#set page(width: 1pt, height: 1pt, margin: 0pt)",
        &dir,
        "main.typ",
        &dir.join("imagen.png"),
        "png",
        Some(72),
        &[],
    );
    assert!(out.is_ok(), "{out:?}");
    std::fs::write(
        dir.join("refs.bib"),
        "@book{clave, title={T}, author={A}, year={2020}}\n",
    )
    .unwrap();

    let check = |what: &str, src: String| {
        let out = crate::engine::compile_pdf(&src, &dir, "main.typ");
        assert!(
            out.pdf.is_some_and(|p| p.starts_with(b"%PDF")),
            "{what} no compila:\n{src}\n---\n{:?}",
            out.diags
        );
    };
    // Símbolos y estructuras: todos juntos en una ecuación.
    // Sin selección (relleno) y con selección (envolviendo).
    let math: Vec<String> = symbols::all()
        .flat_map(|s| {
            [
                snippets::expand(s.insert, "").0,
                snippets::expand(s.insert, "y").0,
            ]
        })
        .collect();
    check("símbolos", format!("$ {} $", math.join(" quad ")));
    for g in snippets::BUILTIN {
        for s in g.items {
            let (mut src, _, _) = snippets::expand(s.body, "x");
            if src.contains("@etiqueta") {
                src.push_str("\n#figure([x]) <etiqueta>");
            }
            if src.contains("cite(") {
                src.push_str("\n#bibliography(\"refs.bib\")");
            }
            check(s.name, src);
            check(
                s.name,
                snippets::expand(s.body, "").0
                    + "\n#figure([x]) <etiqueta>\n#bibliography(\"refs.bib\")",
            );
        }
    }
}

/// Un snippet que solo son ecuaciones, insertado dentro de una ecuación,
/// entra sin sus `$` (nada de ecuaciones anidadas) y con el relleno
/// seleccionado.
#[test]
#[ignore = "necesita GPU"]
fn snippet_de_ecuacion_dentro_de_ecuacion() {
    let tmp = std::env::temp_dir().join(format!("grammata-test-se-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("cfg")).unwrap();
    // SAFETY: único test que toca el entorno; se fija antes de crear la app.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let mut h = harness();
    h.run_steps(5);
    h.state_mut().st.actions.push(Action::NewDoc);
    h.state_mut()
        .st
        .actions
        .push(Action::InsertMath("alpha".into()));
    h.run_steps(3);
    assert_eq!(cur_text(&h), "$alpha$");
    h.state_mut().st.actions.push(Action::InsertSnippet(
        "$ lim_(${x} -> ${0}) ${f(x)} $".into(),
    ));
    h.run_steps(3);
    assert_eq!(cur_text(&h), "$alpha lim_(x -> 0) f(x)$");
    let (a, b) = h.state().st.editor.selection.expect("sin selección");
    assert_eq!(
        cur_text(&h).chars().skip(a).take(b - a).collect::<String>(),
        "x",
        "debería quedar seleccionada la «x»"
    );
    // Fuera de una ecuación, el snippet entra tal cual.
    h.state_mut().st.actions.push(Action::NewDoc);
    h.state_mut().st.actions.push(Action::InsertSnippet(
        "$ lim_(${x} -> ${0}) ${f(x)} $".into(),
    ));
    h.run_steps(3);
    assert_eq!(cur_text(&h), "$ lim_(x -> 0) f(x) $");
}

/// Borrador: carga la ecuación del cursor, Reemplazar la sustituye en el
/// documento y, si el documento cambió entretanto, no pisa nada.
#[test]
#[ignore = "necesita GPU"]
fn borrador_edita_la_ecuacion_del_cursor() {
    use egui_kittest::kittest::Queryable;
    let tmp = std::env::temp_dir().join(format!("grammata-test-be-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("cfg")).unwrap();
    // SAFETY: único test que toca el entorno; se fija antes de crear la app.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let mut h = harness();
    h.run_steps(5);
    h.state_mut().st.actions.push(Action::NewDoc);
    h.state_mut()
        .st
        .actions
        .push(Action::InsertSnippet("Sea $x + 1$ fin".into()));
    h.run_steps(3);
    h.state_mut()
        .st
        .actions
        .push(Action::Select { start: 6, end: 6 });
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Scratch));
    h.run_steps(5);
    h.get_by_label_contains("Editar la ecuación del cursor")
        .click();
    h.run_steps(3);
    {
        let s = &h.state().st.scratch;
        assert_eq!(s.text, "x + 1");
        assert!(!s.block, "era en línea");
        assert!(s.editing.is_some());
    }
    shot(&mut h, "borrador-editando");
    h.state_mut().st.scratch.text = "x + 2".into();
    h.run_steps(2);
    h.get_by_label_contains("Reemplazar en el documento")
        .click();
    h.run_steps(3);
    assert_eq!(cur_text(&h), "Sea $x + 2$ fin");
    // Sigue editando la misma: otro reemplazo también funciona.
    h.state_mut().st.scratch.block = true;
    h.run_steps(2);
    h.get_by_label_contains("Reemplazar en el documento")
        .click();
    h.run_steps(3);
    assert_eq!(cur_text(&h), "Sea $ x + 2 $ fin");
    // El documento cambia por fuera: la ecuación se busca donde quedó.
    {
        let d = h.state_mut().st.docs.cur_mut();
        d.text.insert_str(0, "Hola. ");
        d.touch();
    }
    h.state_mut().st.scratch.text = "y".into();
    h.run_steps(2);
    h.get_by_label_contains("Reemplazar en el documento")
        .click();
    h.run_steps(3);
    assert_eq!(cur_text(&h), "Hola. Sea $ y $ fin");
    // Si ya no está, no toca nada y deja de editar.
    {
        let d = h.state_mut().st.docs.cur_mut();
        d.text = "Nada".into();
        d.touch();
    }
    h.run_steps(2);
    h.get_by_label_contains("Reemplazar en el documento")
        .click();
    h.run_steps(3);
    assert_eq!(cur_text(&h), "Nada");
    assert!(h.state().st.scratch.editing.is_none());
}

/// Biblioteca del proyecto: se crea con ejemplos y una función insertada
/// trae su `#import` (una sola vez) y compila.
#[test]
#[ignore = "necesita GPU"]
fn biblioteca_de_snippets_del_proyecto() {
    use egui_kittest::kittest::Queryable;
    let tmp = std::env::temp_dir().join(format!("grammata-test-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("cfg")).unwrap();
    std::fs::create_dir_all(tmp.join("proj")).unwrap();
    let main = tmp.join("proj").join("main.typ");
    std::fs::write(&main, "= Hola\n").unwrap();
    // SAFETY: único test que toca el entorno; se fija antes de crear la app.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let mut h = harness();
    h.run_steps(5);
    h.state_mut()
        .st
        .actions
        .push(Action::OpenPath(main.clone()));
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Snippets));
    h.run_steps(5);
    h.get_by_label_contains("Crear snippets.typ del proyecto")
        .click();
    h.run_steps(5);
    let lib = tmp.join("proj").join(crate::snippet_lib::FILE);
    assert!(lib.is_file(), "no se creó la biblioteca");
    // Vuelve a main.typ con el cursor al final.
    h.state_mut()
        .st
        .actions
        .push(Action::OpenPath(main.clone()));
    h.run_steps(3);
    h.state_mut()
        .st
        .actions
        .push(Action::Select { start: 7, end: 7 });
    h.run_steps(5);
    shot(&mut h, "snippets-biblioteca");
    h.get_by_label_contains("Caja de aviso con título").click();
    h.run_steps(3);
    let want =
        "#import \"snippets.typ\": *\n= Hola\n#aviso(titulo: \"Aviso\", color: orange)[texto]";
    assert_eq!(cur_text(&h), want);
    // Otra vez: sin segundo `#import`.
    h.state_mut().st.actions.push(Action::Select {
        start: want.chars().count(),
        end: want.chars().count(),
    });
    h.run_steps(3);
    h.get_by_label_contains("Ley de Ohm").click();
    h.run_steps(3);
    let text = cur_text(&h);
    assert_eq!(text.matches("#import").count(), 1, "{text}");
    assert!(text.ends_with("$ V = I R $"), "{text}");
    let out = crate::engine::compile_pdf(&text, &tmp.join("proj"), "main.typ");
    assert!(out.pdf.is_some(), "{:?}", out.diags);
}

/// Vista previa fijada en `main.typ`: al editar el capítulo incluido, se
/// autoguarda y el principal se recompila (y se sigue mostrando él).
#[test]
#[ignore = "necesita GPU"]
fn fijar_vista_previa_y_autoguardado() {
    let tmp = std::env::temp_dir().join(format!("grammata-test-pin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let main = tmp.join("main.typ");
    let cap = tmp.join("cap.typ");
    std::fs::write(&main, "= Libro\n#include \"cap.typ\"\n").unwrap();
    std::fs::write(&cap, "Capítulo uno.\n").unwrap();
    // SAFETY: como los demás tests de UI, aísla la sesión antes de crear la
    // app (sin esto, la sesión real del usuario acaba con estas pestañas).
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let svg = tmp.join("figura.svg");
    std::fs::write(
        &svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60"><circle cx="60" cy="30" r="25" fill="teal"/></svg>"#,
    )
    .unwrap();
    let mut h = harness();
    h.run_steps(5);
    h.state_mut()
        .st
        .actions
        .push(Action::OpenPath(main.clone()));
    h.run_steps(3);
    let main_id = h.state().st.docs.cur_id();
    h.state_mut().st.actions.push(Action::TogglePin);
    h.state_mut().st.actions.push(Action::OpenPath(cap.clone()));
    h.run_steps(3);
    assert_ne!(h.state().st.docs.cur_id(), main_id);
    wait(&mut h, "compila main fijado", |a| {
        a.st.preview.loaded.is_some_and(|k| k.doc == main_id) && rendered_pages(a)
    });
    // Edición del capítulo: autoguardado → sube la generación de disco →
    // el principal se recompila.
    {
        let d = h.state_mut().st.docs.cur_mut();
        d.text.push_str("Añadido al capítulo.\n");
        d.touch();
    }
    wait(&mut h, "autoguardado y recompilación", |a| {
        a.st.disk_gen > 0
            && a.st
                .preview
                .loaded
                .is_some_and(|k| k.doc == main_id && k.disk == a.st.disk_gen)
            && rendered_pages(a)
    });
    assert!(std::fs::read_to_string(&cap).unwrap().contains("Añadido"));
    assert!(!h.state().st.docs.cur().modified());
    shot(&mut h, "preview-fijado");
    // Visor de imágenes: el SVG se abre en su pestaña y se renderiza.
    h.state_mut().st.actions.push(Action::OpenPath(svg.clone()));
    wait(&mut h, "imagen renderizada", |a| {
        a.st.images.list.first().is_some_and(|t| !t_loading(t))
    });
    shot(&mut h, "visor-imagen");
    // Insertar #image en el documento actual (el capítulo).
    let p = svg.canonicalize().unwrap();
    h.state_mut().st.actions.push(Action::InsertImage(p));
    h.run_steps(3);
    assert!(
        cur_text(&h).contains("#image(\"figura.svg\")"),
        "{}",
        cur_text(&h)
    );
    // Ajuste de línea (activo por defecto) con sangría colgante.
    {
        let d = h.state_mut().st.docs.cur_mut();
        d.text.push_str("\n- Una viñeta larga que se parte en varias filas para ver que las continuaciones quedan alineadas bajo el texto y no en el margen.\n  - Y otra anidada, también bastante larga, para comprobar que respeta la sangría de su nivel.\n");
        d.touch();
    }
    h.state_mut().st.actions.push(Action::OpenPath(cap.clone()));
    h.run_steps(4);
    assert!(h.state().st.prefs.wrap);
    shot(&mut h, "ajuste-sangria");
}

fn t_loading(t: &crate::image_view::ImageTab) -> bool {
    t.loading()
}

#[test]
#[ignore = "necesita GPU"]
fn dialogo_de_exportacion() {
    // SAFETY: como los demás tests de UI, aísla la sesión antes de crear la app.
    let tmp = std::env::temp_dir().join(format!("grammata-exp-{}", std::process::id()));
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let mut h = harness();
    h.run_steps(3);
    h.state_mut().st.actions.push(Action::Export("png".into()));
    h.run_steps(3);
    let Some(Modal::Export(d)) = &mut h.state_mut().st.modal else {
        panic!("el diálogo de exportación debía abrirse");
    };
    assert_eq!((d.fmt.as_str(), d.ppi), ("png", 144));
    d.total = 6;
    d.pages = "2-3, 5".into();
    h.run_steps(3);
    shot(&mut h, "exportar-png");
    if let Some(Modal::Export(d)) = &mut h.state_mut().st.modal {
        d.pages = "9".into();
    }
    h.run_steps(3);
    shot(&mut h, "exportar-error");
    // Un rango inválido no exporta (el botón está desactivado).
    assert!(h.state().st.actions.is_empty());
    assert!(matches!(h.state().st.modal, Some(Modal::Export(_))));
}

/// Plugin de ejemplo: el formulario de su plantilla crea un documento que
/// compila contra el paquete del plugin, y su tema se aplica.
#[test]
#[ignore]
fn plugin_formulario_y_tema() {
    use egui_kittest::kittest::Queryable;
    let tmp = std::env::temp_dir().join(format!("grammata-test-plug-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("cfg")).unwrap();
    // SAFETY: se fija antes de crear la app; la sesión y los plugins del
    // usuario no se tocan.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let mut h = harness();
    h.run_steps(5);
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Plugins));
    h.run_steps(5);
    h.get_by_label_contains("Ejemplo: paquete").click();
    h.run_steps(5);
    assert!(h.state().st.docs.cur().fname().contains("plugin.toml"));
    h.get_by_label_contains("Tarjeta de cita").click();
    h.run_steps(5);
    shot(&mut h, "plugins-formulario");
    h.get_by_label_contains("Nuevo documento").click();
    h.run_steps(5);
    let text = h.state().st.docs.cur().text.clone();
    assert!(
        text.starts_with("#import \"@local/tarjetas:0.1.0\": tarjeta") && !text.contains("autor:"),
        "{text}"
    );
    wait(&mut h, "compilar la tarjeta", |a| {
        a.st.compiler.is_built(a.st.main_key())
    });
    let key = h.state().st.main_key();
    let built = &h.state().st.compiler.built[&key.doc];
    assert!(built.diagnostics.is_empty(), "la tarjeta no compila");
    shot(&mut h, "plugins-documento");
    h.get_by_label("Sepia").click();
    h.run_steps(5);
    assert_eq!(h.state().st.prefs.theme.as_deref(), Some("ejemplo/Sepia"));
    assert!(!h.state().st.prefs.dark);
    shot(&mut h, "plugins-tema");
    h.state_mut().st.actions.push(Action::SetTheme(None));
    h.run_steps(3);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Plugin de Pandoc: importa un Markdown como documento nuevo (el programa
/// corre en un hilo) y un comando de plugin transforma la selección.
#[test]
#[ignore]
fn plugin_importa_markdown_y_ejecuta_comando() {
    use egui_kittest::kittest::Queryable;
    if std::process::Command::new("pandoc").arg("--version").output().is_err() {
        eprintln!("sin pandoc: prueba omitida");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("grammata-test-pandoc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let plug = tmp.join("cfg/grammata/plugins/mayus");
    std::fs::create_dir_all(&plug).unwrap();
    std::fs::write(
        plug.join("plugin.toml"),
        "name = \"Mayúsculas\"\n[[command]]\nid = \"mayus\"\nname = \"A mayúsculas\"\nrun = [\"tr\", \"a-z\", \"A-Z\"]\n",
    )
    .unwrap();
    // SAFETY: se fija antes de crear la app; la sesión y los plugins del
    // usuario no se tocan.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.join("cfg")) };
    let md = tmp.join("nota.md");
    std::fs::write(&md, "# Hola\n\nUn *texto* con **negrita**.\n\n- uno\n- dos\n").unwrap();
    let mut h = harness();
    h.run_steps(5);
    h.state_mut()
        .st
        .actions
        .push(Action::ShowPanel(Panel::Plugins));
    h.run_steps(5);
    // Sin permiso, el comando no se ofrece.
    assert!(h.query_by_label_contains("A mayúsculas").is_none());
    shot(&mut h, "plugins-permiso");
    h.get_by_label_contains("Permitir sus programas").click();
    h.run_steps(5);
    h.state_mut()
        .st
        .actions
        .push(Action::PluginInstall("pandoc"));
    h.run_steps(5);
    h.state_mut().st.actions.push(Action::ImportPath(md));
    wait(&mut h, "importar el markdown", |a| {
        a.st.docs.cur().text.starts_with("= Hola")
    });
    let text = h.state().st.docs.cur().text.clone();
    assert!(text.contains("#strong[negrita]") && text.contains("- uno"), "{text}");
    assert!(h.state().st.docs.cur().modified(), "lo importado está sin guardar");
    shot(&mut h, "plugins-importado");
    // Comando sobre el documento entero (sin selección).
    h.get_by_label_contains("A mayúsculas").click();
    wait(&mut h, "comando del plugin", |a| {
        a.st.docs.cur().text.starts_with("= HOLA")
    });
    let _ = std::fs::remove_dir_all(&tmp);
}
