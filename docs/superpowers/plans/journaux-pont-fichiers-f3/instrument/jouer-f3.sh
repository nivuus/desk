#!/usr/bin/env bash
# Joue UNE execution de la recette F3.
#
#     instrument/jouer-f3.sh <etiquette> <secondes-de-maintien>
#
# 🔴 DEUX EXECUTIONS NE SE CHEVAUCHENT JAMAIS. F1 en a perdu une : deux se sont
# recouvertes de 2 min 23 s, la seconde a tue l'agent de la premiere EN PLEINE
# MESURE, et le journal verse sous le nom de la premiere etait celui de la
# seconde. Ce script tue l'agent AVANT, et verifie APRES.
set -uo pipefail
ETIQUETTE="$1"
MAINTIEN="${2:-40}"
# `--sans-purge` : GARDE la racine ET le journal des ecritures dues de
# l'execution precedente. C'est ce qui rend le critere ③ mesurable — le pont
# relance doit RELIRE son journal et repousser ce qui y reste.
SANS_PURGE="${3:-}"
RACINE=/home/mallanic/Projects/Guacamole
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f3/instrument"
J="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f3"

set -a; source "$RACINE/.env"; set +a
source /tmp/f2/env.sh   # ⚠️ le montage de F2, REEMPLOYE : meme VM, meme compte, meme prefixe

echo "=== [$ETIQUETTE] agents survivants AVANT (un agent SURVIT a l hibernation) ==="
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2

if [ "$SANS_PURGE" = "--sans-purge" ]; then
    echo "=== [$ETIQUETTE] SANS PURGE : la racine et le journal de l execution precedente sont GARDES ==="
    node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\etat-pont.ps1' 2>&1 | tail -6
else
    echo "=== [$ETIQUETTE] purge de la racine du pont et de son etat ==="
    node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\purger-pont.ps1' 2>&1 | tail -4
fi

# 🔴 LES ARTEFACTS DE L'EXECUTION PRECEDENTE PARTENT AVANT, TOUS.
# Paye sur place : un `pilote-arme-1.json` laisse par une tentative anterieure a
# ete lu comme le resultat de celle-ci — la mesure y montrait une racine vide et
# des ecritures refusees, et la conclusion aurait ete « le produit est en
# panne » alors que l'execution en cours reussissait. C'est le piege de D8
# (« on mesure le run d'avant en croyant lire le sien »), sous la forme d'un
# fichier de resultat plutot que d'un journal.
rm -f /media/vm/dev/agent.log "/tmp/f3/pilote-$ETIQUETTE.json" \
      "$J/pilote-$ETIQUETTE.json" "$J/agent-$ETIQUETTE.log" "$J/agent-$ETIQUETTE-plat.log"

echo "=== [$ETIQUETTE] pilote (la shell d abord, l agent ensuite) ==="
APRES_CONNEXION="cd $RACINE && set -a && source .env && set +a && export AGENT_VM=$AGENT_VM AGENT_SECRET=$AGENT_SECRET SUPERVISEUR=1 SIGNALING_URL=ws://192.168.3.1:8080 RUST_LOG=${NIVEAU_LOG:-info,agent::pont=debug} ${PONT_ECRITURE:+PONT_ECRITURE=$PONT_ECRITURE} && scripts/run-agent.sh" \
UDD="/tmp/f3/udd-$ETIQUETTE" PORT_CDP="${PORT_CDP:-9450}" \
    node "$I/pilote-f3.mjs" "$MAINTIEN" "/tmp/f3/pilote-$ETIQUETTE.json" \
    2>&1 | tee "$J/pilote-$ETIQUETTE.log"
CODE=${PIPESTATUS[0]}

echo "=== [$ETIQUETTE] copie du journal d agent (APRES la fin reelle) ==="
# ⚠️ APRES la fin reelle : les enfants meurent quand le navigateur se ferme,
# donc APRES la copie, et leurs lignes de liberation partiraient avec le journal
# suivant. Une piece a ete perdue ainsi en D4.
sleep 6
cp /media/vm/dev/agent.log "$J/agent-$ETIQUETTE.log" 2>/dev/null || echo 'agent.log introuvable'
sed 's/\x1b\[[0-9;]*m//g' "$J/agent-$ETIQUETTE.log" > "$J/agent-$ETIQUETTE-plat.log" 2>/dev/null || true
cp "/tmp/f3/pilote-$ETIQUETTE.json" "$J/pilote-$ETIQUETTE.json" 2>/dev/null || true

echo "=== [$ETIQUETTE] agents survivants APRES (a revenir voir meme si l execution a echoue) ==="
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2
echo "=== [$ETIQUETTE] code de sortie du pilote : $CODE ==="
exit $CODE
