#!/usr/bin/env bash
# Lance l'agent dans la session interactive (session 1) de la VM Windows.
#
# WinRM s'exécute en session 0 : un agent lancé directement par WinRM ne peut
# ni capturer une fenêtre (Windows.Graphics.Capture) ni injecter des entrées
# (SendInput), ces API ne franchissant pas la frontière de session. La tâche
# planifiée avec /IT s'exécute dans la session de l'utilisateur connecté.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TASK_NAME="guacamole-agent"
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"
: "${WINDOWS_ADMIN_PASSWORD:?WINDOWS_ADMIN_PASSWORD non défini}"

# Chemin de l'exécutable, assemblé ICI plutôt que dans le heredoc ci-dessous :
# celui-ci n'est pas entre quotes (il doit interpoler les variables), et un
# `\$` y est une échappée — écrire `target\${AGENT_PROFILE}` y produisait
# `target${AGENT_PROFILE}` littéral, séparateur avalé et variable non
# substituée. Une variable unique, sans antislash devant, n'a pas ce piège :
# son contenu n'est plus réinterprété une fois substitué.
AGENT_EXE="C:\\dev\\target\\${AGENT_PROFILE:-release}\\agent.exe"

# Les variables d'environnement passent par un script d'amorçage : schtasks ne
# permet pas de les transmettre directement.
cat > /media/vm/dev/run-agent.ps1 <<PS1
\$env:SIGNALING_URL = '${SIGNALING_URL:-ws://192.168.3.1:8080}'
\$env:SESSION_ID    = '${SESSION_ID:-demo}'
\$env:LOCAL_IP      = '${LOCAL_IP:-192.168.3.2}'
\$env:RUST_LOG      = '${RUST_LOG:-info}'
\$env:WINDOW_TITLE  = '${WINDOW_TITLE:-firefox}'
${TEST_FILE:+\$env:TEST_FILE = '$TEST_FILE'}
${CAPTURE_TEST:+\$env:CAPTURE_TEST = '$CAPTURE_TEST'}
${SOURCE_TRACE:+\$env:SOURCE_TRACE = '$SOURCE_TRACE'}
${ENCODER_FPS:+\$env:ENCODER_FPS = '$ENCODER_FPS'}
${BITRATE:+\$env:BITRATE = '$BITRATE'}
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
${VIGEM_PROBE:+\$env:VIGEM_PROBE = '$VIGEM_PROBE'}
${VIGEM_PROBE_SECS:+\$env:VIGEM_PROBE_SECS = '$VIGEM_PROBE_SECS'}
${INPUT_LINEARITY_PROBE:+\$env:INPUT_LINEARITY_PROBE = '$INPUT_LINEARITY_PROBE'}
${INPUT_LINEARITY_PAS:+\$env:INPUT_LINEARITY_PAS = '$INPUT_LINEARITY_PAS'}
${INPUT_LINEARITY_REPETITIONS:+\$env:INPUT_LINEARITY_REPETITIONS = '$INPUT_LINEARITY_REPETITIONS'}
${INPUT_LINEARITY_NEUTRALISER:+\$env:INPUT_LINEARITY_NEUTRALISER = '$INPUT_LINEARITY_NEUTRALISER'}
${MULTIFENETRE_DXGI:+\$env:MULTIFENETRE_DXGI = '$MULTIFENETRE_DXGI'}
${MULTIFENETRE_WGC:+\$env:MULTIFENETRE_WGC = '$MULTIFENETRE_WGC'}
${MULTIFENETRE_REPLIS:+\$env:MULTIFENETRE_REPLIS = '$MULTIFENETRE_REPLIS'}
& '${AGENT_EXE}' *>&1 | Tee-Object -FilePath 'C:\dev\agent.log'
PS1

# -WindowStyle Hidden : sans ce drapeau, la console PowerShell qui héberge
# agent.exe s'ouvre au premier plan de la session interactive et peut
# recouvrir entièrement la fenêtre que l'agent est censé capturer (constaté
# en tâche 9 : un essai de recadrage lisait la couleur de fond de CETTE
# console — bleu PowerShell #012456 — au lieu du contenu de la fenêtre
# ciblée, sur la totalité de la zone échantillonnée). Masquer la console
# n'affecte ni la session (toujours 1, toujours interactive) ni la sortie
# (toujours redirigée vers agent.log via Tee-Object).
node "$ROOT/scripts/winrm.js" \
    "schtasks /delete /tn $TASK_NAME /f 2>\$null; \
     schtasks /create /tn $TASK_NAME /f /it /ru '$USER_NAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
       /sc once /st 00:00 \
       /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\run-agent.ps1'; \
     schtasks /run /tn $TASK_NAME"

echo "agent lancé en session interactive ; journal : /media/vm/dev/agent.log"
