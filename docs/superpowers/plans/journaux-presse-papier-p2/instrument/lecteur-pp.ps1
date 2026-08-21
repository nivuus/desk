# Sous-bloc P2 (presse-papier) — LE LECTEUR DE RECETTE, un seul processus pour
# toute l'execution, lance dans la SESSION 1 avant le superviseur.
#
# C'est l'inverse exact du COPIEUR de P1 (`copieur-pp.ps1`), et il en reprend
# tout : un processus unique, le meme protocole par fichiers, le meme numero
# d'ordre qui rend l'attente du pilote eprouvable sur un FAIT.
#
# 🔴 POURQUOI UN PROCESSUS UNIQUE PLUTOT QU'UNE TACHE PLANIFIEE PAR LECTURE.
# La premiere execution de la recette de P1 lisait par un `vm-it.sh` a chaque
# geste : **une page navigateur de plus s'ouvrait 0,5 s apres CHAQUE appel**,
# la console PowerShell d'une tache planifiee etant ELIGIBLE a la capture
# (piege de D11), meme en `-WindowStyle Hidden`. L'instrument detruisait ce
# qu'il mesurait. Ici, une seule console existe, elle nait AVANT le
# superviseur, et plus rien ne s'ouvre pendant la mesure.
#
# 🔴 LA SESSION 1 EST OBLIGATOIRE, ET POUR DEUX RAISONS DISTINCTES :
#   - `Get-Clipboard` par WinRM (session 0) rend `-1`, le presse-papier etant
#     par STATION DE FENETRES (mesure, sonde P0 de P1) ;
#   - `EnumWindows` depuis la session 0 ne voit AUCUNE fenetre de la session 1 :
#     un `WM_GETTEXT` lance par WinRM ne trouverait jamais le Bloc-notes.
#
# Protocole, par fichiers sur le partage :
#   C:\dev\pp2-ordre.txt — une ligne : "<n>|<mode>"
#                          modes : clipboard | notepad | vider-notepad | stop
#   C:\dev\pp2-fait.txt  — une ligne : "<n>|<compte-rendu>"
$ErrorActionPreference = 'Stop'

$journal = 'C:\dev\pp2-lecteur.log'
function Dire($m) {
    Add-Content -Path $journal -Value "$([DateTime]::UtcNow.ToString('o')) $m" -Encoding utf8
}

# WM_GETTEXT sur le controle d'edition du Bloc-notes. La fenetre principale
# (classe `Notepad`) ne porte que son titre ; c'est son enfant de classe `Edit`
# qui porte le texte.
Add-Type -Namespace P2 -Name Win -MemberDefinition @'
[DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
public static extern IntPtr FindWindowW(string cls, string titre);
[DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr apres, string cls, string titre);
[DllImport("user32.dll", CharSet=CharSet.Unicode)]
public static extern int GetWindowTextLengthW(IntPtr h);
[DllImport("user32.dll", CharSet=CharSet.Unicode)]
public static extern int SendMessageW(IntPtr h, int msg, int wp, System.Text.StringBuilder lp);
[DllImport("user32.dll")]
public static extern int SendMessageW(IntPtr h, int msg, int wp, int lp);
'@

# 🔴 `[NullString]::Value`, JAMAIS `$null`, et c'est MESURÉ, pas prudentiel.
# PowerShell marshale `$null` en CHAÎNE VIDE pour un paramètre `string` : la
# première rédaction cherchait donc un Bloc-notes dont le TITRE est vide, et
# `FindWindowW($null, <titre>)` cherchait une CLASSE vide — d'où
# `ERROR_INVALID_NAME` (123). Les deux rendaient zéro sur une fenêtre bien
# présente, qu'`EnumWindows` trouvait pourtant. Relevé le 21 août 2026,
# `C:\dev\pp2-fw2.txt` puis `pp2-fw3.txt`.
$NUL = [NullString]::Value

function TexteDuBlocNotes {
    $principal = [P2.Win]::FindWindowW('Notepad', $NUL)
    if ($principal -eq [IntPtr]::Zero) { return $null }
    $edit = [P2.Win]::FindWindowExW($principal, [IntPtr]::Zero, 'Edit', $NUL)
    if ($edit -eq [IntPtr]::Zero) {
        # Bloc-notes moderne (RichEditD2DPT) : on tente la classe alternative.
        $edit = [P2.Win]::FindWindowExW($principal, [IntPtr]::Zero, 'RichEditD2DPT', $NUL)
    }
    if ($edit -eq [IntPtr]::Zero) { return '<aucun controle d''edition>' }
    # WM_GETTEXTLENGTH = 0x000E, WM_GETTEXT = 0x000D
    $n = [P2.Win]::SendMessageW($edit, 0x000E, 0, 0)
    $sb = New-Object System.Text.StringBuilder ($n + 2)
    [void][P2.Win]::SendMessageW($edit, 0x000D, $n + 1, $sb)
    return $sb.ToString()
}

function ViderLeBlocNotes {
    $principal = [P2.Win]::FindWindowW('Notepad', $NUL)
    if ($principal -eq [IntPtr]::Zero) { return 'aucun bloc-notes' }
    $edit = [P2.Win]::FindWindowExW($principal, [IntPtr]::Zero, 'Edit', $NUL)
    if ($edit -eq [IntPtr]::Zero) { $edit = [P2.Win]::FindWindowExW($principal, [IntPtr]::Zero, 'RichEditD2DPT', $NUL) }
    if ($edit -eq [IntPtr]::Zero) { return 'aucun controle d''edition' }
    # WM_SETTEXT = 0x000C, avec une chaine vide.
    $vide = New-Object System.Text.StringBuilder ''
    [void][P2.Win]::SendMessageW($edit, 0x000C, 0, $vide)
    return 'vide'
}

Remove-Item 'C:\dev\pp2-ordre.txt', 'C:\dev\pp2-fait.txt' -ErrorAction SilentlyContinue
Dire 'lecteur pret'
$dernier = ''

while ($true) {
    Start-Sleep -Milliseconds 250
    if (-not (Test-Path 'C:\dev\pp2-ordre.txt')) { continue }
    try { $brut = (Get-Content 'C:\dev\pp2-ordre.txt' -Raw -Encoding utf8).Trim() } catch { continue }
    if ($brut -eq '' -or $brut -eq $dernier) { continue }
    $dernier = $brut
    $bouts = $brut.Split('|', 2)
    $n = $bouts[0]
    $mode = if ($bouts.Length -ge 2) { $bouts[1] } else { '' }
    Dire "ordre $n mode=$mode"
    $rendu = ''
    try {
        switch ($mode) {
            'clipboard' {
                # Ce que l'AGENT a ecrit dans le presse-papier de la VM. Distinct
                # du Bloc-notes : separer les deux rend la mesure DIAGNOSTIQUE —
                # « l'agent a ecrit » et « l'injection a eu lieu » sont deux
                # faits, et les confondre ferait imputer l'un a l'autre.
                $lu = ''
                try { $lu = Get-Clipboard -Raw } catch { $lu = '<illisible>' }
                if ($null -eq $lu) { $lu = '' }
                $debut = if ($lu.Length -gt 60) { $lu.Substring(0, 60) } else { $lu }
                $rendu = "clipboard longueur=$($lu.Length) debut=[$debut]"
            }
            'notepad' {
                $t = TexteDuBlocNotes
                if ($null -eq $t) { $rendu = 'notepad ABSENT' }
                else {
                    $debut = if ($t.Length -gt 120) { $t.Substring(0, 120) } else { $t }
                    $rendu = "notepad longueur=$($t.Length) texte=[$debut]"
                }
            }
            'vider-notepad' { $rendu = 'vider : ' + (ViderLeBlocNotes) }
            'stop' { Dire 'arret demande'; Set-Content -Path 'C:\dev\pp2-fait.txt' -Value "$n|arret" -Encoding utf8; exit 0 }
            default { $rendu = "mode inconnu : $mode" }
        }
    } catch {
        $rendu = "ECHEC : $($_.Exception.Message)"
    }
    Dire "fait $n : $rendu"
    Set-Content -Path 'C:\dev\pp2-fait.txt' -Value "$n|$rendu" -Encoding utf8
}
