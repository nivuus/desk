#!/usr/bin/env bash
# Plays ONE run of acceptance run F4.
#
#     jouer-f4.sh <etiquette>
#
# Variables lues : GABARITS, PLAN_VM, REPOS_MESURE, NEUTRALISER_MOVE,
#                  MONTAGE (m1|m2), NIVEAU_LOG, PONT_MESURE.
#
# ⚠️ AN EMPTY OR ABSENT `PONT_MESURE` IS NOT EXPORTED — the `${VAR:+...}` form
# writes nothing —, and that is what makes the arm WITHOUT the variable playable. A
# default of `1` would make red run R3 impossible.
#
# 🔴 TWO RUNS NEVER OVERLAP. F1 lost one that way: two
# overlapped by 2 min 23 s, the second killed the first one's agent IN THE MIDDLE OF
# MEASURING, and THE LOG FILED UNDER THE FIRST ONE'S NAME WAS THE
# SECOND'S. This script kills the agent BEFORE, and COUNTS AGAIN AFTER — including if
# the run failed.
set -uo pipefail
ETIQUETTE="$1"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f4/instrument"
J="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f4"
MONTAGE="${MONTAGE:-m1}"

# ⚠️ The `chpwd` hook of the host's shell injects an `ls` into any output as soon
# as a `cd` runs in a subshell (S2 had to redo a whole pass).
unset -f chpwd 2>/dev/null || true

set -a; source "$RACINE/.env"; set +a
source /tmp/f2/env.sh   # le montage de F2, REEMPLOYE : meme VM, meme compte, meme prefixe

# ⚠️ THE HOST CARRIES AN UNIDENTIFIED DEFECT: `/dev/null` was found replaced
# by an ordinary file, which prevents any VM from starting. Repaired, cause
# unknown, MAY HAPPEN AGAIN.
[ "$(stat -c '%F' /dev/null)" = 'character special file' ] || { echo 'ARRET : /dev/null n est pas un noeud de caracteres'; exit 3; }

# 🔴 THE VM POWERS OFF ON ITS OWN, AND THE TRIGGER IS IDENTIFIED — IT IS NOT
# THE ONE THIS REPOSITORY HAS SUSPECTED SINCE D1.
#
# `/var/log/libvirt/qemu/Windows.log` carries `terminating on signal 15 from pid
# <N>`, and that PID is `/usr/sbin/libvirtd --timeout 120`: the daemon stops on
# inactivity and TAKES THE DOMAIN WITH IT. Four shutdowns recorded on August 21st, 2026,
# at 04:31:47, 05:11:59, 06:52:12 and 07:32:12 — that is ~40 min apart for the
# last two, TWO of which during this campaign.
#
# ⚠️ IT IS NOT D1'S MECHANISM. That one was a HIBERNATION initiated
# INSIDE the guest (Kernel-Power 187/42, `shutdown.exe`), QEMU terminating ~5 s
# AFTER. Here it is the HOST that kills, and the guest decides nothing. Confusing them
# would send one looking for the cause on the wrong side.
#
# Countermeasure: restart, and wait for the share THROUGH A REAL ACCESS — `/media/vm` is
# a CIFS mount whose entry persists in the table even with the VM off.
echo "=== [$ETIQUETTE] VM et agents survivants AVANT ==="
if ! virsh list --all 2>/dev/null | sed -n '3p' | grep -q "exécution"; then
    echo "VM eteinte : relance"
    virsh start Windows 2>&1 | tail -1
    for i in $(seq 1 60); do timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null && break; sleep 5; done
    for i in $(seq 1 60); do ls /media/vm/dev >/dev/null 2>&1 && break; sleep 5; done
fi
virsh list --all 2>&1 | sed -n '3p'
ls /media/vm/dev >/dev/null 2>&1 || { echo 'ARRET : /media/vm/dev inaccessible'; exit 4; }
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2

echo "=== [$ETIQUETTE] purge de la racine du pont et de son etat ==="
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\purger-pont.ps1' 2>&1 | tail -4

# 🔴 THE PREVIOUS RUN'S ARTEFACTS GO FIRST, ALL OF THEM. Paid for on the
# spot in F3: a JSON left by an earlier attempt was read as this one's
# result, and the conclusion would have been "the product is broken"
# whereas the ongoing run succeeded.
rm -f /media/vm/dev/agent.log /media/vm/dev/mesure-f4-*.json \
      "/tmp/f4/pilote-$ETIQUETTE.json" \
      "$J/pilote-$ETIQUETTE.json" "$J/agent-$ETIQUETTE.log" "$J/agent-$ETIQUETTE-plat.log" \
      "$J/pilote-$ETIQUETTE.log" "$J/mesure-$ETIQUETTE-trace.txt"
mkdir -p /tmp/f4

# 🔵 M1 — THE BRIDGE ALONE: no sensor, no window, no encoder, no video
# PeerConnection. `main.rs` wires `PONT` AFTER `CAPTEUR` and BEFORE the supervisor, and
# `pont.rs` does its OWN signaling on `SESSION_ID` — so an agent launched
# with `PONT=1` and `SESSION_ID=<prefixe>:fichiers` meets the existing shell page
# WITHOUT A SINGLE CLIENT LINE CHANGING.
if [ "$MONTAGE" = "m1" ]; then
    MODE="PONT=1 SESSION_ID=$PREFIXE_VM:fichiers"
else
    MODE="SUPERVISEUR=1"
fi
echo "=== [$ETIQUETTE] montage $MONTAGE : $MODE ==="

APRES_CONNEXION="cd $RACINE && set -a && source .env && set +a && export AGENT_VM=$AGENT_VM AGENT_SECRET=$AGENT_SECRET $MODE SIGNALING_URL=ws://192.168.3.1:8080 RUST_LOG=${NIVEAU_LOG:-info} ${PONT_MESURE:+PONT_MESURE=$PONT_MESURE} && scripts/run-agent.sh" \
UDD="/tmp/f4/udd-$ETIQUETTE" PORT_CDP="${PORT_CDP:-9460}" \
TRACE="$J/mesure-$ETIQUETTE-trace.txt" \
    node "$I/pilote-f4.mjs" "/tmp/f4/pilote-$ETIQUETTE.json" \
    2>&1 | tee "$J/pilote-$ETIQUETTE.log"
CODE=${PIPESTATUS[0]}

echo "=== [$ETIQUETTE] copie du journal d agent (APRES la fin reelle) ==="
# ⚠️ AFTER the real end: the release lines would go with the next
# log. One piece of evidence was lost that way in D4.
sleep 6
cp /media/vm/dev/agent.log "$J/agent-$ETIQUETTE.log" 2>/dev/null || echo 'agent.log introuvable'
sed 's/\x1b\[[0-9;]*m//g' "$J/agent-$ETIQUETTE.log" > "$J/agent-$ETIQUETTE-plat.log" 2>/dev/null || true
cp "/tmp/f4/pilote-$ETIQUETTE.json" "$J/pilote-$ETIQUETTE.json" 2>/dev/null || true

echo "=== [$ETIQUETTE] agents survivants APRES (a revenir voir MEME si l execution a echoue) ==="
node "$RACINE/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>&1 | tail -2
virsh list --all 2>&1 | sed -n '3p'
echo "=== [$ETIQUETTE] code de sortie du pilote : $CODE ==="
exit $CODE
