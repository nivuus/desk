#!/usr/bin/env bash
# Produces agent.exe WITHOUT the VM, and drops it at the given destination.
#
# 🔴 WHY THIS SCRIPT EXISTS. `console/guest/payload.py` declares the agent
# "extracted before the wipe" and `fetch_payload.py` writes "Not fetched, and
# never fetchable": the appliance is rebuilt today around a
# binary nobody knows how to rebuild. This one rebuilds it.
#
# ⚠️ WHAT IT DOES NOT ESTABLISH: that the binary WORKS. It links; it is a
# mingw product where the old one was built on the VM with MSVC, and it has never
# run. See the spec § 4.3: the judge is a run on the VM.
set -euo pipefail
unset -f chpwd 2>/dev/null || true

CIBLE_RUST="${CIBLE_RUST:-x86_64-pc-windows-gnu}"
DESTINATION="${1:-}"

if [ -z "${DESTINATION}" ]; then
    echo "usage: $0 <destination>   (the directory to drop agent.exe into)" >&2
    echo "no default destination: a default would drop 20 MiB at a" >&2
    echo "place nobody asked for." >&2
    exit 2
fi

RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${RACINE}"

if ! rustup target list --installed 2>/dev/null | grep -qx "${CIBLE_RUST}"; then
    echo "🔴 rustup target missing: ${CIBLE_RUST}" >&2
    echo "   install it: rustup target add ${CIBLE_RUST}" >&2
    exit 1
fi

if [ "${CIBLE_RUST}" = "x86_64-pc-windows-gnu" ] \
   && ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    echo "🔴 linker missing: x86_64-w64-mingw32-gcc" >&2
    echo "   install it: apt install gcc-mingw-w64-x86-64" >&2
    exit 1
fi

cargo build --release --target "${CIBLE_RUST}" -p agent

BINAIRE="target/${CIBLE_RUST}/release/agent.exe"
[ -f "${BINAIRE}" ] || { echo "🔴 cargo succeeded but ${BINAIRE} is absent" >&2; exit 1; }

mkdir -p "${DESTINATION}"
cp "${BINAIRE}" "${DESTINATION}/agent.exe"
echo "agent.exe dropped: ${DESTINATION}/agent.exe ($(stat -c%s "${BINAIRE}") bytes)"
