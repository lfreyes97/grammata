// Muestra para las capturas de la web. Necesita el paquete local
// @local/instatypst 0.1.0 (https://github.com/lfreyes97/Instatypst).
#import "@local/instatypst:0.1.0": *

#let t = make-theme(paleta: "terracota", tipografia: "editorial-clasico")
#let papel = rgb("#f3ecdc")
#let tinta = rgb("#2a231c")
#let teja = rgb("#a8442a")

#show: articulo.with(paleta: "terracota", tipografia: "editorial-clasico", paper: "a5")
#set page(fill: papel, margin: (x: 18mm, top: 16mm, bottom: 14mm), numbering: none)
#set text(size: 11.5pt, fill: tinta)
#set par(justify: true, leading: 0.72em)

// Título
#align(center)[
  #text(font: t.fonts.body, size: 9pt, tracking: 3pt, fill: teja)[#upper[Ensayo]]
  #v(8pt)
  #text(font: t.fonts.display, size: 36pt, weight: "bold", fill: teja, top-edge: "cap-height", bottom-edge: "baseline")[El arte de \ leer despacio]
  #v(10pt)
  #text(font: t.fonts.body, style: "italic", size: 12.5pt, fill: tinta.transparentize(25%))[Una defensa de la lectura que no tiene prisa]
  #v(6pt)
  #text(font: t.fonts.body, size: 8.5pt, tracking: 1.2pt, fill: tinta.transparentize(40%))[#upper[Luis Reyes · Noviembre de 2026]]
]

// Epígrafe
#v(10pt)
#align(center)[
  #block(width: 82%)[
    #text(font: t.fonts.display, size: 40pt, fill: teja.transparentize(55%), top-edge: "cap-height", bottom-edge: "baseline")[“]
    #v(-12pt)
    #text(font: t.fonts.body, style: "italic", size: 15pt, fill: tinta)[El corazón tiene razones que la razón no conoce.]
    #v(5pt)
    #text(font: t.fonts.body, size: 8.5pt, tracking: 1.5pt, fill: teja)[#upper[Blaise Pascal · Pensamientos, 1670]]
  ]
]
#v(12pt)

// Cuerpo con capitular ornamentada
#capitular-ornamentada(alto: 5, estilo: "rubricada", theme: t)[Vivimos rodeados de textos que se leen en diagonal. Leer despacio no es una nostalgia: es una forma de atención, y la atención es lo único que de verdad entregamos cuando leemos.]

#v(2pt)
#text(font: t.fonts.display, size: 14pt, weight: "bold", fill: teja)[La lectura como hospitalidad]
#v(1pt)

Un texto largo pide ser recibido como se recibe a un huésped. Se le ofrece tiempo, se le hace sitio y se le deja hablar hasta el final antes de contestar.
