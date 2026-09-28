#!/usr/bin/env bash
# Lot 3, item 7 — LE DIAGNOSTIC D'HORLOGE, SUR LE FIL.
#
# 🔴 CE QU'IL TRANCHE. `metadata.captureTime` peut manquer pour DEUX raisons
# qui ne se ressemblent pas : ① l'agent n'annonce pas l'instant de capture au
# pair, ② le navigateur ne le rend pas. Les confondre attribuerait au produit
# un défaut du navigateur, ou l'inverse.
#
# La discrimination se fait ici, HORS du navigateur : un sender report RTCP
# est un paquet de type 200. En SRTCP l'en-tête (donc le type) n'est PAS
# chiffré — seul le corps l'est — donc il se compte sur le fil. Si des
# paquets 200 circulent de la VM vers l'hôte, ① est éliminé.
#
# ⚠️ IL NE JUGE RIEN : il compte. C'est l'item qui conclut.
#
#   usage : sr-rtcp.sh <secondes> <fichier-de-sortie>
set -uo pipefail
unset -f chpwd 2>/dev/null || true

SECONDES="${1:-60}"
SORTIE="${2:-/var/tmp/lot3-sr-rtcp.txt}"
PCAP="/var/tmp/lot3-sr-rtcp-$(date +%Y%m%dT%H%M%S).pcap"

# ⚠️ Tuer par PID RELEVÉ, jamais par motif : `pkill -f tcpdump` depuis un
# shell dont la ligne de commande porte le motif tue le shell.
tcpdump -i any -n -s 0 -w "${PCAP}" \
    "host 192.168.3.2 and udp and not port 5985" >/dev/null 2>&1 &
PID_TCPDUMP=$!
sleep 2
echo "capture en cours (pid ${PID_TCPDUMP}) pendant ${SECONDES} s : ${PCAP}"
sleep "${SECONDES}"
kill "${PID_TCPDUMP}" 2>/dev/null
wait "${PID_TCPDUMP}" 2>/dev/null

{
    echo "horodatage=$(date -Is)"
    echo "pcap=${PCAP}"
    echo "secondes=${SECONDES}"
    total=$(tcpdump -r "${PCAP}" -nn 2>/dev/null | wc -l)
    echo "paquets_udp_total=${total}"
    # Le second octet de la charge UDP porte le PT RTP/RTCP. 200 = SR,
    # 201 = RR, 96..127 = flux RTP de ce dépôt (dynamiques).
    # ⚠️ Le décalage 9 est celui de la charge UDP DANS le paquet lu par
    # `udp[1]` : tcpdump indexe la charge UDP à partir de 0 en-tête compris,
    # soit 8 octets d'en-tête puis le premier octet RTP à `udp[8]` et le PT
    # à `udp[9]`.
    # 200=SR, 201=RR, 205=transport feedback, 206=PSFB.
    for v in 200 201 202 203 204 205 206 207; do
        n=$(tcpdump -r "${PCAP}" -nn "udp[9] = ${v}" 2>/dev/null | wc -l)
        echo "udp9_eq_${v}=${n}"
    done
    echo "--- provenance des paquets udp[9]=200 (sender report) ---"
    tcpdump -r "${PCAP}" -nn "udp[9] = 200" 2>/dev/null \
        | awk '{print $3, "->", $5}' | sort | uniq -c | head -10
} | tee "${SORTIE}"
