// Presentación en Typst puro: la landing de Grammata como diapositivas 16:9
// sobre papel cuadriculado, con una sección por pantalla.
#let tinta = rgb("#1a1a1a")
#let tinta-2 = rgb("#565656")
#let filete = rgb("#8c8c8c")
#let papel = rgb("#f3f3f3")
#let superficie = rgb("#e2e2e2")
#let turquesa = rgb("#239dad")
#let turquesa-texto = rgb("#177a88")

#let mono = ("DejaVu Sans Mono",)
#let repo = "https://github.com/lfreyes97/grammata"

// Malla de plano: línea fina cada 20 pt y gruesa cada 100 pt.
#let malla = tiling(size: (100pt, 100pt), relative: "parent", {
  place(rect(width: 100%, height: 100%, fill: papel))
  for i in range(1, 5) {
    place(dx: i * 20pt, line(angle: 90deg, length: 100pt, stroke: 0.4pt + rgb("#0000000f")))
    place(dy: i * 20pt, line(length: 100pt, stroke: 0.4pt + rgb("#0000000f")))
  }
  place(line(angle: 90deg, length: 100pt, stroke: 0.6pt + rgb("#00000024")))
  place(line(length: 100pt, stroke: 0.6pt + rgb("#00000024")))
})

#set document(title: "Grammata, un editor de Typst para escritorio", author: "Grammata")
#set page(
  width: 338.67mm,
  height: 190.5mm,
  margin: (x: 22mm, top: 20mm, bottom: 24mm),
  fill: malla,
  footer: context {
    set text(font: mono, size: 9pt, tracking: 0.1em, fill: tinta-2)
    line(length: 100%, stroke: 0.6pt + filete)
    v(4pt)
    [GRAMMATA #h(1fr) #counter(page).display("01 / 01", both: true)]
  },
)
#set text(font: "Libertinus Serif", size: 24pt, lang: "es", fill: tinta)
#set par(leading: 0.62em, spacing: 0.8em)
#show link: it => underline(offset: 4pt, stroke: 1pt, it)
#show raw: it => box(
  fill: superficie, inset: (x: 4pt), outset: (y: 4pt), radius: 1pt,
  text(font: mono, size: 0.72em, it),
)

// Siglas en versalitas.
#let sc(t) = text(size: 0.85em, tracking: 0.05em, upper(t))

// El nombre de Typst como logotipo: minúsculas, negrita y su turquesa.
#let typst = text(weight: "bold", fill: turquesa-texto, tracking: 0.01em)[typst]

// Cursor de la marca: una viga con remates.
#let cursor = box(width: 0.3em, height: 0.74em, baseline: 0.03em, {
  place(center, line(angle: 90deg, length: 100%, stroke: 0.06em + turquesa))
  place(top, line(length: 100%, stroke: 0.05em + turquesa))
  place(bottom, line(length: 100%, stroke: 0.05em + turquesa))
})

// Una diapositiva: etiqueta de hoja, titular y cuerpo.
#let diapo(n, titulo, cuerpo) = {
  pagebreak(weak: true)
  line(length: 12%, stroke: 2.2pt + tinta)
  v(8pt)
  text(font: mono, size: 13pt, weight: "bold", tracking: 0.12em)[
    #text(fill: tinta-2)[#n /] #upper(titulo)
  ]
  v(1.2em)
  cuerpo
}

#let punto(cuerpo) = grid(
  columns: (1.5em, 1fr),
  text(font: mono, fill: turquesa-texto, size: 0.8em)[+],
  cuerpo,
)

// --- Portada ---
#v(1fr)
#text(font: mono, size: 14pt, weight: "bold", tracking: 0.14em)[GRAMMATA]
#v(0.6em)
#block(width: 88%, text(size: 58pt, weight: 500, tracking: -0.012em, hyphenate: false)[
  Editor de #typst con vista previa en tiempo real y paneles reorganizables.#cursor
])
#v(0.5em)
#text(size: 27pt)[Aplicación de escritorio hecha en Rust, con el compilador de #typst embebido.]
#v(1.2em)
#box(
  fill: tinta, inset: (x: 16pt, y: 12pt),
  text(font: mono, size: 14pt, weight: "bold", tracking: 0.08em, fill: white,
    link(repo + "/releases/latest")[#text(fill: white)[DESCARGAR LA ÚLTIMA VERSIÓN]]),
)
#v(1.4fr)

// --- Qué hace ---
#diapo("01", "Qué hace")[
  #set text(size: 27pt)
  #set par(spacing: 1em)
  #punto[Vista previa del #sc[pdf] que genera el compilador. Con errores, conserva la última versión que compiló.]
  #punto[Resaltado con el analizador oficial de #typst, autocompletado, búsqueda, ir a línea y esquema del documento.]
  #punto[Panel de problemas con salto a la línea, también en archivos incluidos.]
  #punto[136 símbolos y 22 estructuras (`frac`, `sqrt`, `mat`, `cases`…) con buscador; borrador con vista previa propia para ecuaciones.]
]

#diapo("01", "Qué hace")[
  #set text(size: 27pt)
  #set par(spacing: 1em)
  #punto[Snippets de serie y propios, con marcadores para la selección y el cursor.]
  #punto[Plugins: carpetas con un `plugin.toml` que aportan paquetes de #typst con formulario, fuentes, snippets y temas, o llaman a programas externos (Pandoc para Markdown). Esos programas requieren tu permiso, que se pierde si cambia su línea de comandos.]
  #punto[Pestañas y paneles que se arrastran y se dividen. La vista previa solo rasteriza las páginas visibles.]
]

// --- Descargas ---
#diapo("02", "Descargas")[
  #set text(size: 19pt)
  Paquetes en la #link(repo + "/releases/latest")[última versión]. Arch y Nix, desde el repositorio.
  #v(0.2em)
  #show table.cell.where(y: 0): set text(font: mono, size: 10pt, weight: "bold", tracking: 0.12em, fill: tinta-2)
  #show table.cell: set par(justify: false)
  #table(
    columns: (48mm, 1fr, 1fr),
    inset: (x: 0pt, y: 7pt, right: 12pt),
    align: top,
    stroke: (x, y) => (bottom: if y == 0 { 1pt + tinta } else { 0.5pt + filete }),
    table.header[SISTEMA][QUÉ INSTALAR][ESTADO],
    text(weight: "bold")[Linux], [`.deb`, AppImage o `.tar.gz` (dentro, `./install.sh`)], [Probado en GNOME con Wayland.],
    text(weight: "bold")[Arch Linux], [`PKGBUILD` en `packaging/arch` (`makepkg -si`)], [Probado.],
    text(weight: "bold")[Windows], [`.zip` con `grammata.exe`], [Se compila en #sc[ci], pero nadie lo ha probado a mano.],
    text(weight: "bold")[Nix], [`nix run github:lfreyes97/grammata`], [El flake todavía no se ha probado.],
    text(weight: "bold")[macOS], [No hay paquete.], [No se ha probado.],
  )
]

#diapo("02", "Descargas")[
  #set text(size: 36pt)
  Grammata aún no lee argumentos de línea de comandos: no abre archivos pasados como argumento ni desde el gestor de archivos.
]

// --- Licencia ---
#diapo("03", "Licencia")[
  #set text(size: 30pt)
  #sc[gnu] #sc[gpl] v3 o posterior. Usa #typst (Apache-2.0) y otras dependencias compatibles.
  #v(0.4em)
  Errores e ideas, en los #link(repo + "/issues")[issues]\; el código, en #link(repo)[GitHub].

  #v(1fr)
  #set text(font: mono, size: 13pt, fill: tinta-2)
  #set par(leading: 0.7em)
  #line(length: 100%, stroke: (paint: filete, thickness: 0.6pt, dash: "dashed"))
  Grammata viene del griego γράμματα, «letras».
  #line(length: 100%, stroke: (paint: filete, thickness: 0.6pt, dash: "dashed"))
  «#typst» es una marca de Typst GmbH. Grammata es un proyecto independiente, sin relación con esa empresa.
]
