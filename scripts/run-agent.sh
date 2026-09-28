#!/usr/bin/env bash
# Launches the agent in the interactive session (session 1) of the Windows VM.
#
# WinRM runs in session 0: an agent launched directly by WinRM can
# neither capture a window (Windows.Graphics.Capture) nor inject input
# (SendInput), these APIs not crossing the session boundary. The scheduled
# task with /IT runs in the logged-in user's session.

# ── DEAD PATH, 29 August 2026 — see scripts/voie-morte.sh ───────────────────
. "$(dirname "$0")/voie-morte.sh"
voie_morte "wrote /media/vm/dev/run-agent.ps1 then launched C:\\dev\\target\\...\\agent.exe" \
"     The appliance's agent is launched by the scheduled task \"guacamole-agent\",
     which runs C:\nivuus\agent\run-agent.ps1 — a file of the console package.
     To set a bench variable, insert it in THAT file AFTER the
     env:SUPERVISEUR anchor (hence BEFORE the agent is invoked):
       docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh
       (functions variable_de_banc, agent_arreter, agent_relancer)."
# ─── Below, the original body, kept as a historical record. ──────

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TASK_NAME="guacamole-agent"
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"
: "${WINDOWS_ADMIN_PASSWORD:?WINDOWS_ADMIN_PASSWORD not set}"

# Path of the executable, assembled HERE rather than in the heredoc below:
# the latter is not quoted (it must interpolate variables), and a
# `\$` in it is an escape — writing `target\${AGENT_PROFILE}` there produced
# a literal `target${AGENT_PROFILE}`, separator swallowed and variable not
# substituted. A single variable, with no backslash in front, has no such trap:
# its content is no longer reinterpreted once substituted.
AGENT_EXE="C:\\dev\\target\\${AGENT_PROFILE:-release}\\agent.exe"

# Environment variables go through a bootstrap script: schtasks does not
# allow passing them directly.
cat > /media/vm/dev/run-agent.ps1 <<PS1
\$env:SIGNALING_URL = '${SIGNALING_URL:-ws://192.168.3.1:8080}'
\$env:SESSION_ID    = '${SESSION_ID:-demo}'
\$env:LOCAL_IP      = '${LOCAL_IP:-192.168.3.2}'
\$env:RUST_LOG      = '${RUST_LOG:-info}'
\$env:WINDOW_TITLE  = '${WINDOW_TITLE:-firefox}'
${SUPERVISEUR:+\$env:SUPERVISEUR = '$SUPERVISEUR'}
${CAPTEUR:+\$env:CAPTEUR = '$CAPTEUR'}
${PONT:+\$env:PONT = '$PONT'}
${PONT_ECRITURE:+\$env:PONT_ECRITURE = '$PONT_ECRITURE'}
${PONT_MUTATION:+\$env:PONT_MUTATION = '$PONT_MUTATION'}
${PONT_MESURE:+\$env:PONT_MESURE = '$PONT_MESURE'}
${PONT_CACHE:+\$env:PONT_CACHE = '$PONT_CACHE'}
${AUDIO:+\$env:AUDIO = '$AUDIO'}
${AUDIO_PERIPHERIQUE:+\$env:AUDIO_PERIPHERIQUE = '$AUDIO_PERIPHERIQUE'}
${PLEIN_ECRAN:+\$env:PLEIN_ECRAN = '$PLEIN_ECRAN'}
${SORTIE_DESIGNEE:+\$env:SORTIE_DESIGNEE = '$SORTIE_DESIGNEE'}
${APPARTENANCE:+\$env:APPARTENANCE = '$APPARTENANCE'}
${ACCENT:+\$env:ACCENT = '$ACCENT'}
${PRESSE_PAPIER:+\$env:PRESSE_PAPIER = '$PRESSE_PAPIER'}
${PRESSE_PAPIER_GARDE:+\$env:PRESSE_PAPIER_GARDE = '$PRESSE_PAPIER_GARDE'}
${APPS:+\$env:APPS = '$APPS'}
${ICONES:+\$env:ICONES = '$ICONES'}
${INSTALLATION_FAUTE:+\$env:INSTALLATION_FAUTE = '$INSTALLATION_FAUTE'}
${APPS_SURVEILLANCE:+\$env:APPS_SURVEILLANCE = '$APPS_SURVEILLANCE'}
${APPS_FAUTE:+\$env:APPS_FAUTE = '$APPS_FAUTE'}
${MICRO_MESURE:+\$env:MICRO_MESURE = '$MICRO_MESURE'}
# Work item E, block E2 — the THREE microphone variables, plus
# MICRO_MESURE above. Without these lines the agent starts without them AND
# WITHOUT REPORTING ANYTHING: a trap paid for in D1 (SUPERVISEUR), D2
# (MULTIFENETRE_REPRISE) and D7 (AUDIO). The check that counts is not
# reading this script but the TRACE — for MICRO_PERIPHERIQUE, the line
# "render cable retained for mic writing" carries the RETAINED
# value, never the mere presence of a line.
${MICRO:+\$env:MICRO = '$MICRO'}
${MICRO_PERIPHERIQUE:+\$env:MICRO_PERIPHERIQUE = '$MICRO_PERIPHERIQUE'}
${MICRO_FAUTE_ECRITURE:+\$env:MICRO_FAUTE_ECRITURE = '$MICRO_FAUTE_ECRITURE'}
${TEST_FILE:+\$env:TEST_FILE = '$TEST_FILE'}
${CAPTURE_TEST:+\$env:CAPTURE_TEST = '$CAPTURE_TEST'}
${SOURCE_TRACE:+\$env:SOURCE_TRACE = '$SOURCE_TRACE'}
${ENCODER_FPS:+\$env:ENCODER_FPS = '$ENCODER_FPS'}
${BITRATE:+\$env:BITRATE = '$BITRATE'}
${BUDGET_BPS:+\$env:BUDGET_BPS = '$BUDGET_BPS'}
${PART_SONDAGE:+\$env:PART_SONDAGE = '$PART_SONDAGE'}
${AUDIO_FAUTE_LECTURE:+\$env:AUDIO_FAUTE_LECTURE = '$AUDIO_FAUTE_LECTURE'}
${AUDIO_FAUTE_LECTURE_MS:+\$env:AUDIO_FAUTE_LECTURE_MS = '$AUDIO_FAUTE_LECTURE_MS'}
${AUDIO_FAUTE_RECONSTRUCTION:+\$env:AUDIO_FAUTE_RECONSTRUCTION = '$AUDIO_FAUTE_RECONSTRUCTION'}
${ENCODE_TEST:+\$env:ENCODE_TEST = '$ENCODE_TEST'}
${ENCODE_TEST_TARGET:+\$env:ENCODE_TEST_TARGET = '$ENCODE_TEST_TARGET'}
${ENCODE_TEST_SECS:+\$env:ENCODE_TEST_SECS = '$ENCODE_TEST_SECS'}
${ENCODER_THROUGHPUT_TEST:+\$env:ENCODER_THROUGHPUT_TEST = '$ENCODER_THROUGHPUT_TEST'}
${ENCODER_THROUGHPUT_TARGET:+\$env:ENCODER_THROUGHPUT_TARGET = '$ENCODER_THROUGHPUT_TARGET'}
${ENCODER_THROUGHPUT_DEADLINE_SECS:+\$env:ENCODER_THROUGHPUT_DEADLINE_SECS = '$ENCODER_THROUGHPUT_DEADLINE_SECS'}
${ENCODER_THROUGHPUT_SUBMIT_HZ:+\$env:ENCODER_THROUGHPUT_SUBMIT_HZ = '$ENCODER_THROUGHPUT_SUBMIT_HZ'}
${AUDIO_PROBE:+\$env:AUDIO_PROBE = '$AUDIO_PROBE'}
${AUDIO_PROBE_SECS:+\$env:AUDIO_PROBE_SECS = '$AUDIO_PROBE_SECS'}
${PROCESS_LOOPBACK_PROBE:+\$env:PROCESS_LOOPBACK_PROBE = '$PROCESS_LOOPBACK_PROBE'}
${PROCESS_LOOPBACK_CAPTURE:+\$env:PROCESS_LOOPBACK_CAPTURE = '$PROCESS_LOOPBACK_CAPTURE'}
${PROCESS_LOOPBACK_SECS:+\$env:PROCESS_LOOPBACK_SECS = '$PROCESS_LOOPBACK_SECS'}
${VIGEM_PROBE:+\$env:VIGEM_PROBE = '$VIGEM_PROBE'}
${VIGEM_PROBE_SECS:+\$env:VIGEM_PROBE_SECS = '$VIGEM_PROBE_SECS'}
${INPUT_LINEARITY_PROBE:+\$env:INPUT_LINEARITY_PROBE = '$INPUT_LINEARITY_PROBE'}
${INPUT_LINEARITY_PAS:+\$env:INPUT_LINEARITY_PAS = '$INPUT_LINEARITY_PAS'}
${INPUT_LINEARITY_REPETITIONS:+\$env:INPUT_LINEARITY_REPETITIONS = '$INPUT_LINEARITY_REPETITIONS'}
${INPUT_LINEARITY_NEUTRALISER:+\$env:INPUT_LINEARITY_NEUTRALISER = '$INPUT_LINEARITY_NEUTRALISER'}
${MULTIFENETRE_DXGI:+\$env:MULTIFENETRE_DXGI = '$MULTIFENETRE_DXGI'}
${MULTIFENETRE_WGC:+\$env:MULTIFENETRE_WGC = '$MULTIFENETRE_WGC'}
${MULTIFENETRE_REPLIS:+\$env:MULTIFENETRE_REPLIS = '$MULTIFENETRE_REPLIS'}
${MULTIFENETRE_BANC:+\$env:MULTIFENETRE_BANC = '$MULTIFENETRE_BANC'}
${MULTIFENETRE_N:+\$env:MULTIFENETRE_N = '$MULTIFENETRE_N'}
${MULTIFENETRE_EPREUVE_FILE_MS:+\$env:MULTIFENETRE_EPREUVE_FILE_MS = '$MULTIFENETRE_EPREUVE_FILE_MS'}
${MULTIFENETRE_SORTIE:+\$env:MULTIFENETRE_SORTIE = '$MULTIFENETRE_SORTIE'}
${MULTIFENETRE_NVENC:+\$env:MULTIFENETRE_NVENC = '$MULTIFENETRE_NVENC'}
${MULTIFENETRE_NVENC_CYCLES:+\$env:MULTIFENETRE_NVENC_CYCLES = '$MULTIFENETRE_NVENC_CYCLES'}
${MULTIFENETRE_CONTRAT:+\$env:MULTIFENETRE_CONTRAT = '$MULTIFENETRE_CONTRAT'}
${MULTIFENETRE_VDD:+\$env:MULTIFENETRE_VDD = '$MULTIFENETRE_VDD'}
${MULTIFENETRE_VDD_VEILLE:+\$env:MULTIFENETRE_VDD_VEILLE = '$MULTIFENETRE_VDD_VEILLE'}
${MULTIFENETRE_VDD_PURGE:+\$env:MULTIFENETRE_VDD_PURGE = '$MULTIFENETRE_VDD_PURGE'}
${MULTIFENETRE_VDD_CAPTURE:+\$env:MULTIFENETRE_VDD_CAPTURE = '$MULTIFENETRE_VDD_CAPTURE'}
${MULTIFENETRE_VDD_PARALLELE:+\$env:MULTIFENETRE_VDD_PARALLELE = '$MULTIFENETRE_VDD_PARALLELE'}
${MULTIFENETRE_REPRISE:+\$env:MULTIFENETRE_REPRISE = '$MULTIFENETRE_REPRISE'}
${MULTIFENETRE_PLAFOND:+\$env:MULTIFENETRE_PLAFOND = '$MULTIFENETRE_PLAFOND'}
${MULTIFENETRE_PLAFOND_SONDE:+\$env:MULTIFENETRE_PLAFOND_SONDE = '$MULTIFENETRE_PLAFOND_SONDE'}
${MULTIFENETRE_MODE_SORTIE:+\$env:MULTIFENETRE_MODE_SORTIE = '$MULTIFENETRE_MODE_SORTIE'}
${MULTIFENETRE_MODE_SORTIE_DRAPEAUX:+\$env:MULTIFENETRE_MODE_SORTIE_DRAPEAUX = '$MULTIFENETRE_MODE_SORTIE_DRAPEAUX'}
${MULTIFENETRE_POINTEUR:+\$env:MULTIFENETRE_POINTEUR = '$MULTIFENETRE_POINTEUR'}
${PRESSE_PAPIER_SONDE:+\$env:PRESSE_PAPIER_SONDE = '$PRESSE_PAPIER_SONDE'}
${SUPERVISEUR_HOOK:+\$env:SUPERVISEUR_HOOK = '$SUPERVISEUR_HOOK'}
${AGENT_TRACE_EXCEPTIONS:+\$env:AGENT_TRACE_EXCEPTIONS = '$AGENT_TRACE_EXCEPTIONS'}
${AGENT_TRACE_EXCEPTIONS_FICHIER:+\$env:AGENT_TRACE_EXCEPTIONS_FICHIER = '$AGENT_TRACE_EXCEPTIONS_FICHIER'} # policy: allow-fr - env var read by agent.exe
${AGENT_TRACE_EXCEPTIONS_AUTOTEST:+\$env:AGENT_TRACE_EXCEPTIONS_AUTOTEST = '$AGENT_TRACE_EXCEPTIONS_AUTOTEST'}
${AGENT_VM:+\$env:AGENT_VM = '$AGENT_VM'}
${AGENT_SECRET:+\$env:AGENT_SECRET = '$AGENT_SECRET'}
# 🔴 AGENT_JETON IS NOT PASSED HERE, AND THAT IS DELIBERATE — the only deliberate
# omission of this file, written rather than suffered. This repository paid three times
# for forgetting a new variable in this script (SUPERVISEUR in D1,
# MULTIFENETRE_REPRISE in D2, AUDIO in D7); this one is not of the same kind.
# AGENT_JETON is a HAND-OVER variable between processes, set by the
# supervisor on its children and on the bridge (agent/src/superviseur/lanceur.rs)
# so that a single process per VM opens the /agent channel. Setting it by hand
# would SKIP the enrolment of the root process: it would no longer beat the heart
# of the VM, would push no catalogue, would receive no launch
# order, and its token would die after ten minutes without renewing itself.
# An operator sets AGENT_VM and AGENT_SECRET; the hand-over is none of their business.
# The StreamWriter below settles the WRITING of the file in UTF-8, but not the
# READING of the child's output: PowerShell decodes the stream of agent.exe
# according to \$OutputEncoding / [Console]::OutputEncoding, which defaults to the
# OEM code page of the console (CP850/CP437), whereas agent.exe writes
# UTF-8 — without this setting, accented characters come out as mojibake
# even once the file is rewritten cleanly. Both settings are
# needed: this one for reading, the StreamWriter for writing.
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding(\$false)

# UTF-8 WITHOUT BOM, and without the line break that \`Out-File\` inserts at the
# console width: \`Tee-Object\` (PS 5.1) writes UTF-16LE and has no
# -Encoding parameter, which made the logs of the previous probe
# unreadable to \`grep\`. \`Out-File -Encoding utf8\` would fix the encoding but
# would reformat long lines. An explicit StreamWriter does neither
# one nor the other.
\$flux = New-Object System.IO.StreamWriter('C:\dev\agent.log', \$false, (New-Object System.Text.UTF8Encoding(\$false)))
try {
  & '${AGENT_EXE}' *>&1 | ForEach-Object { \$flux.WriteLine([string]\$_); \$flux.Flush() }
} finally {
  \$flux.Close()
}
PS1

# -WindowStyle Hidden: without this flag, the PowerShell console hosting
# agent.exe opens in the foreground of the interactive session and may
# entirely cover the window the agent is supposed to capture (observed
# in task 9: a cropping trial read the background colour of THIS
# console — PowerShell blue #012456 — instead of the content of the targeted
# window, over the whole sampled area). Hiding the console
# affects neither the session (still 1, still interactive) nor the output
# (still redirected to agent.log by the UTF-8 StreamWriter above).
node "$ROOT/scripts/winrm.js" \
    "schtasks /delete /tn $TASK_NAME /f 2>\$null; \
     schtasks /create /tn $TASK_NAME /f /it /ru '$USER_NAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
       /sc once /st 00:00 \
       /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\run-agent.ps1'; \
     schtasks /run /tn $TASK_NAME"

echo "agent launched in the interactive session; log: /media/vm/dev/agent.log"
