#!/usr/bin/env bash
# Copies the measurement to the VM, launches it THROUGH AN /it SCHEDULED TASK, and returns its JSON.
#
# 🔴 INTERACTIVE SESSION AND NOT WinRM. The ProjFS root is mounted by a
# session 1 process; writing it from session 0 would still go through
# the provider, but criterion ②'s Notepad has no desktop there. One
# single path for both is one path less to explain.
set -uo pipefail
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f3/instrument"
set -a; source "$RACINE/.env"; set +a

# 🔴 KILL THE FROZEN MEASUREMENTS FIRST, AND IT WAS PAID FOR ON THE SPOT.
#
# A frozen attempt leaves its `powershell` alive, and that process HOLDS
# `mesure-f3.json` through the handle of its `Out-File`. The next measurement then writes
# into the void: `Set-Content` returns "the file is being used
# by another process", and the reading is lost **for a
# reason that has nothing to do with what is measured**. It is the home-grown trap of
# D8 — "a driver that leaves a process alive SILENTLY blocks the
# next attempt" — in a new form.
# ⚠️ THE KILLER GOES THROUGH A FILE, NEVER THROUGH AN INLINE COMMAND.
# `nodejs-winrm` ALWAYS wraps the command in `powershell -Command
# "& { ... }"`: an inline script carrying double quotes collides with
# that wrapper, **and the symptom is a script that never
# runs** (trap D3). A first wording of this killer paid for it: it
# killed no one, silently, and the lock survived three attempts.
cat > /media/vm/dev/tuer-mesures.ps1 <<'PSKILL'
schtasks /end /tn mesure-f3 2>$null | Out-Null
Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*mesure-f3*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
PSKILL
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\tuer-mesures.ps1' >/dev/null 2>&1

cp "$I/mesurer-f3.ps1" /media/vm/dev/mesurer-f3.ps1
HORO=$(date +%H%M%S)
SORTIE="C:\\dev\\mesure-f3-$HORO.json"
LOCAL="/media/vm/dev/mesure-f3-$HORO.json"
rm -f "$LOCAL" /media/vm/dev/mesure-f3.trace.txt
cat > /media/vm/dev/lancer-mesure-f3.ps1 <<PS1
# 🔴 LA TRACE ET LE RELEVE NE PARTAGENT PLUS LE MEME FICHIER.
#
# Ce lanceur redirigeait TOUT vers `mesure-f3.json`. Depuis que la mesure
# ecrit elle-meme ses deux points de reprise dans ce fichier, les deux
# ecrivains se disputeraient la meme poignee : `Out-File` le tronque a
# l'ouverture et le tient ouvert jusqu'a la fin, donc un `Set-Content`
# intercalaire est perdu ou refuse. **Le releve serait vide precisement quand
# le point de reprise sert a quelque chose.**
& 'C:\dev\mesurer-f3.ps1' -sortie '$SORTIE' *>&1 | Out-File -FilePath 'C:\dev\mesure-f3.trace.txt' -Encoding utf8
PS1

node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn mesure-f3 /f 2>\$null; \
   schtasks /create /tn mesure-f3 /f /it /ru '$WINDOWS_ADMIN_USERNAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\lancer-mesure-f3.ps1'; \
   schtasks /run /tn mesure-f3" >/dev/null 2>&1

# Wait for the FACT, never a duration (a home-grown trap, D3).
#
# ⚠️ THE AWAITED FACT IS CHECKPOINT NO. 2 (`apres2`), and the wait is
# BOUNDED: if the measurement freezes on a gesture, checkpoint no. 1 is already
# written and carries criteria ① and ②. **We then return what we have, SAYING
# SO** — an announced partial reading is worth more than an empty reading.
COMPLET=non
for i in $(seq 1 90); do
    if [ -s "$LOCAL" ] && grep -qa 'apres2' "$LOCAL"; then COMPLET=oui; break; fi
    sleep 2
done
cp /media/vm/dev/mesure-f3.trace.txt "${TRACE:-/dev/null}" 2>/dev/null || true
if [ "$COMPLET" = non ]; then
    echo "MESURE PARTIELLE : le point de reprise n°2 n'est pas arrive en 180 s." >&2
fi
cat "$LOCAL" 2>/dev/null || echo '{"erreur":"mesure-f3.json absent"}'
