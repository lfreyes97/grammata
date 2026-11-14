# Paquete de Grammata. Se usa desde flake.nix, pero también sirve solo:
#   nix-build -E 'with import <nixpkgs> {}; callPackage ./nix/package.nix {}'
{
  lib,
  rustPlatform,
  pkg-config,
  makeWrapper,
  openssl,
  zlib,
  zstd,
  brotli,
  fontconfig,
  libGL,
  vulkan-loader,
  libxkbcommon,
  wayland,
  libx11,
  libxcursor,
  libxi,
  libxrandr,
}:

let
  cargoToml = builtins.fromTOML (builtins.readFile ../Cargo.toml);

  # Bibliotecas que winit/eframe cargan con dlopen: hay que ponerlas en el RPATH.
  runtimeLibs = [
    libGL
    vulkan-loader
    libxkbcommon
    wayland
    libx11
    libxcursor
    libxi
    libxrandr
  ];
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
      ../assets
      ../packaging
      ../ejemplo.typ
      ../LICENSE
      ../README.md
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];
  buildInputs = [
    openssl
    zlib
    zstd
    brotli
  ];

  # `typst-kit` pide `vendor-openssl` (para los binarios publicados); aquí se
  # usa el OpenSSL de nixpkgs.
  env.OPENSSL_NO_VENDOR = 1;

  # Las pruebas de interfaz necesitan GPU; el resto corre con `cargo test`.
  doCheck = false;

  postInstall = ''
    for s in 16 24 32 48 64 128 256 512; do
      install -Dm644 assets/icons/grammata-$s.png \
        $out/share/icons/hicolor/''${s}x''${s}/apps/grammata.png
    done
    install -Dm644 assets/logo.svg $out/share/icons/hicolor/scalable/apps/grammata.svg
    install -Dm644 packaging/grammata.desktop $out/share/applications/grammata.desktop
    install -Dm644 LICENSE $out/share/doc/grammata/LICENSE
  '';

  postFixup = ''
    patchelf --add-rpath ${lib.makeLibraryPath runtimeLibs} $out/bin/grammata
    # El tema busca las fuentes con `fc-match`.
    wrapProgram $out/bin/grammata --prefix PATH : ${lib.makeBinPath [ fontconfig ]}
  '';

  passthru = { inherit runtimeLibs; };

  meta = {
    description = cargoToml.package.description;
    homepage = "https://github.com/lfreyes97/grammata";
    license = lib.licenses.gpl3Plus;
    mainProgram = "grammata";
    platforms = lib.platforms.linux;
  };
}
