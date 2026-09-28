#!/usr/bin/env bash
# Launches the agent in the interactive session (session 1) of the Windows VM.
#
# WinRM runs in session 0: an agent launched directly by WinRM can
# neither capture a window (Windows.Graphics.Capture) nor inject input
# (SendInput), these APIs not crossing the session boundary. The scheduled
# task with /IT runs in the logged-in user's session.

# ── DEAD PATH, 29 August 2026 — see scripts/voie-morte.sh ───────────────────
. "$(dirname "$0")/voie-morte.sh"
voie_morte "écrivait /media/vm/dev/run-agent.ps1 puis lançait C:\\dev\\target\\...\\agent.exe" \
"     L'agent de l'appliance est lancé par la tâche planifiée « guacamole-agent »,
     qui exécute C:\nivuus\agent\run-agent.ps1 — un fichier du package console.
     Pour poser une variable de banc, l'insérer dans CE fichier APRÈS l'ancre
     env:SUPERVISEUR (donc AVANT l'invocation de l'agent) :
       docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh
       (fonctions variable_de_banc, agent_arreter, agent_relancer)."
# ─── Below, the original body, kept as a historical record. ──────

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TASK_NAME="guacamole-agent"
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"
: "${WINDOWS_ADMIN_PASSWORD:?WINDOWS_ADMIN_PASSWORD non défini}"

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
# Chantier E, bloc E2 — les TROIS variables du microphone, plus
# MICRO_MESURE ci-dessus. Sans ces lignes l'agent demarre sans elles ET
# SANS RIEN SIGNALER : piege paye en D1 (SUPERVISEUR), D2
# (MULTIFENETRE_REPRISE) et D7 (AUDIO). Le controle qui vaut n'est pas la
# lecture de ce script mais la TRACE — pour MICRO_PERIPHERIQUE, la ligne
# « cable de rendu retenu pour l'ecriture du micro » porte la valeur
# RETENUE, jamais la seule presence d'une ligne.
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
${AGENT_TRACE_EXCEPTIONS_FICHIER:+\$env:AGENT_TRACE_EXCEPTIONS_FICHIER = '$AGENT_TRACE_EXCEPTIONS_FICHIER'}
${AGENT_TRACE_EXCEPTIONS_AUTOTEST:+\$env:AGENT_TRACE_EXCEPTIONS_AUTOTEST = '$AGENT_TRACE_EXCEPTIONS_AUTOTEST'}
${AGENT_VM:+\$env:AGENT_VM = '$AGENT_VM'}
${AGENT_SECRET:+\$env:AGENT_SECRET = '$AGENT_SECRET'}
# 🔴 AGENT_JETON N'EST PAS TRANSMIS ICI, ET C'EST DÉLIBÉRÉ — le seul manquement
# volontaire de ce fichier, écrit plutôt que subi. Ce dépôt a payé trois fois
# l'oubli d'une variable neuve dans ce script (SUPERVISEUR en D1,
# MULTIFENETRE_REPRISE en D2, AUDIO en D7) ; celle-ci n'est pas du même genre.
# AGENT_JETON est une variable de PASSATION entre processus, posée par le
# superviseur sur ses enfants et sur le pont (agent/src/superviseur/lanceur.rs)
# pour qu'un seul processus par VM ouvre le canal /agent. La poser à la main
# ferait SAUTER l'enrôlement du processus racine : il ne battrait plus le cœur
# de la VM, ne pousserait aucun catalogue, ne recevrait aucun ordre de
# lancement, et son jeton mourrait au bout de dix minutes sans se renouveler.
# Un opérateur pose AGENT_VM et AGENT_SECRET ; la passation ne le regarde pas.
# Le StreamWriter ci-dessous règle l'ÉCRITURE du fichier en UTF-8, mais pas la
# LECTURE de la sortie de l'enfant : PowerShell décode le flux d'agent.exe
# selon \$OutputEncoding / [Console]::OutputEncoding, qui vaut par défaut la
# page de code OEM de la console (CP850/CP437), alors que agent.exe écrit de
# l'UTF-8 — sans ce réglage, les caractères accentués ressortent en mojibake
# même une fois le fichier réécrit proprement. Les deux réglages sont
# nécessaires : celui-ci pour la lecture, le StreamWriter pour l'écriture.
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding(\$false)

# UTF-8 SANS BOM, et sans le retour à la ligne que \`Out-File\` insère à la
# largeur de console : \`Tee-Object\` (PS 5.1) écrit en UTF-16LE et n'a pas de
# paramètre -Encoding, ce qui rendait les journaux de la sonde précédente
# illisibles au \`grep\`. \`Out-File -Encoding utf8\` corrigerait l'encodage mais
# reformaterait les lignes longues. Un StreamWriter explicite ne fait ni l'un
# ni l'autre.
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

echo "agent lancé en session interactive ; journal : /media/vm/dev/agent.log"
