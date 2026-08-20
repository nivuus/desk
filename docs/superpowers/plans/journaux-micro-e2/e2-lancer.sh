#!/usr/bin/env bash
# Séquence de lancement d'une exécution de recette E2 : TUER, vérifier, lancer.
#
# 🔴 DEUX PIÈGES SONT CÂBLÉS ICI PARCE QU'ILS ONT ÉTÉ PAYÉS DANS CE BLOC.
#
# 1. Un agent survivant tient C:\dev\agent.log : le nouveau StreamWriter ne peut
#    pas l'ouvrir, et l'on relit alors le journal de la tentative PRÉCÉDENTE en
#    croyant lire le sien. D'où `e2-tuer.ps1` en tête, sans condition.
# 2. `scripts/run-agent.sh` exige `.env` (WINDOWS_ADMIN_PASSWORD), et `.env`
#    porte SIGNALING_URL=…:8080 — la plateforme d'un AUTRE chantier. Un
#    `set -a; source .env` postérieur aux réglages de la recette les ÉCRASE en
#    silence, et l'agent va parler au mauvais service. L'ordre ci-dessous est
#    donc : .env d'abord, réglages de recette ENSUITE.
#
# Usage : SESSION_ID=… MICRO=… [AUDIO_PERIPHERIQUE=…] ./e2-lancer.sh <agent.env>
set -euo pipefail
RACINE=/home/mallanic/Projects/Guacamole
ENVRECETTE="${1:?chemin du fichier agent.env attendu}"
# Les réglages de recette, mémorisés AVANT que .env ne puisse les écraser.
GARDE_SESSION="${SESSION_ID:-}"; GARDE_MICRO="${MICRO:-}"
GARDE_AUDIOP="${AUDIO_PERIPHERIQUE:-}"; GARDE_MICROP="${MICRO_PERIPHERIQUE:-}"
GARDE_FAUTE="${MICRO_FAUTE_ECRITURE:-}"; GARDE_MESURE="${MICRO_MESURE:-}"
cd "$RACINE"
set -a; source .env; source "$ENVRECETTE"; set +a
export SESSION_ID="$GARDE_SESSION"
[ -n "$GARDE_MICRO" ] && export MICRO="$GARDE_MICRO" || true
[ -n "$GARDE_AUDIOP" ] && export AUDIO_PERIPHERIQUE="$GARDE_AUDIOP" || unset AUDIO_PERIPHERIQUE
[ -n "$GARDE_MICROP" ] && export MICRO_PERIPHERIQUE="$GARDE_MICROP" || true
[ -n "$GARDE_FAUTE" ] && export MICRO_FAUTE_ECRITURE="$GARDE_FAUTE" || true
[ -n "$GARDE_MESURE" ] && export MICRO_MESURE="$GARDE_MESURE" || true
echo "SIGNALING_URL=$SIGNALING_URL  SESSION_ID=$SESSION_ID  MICRO=${MICRO:-<absente>}  AUDIO_PERIPHERIQUE=${AUDIO_PERIPHERIQUE:-<absente>}"
node scripts/winrm.js 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\e2-tuer.ps1' 2>&1 | tail -1
rm -f /media/vm/dev/agent.log
scripts/run-agent.sh >/dev/null 2>&1
echo "AGENT LANCE"
