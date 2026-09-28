#!/usr/bin/env bash
# The EMULATED ROOM of block E3: a loudspeaker and a microphone, digitally.
#
#     salle.sh <secondes-de-vie>
#
# 🔴 THIS SCRIPT TOUCHES THE AUDIO GRAPH OF THE MACHINE'S OWNER, and it only
# runs with their consent (the plan's Decision 5). Three consequences,
# all declared:
#
#   1. wireplumber CAN elect the new node as the default output. The reading
#      of August 21st, 2026 — redone THAT DAY and not copied from the plan — shows that the
#      default is already `auto_null`, a fallback output, and that there is
#      **NO `Audio/Source`**: nothing real to disturb. The risk is slim,
#      **it is not zero**;
#   2. the node DISAPPEARS WITH THE PROCESS — `pw-loopback` is a client, not
#      a daemon module. Clean-up is therefore stopping the process, and it is
#      in a `trap`, **never in the last line**;
#   3. no write into `~mallanic`, no persistent configuration,
#      no `systemctl`. **The bench is one process and two windows.**
#
# 🔴 THE GRAPH IS COMPARED BY SET OF NAMES, NEVER BY NUMBER — a lesson from the
# virtual outputs workstream: a third party can add a node, and an
# external addition exactly compensates a removal on a cardinality check.
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
