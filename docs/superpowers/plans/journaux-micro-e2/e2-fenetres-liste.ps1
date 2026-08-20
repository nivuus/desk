$ErrorActionPreference='Continue'
Start-Transcript -Path 'C:\dev\e2-fenetres-liste-trace.log' -Force | Out-Null
Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;using System.Collections.Generic;
public static class W{
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
 delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")] static extern int GetWindowThreadProcessId(IntPtr h,out int pid);
 public static List<string> Liste(){var r=new List<string>();
  EnumWindows((h,l)=>{ if(IsWindowVisible(h)){var sb=new StringBuilder(512);GetWindowTextW(h,sb,512);
   if(sb.Length>0){int pid;GetWindowThreadProcessId(h,out pid);r.Add("HWND=0x"+h.ToInt64().ToString("x")+" pid="+pid+" titre="+sb.ToString());}}
   return true;},IntPtr.Zero); return r;}}
'@
$sortie = [W]::Liste() -join "`r`n"
[System.IO.File]::WriteAllText('C:\dev\e2-fenetres-liste.log', $sortie, (New-Object System.Text.UTF8Encoding($false)))

Stop-Transcript | Out-Null
