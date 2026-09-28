#!/usr/bin/env bash
# Copies the measurement to the VM, launches it THROUGH AN /it SCHEDULED TASK, and returns its JSON.
# Modelled on `mesurer-f3.sh`, from which it takes over the TWO traps paid for there:
#   - kill the frozen measurements first, THROUGH A FILE and never inline
#     (`nodejs-winrm` wraps in `powershell -Command "& { … }"`, and an
#     inline script with double quotes NEVER RUNS, silently);
#   - a FRESH output path at each run, which no frozen process
#     can hold.
set -uo pipefail
PHASE="${1:-lister}"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f5/instrument"
set -a; source "$RACINE/.env"; set +a

cat > /media/vm/dev/tuer-mesures-f5.ps1 <<'PSKILL'
schtasks /end /tn mesure-f5 2>$null | Out-Null
Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*mesure-f5*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
PSKILL
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\tuer-mesures-f5.ps1' >/dev/null 2>&1

cp "$I/mesurer-f5.ps1" /media/vm/dev/mesurer-f5.ps1
HORO=$(date +%H%M%S%N)
SORTIE="C:\\dev\\mesure-f5-$HORO.json"
LOCAL="/media/vm/dev/mesure-f5-$HORO.json"
rm -f "$LOCAL"
cat > /media/vm/dev/lancer-mesure-f5.ps1 <<PS1
& 'C:\dev\mesurer-f5.ps1' -sortie '$SORTIE' -phase '$PHASE' *>&1 | Out-File -FilePath 'C:\dev\mesure-f5.trace.txt' -Encoding utf8
PS1

node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn mesure-f5 /f 2>\$null; \
   schtasks /create /tn mesure-f5 /f /it /ru '$WINDOWS_ADMIN_USERNAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\lancer-mesure-f5.ps1'; \
   schtasks /run /tn mesure-f5" >/dev/null 2>&1

# Wait for the FACT, never a duration. Bound: DELAI_LISTER is 20 s on the bridge side,
# and a listing beyond the wall freezes exactly that long — the bound must therefore
# be SEVERAL TIMES longer, never set on it.
for i in $(seq 1 60); do
    [ -s "$LOCAL" ] && { cat "$LOCAL"; exit 0; }
    sleep 2
done
echo '{"erreur":"mesure-f5 absente au terme de 120 s"}'
