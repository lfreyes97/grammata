// Snippets del plugin de ejemplo: cada #let público es uno.

/// Versículo. Cita en bloque con su referencia. Grupo: Citas
#let versiculo = quote(block: true, attribution: [Juan 1:1])[En el principio era el Verbo.]

/// Nota al margen. Grupo: Citas
#let nota = [#text(size: 0.85em, style: "italic")[Nota: …]]
