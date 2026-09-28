#!/usr/bin/env bash
# Batch 3, item 10 (3.5) — A NATURAL CAUSE OF AUDIO CAPTURE DEATH.
#
#     jouer-item10.sh <bras>    bras : temoin | defaut | veille | service
#
# 🔴 THE `temoin` ARM IS THE MOST IMPORTANT OF THE ITEM, AND IT GOES FIRST.
# The expected conclusion is an ABSENCE — "no natural cause" — and an
# absence returned by an instrument that cannot see the thing is worth NOTHING.
# We therefore PROVOKE a capture death through `AUDIO_FAUTE_LECTURE=15`, and we
# record the SIGNATURE. Without this arm, "no natural cause" would only mean
# "I cannot see one".
#
# THE SIGNATURE, reread in the CODE THAT EMITS IT on September 5th, 2026:
#   agent/src/windows_audio/fil.rs   "audio read fault injection ARMED (bench)"
#   agent/src/windows_audio/fil.rs   "injected fault (AUDIO_FAUTE_LECTURE)"
#   agent/src/windows_audio/fil.rs   "audio read failed, retrying"
#   agent/src/windows_audio/fil.rs   "audio read failed, capture stopped for good"  <-- THE DEATH
#   agent/src/transport/piste_audio.rs  "audio capture rebuilt"
#   agent/src/transport/piste_audio.rs  "audio capture rebuild refused"
# Constants: LECTURES_ECHOUEES_MAX = 10 (audio.rs:118), RECONSTRUCTIONS_MAX = 3 (audio.rs:83).
#
# 🔴 ONE CAUSE PER RUN OF THE BINARY: these APIs fail by CRASHING the
# process, not through an error code, and two causes in the same run
# would make attribution impossible.
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
# 🔴 `APP` IS MANDATORY, AND ITS ABSENCE COST SEVERAL ARMS.
# Without it, `pilote-latence.mjs` falls back to its default pattern
# `chrome|edge|bloc.?notes|notepad`, which matches **Microsoft Edge
# first** — yet THIS CAMPAIGN measured (item 7) that Edge is NEVER
# adopted: launched by desk, its window is SET ASIDE 60 ms later by the
# ownership rule, and no session opens. The arm then returns
# "windows SERVED: 0" for a reason FOREIGN to what it measures.
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
        # Changing the DEFAULT render device during a session.
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
