// Fondo de marca con el motor de capas de Instatypst: curvas de nivel sobre
// un color liso y viñeta, con el titular en tipografía editorial.
#import "@local/instatypst:0.1.0": *

#let fondo = rgb("#0f1d21")
#let marca = (theme.define)("grammata-web", colors: (
  primary: rgb("#3fb5c6"), secondary: rgb("#1d8a99"), accent: rgb("#e5b84b"), dark: fondo))

#canvas(sizes.instagram, theme: marca)[
  #superbg.superbg((capas: (
    (tipo: "plano", color: fondo),
    (tipo: "topo", color: rgb("#3fb5c6"), fondo: none, anillos: 16, separacion: 52, wobble: 22, grosor: 1.8, opacidad: 38%, semilla: 11),
    (tipo: "vignette", intensidad: 55%),
  )), size: sizes.instagram, theme: marca)
  #pad(x: 90pt, y: 100pt)[
    #text(font: "Inter", size: 24pt, weight: 600, tracking: 8pt, fill: rgb("#e5b84b"))[#upper[Grammata]]
    #v(36pt)
    #text(font: "Fraunces", size: 120pt, weight: 800, fill: white, top-edge: "cap-height", bottom-edge: "baseline")[Escribe. \ Mira. \ Publica.]
    #v(44pt)
    #text(font: "Inter", size: 30pt, fill: white.transparentize(25%))[Un editor de Typst con vista previa del PDF en tiempo real.]
  ]
]
