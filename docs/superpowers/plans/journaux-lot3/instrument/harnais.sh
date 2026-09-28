#!/usr/bin/env bash
# The single point of the batch 3 campaign. TO BE SOURCED, not executed.
#
#     source docs/superpowers/plans/journaux-lot3/instrument/harnais.sh
#
# 🔴 WHY IT EXISTS: the VM powers off on its own through TWO distinct mechanisms
# — a hibernation initiated INSIDE the guest (Kernel-Power 187/42), and the HOST
# killing QEMU (libvirtd --timeout 120 stops on inactivity and takes the
# domain with it). Twelve items rely on this VM during long sequences.
unset -f chpwd 2>/dev/null || true
RACINE_HARNAIS="$(git rev-parse --show-toplevel)"

vm_prete() {
    virsh list --all | grep -q "Windows.*en cours" || virsh start Windows
    # 1. WinRM answers.
    for _ in $(seq 1 60); do
        timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null && break
        sleep 5
    done
    timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null || {
        echo "🔴 WinRM ne répond pas"; return 1; }
    # 2. THEN a REAL ACCESS to /media/vm — never `mountpoint -q`: the CIFS
    #    entry persists in the mount table with the VM off and the connection dead.
    for _ in $(seq 1 60); do
        ls /media/vm/dev >/dev/null 2>&1 && break
        sleep 5
    done
    ls /media/vm/dev >/dev/null 2>&1 || { echo "🔴 /media/vm inaccessible"; return 1; }
    echo "vm prête : $(bash "${RACINE_HARNAIS}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh")"
}

agent_absent() {
    # 🔴 A surviving agent holds agent.log, and one then rereads the log of
    # the PREVIOUS attempt believing one reads one's own. To call before EACH
    # attempt, including a failed one.
    #
    # ⚠️ FIXED AFTER A TRIAL (August 28th, 2026): the original version only
    # looked at the output, never at the return code of `winrm.js`. When
    # the latter fails at transport level (WinRM unreachable, auth
    # refused...), it prints the error's stack on STDOUT; `tr -dc
    # '0-9'` then picked up the stack's line numbers and returned an
    # absurd count like « 🔴 22246232650828772271221761422508285591251033905
    # surviving agent(s) » — really seen on the VM on August 28th, 2026,
    # WinRM refusing authentication (see the report of task 0). The return
    # code now distinguishes "the request failed, nothing can be
    # asserted" from "the request succeeded and returns a count".
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
    # A virtual output and a ProjFS root survive a brutal stop:
    # Drop does not run on a TerminateProcess.
    MULTIFENETRE_VDD_PURGE=1 "${RACINE_HARNAIS}/scripts/run-agent.sh" >/tmp/lot3-purge.log 2>&1 || true
    grep -c "purge" /tmp/lot3-purge.log || echo 0
}
