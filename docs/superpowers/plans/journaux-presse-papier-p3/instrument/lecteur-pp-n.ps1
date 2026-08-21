# Sous-bloc P3 (presse-papier) — LE LECTEUR MULTI-FENETRES, un seul processus
# pour toute l'execution, lance dans la SESSION 1 AVANT le superviseur.
#
# Derive de `journaux-presse-papier-p2/instrument/lecteur-pp.ps1`, REEMPLOYE et
# non reecrit : meme protocole par fichiers, meme numero d'ordre qui rend
# l'attente du pilote eprouvable sur un FAIT, meme raison d'etre un processus
# unique. Ce qui change tient en un point, et c'est TOUT le sous-bloc :
#
# 🔴 IL LIT UNE FENETRE DESIGNEE PAR SON PID, JAMAIS « le » Bloc-notes.
# `FindWindowW('Notepad', $null)` rend la PREMIERE fenetre de cette classe : a
# trois Bloc-notes, elle en designe un arbitrairement, et le releve serait
# ininterpretable. Le pilote lit les `pid` dans `agent-plat.log` — la trace
# `fenetre attachee au capteur`, a laquelle la tache 8 de P3 a ajoute ce champ,
# et qui est LA SEULE attribution session ↔ fenetre Windows du depot — puis
# demande ici la fenetre de CE pid.
#
# 🔴 `EnumWindows` ET PAS `FindWindow` POUR LA RESOLUTION PAR PID, et ce n'est
# pas un gout : `Get-Process -Id <n>` expose bien `MainWindowHandle`, mais il
# rend `0` sur une fenetre pourtant presente quand le processus vient de
# demarrer ou que sa fenetre principale n'est pas encore etablie. `EnumWindows`
# la trouve. Les DEUX sont releves, et l'ecart est rendu : c'est le controle
# croise que P2 a paye sur `FindWindowW($null, ...)`.
#
# ⚠️ `[NullString]::Value`, JAMAIS `$null` — PowerShell marshale `$null` en
# CHAINE VIDE pour un parametre `string`, et la premiere rediction de P2
# cherchait donc une classe vide. Releve, pas prudentiel.
#
# 🔴 LA SESSION 1 EST OBLIGATOIRE, ET POUR DEUX RAISONS DISTINCTES :
#   - `Get-Clipboard` par WinRM (session 0) rend `-1`, le presse-papier etant
#     par STATION DE FENETRES (mesure, sonde P0 de P1) ;
#   - `EnumWindows` depuis la session 0 ne voit AUCUNE fenetre de la session 1.
#
# Protocole, par fichiers sur le partage :
#   C:\dev\pp3-ordre.txt — "<n>|<mode>[|<argument>]"
#       clipboard                    ce que l'agent a ecrit dans la VM
#       fenetre|<pid>                le texte de la fenetre de CE pid
#       semer|<pid>|<marqueur>       pre-semer un marqueur distinct (D-P3-9)
#       vider|<pid>                  vider cette fenetre-la
#       inventaire                   pid + hwnd + titre de chaque Bloc-notes
#       stop
#   C:\dev\pp3-fait.txt  — "<n>|<compte-rendu>"
$ErrorActionPreference = 'Stop'

$journal = 'C:\dev\pp3-lecteur.log'
function Dire($m) {
    Add-Content -Path $journal -Value "$([DateTime]::UtcNow.ToString('o')) $m" -Encoding utf8
}

Add-Type -Namespace P3 -Name Win -MemberDefinition @'
[DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr apres, string cls, string titre);
[DllImport("user32.dll", CharSet=CharSet.Unicode)]
public static extern int SendMessageW(IntPtr h, int msg, int wp, System.Text.StringBuilder lp);
[DllImport("user32.dll")]
public static extern int SendMessageW(IntPtr h, int msg, int wp, int lp);
[DllImport("user32.dll")]
public static extern bool EnumWindows(EnumProc cb, IntPtr param);
[DllImport("user32.dll")]
public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll", CharSet=CharSet.Unicode)]
public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll", CharSet=CharSet.Unicode)]
public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
public delegate bool EnumProc(IntPtr h, IntPtr p);
'@

$NUL = [NullString]::Value

# Toutes les fenetres de premier niveau VISIBLES, avec leur pid, leur classe et
# leur titre. Une seule enumeration sert a tout.
function Fenetres {
    $liste = New-Object System.Collections.ArrayList
    $cb = [P3.Win+EnumProc] {
        param($h, $p)
        if ([P3.Win]::IsWindowVisible($h)) {
            $pid_ = 0
            [void][P3.Win]::GetWindowThreadProcessId($h, [ref]$pid_)
            $cls = New-Object System.Text.StringBuilder 256
            [void][P3.Win]::GetClassNameW($h, $cls, 256)
            $tit = New-Object System.Text.StringBuilder 512
            [void][P3.Win]::GetWindowTextW($h, $tit, 512)
            [void]$liste.Add([pscustomobject]@{
                hwnd = $h; pid = $pid_; classe = $cls.ToString(); titre = $tit.ToString()
            })
        }
        return $true
    }
    [void][P3.Win]::EnumWindows($cb, [IntPtr]::Zero)
    return $liste
}

# La fenetre principale d'un PID donne. On prefere une fenetre a titre non vide
# et de classe connue ; a defaut, la premiere visible de ce pid.
function FenetreDuPid([int]$cible) {
    $f = Fenetres | Where-Object { $_.pid -eq $cible }
    if (-not $f) { return $null }
    $bonne = $f | Where-Object { $_.titre -ne '' } | Select-Object -First 1
    if ($bonne) { return $bonne }
    return ($f | Select-Object -First 1)
}

# Le controle d'edition d'une fenetre Bloc-notes. La fenetre principale ne
# porte que son titre ; c'est son enfant `Edit` (ou `RichEditD2DPT` sur le
# Bloc-notes moderne) qui porte le texte.
function ControleDEdition([IntPtr]$principal) {
    $e = [P3.Win]::FindWindowExW($principal, [IntPtr]::Zero, 'Edit', $NUL)
    if ($e -eq [IntPtr]::Zero) {
        $e = [P3.Win]::FindWindowExW($principal, [IntPtr]::Zero, 'RichEditD2DPT', $NUL)
    }
    return $e
}

function TexteDuPid([int]$cible) {
    $f = FenetreDuPid $cible
    if ($null -eq $f) { return $null }
    $e = ControleDEdition $f.hwnd
    if ($e -eq [IntPtr]::Zero) { return '<aucun controle d''edition>' }
    $n = [P3.Win]::SendMessageW($e, 0x000E, 0, 0)   # WM_GETTEXTLENGTH
    $sb = New-Object System.Text.StringBuilder ($n + 2)
    [void][P3.Win]::SendMessageW($e, 0x000D, $n + 1, $sb)   # WM_GETTEXT
    return $sb.ToString()
}

function EcrireDansLePid([int]$cible, [string]$texte) {
    $f = FenetreDuPid $cible
    if ($null -eq $f) { return 'pid sans fenetre' }
    $e = ControleDEdition $f.hwnd
    if ($e -eq [IntPtr]::Zero) { return 'aucun controle d''edition' }
    $sb = New-Object System.Text.StringBuilder $texte
    [void][P3.Win]::SendMessageW($e, 0x000C, 0, $sb)   # WM_SETTEXT
    return "ecrit longueur=$($texte.Length)"
}

Remove-Item 'C:\dev\pp3-ordre.txt', 'C:\dev\pp3-fait.txt' -ErrorAction SilentlyContinue
Dire 'lecteur pret'
$dernier = ''

while ($true) {
    Start-Sleep -Milliseconds 250
    if (-not (Test-Path 'C:\dev\pp3-ordre.txt')) { continue }
    try { $brut = (Get-Content 'C:\dev\pp3-ordre.txt' -Raw -Encoding utf8).Trim() } catch { continue }
    if ($brut -eq '' -or $brut -eq $dernier) { continue }
    $dernier = $brut
    $bouts = $brut.Split('|')
    $n = $bouts[0]
    $mode = if ($bouts.Length -ge 2) { $bouts[1] } else { '' }
    $a1 = if ($bouts.Length -ge 3) { $bouts[2] } else { '' }
    $a2 = if ($bouts.Length -ge 4) { $bouts[3] } else { '' }
    Dire "ordre $n mode=$mode a1=$a1"
    $rendu = ''
    try {
        switch ($mode) {
            'clipboard' {
                $lu = ''
                try { $lu = Get-Clipboard -Raw } catch { $lu = '<illisible>' }
                if ($null -eq $lu) { $lu = '' }
                $debut = if ($lu.Length -gt 60) { $lu.Substring(0, 60) } else { $lu }
                $rendu = "clipboard longueur=$($lu.Length) debut=[$debut]"
            }
            'fenetre' {
                $t = TexteDuPid ([int]$a1)
                if ($null -eq $t) { $rendu = "fenetre pid=$a1 ABSENTE" }
                else {
                    $debut = if ($t.Length -gt 120) { $t.Substring(0, 120) } else { $t }
                    $rendu = "fenetre pid=$a1 longueur=$($t.Length) texte=[$debut]"
                }
            }
            'semer' { $rendu = "semer pid=$a1 : " + (EcrireDansLePid ([int]$a1) $a2) }
            'vider' { $rendu = "vider pid=$a1 : " + (EcrireDansLePid ([int]$a1) '') }
            'inventaire' {
                # Le CONTROLE CROISE : `Get-Process -Id <n>` et `EnumWindows`
                # sont deux voies INDEPENDANTES, et l'ecart entre elles est
                # rendu plutot que masque.
                $parts = @()
                foreach ($f in (Fenetres | Where-Object { $_.classe -like '*Notepad*' -or $_.titre -like '*Bloc-notes*' -or $_.titre -like '*Notepad*' })) {
                    $mw = 0
                    try { $mw = [int64](Get-Process -Id $f.pid -ErrorAction Stop).MainWindowHandle } catch { $mw = -1 }
                    $parts += "pid=$($f.pid) hwnd=$([int64]$f.hwnd) mainwindow=$mw classe=$($f.classe) titre=[$($f.titre)]"
                }
                $rendu = 'inventaire ' + ($parts -join ' ; ')
            }
            'stop' { Dire 'arret demande'; Set-Content -Path 'C:\dev\pp3-fait.txt' -Value "$n|arret" -Encoding utf8; exit 0 }
            default { $rendu = "mode inconnu : $mode" }
        }
    } catch {
        $rendu = "ECHEC : $($_.Exception.Message)"
    }
    Dire "fait $n : $rendu"
    Set-Content -Path 'C:\dev\pp3-fait.txt' -Value "$n|$rendu" -Encoding utf8
}
