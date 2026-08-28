#!/usr/bin/env bash
# Le point unique de la campagne du lot 3. À SOURCER, pas à exécuter.
#
#     source docs/superpowers/plans/journaux-lot3/instrument/harnais.sh
#
# 🔴 POURQUOI IL EXISTE : la VM s'éteint seule par DEUX mécanismes distincts
# — une hibernation initiée DANS l'invité (Kernel-Power 187/42), et l'HÔTE
# qui tue QEMU (libvirtd --timeout 120 s'arrête sur inactivité et emporte le
# domaine). Douze items s'appuient sur cette VM pendant des séquences longues.
unset -f chpwd 2>/dev/null || true
RACINE_HARNAIS="$(git rev-parse --show-toplevel)"

vm_prete() {
    virsh list --all | grep -q "Windows.*en cours" || virsh start Windows
    # 1. WinRM répond.
    for _ in $(seq 1 60); do
        timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null && break
        sleep 5
    done
    timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null || {
        echo "🔴 WinRM ne répond pas"; return 1; }
    # 2. PUIS un ACCÈS RÉEL à /media/vm — jamais `mountpoint -q` : l'entrée
    #    CIFS persiste dans la table de montage VM éteinte et connexion morte.
    for _ in $(seq 1 60); do
        ls /media/vm/dev >/dev/null 2>&1 && break
        sleep 5
    done
    ls /media/vm/dev >/dev/null 2>&1 || { echo "🔴 /media/vm inaccessible"; return 1; }
    echo "vm prête : $(bash "${RACINE_HARNAIS}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh")"
}

agent_absent() {
    # 🔴 Un agent survivant tient agent.log, et l'on relit alors le journal de
    # la tentative PRÉCÉDENTE en croyant lire le sien. À appeler avant CHAQUE
    # tentative, y compris échouée.
    #
    # ⚠️ CORRIGÉ APRÈS ÉPREUVE (28 août 2026) : la version d'origine ne
    # regardait QUE la sortie, jamais le code de retour de `winrm.js`. Quand
    # ce dernier échoue au niveau transport (WinRM injoignable, auth
    # refusée...), il imprime sur STDOUT la pile de l'erreur ; `tr -dc
    # '0-9'` y ramassait alors les numéros de ligne de la pile et rendait un
    # compte absurde du type « 🔴 22246232650828772271221761422508285591251033905
    # agent(s) survivant(s) » — vu réellement sur la VM le 28 août 2026,
    # WinRM refusant l'authentification (voir le rapport de tâche 0). Le code
    # de retour distingue maintenant « la requête a échoué, on ne peut rien
    # affirmer » de « la requête a réussi et rend un compte ».
    local sortie code restants
    sortie=$(node "${RACINE_HARNAIS}/scripts/winrm.js" \
        '(Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>/dev/null)
    code=$?
    if [ "${code}" -ne 0 ]; then
        echo "🔴 winrm.js a échoué (code ${code}) : impossible de confirmer l'absence d'agent"
        return 1
    fi
    restants=$(printf '%s' "${sortie}" | tr -dc '0-9')
    [ "${restants:-0}" = "0" ] || { echo "🔴 ${restants} agent(s) survivant(s)"; return 1; }
    echo "aucun agent survivant"
}

purger_orphelins() {
    # Une sortie virtuelle et une racine ProjFS survivent à un arrêt brutal :
    # le Drop ne court pas sur un TerminateProcess.
    MULTIFENETRE_VDD_PURGE=1 "${RACINE_HARNAIS}/scripts/run-agent.sh" >/tmp/lot3-purge.log 2>&1 || true
    grep -c "purge" /tmp/lot3-purge.log || echo 0
}
