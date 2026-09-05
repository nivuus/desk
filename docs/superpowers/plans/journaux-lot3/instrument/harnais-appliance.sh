#!/usr/bin/env bash
# Le harnais de la campagne du lot 3, RE-SITUÉ SUR L'APPLIANCE. À SOURCER.
#
#     source docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh
#
# 🔴 POURQUOI CE FICHIER EXISTE À CÔTÉ DE `harnais.sh`, ET NE LE REMPLACE PAS.
#
# `harnais.sh` a été écrit le 28 août 2026. Le 29 août 2026, le chantier
# `package-nivuus` a basculé la VM cible : elle n'est plus la machine de
# développement, elle est une APPLIANCE provisionnée par le package voisin
# `packages/installer`. Ce basculement a retiré, DÉLIBÉRÉMENT, les trois
# appuis sur lesquels `harnais.sh` repose :
#
#   1. `/media/vm` — le montage CIFS //192.168.3.2/c. RELEVÉ le 5 septembre
#      2026 : `mount | grep media/vm` ne rend RIEN, et `/media/vm` est un
#      répertoire vide. `vm_prete` y attend `ls /media/vm/dev` pendant 300 s
#      puis rend 1. Il ne peut plus rendre 0.
#   2. `C:\dev` — RELEVÉ le 5 septembre 2026 : `Get-ChildItem C:\` ne le
#      liste pas. Le binaire vit désormais dans `C:\nivuus\agent\agent.exe`
#      et son journal dans `C:\nivuus\agent.log`.
#   3. `scripts/winrm.js` — transport Basic. RELEVÉ le 5 septembre 2026 :
#      « Failed to process the request, status Code: » (l'invité n'offre que
#      Negotiate depuis `Enable-PSRemoting`). Le chemin qui répond est
#      `console/guest/winrm_exec.py`, en NTLM.
#
# CLAUDE.md § « Cycle de vie de la VM Windows » énonce ces trois faits et en
# tire que le lot 3 est SUSPENDU. Ce fichier lève cette suspension en
# re-situant le harnais, et RIEN D'AUTRE : il ne décide pas du sort de
# `scripts/winrm.js`, de `/media/vm` ni de `scripts/build-agent.sh` — ce sort
# appartient au propriétaire du dépôt, et CLAUDE.md le dit. `harnais.sh`
# reste donc en place, intact : c'est une pièce datée du 28 août 2026.
#
# 🔴 CE HARNAIS NE JUGE RIEN, comme celui qu'il re-situe. Il rend des états et
# des comptes ; c'est l'item qui décide si l'état est celui qu'il attendait.
unset -f chpwd 2>/dev/null || true

RACINE_HARNAIS="$(git rev-parse --show-toplevel)"
# Le package voisin, dérivé — jamais en dur : le dépôt a déjà payé 48 scripts
# qui portaient `/home/mallanic/Projects/Guacamole` après un déplacement.
CONSOLE_HARNAIS="$(cd "${RACINE_HARNAIS}/../installer" && pwd)"
WINRM_HARNAIS="${CONSOLE_HARNAIS}/console/guest/winrm_exec.py"
JOURNAL_VM='C:\nivuus\agent.log'
TACHE_VM='guacamole-agent'

# Une commande PowerShell sur l'invité. Le filtre retire le CLIXML que
# PowerShell verse sur stderr au premier chargement de module — il n'est pas
# une erreur, et le laisser passer polluerait chaque relevé.
W() {
    timeout "${2:-150}" python3 "${WINRM_HARNAIS}" ps "$1" 2>&1 \
        | grep -v 'CLIXML' | grep -v '^<Objs'
}

vm_prete() {
    virsh list --all | grep -q "Windows.*en cours" || virsh start Windows
    # 1. Le port WinRM répond.
    local i
    for i in $(seq 1 60); do
        timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null && break
        sleep 5
    done
    timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null || {
        echo "🔴 le port 5985 ne répond pas"; return 1; }
    # 2. PUIS un ACCÈS RÉEL à l'invité — c'est le point du harnais d'origine,
    #    transposé : un port ouvert n'est pas une session WinRM, exactement
    #    comme une entrée CIFS n'était pas un montage vivant. On éprouve donc
    #    un ALLER-RETOUR COMPLET qui lit le journal de l'agent, c'est-à-dire
    #    la ressource dont tous les items dépendent.
    local taille
    for i in $(seq 1 60); do
        taille=$(W "(Get-Item ${JOURNAL_VM} -ErrorAction SilentlyContinue).Length" 30 | tr -dc '0-9')
        [ -n "${taille}" ] && break
        sleep 5
    done
    [ -n "${taille}" ] || { echo "🔴 WinRM ouvre mais ne rend pas ${JOURNAL_VM}"; return 1; }
    echo "vm prête (journal=${taille} octets) : $(bash "${RACINE_HARNAIS}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh")"
}

agent_absent() {
    # 🔴 Un agent survivant tient agent.log EN ÉCRITURE, et l'on relit alors
    # le journal de la tentative PRÉCÉDENTE en croyant lire le sien. À
    # appeler avant CHAQUE tentative, y compris une qui vient d'échouer.
    #
    # ⚠️ Le code de retour de `winrm_exec.py` est le SEUL moyen de distinguer
    # « la requête a réussi et rend 0 » de « la requête a échoué et n'a rien à
    # dire » : sur échec de transport, le message d'erreur porte des chiffres
    # (une adresse, un port) qu'un `tr -dc '0-9'` ramasserait.
    local sortie code restants
    sortie=$(timeout 120 python3 "${WINRM_HARNAIS}" ps \
        '(@(Get-Process agent -ErrorAction SilentlyContinue)).Count' 2>/dev/null)
    code=$?
    if [ "${code}" -ne 0 ]; then
        echo "🔴 winrm_exec.py a échoué (code ${code}) : impossible de confirmer l'absence d'agent"
        return 1
    fi
    restants=$(printf '%s' "${sortie}" | tr -dc '0-9')
    [ "${restants:-0}" = "0" ] || { echo "🔴 ${restants} agent(s) survivant(s)"; return 1; }
    echo "aucun agent survivant"
}

agent_arreter() {
    # 🔴 Par PID RELEVÉ, jamais par motif — et la tâche planifiée d'abord :
    # elle relancerait le processus qu'on vient de tuer.
    W '
Stop-ScheduledTask -TaskName '"${TACHE_VM}"' -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
Start-Sleep -Seconds 5
"agents apres arret : " + (@(Get-Process agent -ErrorAction SilentlyContinue)).Count' 120
}

agent_relancer() {
    # $1 = secondes d'attente avant de rendre la main (défaut 25).
    W '
Start-ScheduledTask -TaskName '"${TACHE_VM}"'
Start-Sleep -Seconds '"${1:-25}"'
"agents vivants : " + (@(Get-Process agent -ErrorAction SilentlyContinue)).Count' 200
}

journal_reperer() {
    # Rend le NOMBRE DE LIGNES du journal à cet instant : le repère qui borne
    # un bras. Sans repère, un `grep` sur 50 Mio rendrait des lignes d'un bras
    # précédent, et le relevé serait celui d'une AUTRE mesure — le dépôt a déjà
    # versé le journal d'une seconde exécution sous le nom de la première (F1).
    #
    # 🔴 POURQUOI UN REPÈRE ET NON UN MARQUEUR ÉCRIT DANS LE JOURNAL. La
    # première version de ce harnais posait le marqueur par `Add-Content`,
    # comme `journaux-lot32/instrument/cycle-de-bras.sh`. MESURÉ le 5 septembre
    # 2026 : « The process cannot access the file 'C:\nivuus\agent.log' because
    # it is being used by another process » — le `StreamWriter` de
    # `run-agent.ps1` tient le fichier EN ÉCRITURE tant que l'agent tourne.
    # `cycle-de-bras.sh` ne s'en apercevait pas parce qu'il ARRÊTE l'agent
    # avant de marquer ; un bras qui mesure l'agent VIVANT ne le peut pas.
    # ⚠️ Un marqueur qui échoue en silence laisserait lire le bras précédent
    # en croyant lire le sien : c'est le défaut exact que le marqueur devait
    # empêcher.
    W "@(Get-Content ${JOURNAL_VM} -Encoding UTF8).Count" 200 | tr -dc '0-9'
}

journal_depuis() {
    # Rend les lignes du journal POSTÉRIEURES au repère $1.
    W '
$t = Get-Content '"${JOURNAL_VM}"' -Encoding UTF8
if ($t.Count -le '"$1"') { "AUCUNE LIGNE APRES LE REPERE '"$1"' (total " + $t.Count + ")"; exit 0 }
$t['"$1"'..($t.Count-1)]' 250
}

variable_de_banc() {
    # Pose (ou retire) une variable de banc dans le run-agent.ps1 GÉNÉRÉ SUR
    # LA VM — jamais dans `scripts/run-agent.sh`, qui vise la VM de
    # développement disparue.
    #   usage : variable_de_banc poser NOM VALEUR | variable_de_banc retirer NOM
    #
    # 🔴 L'INSERTION EST ANCRÉE SUR `env:SUPERVISEUR`, ET C'EST LE POINT.
    # Une ligne ajoutée en fin de fichier tombe APRÈS l'appel à agent.exe,
    # donc n'est JAMAIS exécutée — le fichier la contiendrait, un tracé de
    # code conclurait à tort, et seule la TRACE DANS LE JOURNAL le dirait
    # (piège payé deux fois le 30 août 2026, lot 32).
    local geste="$1" nom="$2" valeur="${3:-}" script
    # ⚠️ LE LITTÉRAL POWERSHELL EST CONSTRUIT ICI, PAS IMBRIQUÉ DANS DES
    # GUILLEMETS DE SHELL. La première rédaction empilait quatre niveaux de
    # citation et produisait `env:MICRO_PERIPHERIQUE = "NVIDIA""` — refusé à
    # l'analyse (« Unexpected token »), donc AUCUNE variable posée. Ce n'est
    # pas la trace qui l'a dit, c'est la RELECTURE DU run-agent.ps1 GÉNÉRÉ,
    # qui ne portait aucune ligne : le contrôle que ce dépôt impose, et qui a
    # servi ici même.
    if [ "${geste}" = "poser" ]; then
        script=$(cat <<PS
\$p = "C:\\nivuus\\agent\\run-agent.ps1"
\$l = @(Get-Content \$p -Encoding UTF8 | Where-Object { \$_ -notmatch "env:${nom}" })
\$idx = (\$l | Select-String "env:SUPERVISEUR" | Select-Object -First 1).LineNumber
if (-not \$idx) { throw "ancre env:SUPERVISEUR introuvable" }
\$neuf = @()
for (\$i=0; \$i -lt \$l.Count; \$i++) {
  \$neuf += \$l[\$i]
  if (\$i -eq (\$idx-1)) { \$neuf += ("{0}env:${nom} = '${valeur}'" -f [char]36) }
}
Set-Content -Path \$p -Value \$neuf -Encoding UTF8
"${nom} posee apres l ancre SUPERVISEUR (ligne \$idx)"
PS
)
    else
        script=$(cat <<PS
\$p = "C:\\nivuus\\agent\\run-agent.ps1"
Set-Content -Path \$p -Value @(Get-Content \$p -Encoding UTF8 | Where-Object { \$_ -notmatch "env:${nom}" }) -Encoding UTF8
"${nom} retiree"
PS
)
    fi
    W "${script}" 90
}
