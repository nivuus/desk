#!/usr/bin/env bash
# Joue UNE exécution de la recette F5.
#
#     instrument/jouer-f5.sh <etiquette> <secondes-de-maintien> [--sans-purge]
#
# 🔴 DEUX EXÉCUTIONS NE SE CHEVAUCHENT JAMAIS. F1 en a perdu une : deux se sont
# recouvertes de 2 min 23 s, la seconde a tué l'agent de la première EN PLEINE
# MESURE, et *le journal versé sous le nom de la première était celui de la
# seconde*. Ce script tue l'agent AVANT, et le vérifie APRÈS.
#
# Modelé sur `jouer-f3.sh`, dont il reprend le montage (`/tmp/f2/env.sh`).
set -uo pipefail
ETIQUETTE="$1"
MAINTIEN="${2:-30}"
SANS_PURGE="${3:-}"
RACINE=/home/mallanic/Projects/Guacamole
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f5/instrument"
J="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f5"
mkdir -p /tmp/f5

set -a; source "$RACINE/.env"; set +a
source /tmp/f2/env.sh   # le montage de F2, RÉEMPLOYÉ : même VM, même compte, même préfixe

echo "=== [$ETIQUETTE] $(date -u '+%Y-%m-%dT%H:%M:%SZ') — un compte n'est attribuable qu'assorti de son heure ==="
echo "=== [$ETIQUETTE] agents survivants AVANT (un agent SURVIT a l hibernation) ==="
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2

if [ "$SANS_PURGE" = "--sans-purge" ]; then
    echo "=== [$ETIQUETTE] SANS PURGE : la racine et le journal de l execution precedente sont GARDES ==="
    node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\etat-pont.ps1' 2>&1 | tail -6
else
    echo "=== [$ETIQUETTE] purge de la racine du pont et de son etat ==="
    node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\purger-pont.ps1' 2>&1 | tail -4
fi

# 🔴 LES ARTEFACTS DE L'EXÉCUTION PRÉCÉDENTE PARTENT AVANT, TOUS. Payé en F3 :
# un fichier de résultat laissé par une tentative antérieure a été lu comme le
# résultat de celle-ci. C'est le piège de D8 — « on mesure le run d'avant en
# croyant lire le sien » — sous la forme d'un fichier de résultat.
rm -f /media/vm/dev/agent.log "/tmp/f5/pilote-$ETIQUETTE.json" \
      "$J/pilote-$ETIQUETTE.json" "$J/agent-$ETIQUETTE.log" "$J/agent-$ETIQUETTE-plat.log"

echo "=== [$ETIQUETTE] pilote (la shell d abord, l agent ensuite) ==="
APRES_CONNEXION="cd $RACINE && set -a && source .env && set +a && export AGENT_VM=$AGENT_VM AGENT_SECRET=$AGENT_SECRET SUPERVISEUR=1 SIGNALING_URL=ws://192.168.3.1:8080 RUST_LOG=${NIVEAU_LOG:-info,agent::pont=debug} ${PONT_CACHE:+PONT_CACHE=$PONT_CACHE} && scripts/run-agent.sh" \
UDD="/tmp/f5/udd-$ETIQUETTE" PORT_CDP="${PORT_CDP:-9455}" \
    node "$I/pilote-f5.mjs" "$MAINTIEN" "/tmp/f5/pilote-$ETIQUETTE.json" \
    2>&1 | tee "$J/pilote-$ETIQUETTE.log"
CODE=${PIPESTATUS[0]}

echo "=== [$ETIQUETTE] copie du journal d agent (APRES la fin reelle) ==="
# ⚠️ APRÈS la fin réelle : les enfants meurent quand le navigateur se ferme,
# donc APRÈS la copie, et leurs lignes de libération partiraient avec le journal
# suivant. Une pièce a été perdue ainsi en D4.
sleep 6
cp /media/vm/dev/agent.log "$J/agent-$ETIQUETTE.log" 2>/dev/null || echo 'agent.log introuvable'
sed 's/\x1b\[[0-9;]*m//g' "$J/agent-$ETIQUETTE.log" > "$J/agent-$ETIQUETTE-plat.log" 2>/dev/null || true
cp "/tmp/f5/pilote-$ETIQUETTE.json" "$J/pilote-$ETIQUETTE.json" 2>/dev/null || true

echo "=== [$ETIQUETTE] agents survivants APRES (y compris si l execution a echoue) ==="
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2
echo "=== [$ETIQUETTE] code du pilote : $CODE ==="
exit "$CODE"
