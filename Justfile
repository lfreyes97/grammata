# Justfile para Grammata — perfiles de compilación según la máquina.
#
#   just              lista las recetas
#   just dev-fast     R3 (4 núcleos): compila rápido, preview PDF lento
#   just dev-full     R7: perfil completo del repo, preview PDF rápido
#
# El perfil "fast" solo cambia los flags de cargo: no toca Cargo.toml,
# así que la misma rama sirve en las dos máquinas. El primer dev-fast
# recompila las dependencias; las iteraciones posteriores son rápidas.

alias df := dev-fast
alias cs := check-fast

# Por defecto, muestra la ayuda.
default:
    @just --list

# R3 — arrancar con dependencias sin optimizar y sin debuginfo.
dev-fast:
    cargo --config 'profile.dev.package."*".opt-level=0' \
          --config 'profile.dev.debug=0' \
          run

# R7 — arrancar con el perfil normal del repositorio.
dev-full:
    cargo run

# R3 — solo comprobar (sin codegen ni link), lo más rápido para iterar.
check-fast:
    cargo --config 'profile.dev.package."*".opt-level=0' \
          --config 'profile.dev.debug=0' \
          check

# R3 — compilar sin arrancar.
build-fast:
    cargo --config 'profile.dev.package."*".opt-level=0' \
          --config 'profile.dev.debug=0' \
          build

# R3 — tests con el perfil rápido.
test-fast:
    cargo --config 'profile.dev.package."*".opt-level=0' \
          --config 'profile.dev.debug=0' \
          test

# Release, sin cambios (LTO + strip, tal como está en Cargo.toml).
release:
    cargo build --release

# Muestra el estado de la máquina antes de decidir qué receta usar.
info:
    @echo "CPUs: $(nproc)"
    @free -h | head -2
    @df -h . | tail -1
