#!/usr/bin/env bash
# Plays ONE run of block E3's acceptance.
#
#     instrument/jouer-e3.sh <etiquette>
#
# 🔴 TWO RUNS NEVER OVERLAP. F1 lost one that way: two
# overlapped by 2 min 23 s, the second killed the first one's agent IN THE MIDDLE OF
# MEASURING, and *the log filed under the first one's name was the
# second's*. This script kills BEFORE, and checks AFTER.
#
# 🔴 THE ORDER IS CONSTRAINED THREE TIMES, AND EACH CONSTRAINT WAS PAID FOR:
#   1. `.env` FIRST, never after the acceptance settings — it carries
#      `SIGNALING_URL` and a later `source` overwrites them SILENTLY;
#   2. the WINDOWS before the SUPERVISOR — it finds them through
#      `enumerer_existantes`, and the reverse order makes it capture the
#      scheduled task's PowerShell console (D11);
#   3. the SHELL page before the AGENT — signaling only remembers SDP
#      offers, a `fenetre-ouverte` announcement emitted before is LOST WITHOUT A TRACE
#      (D1). It is the driver that holds this third order, through `APRES_CONNEXION`.
set -uo pipefail
ETIQUETTE="${1:?etiquette}"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-micro-e3/instrument"
J="$RACINE/docs/superpowers/plans/journaux-micro-e3"
mkdir -p /tmp/e3

# 🔴 `.env` FIRST, but the acceptance settings are REMEMBERED before it:
# `.env` carries `SIGNALING_URL`, and a later `source` would overwrite them
# SILENTLY. Same guard as `e2-lancer.sh`.
GARDE_FAUTE="${MICRO_FAUTE_ECRITURE:-}"
set -a; source "$RACINE/.env"; set +a
[ -n "$GARDE_FAUTE" ] && export MICRO_FAUTE_ECRITURE="$GARDE_FAUTE" || true
source /tmp/f2/env.sh   # F2's setup, REUSED: same VM, same account, same prefix

echo "=== [$ETIQUETTE] $(date -u '+%Y-%m-%dT%H:%M:%SZ') — un compte n'est attribuable qu'assorti de son heure ==="
echo "=== [$ETIQUETTE] espace disque (OPFS et les WAV vivent dans /tmp) ==="
df -h /tmp | tail -1

# 🔴 THE VM DIES ON ITS OWN, AND THE CAUSE IS IDENTIFIED: `libvirtd --timeout
# 120` receives a signal 15 and takes the domain with it. ⚠️ **IT IS NOT D1's
# hibernation mechanism**, and confusing them would send one looking on the wrong
# side — D1 saw a GUEST `shutdown.exe` (Kernel-Power 187/42), here it is
# the HOST that kills QEMU. The libvirt counter tells them apart.
#
# This run lost one that way: the VM died at the end of trial 3, and
# trial 4 failed on the "target host is down" error.
COMPTEUR_AVANT="$(grep -c 'terminating on signal\|shutting down' /var/log/libvirt/qemu/Windows.log 2>/dev/null || echo '?')"
echo "=== [$ETIQUETTE] compteur libvirt AVANT : $COMPTEUR_AVANT ==="
if [ "$(virsh list --all 2>/dev/null | grep -c 'Windows.*en cours')" -eq 0 ]; then
    echo "=== [$ETIQUETTE] la VM est ETEINTE : demarrage ==="
    virsh start Windows >/dev/null 2>&1 || true
    # ⚠️ We wait for REAL ACCESS to /media/vm, never port 5985 alone nor the
    # mere presence of the CIFS mount — whose entry persists with the VM off.
    for _ in $(seq 1 90); do ls /media/vm/dev >/dev/null 2>&1 && break; sleep 5; done
fi
ls /media/vm/dev >/dev/null 2>&1 || { echo "=== [$ETIQUETTE] /media/vm INJOIGNABLE : on s arrete ==="; exit 3; }

echo "=== [$ETIQUETTE] agents et fenetres survivants AVANT ==="
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\e3-tuer.ps1' 2>&1 | tail -2

echo "=== [$ETIQUETTE] artefacts de l execution precedente ==="
rm -f /media/vm/dev/agent.log /media/vm/dev/e2-juge-e3-*.log \
      "$J/pilote-$ETIQUETTE.json" "$J/agent-$ETIQUETTE.log" "$J/agent-$ETIQUETTE-plat.log"

echo "=== [$ETIQUETTE] DEUX fenetres Bloc-notes en session 1, AVANT le superviseur ==="
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\e3-fenetres.ps1' 2>&1 | tail -4

echo "=== [$ETIQUETTE] pilote (la shell d abord, l agent ensuite) ==="
# ⚠️ `AUDIO_PERIPHERIQUE` is NOT set, and it is the plan's Decision 8: with
# multiple windows `loopback_de_session` is `config.audio && fenetre_hwnd.is_none()`,
# and `fenetre_hwnd` is `Some` in a child — the local loop guard is
# INERT by construction. E2's whole acceptance run set it; it is an artefact
# of single-window mode, and setting it here would measure something other than the product.
APRES_CONNEXION="cd $RACINE && set -a && source .env && set +a && export AGENT_VM=$AGENT_VM AGENT_SECRET=$AGENT_SECRET SUPERVISEUR=1 SIGNALING_URL=ws://192.168.3.1:8080 RUST_LOG=${NIVEAU_LOG:-info} ${MICRO_FAUTE_ECRITURE:+MICRO_FAUTE_ECRITURE=$MICRO_FAUTE_ECRITURE} && scripts/run-agent.sh" \
UDD="/tmp/e3/udd-$ETIQUETTE" PORT_CDP="${PORT_CDP:-9470}" WAV="${WAV:-/tmp/e3/ton-440.wav}" \
    node "$I/pilote-e3.mjs" "/tmp/e3/pilote-$ETIQUETTE.json" \
    2>&1 | tee "$J/pilote-$ETIQUETTE.log"
CODE=${PIPESTATUS[0]}

echo "=== [$ETIQUETTE] copie du journal d agent (APRES la fin reelle) ==="
# ⚠️ AFTER the real end: the children die when the browser closes,
# hence AFTER the copy, and their release lines would go with the next
# log. One piece of evidence was lost that way in D4.
sleep 6
cp /media/vm/dev/agent.log "$J/agent-$ETIQUETTE.log" 2>/dev/null || echo 'agent.log introuvable'
sed 's/\x1b\[[0-9;]*m//g' "$J/agent-$ETIQUETTE.log" > "$J/agent-$ETIQUETTE-plat.log" 2>/dev/null || true
cp "/tmp/e3/pilote-$ETIQUETTE.json" "$J/pilote-$ETIQUETTE.json" 2>/dev/null || true
for f in /media/vm/dev/e2-juge-e3-*.log; do
    [ -e "$f" ] || continue
    cp "$f" "$J/juge-$ETIQUETTE-$(basename "$f" .log | sed 's/^e2-juge-e3-//').log" 2>/dev/null || true
done

echo "=== [$ETIQUETTE] agents survivants APRES (y compris si l execution a echoue) ==="
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\e3-tuer.ps1' 2>&1 | tail -2
echo "=== [$ETIQUETTE] survie de la VM : compteur libvirt AVANT=$COMPTEUR_AVANT APRES=$(grep -c "terminating on signal\|shutting down" /var/log/libvirt/qemu/Windows.log 2>/dev/null || echo '?') ==="
echo "=== [$ETIQUETTE] code du pilote : $CODE ==="
exit "$CODE"
