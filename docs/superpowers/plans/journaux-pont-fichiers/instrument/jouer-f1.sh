#!/usr/bin/env bash
# Joue UNE execution complete de la recette F1.
#
#     instrument/jouer-f1.sh <etiquette> <secondes-de-maintien> [--sans-projfs]
#
# 🔴 L'ORDRE EST LE POINT DE CE SCRIPT. La page-shell doit etre connectee AVANT
# que le superviseur ne demarre : le signaling ne memorise que les offres SDP,
# et les annonces `fenetre-ouverte` emises avant la connexion de la shell sont
# PERDUES SANS TRACE (CLAUDE.md, sous-bloc D1). C'est pourquoi le pilote lance
# l'agent lui-meme, par `APRES_CONNEXION`, plutot que ce script avant lui.
set -uo pipefail

ETIQUETTE="$1"
MAINTIEN="$2"
SANS_PROJFS="${3:-}"

RACINE=/home/mallanic/Projects/Guacamole
INSTR="$RACINE/docs/superpowers/plans/journaux-pont-fichiers/instrument"
J="$RACINE/docs/superpowers/plans/journaux-pont-fichiers"
SCRATCH="${SCRATCH:?SCRATCH doit pointer le repertoire de travail hors depot}"

set -a; source "$RACINE/.env"; set +a
source "$SCRATCH/f1/env.sh"
export AGENT_VM AGENT_SECRET

echo "=== [$ETIQUETTE] nettoyage de la VM ==="
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\nettoyer-vm.ps1' 2>&1 | tail -8

if [ "$SANS_PROJFS" = "--sans-projfs" ]; then
    echo "=== [$ETIQUETTE] renommage de ProjectedFSLib.dll ==="
    node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\dll-renommer.ps1' 2>&1 | tail -3
fi

echo "=== [$ETIQUETTE] ouverture de la mire (session interactive) ==="
node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn it-mire /f 2>\$null; \
   schtasks /create /tn it-mire /f /it /ru '$WINDOWS_ADMIN_USERNAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\it-mire.ps1'; \
   schtasks /run /tn it-mire" >/dev/null 2>&1
sleep 10

rm -f /media/vm/dev/agent.log

echo "=== [$ETIQUETTE] pilote (la shell d'abord, l'agent ensuite) ==="
SUPERVISEUR=1 SIGNALING_URL=ws://192.168.3.1:8080 RUST_LOG="${NIVEAU_LOG:-info}" \
APRES_CONNEXION="cd $RACINE && set -a && source .env && set +a && export AGENT_VM=$AGENT_VM AGENT_SECRET=$AGENT_SECRET SUPERVISEUR=1 SIGNALING_URL=ws://192.168.3.1:8080 RUST_LOG=${NIVEAU_LOG:-info} && scripts/run-agent.sh" \
PENDANT_MAINTIEN="${PENDANT:-DELAI_MESURE=${DELAI_MESURE:-540} bash $INSTR/mesurer.sh}" \
PREFIXE_VM="$PREFIXE_VM" UDD="$SCRATCH/f1/udd-$ETIQUETTE" PORT_CDP="${PORT_CDP:-9420}" \
    node "$INSTR/pilote-f1.mjs" "$MAINTIEN" "$SCRATCH/f1/pilote-$ETIQUETTE.json" \
    2>&1 | tee "$J/pilote-$ETIQUETTE.log"
CODE=${PIPESTATUS[0]}

echo "=== [$ETIQUETTE] copie du journal d'agent (APRES la fin reelle) ==="
# ⚠️ Copier APRES la fin reelle : les enfants meurent quand le navigateur se
# ferme, donc APRES la copie, et leurs lignes de liberation partiraient avec le
# journal suivant. Une piece a ete perdue ainsi en D4.
sleep 5
cp /media/vm/dev/agent.log "$J/agent-$ETIQUETTE.log" 2>/dev/null || echo 'agent.log introuvable'
sed 's/\x1b\[[0-9;]*m//g' "$J/agent-$ETIQUETTE.log" > "$J/agent-$ETIQUETTE-plat.log" 2>/dev/null || true

if [ "$SANS_PROJFS" = "--sans-projfs" ]; then
    echo "=== [$ETIQUETTE] RESTAURATION de ProjectedFSLib.dll ==="
    node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\dll-restaurer.ps1' 2>&1 | tail -3
fi

echo "=== [$ETIQUETTE] code de sortie du pilote : $CODE ==="
exit $CODE
