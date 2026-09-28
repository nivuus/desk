#!/usr/bin/env bash
# Lot 3, item 10 (3.5) — UNE CAUSE NATURELLE DE MORT DE CAPTURE AUDIO.
#
#     jouer-item10.sh <bras>    bras : temoin | defaut | veille | service
#
# 🔴 LE BRAS `temoin` EST LE PLUS IMPORTANT DE L'ITEM, ET IL PASSE EN PREMIER.
# La conclusion attendue est une ABSENCE — « aucune cause naturelle » — et une
# absence rendue par un instrument qui ne sait pas voir la chose ne vaut RIEN.
# On PROVOQUE donc une mort de capture par `AUDIO_FAUTE_LECTURE=15`, et on
# releve la SIGNATURE. Sans ce bras, « aucune cause naturelle » ne voudrait
# dire que « je ne sais pas en voir une ».
#
# LA SIGNATURE, relue dans le CODE QUI L'EMET le 5 septembre 2026 :
#   agent/src/windows_audio/fil.rs   "injection de fautes de lecture audio ARMEE (banc)"
#   agent/src/windows_audio/fil.rs   "faute injectée (AUDIO_FAUTE_LECTURE)"
#   agent/src/windows_audio/fil.rs   "lecture audio échouée, nouvelle tentative"
#   agent/src/windows_audio/fil.rs   "lecture audio échouée, capture arrêtée définitivement"  <-- LA MORT
#   agent/src/transport/piste_audio.rs  "capture audio reconstruite"
#   agent/src/transport/piste_audio.rs  "reconstruction de la capture audio refusée"
# Constantes : LECTURES_ECHOUEES_MAX = 10 (audio.rs:118), RECONSTRUCTIONS_MAX = 3 (audio.rs:83).
#
# 🔴 UNE CAUSE PAR EXECUTION DU BINAIRE : ces API echouent par PLANTAGE du
# processus, pas par code d'erreur, et deux causes dans la meme execution
# rendraient l'attribution impossible.
set -uo pipefail
unset -f chpwd 2>/dev/null || true

BRAS="${1:?usage : jouer-item10.sh <temoin|defaut|veille|service>}"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git" >&2; exit 1; }
J="${RACINE}/docs/superpowers/plans/journaux-lot3-audio"
D="${RACINE}/docs/superpowers/plans/journaux-lot3-latence/instrument"

set -a; . "${RACINE}/.env"; set +a
. "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh"
etape() { echo; echo "=== $(date -Is) $* ==="; }

etape "PRE-VOL"
vm_prete || exit 1
agent_arreter
for v in AUDIO_FAUTE_LECTURE AUDIO_FAUTE_LECTURE_MS AUDIO_FAUTE_RECONSTRUCTION; do
    variable_de_banc retirer "$v" >/dev/null
done
W 'Get-Process notepad -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
   Start-Sleep 2; "notepad restants : " + (@(Get-Process notepad -ErrorAction SilentlyContinue)).Count' 90

if [ "${BRAS}" = "temoin" ]; then
    etape "ARMEMENT DU TEMOIN : AUDIO_FAUTE_LECTURE=15 (> LECTURES_ECHOUEES_MAX = 10)"
    variable_de_banc poser AUDIO_FAUTE_LECTURE 15
fi
echo "--- les lignes REELLEMENT dans le run-agent.ps1 GENERE, avec leur POSITION ---"
W 'Get-Content C:\nivuus\agent\run-agent.ps1 -Encoding UTF8 | Select-String "env:AUDIO|agent.exe" | ForEach-Object { "  ligne " + $_.LineNumber + " : " + $_.Line.Trim() }' 90

REPERE=$(journal_reperer)
agent_relancer 30
echo "REPERE=${REPERE}"

etape "UNE SESSION AVEC AUDIO — sans fenetre servie, aucune capture audio n'existe"
# 🔴 `APP` EST OBLIGATOIRE, ET SON ABSENCE A COUTE PLUSIEURS BRAS.
# Sans elle, `pilote-latence.mjs` retombe sur son motif par defaut
# `chrome|edge|bloc.?notes|notepad`, qui apparie **Microsoft Edge en
# premier** — or CETTE CAMPAGNE a mesure (item 7) qu'Edge n'est JAMAIS
# adopte : lance par desk, sa fenetre est ECARTEE 60 ms plus tard par la
# regle d'appartenance, et aucune session ne s'ouvre. Le bras rend alors
# « fenetres SERVIES : 0 » pour une raison ETRANGERE a ce qu'il mesure.
APP='^Notepad$' nohup node "${D}/pilote-latence.mjs" --etiquette="item10-${BRAS}" --fenetres=1 --duree=110 --animer=0 \
      --sortie="/var/tmp/lot3-item10-${BRAS}.json" > "${J}/pilote-${BRAS}.log" 2>&1 &
PILOTE_PID=$!
sleep 55
echo "--- la session est-elle etablie, et une fenetre SERVIE ? ---"
grep -ac "page de session" "${J}/pilote-${BRAS}.log" | sed 's/^/  pages de session : /'

if [ "${BRAS}" != "temoin" ]; then
    etape "LA CAUSE TENTEE : ${BRAS}"
    case "${BRAS}" in
      defaut)
        # Changer le peripherique de rendu PAR DEFAUT en cours de session.
        W '$d = Get-CimInstance Win32_SoundDevice | Where-Object { $_.Status -eq "OK" }
           "peripheriques de rendu vus : " + (@($d).Count)
           $d | ForEach-Object { "  - " + $_.Name }
           "⚠️ AUCUN module de bascule de peripherique par defaut n est installe sur cette VM."
           "   Set-AudioDevice (AudioDeviceCmdlets) : " + (@(Get-Module -ListAvailable -Name AudioDeviceCmdlets)).Count + " module(s)"' 120 ;;
      veille)
        # Desactiver puis reactiver le point de terminaison de rendu actif.
        W '$p = Get-PnpDevice -Class AudioEndpoint -Status OK -ErrorAction SilentlyContinue
           "points de terminaison AudioEndpoint OK : " + (@($p).Count)
           $p | ForEach-Object { "  - " + $_.FriendlyName + "  [" + $_.InstanceId + "]" }
           $cible = $p | Select-Object -First 1
           if ($cible) {
             "on desactive : " + $cible.FriendlyName
             Disable-PnpDevice -InstanceId $cible.InstanceId -Confirm:$false -ErrorAction Continue
             Start-Sleep -Seconds 12
             "on reactive"
             Enable-PnpDevice -InstanceId $cible.InstanceId -Confirm:$false -ErrorAction Continue
             Start-Sleep -Seconds 8
             "etat final : " + (Get-PnpDevice -InstanceId $cible.InstanceId).Status
           } else { "aucun point de terminaison a mettre en veille" }' 300 ;;
      service)
        W '"Audiosrv avant : " + (Get-Service Audiosrv).Status
           Stop-Service Audiosrv -Force -ErrorAction Continue
           Start-Sleep -Seconds 12
           "Audiosrv pendant : " + (Get-Service Audiosrv).Status
           Start-Service Audiosrv -ErrorAction Continue
           Start-Sleep -Seconds 8
           "Audiosrv apres : " + (Get-Service Audiosrv).Status' 300 ;;
    esac
fi

etape "ATTENTE de la fin du pilote"
wait "${PILOTE_PID}" 2>/dev/null

etape "LA SIGNATURE — chaque chaine comptee vient du CODE QUI L EMET"
journal_depuis "${REPERE}" | sed 's/\x1b\[[0-9;]*m//g' > /var/tmp/lot3-item10-segment.log
{
echo "bras=${BRAS}"
echo "lignes du segment                                    : $(wc -l < /var/tmp/lot3-item10-segment.log)"
echo "--- TEMOIN DE VIE : la capture audio a-t-elle seulement existe ? ---"
echo "  'fenetre servie'                                   : $(grep -ac 'sortie retenue pour cette fenetre' /var/tmp/lot3-item10-segment.log)"
echo "  'audio'                                            : $(grep -ac 'audio' /var/tmp/lot3-item10-segment.log)"
echo "--- LA SIGNATURE D UNE MORT DE CAPTURE ---"
echo "  injection ARMEE (banc)                             : $(grep -ac 'injection de fautes de lecture audio ARMEE' /var/tmp/lot3-item10-segment.log)"
echo "  'faute injectée (AUDIO_FAUTE_LECTURE)'             : $(grep -ac 'faute injectée (AUDIO_FAUTE_LECTURE)' /var/tmp/lot3-item10-segment.log)"
echo "  'lecture audio échouée, nouvelle tentative'        : $(grep -ac 'lecture audio échouée, nouvelle tentative' /var/tmp/lot3-item10-segment.log)"
echo "  🔴 'capture arrêtée définitivement'                : $(grep -ac 'capture arrêtée définitivement' /var/tmp/lot3-item10-segment.log)"
echo "  'capture audio reconstruite'                       : $(grep -ac 'capture audio reconstruite' /var/tmp/lot3-item10-segment.log)"
echo "  'reconstruction de la capture audio refusée'       : $(grep -ac 'reconstruction de la capture audio refusée' /var/tmp/lot3-item10-segment.log)"
} 2>&1 | tee "${J}/comptes-${BRAS}.log"
cp /var/tmp/lot3-item10-segment.log "${J}/segment-${BRAS}.log"

if [ "${BRAS}" = "temoin" ]; then
    etape "RETOUR A L ETAT LIVRE"
    agent_arreter
    variable_de_banc retirer AUDIO_FAUTE_LECTURE
    agent_relancer 30
fi
bash "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh"
