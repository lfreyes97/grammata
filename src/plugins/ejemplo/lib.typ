// Paquete del plugin de ejemplo de Grammata.

/// Blanco o negro, lo que se lea mejor sobre `fondo`.
#let _tinta(fondo) = if luma(fondo).components().at(0) > 55% { black } else { white }

/// Una frase sobre fondo de color. `formato` es (ancho, alto) en puntos.
#let tarjeta(
  texto,
  autor: none,
  fondo: rgb("#1d4ed8"),
  formato: (1080, 1350),
  tamaño: 72pt,
  centrado: false,
) = page(
  width: formato.at(0) * 1pt,
  height: formato.at(1) * 1pt,
  margin: 90pt,
  fill: fondo,
)[
  #set text(fill: _tinta(fondo), size: tamaño, weight: 700)
  #set par(leading: 0.5em)
  #set align(if centrado { center + horizon } else { left + horizon })
  #texto
  #if autor != none {
    v(0.6em)
    text(size: 0.4em, weight: 400, autor)
  }
]
