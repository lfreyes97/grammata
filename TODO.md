# Pendientes de Grammata

Actualizado el 2026-11-06. Marcar con `[x]` al terminar.

## 1. Estado del árbol
- [x] Commits de rendimiento del editor, licencia GPL-3.0-or-later, renombrado a `grammata`, empaquetado y flake de Nix.
- [ ] Reinstalar en `~/.local` el binario con el arreglo (`packaging/build-linux.sh` y `install.sh`).

## 2. Para poder publicar
- [ ] **Argumentos de línea de comandos**: abrir archivos y carpetas, `--version`, `--help`. Hoy se ignoran, así que `Exec=grammata %F` del `.desktop` no abre el archivo.
- [ ] Repositorio en GitHub; sustituir `OWNER` en `nix/package.nix` y `README.md`; añadir `repository` a `Cargo.toml`.
- [ ] Workflow de GitHub Actions al subir una etiqueta `v*`: compilar Linux y Windows, y adjuntar los binarios a la release.
  - [ ] Linux: `.tar.gz` (ya existe el script), `.deb` y AppImage.
  - [ ] Windows: `.zip` e instalador MSI.
  - [ ] macOS: `.app`/`.dmg` solo desde el CI de GitHub, con aviso de «sin probar».
- [ ] `THIRD-PARTY-LICENSES` (por ejemplo con `cargo-about`) incluido en los paquetes; `stage.sh` ya lo instala si existe.
- [x] `PKGBUILD` para Arch (`packaging/arch`, `makepkg -si`; probado: el paquete se genera e instala con iconos).
- [ ] README y notas de la primera versión: decir que Windows y macOS no están probados a mano; aclarar que «Typst» es marca de Typst GmbH.
- [ ] Cabeceras de licencia o aviso `GPL-3.0-or-later` en el código fuente (decidir si hacen falta).

## 3. Nix
- [ ] Probar el flake (`nix build`, `nix run`, `nix develop`): no hay `nix` en esta máquina; puede hacerse en el CI.
- [ ] Generar y commitear `flake.lock` (`nix flake lock`); recordar que un flake solo ve archivos que ya están en git.
- [ ] Añadir `aarch64-darwin` como **experimental**: condicionar con `lib.optionals stdenv.hostPlatform.isLinux` el `patchelf`, el RPATH, `fontconfig`, el `.desktop` y los iconos. Comprobar antes si `x86_64-darwin` sigue soportado en `nixpkgs`.

## 4. Rendimiento y memoria
- [ ] El editor gasta ~240 MB en documentos de ~600 KB: maquetar solo las líneas visibles en vez del texto completo.
- [ ] **Proceso trabajador `grammata --compile-worker`** (mismo binario, tuberías de `std::process`): aísla fallos de typst, permite cancelar y libera memoria al reiniciarlo. Empezar con un experimento acotado: un solo viaje `Compile`, detrás de una opción, midiendo latencia y memoria. Tras publicar.
- [ ] Opcional: no retener la caché de typst en libros muy grandes (recompilar en ~3 s en vez de ~1 s). Descartado como valor por defecto.
- [ ] Sin resolver: ¿iba fluido el libro grande con `cargo run --release` antes del arreglo? Si sí, medir el perfil de release anterior (sin LTO ni `strip`).

## 5. Exportación
- [ ] Recordar el último ppi elegido en el diálogo de exportación.
- [ ] PDF/UA (descartado de momento: typst no admite rango con el PDF etiquetado).

## 6. Terminal (decidido: sin terminal embebida)
- [ ] Opcional: botón «Abrir terminal aquí» con detección por sistema y comando configurable.
- [ ] Opcional: panel «Salida» solo para `typst` (con argumentos, sin shell).
- Descartado: `egui_term` 0.1.0 depende de egui 0.31 (usamos 0.36) y es el peor caso multiplataforma.

## 7. Arrastrado de antes
- [ ] Snippets extensibles con typst, parte 3: formularios según el tipo del valor por defecto de los parámetros (color → selector, longitud → número…).
- [ ] Borrador: extras «Recientes» y compactar la barra de estructuras.
- [ ] Formato: el botón de enlace no lee un enlace existente para editarlo.

## 8. Ideas: `typst-ide`
- [ ] Evaluar la crate `typst-ide` (la usan la web app de Typst y `tinymist`): da los datos, la interfaz hay que dibujarla en egui. Hoy no se usa.
  - [x] Autocompletado semántico (funciones, parámetros, variables, rutas) con el menú propio, en `src/ide.rs`.
  - [ ] Autocompletado: etiquetas y bibliografía (necesitan pasar el último `PagedDocument` a `autocomplete`), paquetes (`IdeWorld::packages`) y mostrar el tipo de cada ítem.
  - [ ] Tooltips al pasar el ratón y ayuda de firma de la llamada.
  - [ ] Ir a la definición.
  - [ ] Saltar entre código y vista previa: `jump_from_click` (clic en el PDF) y `jump_from_cursor` (cursor al PDF).

## 9. Plugins
- [x] Fase 1 (declarativa): `plugin.toml` con paquete typst, plantillas con formulario, fuentes, snippets y temas; panel «Plugins»; clave de licencia guardada. Ver `docs/plugins.md`.
- [x] Protocolo 1 de programas externos (`[[import]]`, `[[export]]`, `[[command]]`; texto o JSON por stdin/stdout, con permiso del usuario) y plugin de Pandoc para Markdown.
- [ ] Protocolo: proceso persistente con eventos (guardar, compilar), comandos en un menú o paleta, y cancelar un programa en marcha.
- [ ] Importar al abrir o arrastrar un `.md`; probar a mano el diálogo de exportar de los plugins (las pruebas no cubren el diálogo nativo).
- [ ] Seguridad de plugins: el permiso no cubre el contenido de los `./programa` ni `protocol`/`timeout`; la salida de un programa no tiene límite de tamaño y al vencer el tiempo solo se mata el proceso hijo (no su grupo).
- [ ] Pandoc: Word y otros formatos (ya están comentados en su manifiesto).
- [ ] Vista previa en vivo del formulario (hoy crea el documento o inserta el código) y volver a editar una llamada ya insertada.
- [ ] Instalar un plugin desde un `.zip` y botón para abrir la carpeta.
- [ ] Los temas no cambian aún el fondo de los campos de texto ni el color de selección (`theme::apply`).
- [ ] Empaquetar Instatypst como plugin (manifiesto con sus presets de `carteles`).
- [ ] Arrancar con el tema claro guardado sale con colores mezclados en las pruebas de UI (ya pasaba antes de los plugins): comprobar en la app real.

## Límites conocidos
- Solo se puede probar Linux (GNOME/Wayland). macOS no se puede probar nunca desde aquí; Windows solo compila en el CI.
- En Wayland el icono de ventana solo sale con el `.desktop` instalado (winit 0.30 ignora `with_icon`).
- Avisos de `rustfmt` ya existentes en el repositorio (`engine.rs`, `chrome.rs`, `app.rs`, `ui_tests.rs`): no formatear esos archivos enteros.
