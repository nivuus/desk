#!/usr/bin/env bash
# Joue UNE execution de la recette F4.
#
#     jouer-f4.sh <etiquette>
#
# Variables lues : GABARITS, PLAN_VM, REPOS_MESURE, NEUTRALISER_MOVE,
#                  MONTAGE (m1|m2), NIVEAU_LOG, PONT_MESURE.
#
# 🔴 DEUX EXECUTIONS NE SE CHEVAUCHENT JAMAIS. F1 en a perdu une : deux se sont
# recouvertes de 2 min 23 s, la seconde a tue l'agent de la premiere EN PLEINE
# MESURE, et LE JOURNAL VERSE SOUS LE NOM DE LA PREMIERE ETAIT CELUI DE LA
# SECONDE. Ce script tue l'agent AVANT, et RECOMPTE APRES — y compris si
# l'execution a echoue.
set -uo pipefail
ETIQUETTE="$1"
RACINE=/home/mallanic/Projects/Guacamole
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f4/instrument"
J="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f4"
MONTAGE="${MONTAGE:-m1}"

# ⚠️ Le hook `chpwd` du shell de l'hote injecte un `ls` dans toute sortie des
# qu'un `cd` court dans un sous-shell (S2 a du reprendre une passe entiere).
unset -f chpwd 2>/dev/null || true

set -a; source "$RACINE/.env"; set +a
source /tmp/f2/env.sh   # le montage de F2, REEMPLOYE : meme VM, meme compte, meme prefixe

# ⚠️ L'HOTE PORTE UN DEFAUT NON IDENTIFIE : `/dev/null` a ete trouve remplace
# par un fichier ordinaire, ce qui empeche toute VM de demarrer. Repare, cause
# inconnue, PEUT SE REPRODUIRE.
[ "$(stat -c '%F' /dev/null)" = 'character special file' ] || { echo 'ARRET : /dev/null n est pas un noeud de caracteres'; exit 3; }

echo "=== [$ETIQUETTE] VM et agents survivants AVANT ==="
virsh list --all 2>&1 | sed -n '3p'
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2

echo "=== [$ETIQUETTE] purge de la racine du pont et de son etat ==="
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\purger-pont.ps1' 2>&1 | tail -4

# 🔴 LES ARTEFACTS DE L'EXECUTION PRECEDENTE PARTENT AVANT, TOUS. Paye sur
# place en F3 : un JSON laisse par une tentative anterieure a ete lu comme le
# resultat de celle-ci, et la conclusion aurait ete « le produit est en panne »
# alors que l'execution en cours reussissait.
rm -f /media/vm/dev/agent.log /media/vm/dev/mesure-f4-*.json \
      "/tmp/f4/pilote-$ETIQUETTE.json" \
      "$J/pilote-$ETIQUETTE.json" "$J/agent-$ETIQUETTE.log" "$J/agent-$ETIQUETTE-plat.log" \
      "$J/pilote-$ETIQUETTE.log" "$J/mesure-$ETIQUETTE-trace.txt"
mkdir -p /tmp/f4

# 🔵 M1 — LE PONT SEUL : ni capteur, ni fenetre, ni encodeur, ni PeerConnection
# video. `main.rs` branche `PONT` APRES `CAPTEUR` et AVANT le superviseur, et
# `pont.rs` fait son PROPRE signaling sur `SESSION_ID` — donc un agent lance
# avec `PONT=1` et `SESSION_ID=<prefixe>:fichiers` rencontre la page-shell
# existante SANS QU'AUCUNE LIGNE DE CLIENT NE CHANGE.
if [ "$MONTAGE" = "m1" ]; then
    MODE="PONT=1 SESSION_ID=$PREFIXE_VM:fichiers"
else
    MODE="SUPERVISEUR=1"
fi
echo "=== [$ETIQUETTE] montage $MONTAGE : $MODE ==="

APRES_CONNEXION="cd $RACINE && set -a && source .env && set +a && export AGENT_VM=$AGENT_VM AGENT_SECRET=$AGENT_SECRET $MODE SIGNALING_URL=ws://192.168.3.1:8080 RUST_LOG=${NIVEAU_LOG:-info} PONT_MESURE=${PONT_MESURE:-1} && scripts/run-agent.sh" \
UDD="/tmp/f4/udd-$ETIQUETTE" PORT_CDP="${PORT_CDP:-9460}" \
TRACE="$J/mesure-$ETIQUETTE-trace.txt" \
    node "$I/pilote-f4.mjs" "/tmp/f4/pilote-$ETIQUETTE.json" \
    2>&1 | tee "$J/pilote-$ETIQUETTE.log"
CODE=${PIPESTATUS[0]}

echo "=== [$ETIQUETTE] copie du journal d agent (APRES la fin reelle) ==="
# ⚠️ APRES la fin reelle : les lignes de liberation partiraient avec le journal
# suivant. Une piece a ete perdue ainsi en D4.
sleep 6
cp /media/vm/dev/agent.log "$J/agent-$ETIQUETTE.log" 2>/dev/null || echo 'agent.log introuvable'
sed 's/\x1b\[[0-9;]*m//g' "$J/agent-$ETIQUETTE.log" > "$J/agent-$ETIQUETTE-plat.log" 2>/dev/null || true
cp "/tmp/f4/pilote-$ETIQUETTE.json" "$J/pilote-$ETIQUETTE.json" 2>/dev/null || true

echo "=== [$ETIQUETTE] agents survivants APRES (a revenir voir MEME si l execution a echoue) ==="
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2
virsh list --all 2>&1 | sed -n '3p'
echo "=== [$ETIQUETTE] code de sortie du pilote : $CODE ==="
exit $CODE
