// Artículo en Typst puro: la landing de Grammata maquetada como hoja A4,
// con los titulares colgados en el margen izquierdo.
#let tinta = rgb("#1a1a1a")
#let tinta-2 = rgb("#565656")
#let filete = rgb("#8c8c8c")
#let superficie = rgb("#e6e6e6")
#let turquesa = rgb("#239dad")
#let turquesa-texto = rgb("#177a88")

#let mono = ("DejaVu Sans Mono",)
#let margen = 42mm // ancho de la columna de los titulares
#let hueco = 6mm

#set document(title: "Grammata, un editor de Typst para escritorio", author: "Grammata")
#set page(
  paper: "a4",
  margin: (top: 24mm, bottom: 24mm, left: 24mm + margen + hueco, right: 22mm),
  footer: context {
    let n = counter(page).get().first()
    set text(font: mono, size: 7pt, tracking: 0.08em, fill: tinta-2)
    move(dx: -(margen + hueco), block(width: 100% + margen + hueco)[
      #line(length: 100%, stroke: 0.5pt + filete)
      #v(3pt)
      GRAMMATA #h(1fr) #str(n)
    ])
  },
)
#set text(font: "Libertinus Serif", size: 10.5pt, lang: "es", fill: tinta)
#set par(justify: true, leading: 0.7em, spacing: 1em)
#show link: it => underline(offset: 2pt, stroke: 0.5pt, it)
#show raw: it => box(
  fill: superficie, inset: (x: 2pt), outset: (y: 2.5pt), radius: 1pt,
  text(font: mono, size: 0.8em, it),
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

// Titulares de sección numerados como las hojas de un plano.
#show heading.where(level: 1): it => block(sticky: true, above: 1.8em, below: 0.3em, height: 1.7em, {
  place(left, dx: -(margen + hueco), block(width: margen, {
    line(length: 100%, stroke: 1.2pt + tinta)
    v(5pt)
    set text(font: mono, size: 7.5pt, weight: "bold", tracking: 0.1em, fill: tinta)
    set par(justify: false, leading: 0.5em)
    text(fill: tinta-2)[#counter(heading).display("01") / ]
    upper(it.body)
  }))
})
#set heading(numbering: "1")

// --- Portada ---
#move(dx: -(margen + hueco), block(width: 100% + margen + hueco)[
  #set par(justify: false)
  #text(font: mono, size: 8pt, weight: "bold", tracking: 0.12em)[GRAMMATA]
  #v(1.2em)
  #text(size: 27pt, weight: 500, tracking: -0.012em, hyphenate: false)[
    Editor de #typst con vista previa en tiempo real y paneles reorganizables.#cursor
  ]
])

#v(1.2em)
#text(size: 13.5pt, style: "normal")[
  Aplicación de escritorio hecha en Rust, con el compilador de
  #link("https://typst.app")[#typst] embebido.
]

#v(0.8em)
#box(
  fill: tinta, inset: (x: 11pt, y: 8pt),
  text(font: mono, size: 8pt, weight: "bold", tracking: 0.08em, fill: white)[
    #link("https://github.com/lfreyes97/grammata/releases/latest")[
      #text(fill: white)[DESCARGAR LA ÚLTIMA VERSIÓN]
    ]
  ],
)

// --- Qué hace ---
= Qué hace

#let punto(cuerpo) = grid(
  columns: (1.4em, 1fr),
  text(font: mono, fill: turquesa-texto)[+],
  cuerpo,
)
#set par(spacing: 0.7em)

#punto[Vista previa del #sc[pdf] que genera el compilador. Con errores, conserva la última versión que compiló.]
#punto[Resaltado con el analizador oficial de #typst, autocompletado, búsqueda, ir a línea y esquema del documento.]
#punto[Panel de problemas con salto a la línea, también en archivos incluidos.]
#punto[136 símbolos y 22 estructuras (`frac`, `sqrt`, `mat`, `cases`…) con buscador; borrador con vista previa propia para ecuaciones.]
#punto[Snippets de serie y propios, con marcadores para la selección y el cursor.]
#punto[Plugins: carpetas con un `plugin.toml` que aportan paquetes de #typst con formulario, fuentes, snippets y temas, o llaman a programas externos (Pandoc para Markdown). Esos programas requieren tu permiso, que se pierde si cambia su línea de comandos.]
#punto[Pestañas y paneles que se arrastran y se dividen. La vista previa solo rasteriza las páginas visibles.]

// --- Descargas ---
= Descargas

#set par(spacing: 1em)
Paquetes en la #link("https://github.com/lfreyes97/grammata/releases/latest")[última versión]. Arch y Nix, desde el repositorio.

#let linea = (stroke: (x, y) => (bottom: if y == 0 { 0.8pt + tinta } else { 0.4pt + filete }))
#show table.cell.where(y: 0): set text(font: mono, size: 7pt, weight: "bold", tracking: 0.1em, fill: tinta-2)
#show table.cell: set par(justify: false)
#show table: set block(breakable: false)
#table(
  columns: (21mm, 1fr, 1fr),
  inset: (x: 0pt, y: 6pt, right: 8pt),
  align: top,
  ..linea,
  table.header[SISTEMA][QUÉ INSTALAR][ESTADO],
  text(weight: "bold")[Linux], [`.deb`, AppImage o `.tar.gz` (dentro, `./install.sh`)], [Probado en GNOME con Wayland.],
  text(weight: "bold")[Arch Linux], [`PKGBUILD` en `packaging/arch` (`makepkg -si`)], [Probado.],
  text(weight: "bold")[Windows], [`.zip` con `grammata.exe`], [Se compila en #sc[ci], pero nadie lo ha probado a mano.],
  text(weight: "bold")[Nix], [`nix run github:lfreyes97/grammata`], [El flake todavía no se ha probado.],
  text(weight: "bold")[macOS], [No hay paquete.], [No se ha probado.],
)

#v(0.4em)
Grammata aún no lee argumentos de línea de comandos: no abre archivos pasados como argumento ni desde el gestor de archivos.

// --- Licencia ---
= Licencia

#sc[gnu] #sc[gpl] v3 o posterior. Usa #typst (Apache-2.0) y otras dependencias compatibles. Errores e ideas, en los #link("https://github.com/lfreyes97/grammata/issues")[issues]\; el código, en #link("https://github.com/lfreyes97/grammata")[GitHub].

#v(2em)
#set text(font: mono, size: 7.5pt, fill: tinta-2)
#set par(justify: false, leading: 0.6em)
#line(length: 100%, stroke: (paint: filete, thickness: 0.5pt, dash: "dashed"))
Grammata viene del griego γράμματα, «letras».
#line(length: 100%, stroke: (paint: filete, thickness: 0.5pt, dash: "dashed"))
«#typst» es una marca de Typst GmbH. Grammata es un proyecto independiente, sin relación con esa empresa.
