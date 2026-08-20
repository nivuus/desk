# Enveloppe MUETTE du juge, pour invocation par WinRM.
#
# 🔴 nodejs-winrm REND LA MAIN DES LA PREMIERE LIGNE RECUE ET TUE LE PROCESSUS
# DISTANT. Or micro-ecoute-e2.ps1 ecrit sa premiere ligne AVANT de mesurer :
# invoque nu, il serait tue avant d'avoir ouvert le point de terminaison. Cette
# enveloppe avale toute sa sortie (*> $null) et n'ecrit qu'une seule ligne, a la
# fin. Le juge, lui, tient son propre journal par StreamWriter : rien n'est perdu.
param([string]$Etiquette, [int]$Secondes = 8, [string]$Prefixe = 'CABLE Output')
$ErrorActionPreference = 'Continue'
$journal = 'C:\dev\e2-juge-' + $Etiquette + '.log'
& powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\micro-ecoute-e2.ps1 -Prefixe $Prefixe -Secondes $Secondes -Journal $journal *> $null
'JUGE_TERMINE ' + $Etiquette
