#!/usr/bin/env bash
# THE DEAD PATH SIGN — to SOURCE at the top of a script of the
# development path from before the switch to the appliance.
#
#     . "$(dirname "$0")/voie-morte.sh"
#     voie_morte "what this script did" "what replaces it"
#
# 🔴 WHY THIS FILE EXISTS, AND WHY IT ERASES NOTHING.
#
# On 29 August 2026, the `package-nivuus` work item turned the target VM into an
# APPLIANCE and removed — deliberately — `C:\dev`, the guest's Rust toolchain
# and the CIFS mount `/media/vm`. The scripts in this directory that target
# that machine can no longer do anything.
#
# **DELETING them would lose the trace of what they did and of why they
# were replaced.** **Leaving them as they are is WORSE**: they are
# DEAD SCRIPTS THAT LOOK ALIVE, and this repository has paid for this pattern a number
# of times it documents itself — a log that showed only a
# constant, a witness doomed to red, an absence assertion green over a
# crash. One more instance does not deserve to be kept silently.
#
# Hence: they FAIL FAST, NAMING THEIR SUCCESSOR. It is cheap,
# reversible, and it turns a silent trap into a sign.
#
# ⚠️ THIS IS NOT A REMOVAL. The body of each script stays below, readable,
# as a historical record. The real removal is a NAMED DEBT, to be played the
# day nothing quotes them any more — as of 5 September 2026, there are still **48**
# executable callers of `scripts/winrm.js`, **57** of `/media/vm` and **2**
# of `scripts/build-agent.sh` in the tree tracked by git (found by the
# command written in the "Open legacy items" § of `CLAUDE.md` — rerun it,
# never copy these three numbers).
#
# 🔴 THIS GUIDE DOES NOT DECIDE THE FATE OF THE SCRIPTS: it makes it visible.
# Removing them, rewriting them for the appliance, or keeping them this way remains a
# decision of the repository owner.

voie_morte() {
    local faisait="$1" successeur="$2"
    local rapport="docs/superpowers/plans/2026-09-05-lot3-campagne-vm-resultats-partiels.md"  # policy: allow-fr - real file name
    cat >&2 <<FIN
🔴 DEAD PATH: $(basename "${0}")

  This script $(printf '%s' "${faisait}").

  It targets the DEVELOPMENT VM, which no longer exists in that form since the
  appliance switch of 29 August 2026 (package-nivuus work item): \`C:\\dev\`, the
  guest's Rust toolchain and the CIFS mount \`/media/vm\` were removed
  DELIBERATELY. Checked on 5 September 2026: \`mount | grep media/vm\` returns
  nothing, \`/media/vm\` is an empty directory, \`Get-ChildItem C:\\\` lists
  no \`dev\`, and WinRM refuses the Basic transport.

  WHAT REPLACES IT:
${successeur}

  See CLAUDE.md § "Windows VM lifecycle", and
  ${rapport} § 1.

  ⚠️ This script is NOT deleted: its body stays readable under this guard,
  as a historical record. To read it without running it: \`cat \$0\`.
FIN
    exit 78   # EX_CONFIG: the configuration of the world does not allow this gesture.
}
