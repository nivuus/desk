#!/usr/bin/env bash
# Copies the measurement to the VM, launches it THROUGH AN /it SCHEDULED TASK, and returns its JSON.
#
# 🔴 INTERACTIVE SESSION AND NOT WinRM. The ProjFS root is mounted by a
# session 1 process; writing it from session 0 would still go through
# the provider, but criterion ②'s Notepad has no desktop there. One
# single path for both is one path less to explain.
set -uo pipefail
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f2/instrument"
set -a; source "$RACINE/.env"; set +a

cp "$I/mesurer-f2.ps1" /media/vm/dev/mesurer-f2.ps1
cat > /media/vm/dev/lancer-mesure-f2.ps1 <<'PS1'
& 'C:\dev\mesurer-f2.ps1' *>&1 | Out-File -FilePath 'C:\dev\mesure-f2.json' -Encoding utf8
PS1
rm -f /media/vm/dev/mesure-f2.json

node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn mesure-f2 /f 2>\$null; \
   schtasks /create /tn mesure-f2 /f /it /ru '$WINDOWS_ADMIN_USERNAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\lancer-mesure-f2.ps1'; \
   schtasks /run /tn mesure-f2" >/dev/null 2>&1

# Wait for the FACT, never a duration (a home-grown trap, D3).
for i in $(seq 1 60); do
    if [ -s /media/vm/dev/mesure-f2.json ] && grep -qa 'apres_compte' /media/vm/dev/mesure-f2.json; then break; fi
    sleep 2
done
cat /media/vm/dev/mesure-f2.json 2>/dev/null || echo '{"erreur":"mesure-f2.json absent"}'
