# Remise a zero de l'etat de fenetres entre deux passages de recette.
# Les Bloc-notes du passage precedent portent des modifications non
# enregistrees (le texte injecte au clavier) : WM_CLOSE y ouvre une boite de
# dialogue « Enregistrer ? » et la fenetre reste ouverte. On termine donc ces
# processus de test, puis on rouvre la seule fenetre preexistante voulue.
Get-Process notepad, mspaint, wordpad -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
Add-Type @"
using System;using System.Runtime.InteropServices;
public class C3 { [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h,uint m,IntPtr w,IntPtr l,uint f,uint t,out IntPtr r); }
"@
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 } | ForEach-Object {
  $r = [IntPtr]::Zero
  [void][C3]::SendMessageTimeout($_.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero, 2, 3000, [ref]$r)
}
Start-Sleep -Seconds 3
1..6 | ForEach-Object { Set-Content -Path ("C:\dev\fenetre-{0}.txt" -f $_) -Value ("contenu de la fenetre {0}" -f $_) -Encoding ASCII }
Start-Process notepad -ArgumentList 'C:\dev\fenetre-1.txt'
Start-Sleep -Seconds 2
