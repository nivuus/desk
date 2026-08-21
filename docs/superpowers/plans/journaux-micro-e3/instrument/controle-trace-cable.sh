#!/usr/bin/env bash
# Le contrôle de recette du livrable ③ (bloc E3) : la trace du câble porte-t-elle
# `occupation_ms` et `famines` ?
#
# 🔴 **CE CONTRÔLE N'A PAS ÉTÉ JOUÉ**, et son absence de journal est déclarée
# plutôt que comblée : il exige un `agent.log` produit par la VM Windows, que le
# chantier F5 tenait pendant tout E3. Il est versé PRÊT, pas VERT.
#
# ⚠️ **La rouge R6 du plan est signalée d'avance comme la plus faible**, et la
# jouer sur l'hôte serait une tautologie : `windows_micro.rs` est
# `#[cfg(windows)]`, aucun test d'hôte ne peut l'exécuter, et « le champ est
# dans la source » n'est pas « le champ sort dans le journal ». La seule rouge
# qui vaut est de retirer le champ, rebâtir, relancer, et constater que ce
# `grep` retombe à zéro.
#
# Usage : controle-trace-cable.sh <agent.log>

set -uo pipefail
# ❌ Cette ligne portait `${1:?journal d'agent}` — l'apostrophe y OUVRE une
# quote, car bash ANALYSE le mot d'une expansion `${par:?mot}` même entre
# guillemets doubles : le script ne se PARSAIT PAS (« EOF prématurée lors de la
# recherche du ' correspondant », ligne 37, c'est-à-dire trente lignes plus
# bas). 🔵 **Il avait été versé « PRÊT, pas VERT » — et il ne l'était même
# pas.** C'est la doctrine du dépôt prise à la lettre : un contrôle de recette
# doit être EXÉCUTÉ avant d'être prescrit, et celui-ci ne l'avait pas été.
journal="${1:?chemin du journal d agent attendu}"

# 🔴 `grep -a` OBLIGATOIRE : un journal à queue d'octets NUL est classé
# « binaire », et `grep` rend alors une SORTIE VIDE — pas un zéro, et les deux
# se lisent pareil (piège payé en D10).
plat="$(mktemp)"
sed 's/\x1b\[[0-9;]*m//g' "$journal" > "$plat"

lignes="$(grep -ac 'micro ecrit sur le cable' "$plat" || true)"
occ="$(grep -a 'micro ecrit sur le cable' "$plat" | grep -ac 'occupation_ms=' || true)"
fam="$(grep -a 'micro ecrit sur le cable' "$plat" | grep -ac 'famines=' || true)"

echo "lignes 'micro ecrit sur le cable' : ${lignes}"
echo "  dont portant occupation_ms=     : ${occ}"
echo "  dont portant famines=           : ${fam}"
rm -f "$plat"

# ⚠️ `lignes = 0` N'EST PAS UN VERDICT : c'est une mesure NON PRISE. La trace
# est périodique (`PERIODE_TRACE`), et un micro jamais allumé n'en émet aucune.
if [ "$lignes" -eq 0 ]; then
    echo "VERDICT : NON MESURABLE — aucune trace de câble dans ce journal."
    exit 2
fi
if [ "$occ" -eq "$lignes" ] && [ "$fam" -eq "$lignes" ]; then
    echo "VERDICT : VERT — les deux champs sont sur TOUTES les lignes."
    exit 0
fi
echo "VERDICT : ROUGE — un champ manque sur au moins une ligne."
exit 1
