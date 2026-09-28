#!/usr/bin/env bash
# Prints the VM's state and the shutdown counter, on ONE line.
#
# 🔴 THIS SCRIPT JUDGES NOTHING. It returns the state; it is the item that judges. A
# harness that itself classified a sequence as "valid" would mask the
# distinction between "the measurement was made" and "the VM held".
set -uo pipefail
unset -f chpwd 2>/dev/null || true

etat=$(virsh list --all 2>/dev/null | awk '$2=="Windows" {for (i=3; i<=NF; i++) printf "%s", $i}')
[ -z "${etat}" ] && etat="inconnu"
# The shutdown counter: the domain's stops traced by libvirt.
extinctions=$(grep -c "terminating on signal\|shutting down" \
    /var/log/libvirt/qemu/Windows.log 2>/dev/null || echo 0)
echo "etat=${etat} extinctions=${extinctions} horodatage=$(date -Is)"
