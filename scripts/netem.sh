#!/usr/bin/env bash
# Banc de dégradation réseau pour la recette du chantier C.
#
# Pose un profil sur l'interface du pont de la VM, DANS LES DEUX SENS :
#   - sortant (hôte → VM)      : qdisc netem directement sur $IFACE
#   - entrant (VM → hôte)      : redirigé vers $IFB, car tc ne façonne qu'en
#                                sortie. C'est ce sens qui porte la vidéo.
#
# Usage : scripts/netem.sh <lan|adsl|4g|congestionné|effondrement|off>
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

nettoyer_sans_trap  # Nettoyage initial, avant toute pose

# `lan` et `off` sont le même état du réseau — aucune qdisc — mais deux
# intentions différentes : `lan` est le profil TÉMOIN de la recette, `off`
# est le retrait du banc. Les distinguer évite d'écrire « recette passée
# sous lan » alors qu'on avait simplement tout retiré.
if [ "$profil" = "lan" ] || [ "$profil" = "off" ]; then
    echo "profil ${profil} : aucune dégradation posée sur ${IFACE}"
    exit 0
fi

# Installer le trap pour toute erreur durant la pose : garantir l'atomicité
# en cas d'échec partiel.
trap nettoyer_sur_erreur ERR

modprobe ifb numifbs=1
ip link set dev "$IFB" up

# Sens sortant (hôte → VM).
tc qdisc add dev "$IFACE" root netem \
    delay "$latence" "$gigue" distribution normal \
    loss "$perte" \
    rate "$debit"

# Sens entrant (VM → hôte) : c'est celui qui porte la vidéo.
tc qdisc add dev "$IFACE" handle ffff: ingress
tc filter add dev "$IFACE" parent ffff: protocol all u32 match u32 0 0 \
    action mirred egress redirect dev "$IFB"
tc qdisc add dev "$IFB" root netem \
    delay "$latence" "$gigue" distribution normal \
    loss "$perte" \
    rate "$debit"

# Pose réussie — désarmer le trap.
trap - ERR

echo "profil ${profil} posé sur ${IFACE} et ${IFB} : ${debit}, ${latence} ±${gigue}, perte ${perte}"
