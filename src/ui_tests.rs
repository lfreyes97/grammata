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
    let tmp = std::env::temp_dir().join(format!("typst-editor-test-{}", std::process::id()));
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
        cur_text(&h).contains("#table(columns: 2, [a], [b])"),
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
    let tmp = std::env::temp_dir().join(format!("typst-editor-test-outline-{}", std::process::id()));
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
        let c = a
            .st
            .editor
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
    assert_eq!(l, esperado, "el clic en el esquema no llevó al cursor a su línea");
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
    let tmp = std::env::temp_dir().join(format!("typst-editor-test-fm-{}", std::process::id()));
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

/// Borrador: un snippet de la biblioteca se inserta en su buffer.
#[test]
#[ignore = "necesita GPU"]
fn borrador_inserta_snippet() {
    use egui_kittest::kittest::Queryable;
    let tmp = std::env::temp_dir().join(format!("typst-editor-test-ss-{}", std::process::id()));
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
    h.get_by_label_contains("Matemáticas").click();
    h.run_steps(5);
    h.get_by_label_contains("Fracción").click();
    h.run_steps(5);
    let t = h.state().st.scratch.text.clone();
    assert!(t.contains("frac"), "el snippet no llegó al borrador: {t:?}");
    // Un snippet con sus propios `$…$` no se duplica en modo ecuación.
    h.get_by_label_contains("Añadir snippet").click();
    h.run_steps(5);
    h.get_by_label_contains("Matemáticas").click();
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
    let tmp = std::env::temp_dir().join(format!("typst-editor-test-cc-{}", std::process::id()));
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
    h.get_by_label_contains("Cálculo").click();
    h.run_steps(5);
    h.get_by_label_contains("Discriminante").click();
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
    let dir = std::env::temp_dir().join(format!("typst-editor-snip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Recursos a los que apuntan algunos snippets.
    let out = crate::engine::export_pages(
        "#set page(width: 1pt, height: 1pt, margin: 0pt)",
        &dir,
        "main.typ",
        &dir.join("imagen.png"),
        "png",
        Some(72),
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
