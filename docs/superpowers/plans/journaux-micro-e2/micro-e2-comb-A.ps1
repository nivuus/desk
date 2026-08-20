# Combinaison A : rendu session 1, capture session 1. LE cas du produit.
# AUCUNE sortie avant la fin : nodejs-winrm rend la main des la premiere
# ligne recue et TUE le processus distant. Mesure : la premiere version de ce
# script journalisait le retour de schtasks, et le second /run n'a jamais eu
# lieu (Dernier resultat 267011 = SCHED_S_TASK_HAS_NOT_RUN).
Remove-Item C:\dev\micro-e2-ecoute-A.log -ErrorAction SilentlyContinue
schtasks /run /tn micro-e2-jouer *> $null
Start-Sleep -Seconds 4
schtasks /run /tn micro-e2-ecouter-A *> $null
Start-Sleep -Seconds 24
Write-Output 'COMBINAISON A TERMINEE'
