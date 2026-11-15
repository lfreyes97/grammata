/* Carrusel de las capturas: botones, contador y teclado sobre el scroll-snap. */
(() => {
  const pista = document.querySelector(".capturas .cuerpo");
  const laminas = pista ? [...pista.children] : [];
  if (laminas.length < 2) return;

  const es = document.documentElement.lang === "es";
  const reducir = matchMedia("(prefers-reduced-motion: reduce)");
  const dos = (n) => String(n).padStart(2, "0");

  pista.tabIndex = 0;
  pista.setAttribute("role", "group");
  pista.setAttribute("aria-roledescription", es ? "carrusel" : "carousel");
  pista.setAttribute("aria-label", es ? "Capturas de pantalla" : "Screenshots");

  const boton = (texto, etiqueta, paso) => {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = texto;
    b.setAttribute("aria-label", etiqueta);
    b.addEventListener("click", () => ir(actual() + paso));
    return b;
  };
  const anterior = boton("←", es ? "Lámina anterior" : "Previous plate", -1);
  const siguiente = boton("→", es ? "Lámina siguiente" : "Next plate", 1);
  const cuenta = document.createElement("output");
  const nav = document.createElement("div");
  nav.className = "pasar";
  nav.append(anterior, cuenta, siguiente);
  pista.before(nav);

  // La lámina más cercana al centro visible de la pista.
  const actual = () => {
    const centro = pista.scrollLeft + pista.clientWidth / 2;
    let mejor = 0;
    laminas.forEach((l, i) => {
      const d = Math.abs(l.offsetLeft + l.offsetWidth / 2 - centro);
      if (d < Math.abs(laminas[mejor].offsetLeft + laminas[mejor].offsetWidth / 2 - centro)) mejor = i;
    });
    return mejor;
  };
  const ir = (i) => {
    const l = laminas[Math.max(0, Math.min(laminas.length - 1, i))];
    pista.scrollTo({
      left: l.offsetLeft + l.offsetWidth / 2 - pista.clientWidth / 2,
      behavior: reducir.matches ? "auto" : "smooth",
    });
  };
  const pintar = () => {
    const i = actual();
    cuenta.textContent = `${dos(i + 1)} / ${dos(laminas.length)}`;
    anterior.disabled = i === 0;
    siguiente.disabled = i === laminas.length - 1;
  };

  pista.addEventListener("scroll", pintar, { passive: true });
  addEventListener("resize", pintar);
  pintar();
})();
