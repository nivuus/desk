Add-Type @"
using System;using System.Runtime.InteropServices;
public class C { [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h,uint m,IntPtr w,IntPtr l,uint f,uint t,out IntPtr r); }
"@
# Etat de depart de la recette D4 : ZERO fenetre Bloc-notes/Paint/Wordpad
# ouverte quand le superviseur demarre. Les fenetres sont ouvertes une par
# une APRES lui : c'est le cas produit (un utilisateur ouvre une
# application), pas l'enumeration initiale.
#
# Douze fichiers, la ou D3 en preparait quatre : la montee du critere 1 va
# au-dela de quatre par construction.
Get-Process notepad, mspaint, wordpad -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like '*fenetre-*' } | ForEach-Object {
  $r = [IntPtr]::Zero
  [void][C]::SendMessageTimeout($_.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero, 2, 3000, [ref]$r)
}
Start-Sleep -Seconds 2
1..12 | ForEach-Object { Set-Content -Path ("C:\dev\fenetre-{0}.txt" -f $_) -Value ("contenu de la fenetre {0}" -f $_) -Encoding ASCII }
