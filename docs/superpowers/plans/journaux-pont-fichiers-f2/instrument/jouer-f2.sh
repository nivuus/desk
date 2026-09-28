#!/usr/bin/env bash
# Plays ONE run of acceptance run F2.
#
#     instrument/jouer-f2.sh <etiquette> <secondes-de-maintien>
#
# 🔴 TWO RUNS NEVER OVERLAP. F1 lost one that way: two
# overlapped by 2 min 23 s, the second killed the first one's agent IN THE MIDDLE OF
# MEASURING, and the log filed under the first one's name was the
# second's. This script kills the agent BEFORE, and checks AFTER.
set -uo pipefail
ETIQUETTE="$1"
MAINTIEN="${2:-40}"
# `--sans-purge`: KEEPS the root AND the owed-writes journal of the
# previous run. It is what makes criterion ③ measurable — the restarted
# bridge must REREAD its journal and push again what remains in it.
SANS_PURGE="${3:-}"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f2/instrument"
J="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f2"

set -a; source "$RACINE/.env"; set +a
source /tmp/f2/env.sh

echo "=== [$ETIQUETTE] agents survivants AVANT (un agent SURVIT a l hibernation) ==="
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2

if [ "$SANS_PURGE" = "--sans-purge" ]; then
    echo "=== [$ETIQUETTE] SANS PURGE : la racine et le journal de l execution precedente sont GARDES ==="
    node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\etat-pont.ps1' 2>&1 | tail -6
else
    echo "=== [$ETIQUETTE] purge de la racine du pont et de son etat ==="
    node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\purger-pont.ps1' 2>&1 | tail -4
fi

# 🔴 THE PREVIOUS RUN'S ARTEFACTS GO FIRST, ALL OF THEM.
# Paid for on the spot: a `pilote-arme-1.json` left by an earlier attempt
# was read as this one's result — the measurement showed an empty root and
# refused writes there, and the conclusion would have been "the product is
# broken" whereas the ongoing run succeeded. It is D8's trap
# ("one measures the previous run believing one reads one's own"), in the form of a
# result file rather than a log.
rm -f /media/vm/dev/agent.log "/tmp/f2/pilote-$ETIQUETTE.json" \
      "$J/pilote-$ETIQUETTE.json" "$J/agent-$ETIQUETTE.log" "$J/agent-$ETIQUETTE-plat.log"

echo "=== [$ETIQUETTE] pilote (la shell d abord, l agent ensuite) ==="
APRES_CONNEXION="cd $RACINE && set -a && source .env && set +a && export AGENT_VM=$AGENT_VM AGENT_SECRET=$AGENT_SECRET SUPERVISEUR=1 SIGNALING_URL=ws://192.168.3.1:8080 RUST_LOG=${NIVEAU_LOG:-info,agent::pont=debug} ${PONT_ECRITURE:+PONT_ECRITURE=$PONT_ECRITURE} && scripts/run-agent.sh" \
UDD="/tmp/f2/udd-$ETIQUETTE" PORT_CDP="${PORT_CDP:-9440}" \
    node "$I/pilote-f2.mjs" "$MAINTIEN" "/tmp/f2/pilote-$ETIQUETTE.json" \
    2>&1 | tee "$J/pilote-$ETIQUETTE.log"
CODE=${PIPESTATUS[0]}

echo "=== [$ETIQUETTE] copie du journal d agent (APRES la fin reelle) ==="
# ⚠️ AFTER the real end: the children die when the browser closes,
# hence AFTER the copy, and their release lines would go with the next
# log. One piece of evidence was lost that way in D4.
sleep 6
cp /media/vm/dev/agent.log "$J/agent-$ETIQUETTE.log" 2>/dev/null || echo 'agent.log introuvable'
sed 's/\x1b\[[0-9;]*m//g' "$J/agent-$ETIQUETTE.log" > "$J/agent-$ETIQUETTE-plat.log" 2>/dev/null || true
cp "/tmp/f2/pilote-$ETIQUETTE.json" "$J/pilote-$ETIQUETTE.json" 2>/dev/null || true

echo "=== [$ETIQUETTE] agents survivants APRES (a revenir voir meme si l execution a echoue) ==="
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2
echo "=== [$ETIQUETTE] code de sortie du pilote : $CODE ==="
exit $CODE
