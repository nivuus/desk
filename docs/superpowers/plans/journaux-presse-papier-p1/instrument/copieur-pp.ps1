# Sous-bloc P1 (presse-papier) — LE COPIEUR DE RECETTE, un seul processus pour
# toute l'exécution, lancé dans la SESSION 1 avant le superviseur.
#
# 🔴 POURQUOI UN PROCESSUS UNIQUE PLUTÔT QU'UNE TÂCHE PLANIFIÉE PAR COPIE.
# La première exécution de cette recette copiait par un `vm-it.sh` à chaque
# geste. Relevé : **une page navigateur de plus s'ouvrait 0,5 s après CHAQUE
# appel**, la fenêtre de session perdait le focus, sa page était renavigée, la
# session tombait et le superviseur relançait un enfant. La cause est le piège
# déjà écrit par D11 : *la console PowerShell d'une tâche planifiée est
# ÉLIGIBLE à la capture*, même en `-WindowStyle Hidden` — le superviseur lui
# donne une session, la page-shell lui ouvre une fenêtre, et l'instrument
# détruit ce qu'il mesure. Ici, une seule console existe, elle naît AVANT le
# superviseur, et plus rien ne s'ouvre pendant la mesure.
#
# ⚠️ LA SESSION 1 EST OBLIGATOIRE : `Get-Clipboard` par WinRM (session 0) rend
# `-1`, le presse-papier étant par station de fenêtres (mesuré, sonde P0).
#
# Protocole, par fichiers sur le partage :
#   C:\dev\pp-ordre.txt  — une ligne : "<n>|<mode>|<texte>"
#                          modes : texte | gros | notepad | stop
#   C:\dev\pp-fait.txt   — une ligne : "<n>|<compte-rendu>"
#
# Le numéro `<n>` est ce qui rend l'attente du pilote ÉPROUVABLE : il attend
# que `pp-fait.txt` porte le MÊME numéro, jamais un simple délai.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms

$journal = 'C:\dev\pp-copieur.log'
function Dire($m) {
    $ligne = "$([DateTime]::UtcNow.ToString('o')) $m"
    Add-Content -Path $journal -Value $ligne -Encoding utf8
}

Remove-Item 'C:\dev\pp-ordre.txt', 'C:\dev\pp-fait.txt' -ErrorAction SilentlyContinue
Dire 'copieur pret'
$dernier = ''

while ($true) {
    Start-Sleep -Milliseconds 300
    if (-not (Test-Path 'C:\dev\pp-ordre.txt')) { continue }
    try { $brut = (Get-Content 'C:\dev\pp-ordre.txt' -Raw -Encoding utf8).Trim() } catch { continue }
    if ($brut -eq '' -or $brut -eq $dernier) { continue }
    $dernier = $brut
    $bouts = $brut.Split('|', 3)
    $n = $bouts[0]
    $mode = $bouts[1]
    $texte = if ($bouts.Length -ge 3) { $bouts[2] } else { '' }
    Dire "ordre $n mode=$mode longueur=$($texte.Length)"
    $rendu = ''
    try {
        switch ($mode) {
            'texte' {
                Set-Clipboard -Value $texte
                $rendu = "texte pose, longueur=$($texte.Length)"
            }
            'gros' {
                # 100 KiB : au-dessus de PRESSE_PAPIER_MAX (64 KiB), donc REFUS
                # attendu — jamais une troncature.
                $t = 'g' * 102400
                Set-Clipboard -Value $t
                $rendu = "gros pose, longueur=$($t.Length)"
            }
            'notepad' {
                # LA VRAIE COPIE : Bloc-notes, saisie, Ctrl+A, Ctrl+C. C'est le
                # geste que le critere ① nomme, et non un `Set-Clipboard` de plus.
                # ⚠️ Elle ouvre une fenetre ELIGIBLE : le superviseur lui donnera
                # une session. C'est assume, et c'est pourquoi elle vient EN
                # DERNIER, apres toutes les autres mesures.
                $p = Start-Process notepad -PassThru
                Start-Sleep -Seconds 3
                [System.Windows.Forms.SendKeys]::SendWait($texte)
                Start-Sleep -Milliseconds 600
                [System.Windows.Forms.SendKeys]::SendWait('^a')
                Start-Sleep -Milliseconds 400
                [System.Windows.Forms.SendKeys]::SendWait('^c')
                Start-Sleep -Seconds 2
                # Relire ce que le presse-papier porte VRAIMENT : sans cela, un
                # SendKeys parti dans une autre fenetre se lirait comme un echec
                # du produit.
                $lu = ''
                try { $lu = Get-Clipboard -Raw } catch { $lu = '<illisible>' }
                $extrait = if ($lu.Length -gt 40) { $lu.Substring(0, 40) } else { $lu }
                $p | Stop-Process -Force
                $rendu = "notepad, presse-papier=$($lu.Length) octets, debut=$extrait"
            }
            'stop' { Dire 'arret demande'; Set-Content -Path 'C:\dev\pp-fait.txt' -Value "$n|arret" -Encoding utf8; exit 0 }
            default { $rendu = "mode inconnu : $mode" }
        }
    } catch {
        $rendu = "ECHEC : $($_.Exception.Message)"
    }
    Dire "fait $n : $rendu"
    Set-Content -Path 'C:\dev\pp-fait.txt' -Value "$n|$rendu" -Encoding utf8
}
