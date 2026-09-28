#!/usr/bin/env bash
# Network degradation bench for the work item C acceptance.
#
# Sets a profile on the VM bridge interface, IN BOTH DIRECTIONS:
#   - outgoing (host → VM)     : netem qdisc directly on $IFACE
#   - incoming (VM → host)     : redirected to $IFB, since tc only shapes on
#                                egress. This is the direction carrying the video.
#
# Usage: scripts/netem.sh <lan|adsl|4g|congestionné|effondrement|off>
set -euo pipefail

IFACE="${IFACE:-internalBridge}"
IFB="${IFB:-ifb0}"

profil="${1:-}"
case "$profil" in
    lan)            debit=""        latence=""      gigue=""     perte="" ;;
    adsl)           debit="8mbit"   latence="30ms"  gigue="5ms"  perte="0%" ;;
    4g)             debit="10mbit"  latence="60ms"  gigue="20ms" perte="1%" ;;
    congestionné)   debit="3mbit"   latence="100ms" gigue="20ms" perte="3%" ;;
    effondrement)   debit="500kbit" latence="150ms" gigue="30ms" perte="5%" ;;
    off)            debit=""        latence=""      gigue=""     perte="" ;;
    *)
        echo "profil inconnu : '${profil}'" >&2
        echo "attendus : lan adsl 4g congestionné effondrement off" >&2
        exit 2
        ;;
esac

nettoyer_sans_trap() {
    tc qdisc del dev "$IFACE" root        2>/dev/null || true
    tc qdisc del dev "$IFACE" ingress     2>/dev/null || true
    tc qdisc del dev "$IFB"   root        2>/dev/null || true
}

nettoyer_sur_erreur() {
    echo "Erreur lors de la pose du profil ${profil} — nettoyage d'urgence" >&2
    nettoyer_sans_trap
    exit 1
}

nettoyer_sans_trap  # Initial cleanup, before any setup

# `lan` and `off` are the same network state — no qdisc — but two
# different intents: `lan` is the CONTROL profile of the acceptance, `off`
# is the removal of the bench. Telling them apart avoids writing "acceptance passed
# under lan" when we had simply removed everything.
if [ "$profil" = "lan" ] || [ "$profil" = "off" ]; then
    echo "profil ${profil} : aucune dégradation posée sur ${IFACE}"
    exit 0
fi

# Install the trap for any error during setup: guarantee atomicity
# in case of partial failure.
trap nettoyer_sur_erreur ERR

modprobe ifb numifbs=1
ip link set dev "$IFB" up

# Outgoing direction (host → VM).
tc qdisc add dev "$IFACE" root netem \
    delay "$latence" "$gigue" distribution normal \
    loss "$perte" \
    rate "$debit"

# Incoming direction (VM → host): the one carrying the video.
tc qdisc add dev "$IFACE" handle ffff: ingress
tc filter add dev "$IFACE" parent ffff: protocol all u32 match u32 0 0 \
    action mirred egress redirect dev "$IFB"
tc qdisc add dev "$IFB" root netem \
    delay "$latence" "$gigue" distribution normal \
    loss "$perte" \
    rate "$debit"

# Setup succeeded — disarm the trap.
trap - ERR

echo "profil ${profil} posé sur ${IFACE} et ${IFB} : ${debit}, ${latence} ±${gigue}, perte ${perte}"
