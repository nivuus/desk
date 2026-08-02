Add-Type @"
using System;using System.Runtime.InteropServices;
public class C2 { [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h,uint m,IntPtr w,IntPtr l,uint f,uint t,out IntPtr r); }
"@
# Ferme la SEULE fenetre dont le titre porte fenetre-2 : une fermeture reelle,
# donc une cloture de session SOLLICITEE.
Get-Process notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like '*fenetre-2*' } | ForEach-Object {
  $r = [IntPtr]::Zero
  [void][C2]::SendMessageTimeout($_.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero, 2, 3000, [ref]$r)
}
Start-Sleep -Seconds 3
