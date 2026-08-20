# Rouge (i) du Step 6 : TUER LE PONT EN PLEINE LECTURE, par PID releve.
#
# Attendu (plan, Step 6) : l'Explorateur rend une erreur d'E/S en moins de 5 s
# et NE SE FIGE JAMAIS ; la video continue ; le superviseur relance le pont.
$racine = Join-Path $env:USERPROFILE 'Mes Fichiers'
$flux = New-Object System.IO.StreamWriter('C:\dev\rouge-i.txt', $false, (New-Object System.Text.UTF8Encoding($false)))
function Note([string]$m) { $flux.WriteLine($m); $flux.Flush() }

Note ("racine_presente=" + (Test-Path $racine))

# 🔴 LE PID DU PONT EST RELEVE DANS LE JOURNAL, JAMAIS DEVINE.
# Une premiere version prenait le plus JEUNE des agents, en supposant que le
# pont etait lance en dernier. C'est FAUX : l'ordre est superviseur, capteur,
# pont, PUIS l'enfant de chaque fenetre -- l'enfant est donc le plus jeune, et
# c'est LUI qui a ete tue. Le releve qui en est sorti ne mesurait pas ce qu'il
# annoncait. Le pilote passe desormais le PID lu sur la ligne
# `pont fichiers lancé pid=...` de `agent.log`.
$cibleId = [int](Get-Content 'C:\dev\pid-pont.txt' -ErrorAction SilentlyContinue)
$agents = @(Get-Process agent -ErrorAction SilentlyContinue | Sort-Object StartTime)
Note ("agents_avant=" + $agents.Count)
foreach ($a in $agents) { Note ("agent|" + $a.Id + "|" + $a.StartTime.ToString('HH:mm:ss.fff')) }

# Lancer une lecture EN ARRIERE-PLAN sur le gros fichier.
$job = Start-Job -ScriptBlock {
    param($p)
    $t = Get-Date
    try {
        $fs = [System.IO.File]::Open($p, 'Open', 'Read', 'ReadWrite')
        $tampon = New-Object byte[] 12582912
        $total = 0
        while ($total -lt $tampon.Length) {
            $lu = $fs.Read($tampon, $total, $tampon.Length - $total)
            if ($lu -le 0) { break }
            $total += $lu
        }
        $fs.Close()
        "LECTURE|OK|" + [int]((Get-Date) - $t).TotalMilliseconds + "|lus=$total"
    } catch {
        "LECTURE|ECHEC|" + [int]((Get-Date) - $t).TotalMilliseconds + "|" + ($_.Exception.Message -replace "`r|`n", ' ')
    }
} -ArgumentList (Join-Path $racine 'gros.bin')

Start-Sleep -Seconds 2
Note 'lecture en vol depuis 2 s : mise a mort du pont'

# Tuer le pont : le plus recemment demarre des agents (le pont est lance apres
# le superviseur et le capteur).
Note ("pont_pid_du_journal=" + $cibleId)
$tMort = Get-Date
Stop-Process -Id $cibleId -Force -ErrorAction SilentlyContinue

# Attendre l'issue de la lecture, BORNEE : le critere est « moins de 5 s »,
# on observe jusqu'a 60 s pour pouvoir DIRE si le seuil est franchi.
$fini = Wait-Job $job -Timeout 60
if ($fini) {
    $r = Receive-Job $job
    Note ("issue|" + $r)
    Note ("delai_apres_mort_ms=" + [int]((Get-Date) - $tMort).TotalMilliseconds)
} else {
    Note 'issue|TOUJOURS EN VOL APRES 60 s (FIGE)'
    Note ("delai_apres_mort_ms=" + [int]((Get-Date) - $tMort).TotalMilliseconds)
}
Remove-Job $job -Force -ErrorAction SilentlyContinue

Start-Sleep -Seconds 5
$apres = @(Get-Process agent -ErrorAction SilentlyContinue)
Note ("agents_apres=" + $apres.Count)
foreach ($a in $apres) { Note ("agent_apres|" + $a.Id) }
Note 'FIN ROUGE I'
$flux.Close()
