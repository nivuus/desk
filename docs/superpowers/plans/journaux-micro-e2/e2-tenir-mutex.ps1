# Tient le mutex nomme du cable, puis attend d'etre TUE.
#
# 🔴 CE PROCESSUS EST DESTINE A MOURIR BRUTALEMENT. C'est ce qui produit
# WAIT_ABANDONED cote agent : a la mort du proprietaire, Windows abandonne le
# mutex et le suivant l'obtient quand meme. La spec §9 demande cette semantique
# de liberation ; rien dans ce depot ne l'avait jamais observee.
$ErrorActionPreference = 'Continue'
$journal = New-Object System.IO.StreamWriter('C:\dev\e2-tenir-mutex.log', $false, (New-Object System.Text.UTF8Encoding($false)))
try {
  $cree = $false
  $m = New-Object System.Threading.Mutex($false, 'Global\guacamole-agent-micro-cable', [ref]$cree)
  $journal.WriteLine('MUTEX ouvert, cree=' + $cree + ' pid=' + $PID + ' ' + (Get-Date).ToString('o')); $journal.Flush()
  $obtenu = $m.WaitOne(2000)
  $journal.WriteLine('ACQUIS=' + $obtenu + ' ' + (Get-Date).ToString('o')); $journal.Flush()
  if (-not $obtenu) { $journal.WriteLine('REFUS : quelqu un d autre le tient deja'); $journal.Flush(); return }
  # On NE relache JAMAIS : le test est la mort brutale.
  Start-Sleep -Seconds 600
  $journal.WriteLine('FIN DU DELAI SANS AVOIR ETE TUE ' + (Get-Date).ToString('o'))
} catch { $journal.WriteLine('ECHEC : ' + $_.Exception.ToString()) } finally { $journal.Flush(); $journal.Close() }
