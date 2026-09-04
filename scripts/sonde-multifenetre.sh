#!/usr/bin/env bash
# Enchaîne les sondes du chantier D, UNE PAR EXÉCUTION du binaire.
#
# Ce n'est pas une commodité : `captureservice.dll` plantait en 0xc0000005 au
# jalon 1, et un plantage de ce genre emporte le processus. Éprouver les
# quatre voies dans une même exécution ferait perdre les trois autres avec la
# première. Chaque voie tourne donc seule, et son journal est récolté avant
# la suivante.

# ── VOIE MORTE, 29 août 2026 — voir scripts/voie-morte.sh ───────────────────
. "$(dirname "$0")/voie-morte.sh"
voie_morte "enchaînait les sondes du chantier D, une par exécution, à travers /media/vm" \
"     Rien ne le remplace tel quel. Les sondes se lancent aujourd'hui en posant
     leur variable (MULTIFENETRE_*) dans C:\nivuus\agent\run-agent.ps1 —
     voir le successeur de scripts/run-agent.sh."
# ─── Ci-dessous, le corps d'origine, conservé comme relevé historique. ──────

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JOURNAUX="$ROOT/docs/superpowers/plans/journaux-sonde-multifenetre"
mkdir -p "$JOURNAUX"

# `run-agent.sh` exige les identifiants Windows ; les charger ici évite
# d'imposer un `set -a && source .env` à chaque invocation.
if [ -f "$ROOT/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$ROOT/.env"
    set +a
fi

executer() {
    local nom="$1"; shift
    # Lu à CHAQUE appel : le banc du temps 2 relève SONDE_SECS pour ses
    # propres passes, sans que les sondes courtes du temps 1 en héritent.
    local secs="${SONDE_SECS:-25}"
    echo "── sonde : $nom ─────────────────────────────"
    rm -f /media/vm/dev/agent.log
    env "$@" "$ROOT/scripts/run-agent.sh"
    sleep "$secs"
    cp /media/vm/dev/agent.log "$JOURNAUX/$nom.log" 2>/dev/null \
        || echo "AUCUN JOURNAL — la sonde a-t-elle planté au démarrage ?"
    # Le journal sort de PowerShell en UTF-16LE : sans ce décodage, chaque
    # caractère ASCII s'affiche espacé d'un octet nul (« t e x t » au lieu de
    # « text »), rendant `tail` illisible sur tout le journal, pas seulement
    # sur les accents.
    iconv -f UTF-16LE -t UTF-8 "$JOURNAUX/$nom.log" 2>/dev/null | tail -30 || true
}

executer dxgi MULTIFENETRE_DXGI=1
executer wgc MULTIFENETRE_WGC=1
executer replis MULTIFENETRE_REPLIS=1
executer nvenc MULTIFENETRE_NVENC=1

# Temps 2 : le banc, sur les voies déclarées survivantes par le temps 1.
# `VOIES` est posée à la main d'après les verdicts — le script n'infère rien.
for voie in ${VOIES:-}; do
    for n in 1 2 4 8; do
        SONDE_SECS=45 executer "banc-$voie-$n" "MULTIFENETRE_BANC=$voie" "MULTIFENETRE_N=$n"
    done
done
