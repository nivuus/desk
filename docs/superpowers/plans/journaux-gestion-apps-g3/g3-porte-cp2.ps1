$out = 'C:\dev\g3-porte-cp2.txt'
function L($m) { $m | Out-File -FilePath $out -Append -Encoding utf8 }
"=== (e) CreateProcess (via .NET, UseShellExecute=false) ===" | Out-File -FilePath $out -Encoding utf8
L ("appelant deja eleve : " + (New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator))
L ("integrite du jeton  : " + ((whoami /groups | Select-String 'S-1-16-') -replace '\s+',' '))
L ""
foreach ($c in @('wusa.exe','SystemPropertiesAdvanced.exe')) {
    $p = "C:\Windows\System32\$c"
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $p
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    try {
        $pr = [System.Diagnostics.Process]::Start($psi)
        L ("  " + $c + " (requireAdministrator) : REUSSI, pid=" + $pr.Id + "  -> AUCUN 740")
        Start-Sleep -Milliseconds 500
        try { $pr.Kill() } catch {}
    } catch [System.ComponentModel.Win32Exception] {
        $e = $_.Exception
        L ("  " + $c + " : ECHEC, NativeErrorCode=" + $e.NativeErrorCode + $(if ($e.NativeErrorCode -eq 740) { '  <-- ERROR_ELEVATION_REQUIRED (740)' } else { '' }))
        L ("       message : " + $e.Message)
    } catch {
        L ("  " + $c + " : ECHEC AUTRE, " + $_.Exception.GetType().FullName + " : " + $_.Exception.Message)
    }
}
