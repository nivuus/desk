# Copieur de la sonde P0 (presse-papier) : il joue, dans la SESSION 1, les
# copies que la phase B de la sonde doit observer. Il ecrit son propre
# journal horodate, parce qu'une copie qui NE fait PAS bouger le compteur ne
# produit aucune ligne cote agent -- indiscernable d'un script qui n'a pas
# tourne.
$log = 'C:\dev\p0-copieur.log'
function J($m) { "$([DateTime]::UtcNow.ToString('o')) $m" | Out-File -Append -Encoding utf8 $log }
J 'COPIEUR DEBUT'
Start-Sleep -Seconds 6
J 'C1 Set-Clipboard alpha'
Set-Clipboard -Value 'sonde-alpha'
Start-Sleep -Seconds 5
J 'C2 Set-Clipboard beta'
Set-Clipboard -Value 'sonde-beta-un-peu-plus-long-que-alpha'
Start-Sleep -Seconds 5
J 'C3 Set-Clipboard gamma'
Set-Clipboard -Value 'sonde-gamma'
Start-Sleep -Seconds 5
J 'C4 Set-Clipboard gamma IDENTIQUE a C3 (autre processus que la sonde)'
Set-Clipboard -Value 'sonde-gamma'
Start-Sleep -Seconds 5
J 'C5 bloc-notes : ouverture'
try {
  $np = Start-Process notepad -PassThru
  Start-Sleep -Seconds 3
  $sh = New-Object -ComObject WScript.Shell
  $null = $sh.AppActivate($np.Id)
  Start-Sleep -Milliseconds 800
  $sh.SendKeys('copie-reelle-depuis-le-bloc-notes')
  Start-Sleep -Milliseconds 800
  $sh.SendKeys('^a')
  Start-Sleep -Milliseconds 300
  $sh.SendKeys('^c')
  J 'C5 bloc-notes : Ctrl+C envoye'
  Start-Sleep -Seconds 4
  Stop-Process -Id $np.Id -Force
  J 'C5 bloc-notes : ferme'
} catch {
  J "C5 bloc-notes ECHEC : $_"
}
J 'COPIEUR FIN'
