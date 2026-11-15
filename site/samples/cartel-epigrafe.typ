// Cartel editorial en Typst puro: cita en cursiva grande, filete y atribución.
#set page(width: 1080pt, height: 1350pt, margin: 0pt, fill: rgb("#14201f"))
#set text(lang: "es")

#place(top + left, dx: 96pt, dy: 96pt,
  text(font: "Inter", size: 22pt, weight: 600, tracking: 6pt, fill: rgb("#c9a24a"), upper[Confesiones]))

#place(horizon + left, dx: 96pt, dy: -40pt, block(width: 880pt)[
  #set text(font: "EB Garamond", size: 92pt, style: "italic", fill: rgb("#f1e9d8"))
  #set par(leading: 22pt)
  «Nos hiciste para ti, y nuestro corazón está inquieto hasta que descanse en ti.»
  #v(56pt)
  #line(length: 120pt, stroke: 3pt + rgb("#c9a24a"))
  #v(28pt)
  #text(font: "Inter", size: 26pt, style: "normal", tracking: 2pt, fill: rgb("#c9a24a"),
    upper[Agustín de Hipona · Libro I])
])

#place(bottom + right, dx: -96pt, dy: -80pt,
  text(font: "EB Garamond", size: 180pt, fill: rgb("#c9a24a").transparentize(75%), "Γ"))
