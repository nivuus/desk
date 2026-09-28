#!/usr/bin/env bash
# Lance `mesurer.ps1` sur la VM SANS BLOQUER WinRM, puis attend le fichier.
#
# 🔴 POURQUOI ASYNCHRONE. Une premiere version appelait `mesurer.ps1` par
# `winrm.js` en synchrone. La copie du fichier de 12 Mio a pris 182 s ; l'appel
# WinRM est rentre BIEN AVANT (~62 s), le pilote a cru la mesure finie, a lu un
# fichier PARTIEL (12 lignes), puis a rendu la main -- et la fermeture du
# navigateur a coupe le pont EN PLEINE COPIE. Le condensat qui en sortait ne
# mesurait donc plus le produit mais la course entre la copie et l'arret du
# pilote.
#
# Ici : on demarre la mesure detachee, et on attend `FIN DE MESURE` dans le
# fichier -- le FAIT, jamais une duree (piege maison, sous-bloc D3).
set -uo pipefail
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
DELAI="${DELAI_MESURE:-900}"

set -a; source "$RACINE/.env"; set +a
rm -f /media/vm/dev/mesure.txt

# ⚠️ LE LANCEMENT PASSE PAR UN `.ps1`, JAMAIS PAR UNE COMMANDE EN LIGNE.
# `nodejs-winrm` enveloppe TOUT dans `powershell -Command "& { ... }"` : un
# `Start-Process ... -ArgumentList '-NoProfile','-File','C:\dev\mesurer.ps1'`
# passe en ligne y perd ses quotes, la commande ne tourne JAMAIS, et le symptome
# est un fichier de mesure qui n'apparait pas -- indiscernable d'une mesure
# lente. Mesure : lance par `.ps1`, le fichier existe en moins de 3 s.
node "$RACINE/scripts/winrm.js" \
  'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\lancer-mesure.ps1' \
  >/dev/null 2>&1

debut=$(date +%s)
while true; do
    if [ -f /media/vm/dev/mesure.txt ] && grep -aq 'FIN DE MESURE' /media/vm/dev/mesure.txt 2>/dev/null; then
        echo "# mesure terminee en $(( $(date +%s) - debut )) s"
        break
    fi
    if [ $(( $(date +%s) - debut )) -ge "$DELAI" ]; then
        echo "# DELAI DE $DELAI s DEPASSE : mesure INCOMPLETE, telle quelle"
        break
    fi
    sleep 3
done
cat /media/vm/dev/mesure.txt 2>/dev/null || echo '# aucun fichier de mesure'
