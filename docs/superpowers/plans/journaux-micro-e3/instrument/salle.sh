#!/usr/bin/env bash
# La SALLE ÉMULÉE du bloc E3 : un haut-parleur et un microphone, en numérique.
#
#     salle.sh <secondes-de-vie>
#
# 🔴 CE SCRIPT TOUCHE LE GRAPHE AUDIO DU PROPRIÉTAIRE DE LA MACHINE, et il ne
# tourne qu'avec son consentement (Décision 5 du plan). Trois conséquences,
# toutes déclarées :
#
#   1. wireplumber PEUT élire le nœud neuf comme sortie par défaut. Le relevé
#      du 21 août 2026 — refait CE JOUR et non recopié du plan — montre que le
#      défaut est déjà `auto_null`, une sortie de repli, et qu'il n'y a
#      **AUCUN `Audio/Source`** : rien de réel à perturber. Le risque est mince,
#      **il n'est pas nul** ;
#   2. le nœud DISPARAÎT AVEC LE PROCESSUS — `pw-loopback` est un client, pas
#      un module du démon. Le nettoyage est donc l'arrêt du processus, et il est
#      dans un `trap`, **jamais dans la dernière ligne** ;
#   3. aucune écriture dans `~mallanic`, aucune configuration persistante,
#      aucun `systemctl`. **Le banc est un processus et deux fenêtres.**
#
# 🔴 LE GRAPHE EST COMPARÉ PAR ENSEMBLE DE NOMS, JAMAIS PAR NOMBRE — leçon du
# chantier des sorties virtuelles : un tiers peut ajouter un nœud, et une
# addition externe compense exactement un retrait sur un contrôle par cardinal.
set -uo pipefail
VIE="${1:-120}"
export XDG_RUNTIME_DIR=/run/user/1000
J="$(dirname "$0")/.."

noms() { pw-dump 2>/dev/null | python3 -c "
import json,sys
try: d=json.load(sys.stdin)
except Exception: sys.exit(0)
for o in d:
    if o.get('type')!='PipeWire:Interface:Node': continue
    p=o.get('info',{}).get('props',{})
    if p.get('media.class','').startswith('Audio/'): print(p.get('node.name','?'))
" | sort -u; }

noms > /tmp/e3/graphe-avant.txt
echo "=== graphe AVANT ($(wc -l < /tmp/e3/graphe-avant.txt) nœuds Audio/*) ==="
cat /tmp/e3/graphe-avant.txt

PID=""
nettoyer() {
    [ -n "$PID" ] && kill "$PID" 2>/dev/null
    sleep 1
    noms > /tmp/e3/graphe-apres.txt
    echo "=== graphe APRÈS ==="
    cat /tmp/e3/graphe-apres.txt
    echo "=== écart d'ENSEMBLES DE NOMS (vide = le graphe est rendu intact) ==="
    diff /tmp/e3/graphe-avant.txt /tmp/e3/graphe-apres.txt && echo "AUCUN ÉCART"
}
trap nettoyer EXIT INT TERM

pw-loopback \
    --capture-props='media.class=Audio/Sink node.name=salle_e3 node.description="Salle E3 (haut-parleur)"' \
    --playback-props='media.class=Audio/Source node.name=salle_e3_micro node.description="Salle E3 (microphone)"' \
    >/tmp/e3/salle.log 2>&1 &
PID=$!
sleep 2
if ! kill -0 "$PID" 2>/dev/null; then
    echo "🔴 pw-loopback est mort : la salle n'existe pas"
    cat /tmp/e3/salle.log
    exit 2
fi
echo "=== la salle existe ? (les deux nœuds, par leur NOM) ==="
noms | grep -E '^salle_e3' || { echo "🔴 les nœuds de la salle sont ABSENTS du graphe"; exit 3; }
echo "SALLE_PID=$PID"
sleep "$VIE"
