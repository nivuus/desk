#!/usr/bin/env bash
# Lot 3, tache 5 (item 4) — LE REJEU DES DOUZE PILOTES.
#
#     rejouer.sh <etiquette>
#
# 🔴 THE PARAMETERS ARE IMPOSED, NEVER LEFT TO A DEFAULT. It is the lesson
# this campaign has just paid for on four arms: a parameter one does not
# pass does not fail, it takes a value. Here, `SIGNALING_WS` falls back to
# `ws://192.168.3.1:8080` in NINE of the twelve — the port from before batch 10A —
# whereas the service listens on 3445. **It is therefore NOT a defect of the
# driver**: it is a calling defect, and it is fixed by passing the variable.
#
# ⚠️ WE RECORD EACH ONE'S FIRST ERROR, not a verdict: it is what
# says what blocks, and several blockages are DECISIONS (the
# authentication mode) and not failures.
set -uo pipefail
unset -f chpwd 2>/dev/null || true
ETIQUETTE="${1:?usage : rejouer.sh <etiquette>}"
RACINE="$(git rev-parse --show-toplevel)" || exit 1
J="${RACINE}/docs/superpowers/plans/journaux-lot3-pilotes"
P="${RACINE}/docs/superpowers/plans"

set -a; . "${RACINE}/.env"; set +a

# Today's setup parameters, imposed.
export SIGNALING_WS='ws://192.168.3.1:3445'
export PLATEFORME_URL='http://192.168.3.1:3445'
export CLIENT_URL='http://192.168.3.1:3445'
export PREFIXE_VM='3sxuA9dd56NpVdHi37R86g'
export RECETTE_EMAIL="${COURRIEL:-maxime.g.allanic@gmail.com}"
export RECETTE_MOTDEPASSE='sans-objet-en-mode-pomerium'
export APP='^Notepad$'

DOUZE="
journaux-accent-a1/instrument/pilote-accent-a1.mjs
journaux-micro-e2/pilote-recette-e2.mjs
journaux-micro-e3/instrument/pilote-e3.mjs
journaux-micro-e3/instrument/injection-e3.js
journaux-pont-fichiers/instrument/pilote-f1.mjs
journaux-pont-fichiers-f2/instrument/pilote-f2.mjs
journaux-pont-fichiers-f3/instrument/pilote-f3.mjs
journaux-pont-fichiers-f4/instrument/pilote-f4.mjs
journaux-pont-fichiers-f5/instrument/pilote-f5.mjs
journaux-presse-papier-p1/instrument/pilote-pp-p1.mjs
journaux-presse-papier-p2/instrument/pilote-pp-p2.mjs
journaux-presse-papier-p3/instrument/pilote-pp-p3.mjs
"

: > "${J}/rejeu-${ETIQUETTE}.log"
for rel in ${DOUZE}; do
    f="${P}/${rel}"
    nom="$(basename "${rel}")"
    echo "======== ${nom} ========" | tee -a "${J}/rejeu-${ETIQUETTE}.log"
    if [ ! -f "${f}" ]; then echo "  ABSENT" | tee -a "${J}/rejeu-${ETIQUETTE}.log"; continue; fi
    # 1. The syntax, always — it is the only check the legs-sans-vm batch
    #    could play, and it stays true.
    if node --check "${f}" 2>/dev/null; then echo "  node --check : OK" | tee -a "${J}/rejeu-${ETIQUETTE}.log"
    else echo "  node --check : 🔴 ECHEC" | tee -a "${J}/rejeu-${ETIQUETTE}.log"; fi
    # 2. `injection-e3.js` is not a program: it is injected. We do not launch it.
    case "${nom}" in injection-e3.js)
        echo "  NON EXECUTABLE : c'est une injection, posee par pilote-e3.mjs" | tee -a "${J}/rejeu-${ETIQUETTE}.log"
        continue ;;
    esac
    # 3. The FIRST error, with the imposed parameters.
    sortie=$(cd "$(dirname "${f}")" && timeout 70 node "${f}" 12 "/var/tmp/lot3-rejeu-${nom}.json" 2>&1 | head -40)
    prem=$(printf '%s\n' "${sortie}" | grep -aoE "Error: .{0,150}|🔴 VOIE MORTE.{0,60}|ECONNREFUSED.{0,40}|a rendu [0-9]{3}.{0,60}|obligatoire.{0,60}" | head -1)
    echo "  premiere erreur : ${prem:-<aucune : voir le journal>}" | tee -a "${J}/rejeu-${ETIQUETTE}.log"
    printf '%s\n' "${sortie}" | tail -12 >> "${J}/rejeu-${ETIQUETTE}.log"
done
echo; echo "journal : ${J}/rejeu-${ETIQUETTE}.log"
