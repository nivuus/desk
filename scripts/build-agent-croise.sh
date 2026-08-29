#!/usr/bin/env bash
# Produit agent.exe SANS la VM, et le dépose à la destination donnée.
#
# 🔴 POURQUOI CE SCRIPT EXISTE. `console/guest/payload.py` déclare l'agent
# « extracted before the wipe » et `fetch_payload.py` écrit « Not fetched, and
# never fetchable » : l'appliance se reconstruit aujourd'hui autour d'un
# binaire que personne ne sait refabriquer. Celui-ci le refabrique.
#
# ⚠️ CE QU'IL N'ÉTABLIT PAS : que le binaire FONCTIONNE. Il se lie ; c'est un
# produit mingw là où l'ancien était bâti sur la VM en MSVC, et il n'a jamais
# tourné. Voir la spec § 4.3 : le juge est une exécution sur la VM.
set -euo pipefail
unset -f chpwd 2>/dev/null || true

CIBLE_RUST="${CIBLE_RUST:-x86_64-pc-windows-gnu}"
DESTINATION="${1:-}"

if [ -z "${DESTINATION}" ]; then
    echo "usage : $0 <destination>   (le répertoire où déposer agent.exe)" >&2
    echo "aucune destination par défaut : un défaut déposerait 20 Mio à un" >&2
    echo "endroit que personne n'a demandé." >&2
    exit 2
fi

RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${RACINE}"

if ! rustup target list --installed 2>/dev/null | grep -qx "${CIBLE_RUST}"; then
    echo "🔴 cible rustup absente : ${CIBLE_RUST}" >&2
    echo "   l'installer : rustup target add ${CIBLE_RUST}" >&2
    exit 1
fi

if [ "${CIBLE_RUST}" = "x86_64-pc-windows-gnu" ] \
   && ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    echo "🔴 éditeur de liens absent : x86_64-w64-mingw32-gcc" >&2
    echo "   l'installer : apt install gcc-mingw-w64-x86-64" >&2
    exit 1
fi

cargo build --release --target "${CIBLE_RUST}" -p agent

BINAIRE="target/${CIBLE_RUST}/release/agent.exe"
[ -f "${BINAIRE}" ] || { echo "🔴 cargo a réussi mais ${BINAIRE} est absent" >&2; exit 1; }

mkdir -p "${DESTINATION}"
cp "${BINAIRE}" "${DESTINATION}/agent.exe"
echo "agent.exe déposé : ${DESTINATION}/agent.exe ($(stat -c%s "${BINAIRE}") octets)"
