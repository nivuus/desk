#!/usr/bin/env bash
# Plays ONE run of bench S1 / S1bis.
#
#     jouer-banc.sh <s1|s1bis> <etiquette> [--xvfb]
#
# 🔴 THE CLEAN-UP IS IN A `trap`, NEVER IN THE LAST LINE: the room
# and the user's audio defaults are given back even if the bench fails,
# even if it is interrupted.
set -uo pipefail
MODE="${1:?s1 ou s1bis}"; ETIQ="${2:?etiquette}"; XV="${3:-}"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-micro-e3/instrument"
J="$RACINE/docs/superpowers/plans/journaux-micro-e3"
export XDG_RUNTIME_DIR=/run/user/1000
# 🔴 EXPLICIT `PULSE_SERVER`: as root, the PulseAudio library refuses an
# `XDG_RUNTIME_DIR` that does not belong to it, and the WHOLE bench is
# skewed by it without saying so — see the § of `banc-aec.mjs`.
export PULSE_SERVER=unix:/run/user/1000/pulse/native
mkdir -p /tmp/e3

echo "=== [$ETIQ] $(date -u '+%Y-%m-%dT%H:%M:%SZ') ==="
echo "=== [$ETIQ] espace disque (les WAV et les profils Chrome vivent dans /tmp) ==="
df -h /tmp | tail -1

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
DEF_SINK="$(pactl get-default-sink 2>/dev/null || echo '?')"
DEF_SRC="$(pactl get-default-source 2>/dev/null || echo '?')"
echo "=== [$ETIQ] graphe AVANT : $(tr '\n' ' ' < /tmp/e3/graphe-avant.txt)"
echo "=== [$ETIQ] défauts AVANT : sink=$DEF_SINK source=$DEF_SRC"

SALLE=""; XPID=""
nettoyer() {
    echo "=== [$ETIQ] NETTOYAGE (trap) ==="
    [ -n "$SALLE" ] && kill "$SALLE" 2>/dev/null
    [ -n "$XPID" ] && kill "$XPID" 2>/dev/null
    sleep 1
    # ⚠️ The defaults are RESTORED TO THEIR READ VALUE, never to an assumed
    # value: `auto_null` is THIS machine's default today, it
    # might not be tomorrow.
    [ "$DEF_SINK" != "?" ] && pactl set-default-sink "$DEF_SINK" 2>/dev/null
    [ "$DEF_SRC" != "?" ] && pactl set-default-source "$DEF_SRC" 2>/dev/null
    noms > /tmp/e3/graphe-apres.txt
    echo "=== [$ETIQ] graphe APRÈS : $(tr '\n' ' ' < /tmp/e3/graphe-apres.txt)"
    echo "=== [$ETIQ] défauts APRÈS : sink=$(pactl get-default-sink 2>/dev/null) source=$(pactl get-default-source 2>/dev/null)"
    echo "=== [$ETIQ] écart d'ENSEMBLES DE NOMS (vide = graphe rendu intact) ==="
    diff /tmp/e3/graphe-avant.txt /tmp/e3/graphe-apres.txt && echo "AUCUN ÉCART"
}
trap nettoyer EXIT INT TERM

pw-loopback \
    --capture-props='media.class=Audio/Sink node.name=salle_e3 node.description="Salle E3 (haut-parleur)"' \
    --playback-props='media.class=Audio/Source node.name=salle_e3_micro node.description="Salle E3 (microphone)"' \
    >/tmp/e3/salle.log 2>&1 &
SALLE=$!
sleep 2
noms | grep -qE '^salle_e3$' || { echo "🔴 [$ETIQ] la salle n'existe pas"; cat /tmp/e3/salle.log; exit 2; }
# ⚠️ **wireplumber ELECTS THE ROOM AS DEFAULT ON ITS OWN** — it is risk no. 1
# of Decision 5, and it DOES HAPPEN. It is benign here (the previous default is
# `auto_null`, a fallback output, and nothing was playing), and **it undoes itself
# when the process dies**: measured, `auto_null` becomes the default again.
# We still set the defaults explicitly — an automatic election is
# not a guarantee —, and we RESTORE them to their READ value in the `trap`.
pactl set-default-sink salle_e3 2>/dev/null
pactl set-default-source salle_e3_micro 2>/dev/null
echo "=== [$ETIQ] salle posée ; défauts : sink=$(pactl get-default-sink) source=$(pactl get-default-source)"

DISP=""
if [ "$XV" = "--xvfb" ]; then
    Xvfb :77 -screen 0 1280x800x24 >/tmp/e3/xvfb.log 2>&1 &
    XPID=$!
    sleep 2
    kill -0 "$XPID" 2>/dev/null || { echo "🔴 [$ETIQ] Xvfb est mort"; cat /tmp/e3/xvfb.log; exit 3; }
    DISP=":77"
    echo "=== [$ETIQ] Xvfb :77 (pid $XPID)"
fi

DISPLAY="$DISP" PORT_CDP="${PORT_CDP:-9481}" PORT_HTTP="${PORT_HTTP:-5391}" \
    timeout 300 node "$I/banc-aec.mjs" "$MODE" "/tmp/e3/banc-$ETIQ.json" 2>&1 | tee "$J/banc-$ETIQ.log"
cp "/tmp/e3/banc-$ETIQ.json" "$J/banc-$ETIQ.json" 2>/dev/null || true
