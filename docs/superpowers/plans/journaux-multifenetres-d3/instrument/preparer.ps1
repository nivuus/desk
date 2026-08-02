Add-Type @"
using System;using System.Runtime.InteropServices;
public class C { [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h,uint m,IntPtr w,IntPtr l,uint f,uint t,out IntPtr r); }
"@
# Fermeture douce (WM_CLOSE) de toutes les fenetres principales : on ne tue
# aucun processus, explorer.exe compris.
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 } | ForEach-Object {
  $r = [IntPtr]::Zero
  [void][C]::SendMessageTimeout($_.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero, 2, 3000, [ref]$r)
}
Start-Sleep -Seconds 3
1..5 | ForEach-Object { Set-Content -Path ("C:\dev\fenetre-{0}.txt" -f $_) -Value ("contenu de la fenetre {0}" -f $_) -Encoding ASCII }
Start-Process notepad -ArgumentList 'C:\dev\fenetre-1.txt'
Start-Sleep -Seconds 2
