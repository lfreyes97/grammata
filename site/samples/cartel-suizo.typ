// Cartel de estilo suizo en Typst puro: comilla gigante y texto al pie.
#set page(width: 1080pt, height: 1350pt, margin: 0pt, fill: rgb("#f4f1ea"))
#set text(lang: "es")

#place(top + left, dx: 70pt, dy: 60pt,
  text(font: "Fraunces", size: 700pt, weight: 700, fill: rgb("#c0392b"), "\u{201C}"))

#place(bottom + left, dx: 96pt, dy: -110pt, block(width: 900pt)[
  #set text(font: "Urbanist", size: 72pt, weight: 600, fill: rgb("#111"))
  #set par(leading: 16pt)
  Leer despacio es una forma de atención.
  #v(36pt)
  #line(length: 100%, stroke: 4pt + rgb("#111"))
  #v(20pt)
  #text(size: 28pt, weight: 500, tracking: 3pt, upper[Ensayo · Noviembre de 2026])
])
