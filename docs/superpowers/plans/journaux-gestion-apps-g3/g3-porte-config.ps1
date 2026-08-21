$ErrorActionPreference = 'Continue'
$k = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
Write-Output "=== CONFIGURATION UAC ==="
foreach ($n in @('EnableLUA','ConsentPromptBehaviorAdmin','PromptOnSecureDesktop','ConsentPromptBehaviorUser','FilterAdministratorToken')) {
    $v = (Get-ItemProperty -Path $k -Name $n -ErrorAction SilentlyContinue).$n
    if ($null -eq $v) { Write-Output ("{0} = <ABSENT>" -f $n) } else { Write-Output ("{0} = {1}" -f $n, $v) }
}
Write-Output ""
Write-Output "=== SESSION INTERACTIVE ==="
Write-Output ("whoami            : " + (whoami))
Write-Output ("session id        : " + [System.Diagnostics.Process]::GetCurrentProcess().SessionId)
$id = [Security.Principal.WindowsIdentity]::GetCurrent()
$p  = New-Object Security.Principal.WindowsPrincipal($id)
Write-Output ("deja eleve        : " + $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator))
Write-Output ""
Write-Output "=== GROUPES (administrateurs ?) ==="
whoami /groups | Select-String -Pattern 'Administrateurs|Administrators|S-1-5-32-544|Niveau|Label'
Write-Output ""
Write-Output "=== VERSION ==="
Write-Output ((Get-CimInstance Win32_OperatingSystem).Caption + " build " + [System.Environment]::OSVersion.Version.Build)
