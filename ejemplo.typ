#set page(paper: "a4", margin: 2cm)
#set text(size: 11pt, lang: "es")

= Hola Typst
Este archivo es de prueba para el editor en Rust.

== Formato
Puedes usar *negrita*, _cursiva_, `código`, #text(red)[rojo] y enlaces #link("https://typst.app")[Typst].

#let nombre = "mundo"
Hola, #nombre! $ x^2 + y^2 = z^2 $

// comentario: esta línea es un comentario

#table(
  columns: 2,
  [*Nombre*], [*Valor*],
  [ancho], [2cm],
)
