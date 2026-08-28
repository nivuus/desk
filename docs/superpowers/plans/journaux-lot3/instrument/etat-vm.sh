#!/usr/bin/env bash
# Imprime l'état de la VM et le compteur d'extinctions, sur UNE ligne.
#
# 🔴 CE SCRIPT NE JUGE RIEN. Il rend l'état ; c'est l'item qui juge. Un
# harnais qui classerait lui-même une séquence « valide » masquerait la
# distinction entre « la mesure a été faite » et « la VM a tenu ».
set -uo pipefail
unset -f chpwd 2>/dev/null || true

etat=$(virsh list --all 2>/dev/null | awk '$2=="Windows" {for (i=3; i<=NF; i++) printf "%s", $i}')
[ -z "${etat}" ] && etat="inconnu"
# Le compteur d'extinctions : les arrêts du domaine tracés par libvirt.
extinctions=$(grep -c "terminating on signal\|shutting down" \
    /var/log/libvirt/qemu/Windows.log 2>/dev/null || echo 0)
echo "etat=${etat} extinctions=${extinctions} horodatage=$(date -Is)"
