# Le residu de la sonde 3 de la spec §12, par WinRT plutot que par COM brut.
#
# 🔴 LA VOIE COM A ECHOUE, ET C'EST RELEVE : le transtypage d'un
# `__ComObject` vers une interface `ComImport` rend $null EN SILENCE sous ce
# PowerShell (`ENUMERATEUR_NUL=True`), interfaces imbriquees comme au niveau du
# namespace, ordre de vtable corrige. Le symptome -- « Impossible d'appeler une
# methode dans une expression Null » -- ne designe pas sa cause.
#
# `Windows.Media.Devices.MediaDevice` rend le MEME identifiant d'endpoint, en
# une ligne et sans vtable a declarer.
$ErrorActionPreference = 'Continue'
Start-Transcript -Path 'C:\dev\e3-defaut-capture.log' -Force | Out-Null
'SESSION=' + [System.Diagnostics.Process]::GetCurrentProcess().SessionId
try {
    $t = [Windows.Media.Devices.MediaDevice, Windows.Media, ContentType = WindowsRuntime]
    foreach ($r in 'Default', 'Communications') {
        $id = $t::GetDefaultAudioCaptureId($r)
        'CAPTURE_' + $r + '=' + $id
    }
} catch { 'WINRT_ECHEC=' + $_.Exception.Message }
# Le nom convivial des points de terminaison de CAPTURE presents, pour lire
# l'identifiant ci-dessus. ⚠️ `Get-PnpDevice` ne dit PAS lequel est par defaut.
Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue |
    Where-Object { $_.Status -eq 'OK' } |
    ForEach-Object { 'ENDPOINT=' + $_.FriendlyName + ' | ' + $_.InstanceId }
Stop-Transcript | Out-Null
