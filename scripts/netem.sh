#!/usr/bin/env bash
# Network degradation bench for the work item C acceptance.
#
# Sets a profile on the VM bridge interface, IN BOTH DIRECTIONS:
#   - outgoing (host → VM)     : netem qdisc directly on $IFACE
#   - incoming (VM → host)     : redirected to $IFB, since tc only shapes on
#                                egress. This is the direction carrying the video.
#
# Usage: scripts/netem.sh <lan|adsl|4g|congested|collapse|off>
set -euo pipefail

IFACE="${IFACE:-internalBridge}"
IFB="${IFB:-ifb0}"

profil="${1:-}"
case "$profil" in
    lan)            debit=""        latence=""      gigue=""     perte="" ;;
    adsl)           debit="8mbit"   latence="30ms"  gigue="5ms"  perte="0%" ;;
    4g)             debit="10mbit"  latence="60ms"  gigue="20ms" perte="1%" ;;
    congested)      debit="3mbit"   latence="100ms" gigue="20ms" perte="3%" ;;
    collapse)       debit="500kbit" latence="150ms" gigue="30ms" perte="5%" ;;
    off)            debit=""        latence=""      gigue=""     perte="" ;;
    *)
        echo "unknown profile: '${profil}'" >&2
        echo "expected: lan adsl 4g congested collapse off" >&2
        exit 2
        ;;
esac

cleanup_without_trap() {
    tc qdisc del dev "$IFACE" root        2>/dev/null || true
    tc qdisc del dev "$IFACE" ingress     2>/dev/null || true
    tc qdisc del dev "$IFB"   root        2>/dev/null || true
}

cleanup_on_error() {
    echo "Error while setting profile ${profil} — emergency cleanup" >&2
    cleanup_without_trap
    exit 1
}

cleanup_without_trap  # Initial cleanup, before any setup

# `lan` and `off` are the same network state — no qdisc — but two
# different intents: `lan` is the CONTROL profile of the acceptance, `off`
# is the removal of the bench. Telling them apart avoids writing "acceptance passed
# under lan" when we had simply removed everything.
if [ "$profil" = "lan" ] || [ "$profil" = "off" ]; then
    echo "profile ${profil}: no degradation set on ${IFACE}"
    exit 0
fi

# Install the trap for any error during setup: guarantee atomicity
# in case of partial failure.
trap cleanup_on_error ERR

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

echo "profile ${profil} set on ${IFACE} and ${IFB}: ${debit}, ${latence} ±${gigue}, loss ${perte}"
