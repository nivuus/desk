#!/usr/bin/env bash
# Joue UNE exécution du banc S1 / S1bis.
#
#     jouer-banc.sh <s1|s1bis> <etiquette> [--xvfb]
#
# 🔴 LE NETTOYAGE EST DANS UN `trap`, JAMAIS DANS LA DERNIÈRE LIGNE : la salle
# et les défauts audio de l'utilisateur sont rendus même si le banc échoue,
# même s'il est interrompu.
set -uo pipefail
MODE="${1:?s1 ou s1bis}"; ETIQ="${2:?etiquette}"; XV="${3:-}"
RACINE=/home/mallanic/Projects/Guacamole
I="$RACINE/docs/superpowers/plans/journaux-micro-e3/instrument"
J="$RACINE/docs/superpowers/plans/journaux-micro-e3"
export XDG_RUNTIME_DIR=/run/user/1000
# 🔴 `PULSE_SERVER` EXPLICITE : en root, la bibliothèque PulseAudio refuse un
# `XDG_RUNTIME_DIR` qui ne lui appartient pas, et TOUT le banc s'en trouve
# faussé sans le dire — voir le § de `banc-aec.mjs`.
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
    # ⚠️ Les défauts sont RESTAURÉS À LEUR VALEUR RELEVÉE, jamais à une valeur
    # supposée : `auto_null` est le défaut de CETTE machine aujourd'hui, il
    # pourrait ne pas l'être demain.
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
# ⚠️ **wireplumber ÉLIT LA SALLE COMME DÉFAUT TOUT SEUL** — c'est le risque n°1
# de la Décision 5, et il SE PRODUIT. Il est bénin ici (le défaut d'avant est
# `auto_null`, une sortie de repli, et rien ne jouait), et **il se défait tout
# seul à la mort du processus** : mesuré, `auto_null` redevient le défaut.
# On pose quand même les défauts explicitement — une élection automatique n'est
# pas une garantie —, et on les RESTAURE à leur valeur RELEVÉE dans le `trap`.
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
