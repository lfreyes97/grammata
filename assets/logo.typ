// Marca de Grammata: una Γ (gamma mayúscula) con un cursor de texto.
// Solo usa fuentes embebidas en typst (Libertinus Serif), así que compila igual
// en cualquier máquina:  typst compile --ignore-system-fonts …
// Entradas (`--input clave=valor`):
//   v  tile | negro | blanco | horizontal-claro | horizontal-oscuro   (tile)
//   s  lado en px del tile o de la marca                              (512)

#let v = sys.inputs.at("v", default: "tile")
#let s = float(sys.inputs.at("s", default: "512")) * 1pt
#let font = "Libertinus Serif"

// Tamaños pequeños: bold y letra grande (hace de «tamaño óptico»);
// grandes: semibold, con el cursor más fino.
#let pequeno(side) = side <= 48pt

// La Γ y su cursor (viga con remates), centrados en un cuadro de `side`.
#let glifo(side, fg) = {
  let small = pequeno(side)
  let sz = side * if small { 0.9 } else { 0.96 }
  set text(
    font: font, weight: if small { "bold" } else { "semibold" }, size: sz, fill: fg,
    top-edge: "cap-height", bottom-edge: "baseline",
  )
  context {
    let g = text[Γ]
    let h = measure(g).height
    let t = sz * if small { 0.075 } else { 0.062 }
    let r = t * 1.9
    let viga = box(width: r, stack(dir: ttb,
      rect(width: r, height: t * 0.7, fill: fg),
      align(center, rect(width: t, height: h - t * 1.4, fill: fg)),
      rect(width: r, height: t * 0.7, fill: fg)))
    box(width: side, height: side, align(center + horizon,
      stack(dir: ltr, spacing: sz * 0.07, g, align(horizon, viga))))
  }
}

// Marca sobre cuadrado redondeado.
#let tile(side, fg, bg) = box(width: side, height: side, fill: bg, radius: side * 0.22, glifo(side, fg))

#let horizontal(fg, bg) = {
  let mark = 96pt
  grid(columns: 2, gutter: 22pt, align: horizon,
    tile(mark, bg, fg),
    stack(spacing: 7pt,
      text(font: font, size: 54pt, weight: "semibold", fill: fg, top-edge: "ascender", bottom-edge: "baseline")[Grammata],
      text(font: font, size: 21pt, style: "italic", fill: fg.transparentize(35%))[γράμματα],
    ))
}

#set page(width: auto, height: auto, fill: none)
#if v == "tile" {
  set page(margin: 0pt)
  tile(s, white, black)
} else if v == "negro" {
  set page(margin: 0pt)
  glifo(s, black)
} else if v == "blanco" {
  set page(margin: 0pt)
  glifo(s, white)
} else if v == "horizontal-claro" {
  set page(margin: 6pt)
  horizontal(rgb("#111111"), white)
} else if v == "horizontal-oscuro" {
  set page(margin: 6pt)
  horizontal(rgb("#f2f2f2"), rgb("#111111"))
} else {
  panic("v desconocida: " + v)
}
