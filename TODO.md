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

## Límites conocidos
- Solo se puede probar Linux (GNOME/Wayland). macOS no se puede probar nunca desde aquí; Windows solo compila en el CI.
- En Wayland el icono de ventana solo sale con el `.desktop` instalado (winit 0.30 ignora `with_icon`).
- Avisos de `rustfmt` ya existentes en el repositorio (`engine.rs`, `chrome.rs`, `app.rs`, `ui_tests.rs`): no formatear esos archivos enteros.
