#!/usr/bin/env bash
# Sous-bloc D11 — la SÉQUENCE VM des recettes ①, ② et ③, versée plutôt que
# retapée à la main : sans elle, la moitié « VM » de chaque mesure vivrait
# uniquement dans l'historique d'un agent, c'est-à-dire nulle part (leg 3 de
# D10, six constats perdus avec un rapport gitignoré).
#
# Elle NE lance PAS le navigateur pilote : c'est `pilote-audio-d11.mjs`, qui
# tourne sur l'HÔTE, jamais sur la VM (sur la VM sa fenêtre serait elle-même
# capturée, en cascade).
#
# Préalables, à vérifier AVANT (préambule commun du plan) :
#   virsh list --all && virsh start Windows
#   until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985'; do sleep 5; done
#   until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
#   set -a && source .env && set +a
#
# ⚠️ `Get-Process agent` se revérifie APRÈS chaque tentative, y compris
# échouée : un superviseur resté vivant empêche le nouveau StreamWriter
# d'ouvrir `agent.log`, et la copie relue est celle, PÉRIMÉE, de la tentative
# précédente. Rencontré trois fois sur trois en D8. D'où `tuer_agent` ci-dessous.
#
# Usage :
#   recette-audio-d11.sh preparer
#   recette-audio-d11.sh ouvrir <n> <hz> <profil-chrome>
#   recette-audio-d11.sh fenetres
#   recette-audio-d11.sh tuer-agent
#   recette-audio-d11.sh copier <etiquette>
set -euo pipefail
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
D11="$RACINE/docs/superpowers/plans/journaux-multifenetres-d11"
VMIT="$RACINE/docs/superpowers/plans/journaux-multifenetres-d10/instrument/vm-it.sh"
winrm() { node "$RACINE/scripts/winrm.js" "$1"; }

case "${1:?commande attendue}" in
preparer)
    # Le registre N'EST PAS touché : la pollution laissée par les sondes de
    # mode de sortie est indifférente au mode mono-fenêtre, et le sous-bloc
    # D10 a rendu le superviseur tolérant à une sortie surdimensionnée.
    bash "$VMIT" prep-d11 "Get-Process agent, chrome, notepad, mspaint -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 3"
    sleep 6
    winrm 'Get-Process agent,chrome -ErrorAction SilentlyContinue | Select-Object Id,ProcessName | Format-List | Out-String; Write-Output FIN'
    ;;
ouvrir)
    # `--user-data-dir` : UN PAR FENÊTRE pour la recette ① ; PARTAGÉ pour ②
    # et ③, qui exigent un `chrome.exe` UNIQUE donc un seul groupe de PID.
    # C'est l'appelant qui tranche, en passant le même profil ou non.
    n="${2:?rang}"; hz="${3:?frequence}"; profil="${4:?dossier de profil chrome}"
    cp "$RACINE/docs/superpowers/plans/journaux-multifenetres-d7/instrument/ton.html" "/media/vm/dev/ton-$hz.html"
    bash "$VMIT" "ouvrir-ton-$hz" "\$a = @(
  '--app=file:///C:/dev/ton-$hz.html?hz=$hz&gain=0.25',
  '--user-data-dir=$profil',
  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',
  '--window-size=1280,720','--window-position=$((30 + n * 40)),$((30 + n * 40))',
  '--disable-features=CalculateNativeWinOcclusion',
  '--autoplay-policy=no-user-gesture-required',
  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',
  '--disable-renderer-backgrounding')
Start-Process 'C:\Program Files\Google\Chrome\Application\chrome.exe' -ArgumentList \$a
Start-Sleep -Seconds 3"
    sleep 12
    ;;
fenetres)
    # RELEVER les titres depuis la SESSION INTERACTIVE : `MainWindowTitle` lu
    # depuis WinRM (session 0) rend vide pour toutes les fenêtres de session 1.
    cp "$RACINE/docs/superpowers/plans/journaux-multifenetres-d10/instrument/listefen.ps1" /media/vm/dev/listefen.ps1
    bash "$VMIT" listefen 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\listefen.ps1'
    sleep 8
    cat /media/vm/dev/fenetres.txt
    ;;
tuer-agent)
    winrm 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep -Seconds 3; Get-Process agent -ErrorAction SilentlyContinue | Select-Object Id | Format-List | Out-String; Write-Output AGENT-FIN'
    ;;
copier)
    # ⚠️ APRÈS la fin réelle de l'exécution, donc après `tuer-agent` : les
    # enfants meurent quand le navigateur se ferme, donc APRÈS le pilote.
    e="${2:?etiquette}"
    cp /media/vm/dev/agent.log "$D11/agent-$e.log"
    sed 's/\x1b\[[0-9;]*m//g' "$D11/agent-$e.log" > "$D11/agent-$e-plat.log"
    ;;
*) echo "commande inconnue : $1" >&2; exit 2 ;;
esac
