#!/usr/bin/env bash
# Batch 3, item 7 — THE CLOCK DIAGNOSIS, ON THE WIRE.
#
# 🔴 WHAT IT SETTLES. `metadata.captureTime` can be missing for TWO reasons
# that do not resemble each other: ① the agent does not announce the capture instant to the
# peer, ② the browser does not return it. Confusing them would attribute to the product
# a defect of the browser, or the reverse.
#
# The discrimination happens here, OUTSIDE the browser: an RTCP sender report
# is a packet of type 200. In SRTCP the header (hence the type) is NOT
# encrypted — only the body is — so it can be counted on the wire. If
# 200 packets flow from the VM to the host, ① is eliminated.
#
# ⚠️ IT JUDGES NOTHING: it counts. It is the item that concludes.
#
#   usage: sr-rtcp.sh <seconds> <output-file>
set -uo pipefail
unset -f chpwd 2>/dev/null || true

SECONDES="${1:-60}"
SORTIE="${2:-/var/tmp/lot3-sr-rtcp.txt}"
PCAP="/var/tmp/lot3-sr-rtcp-$(date +%Y%m%dT%H%M%S).pcap"

# ⚠️ Kill by READ PID, never by pattern: `pkill -f tcpdump` from a
# shell whose command line carries the pattern kills the shell.
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
    # The second byte of the UDP payload carries the RTP/RTCP PT. 200 = SR,
    # 201 = RR, 96..127 = this repository's RTP streams (dynamic).
    # ⚠️ Offset 9 is that of the UDP payload IN the packet read by
    # `udp[1]`: tcpdump indexes the UDP payload from 0 header included,
    # that is 8 header bytes then the first RTP byte at `udp[8]` and the PT
    # at `udp[9]`.
    # 200=SR, 201=RR, 205=transport feedback, 206=PSFB.
    for v in 200 201 202 203 204 205 206 207; do
        n=$(tcpdump -r "${PCAP}" -nn "udp[9] = ${v}" 2>/dev/null | wc -l)
        echo "udp9_eq_${v}=${n}"
    done
    echo "--- provenance des paquets udp[9]=200 (sender report) ---"
    tcpdump -r "${PCAP}" -nn "udp[9] = 200" 2>/dev/null \
        | awk '{print $3, "->", $5}' | sort | uniq -c | head -10
} | tee "${SORTIE}"
