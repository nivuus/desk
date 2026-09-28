#!/usr/bin/env bash
# Launches `mesurer.ps1` on the VM WITHOUT BLOCKING WinRM, then waits for the file.
#
# 🔴 WHY ASYNCHRONOUS. A first version called `mesurer.ps1` through
# `winrm.js` synchronously. Copying the 12 MiB file took 182 s; the
# WinRM call came back WELL BEFORE (~62 s), the driver believed the measurement finished, read a
# PARTIAL file (12 lines), then handed back control -- and closing the
# browser cut the bridge IN THE MIDDLE OF THE COPY. The digest coming out of it
# therefore no longer measured the product but the race between the copy and the
# driver's stop.
#
# Here: we start the measurement detached, and wait for `FIN DE MESURE` in the
# file -- the FACT, never a duration (a home-grown trap, sub-block D3).
set -uo pipefail
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
DELAI="${DELAI_MESURE:-900}"

set -a; source "$RACINE/.env"; set +a
rm -f /media/vm/dev/mesure.txt

# ⚠️ THE LAUNCH GOES THROUGH A `.ps1`, NEVER THROUGH AN INLINE COMMAND.
# `nodejs-winrm` wraps EVERYTHING in `powershell -Command "& { ... }"`: a
# `Start-Process ... -ArgumentList '-NoProfile','-File','C:\dev\mesurer.ps1'`
# passed inline loses its quotes there, the command NEVER runs, and the symptom
# is a measurement file that does not appear -- indistinguishable from a slow
# measurement. Measured: launched through `.ps1`, the file exists in under 3 s.
node "$RACINE/scripts/winrm.js" \
  'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\lancer-mesure.ps1' \
  >/dev/null 2>&1

debut=$(date +%s)
while true; do
    if [ -f /media/vm/dev/mesure.txt ] && grep -aq 'FIN DE MESURE' /media/vm/dev/mesure.txt 2>/dev/null; then
        echo "# mesure terminee en $(( $(date +%s) - debut )) s"
        break
    fi
    if [ $(( $(date +%s) - debut )) -ge "$DELAI" ]; then
        echo "# DELAI DE $DELAI s DEPASSE : mesure INCOMPLETE, telle quelle"
        break
    fi
    sleep 3
done
cat /media/vm/dev/mesure.txt 2>/dev/null || echo '# aucun fichier de mesure'
