# Releve d'etat par micro-format-e1.ps1 (E1), depuis un processus NEUF, dans
# un fichier : nodejs-winrm tue le processus distant des la premiere ligne
# recue sur la sortie standard, donc rien ne doit y etre ecrit avant la fin.
param([string]$Journal = 'C:\dev\micro-e2-format.log')
& powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\micro-format-e1.ps1 *>&1 |
  Out-File -FilePath $Journal -Encoding utf8 -Width 500
