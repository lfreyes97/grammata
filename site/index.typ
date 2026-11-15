// Landing de Grammata. Se compila a HTML con:  site/build.sh
#set document(title: "Grammata, un editor de Typst para escritorio")
#set text(lang: "es")

#let el(tag, ..args) = html.elem(tag, attrs: args.named(), args.pos().join())
#let enlace(href, cuerpo, clase: none) = html.elem(
  "a", attrs: if clase == none { (href: href) } else { (href: href, class: clase) }, cuerpo)
// Siglas en versalitas.
#let sg(t, ..) = html.elem("abbr", t)
#let pdf = sg("PDF", "Portable Document Format")
// El nombre de Typst como logotipo (el tema le da forma, como el de LaTeX).
#let typst = html.elem("span", attrs: (class: "typst"))[Typst]

#let repo = "https://github.com/lfreyes97/grammata"

#html.elem("meta", attrs: (name: "description", content: "Editor de escritorio para Typst, escrito en Rust, con el compilador embebido y vista previa en tiempo real."))
#html.elem("meta", attrs: (name: "color-scheme", content: "light dark"))
#html.elem("link", attrs: (rel: "stylesheet", href: "style.css"))
#html.elem("script", attrs: (src: "tema.js", defer: ""))[]
#html.elem("link", attrs: (rel: "icon", href: "img/grammata-32.png"))

#el("header", class: "portada")[
  #el("a", class: "marca", href: repo)[
    #html.elem("picture")[
      #html.elem("source", attrs: (media: "(prefers-color-scheme: dark)", srcset: "img/logo-horizontal-oscuro.svg"))
      #html.elem("img", attrs: (src: "img/logo-horizontal-claro.svg", alt: "Grammata", width: "193", height: "56"))
    ]
  ]
  #html.elem("h1")[#el("span")[Editor de #typst con vista previa en tiempo real] #el("span")[y paneles reorganizables.]]
  #el("p", class: "entrada")[
    Aplicación de escritorio hecha en Rust, con el compilador de #enlace("https://typst.app")[#typst] embebido.
  ]
  #el("div", class: "acciones")[
    #enlace(repo + "/releases/latest", clase: "boton")[Descargar la última versión]
  ]
]

#let captura(nombre, alt, perezosa: true) = html.elem("picture")[
  #html.elem("source", attrs: (media: "(prefers-color-scheme: dark)", srcset: "img/" + nombre + "-oscuro.webp"))
  #html.elem("img", attrs: (
    src: "img/" + nombre + "-claro.webp", alt: alt, width: "1400", height: "875", decoding: "async",
    loading: if perezosa { "lazy" } else { "eager" }))
]

#let lamina(n, titulo, nota, ..args) = el("figure", class: "captura ancha")[
  #captura(..args)
  #html.elem("figcaption")[#html.elem("b")[Lámina #n] #html.elem("span")[#titulo. #nota]]
]

#let seccion(titulo, id: none, clase: none, cuerpo) = {
  let attrs = (class: if clase == none { "seccion" } else { "seccion " + clase })
  if id != none { attrs.insert("id", id) }
  html.elem("section", attrs: attrs)[
    #html.elem("h2", titulo)
    #el("div", class: "cuerpo", cuerpo)
  ]
}

#el("main")[
  #seccion("Qué hace")[
    #el("ul", class: "lista")[
      #html.elem("li")[Vista previa del #pdf que genera el compilador. Con errores, conserva la última versión que compiló.]
      #html.elem("li")[Resaltado con el analizador oficial de #typst, autocompletado, búsqueda, ir a línea y esquema del documento.]
      #html.elem("li")[Panel de problemas con salto a la línea, también en archivos incluidos.]
      #html.elem("li")[136 símbolos y 22 estructuras (`frac`, `sqrt`, `mat`, `cases`…) con buscador; borrador con vista previa propia para ecuaciones.]
      #html.elem("li")[Snippets de serie y propios, con marcadores para la selección y el cursor.]
      #html.elem("li")[Plugins: carpetas con un `plugin.toml` que aportan paquetes de #typst con formulario, fuentes, snippets y temas, o llaman a programas externos (Pandoc para Markdown). Esos programas requieren tu permiso, que se pierde si cambia su línea de comandos.]
      #html.elem("li")[Pestañas y paneles que se arrastran y se dividen. La vista previa solo rasteriza las páginas visibles.]
    ]
  ]

  #let fila(..celdas) = html.elem("tr", celdas.pos().enumerate().map(((i, c)) =>
    if i == 0 { html.elem("th", attrs: (scope: "row"), c) } else { html.elem("td", c) }).join())

  #seccion("Descargas", id: "descargas")[
    #el("p")[
      Paquetes en la #enlace(repo + "/releases/latest")[última versión]. Arch y Nix, desde el repositorio.
    ]
    #el("div", class: "tabla")[
      #html.elem("table")[
        #html.elem("thead")[#html.elem("tr")[
          #html.elem("th", attrs: (scope: "col"))[Sistema]
          #html.elem("th", attrs: (scope: "col"))[Qué instalar]
          #html.elem("th", attrs: (scope: "col"))[Estado]
        ]]
        #html.elem("tbody")[
          #fila[Linux][`.deb`, AppImage o `.tar.gz` (dentro, `./install.sh`)][Probado en GNOME con Wayland.]
          #fila[Arch Linux][`PKGBUILD` en `packaging/arch` (`makepkg -si`)][Probado.]
          #fila[Windows][`.zip` con `grammata.exe`][Se compila en #sg("CI"), pero nadie lo ha probado a mano.]
          #fila[Nix][`nix run github:lfreyes97/grammata`][El flake todavía no se ha probado.]
          #fila[macOS][No hay paquete.][No se ha probado.]
        ]
      ]
    ]
    #el("p", class: "aviso")[
      Grammata aún no lee argumentos de línea de comandos: no abre archivos pasados como argumento
      ni desde el gestor de archivos.
    ]
  ]

  #seccion("Capturas de pantalla", clase: "capturas")[
    #lamina("01", "Ensayo", "Código a la izquierda y, a la derecha, la página con título, epígrafe y capitular ornamentada.",
      "ensayo", "Grammata con un ensayo abierto: el código a la izquierda y, a la derecha, la página con título, epígrafe y capitular ornamentada", perezosa: false)
    #lamina("02", "Cartel", "Una cita en cursiva sobre fondo verde oscuro.",
      "cartel-epigrafe", "Grammata con un cartel abierto: una cita en cursiva sobre fondo verde oscuro")
  ]

  #seccion("Licencia")[
    #el("p")[
      #sg("GNU") #sg("GPL") v3 o posterior. Usa #typst (Apache-2.0) y otras dependencias compatibles.
      Errores e ideas, en los #enlace(repo + "/issues")[issues]\; el código, en #enlace(repo)[GitHub].
    ]
  ]
]

#el("footer")[
  #el("p")[Grammata viene del griego γράμματα, «letras».]
  #el("p")[«#typst» es una marca de Typst GmbH. Grammata es un proyecto independiente, sin relación con esa empresa.]
]
