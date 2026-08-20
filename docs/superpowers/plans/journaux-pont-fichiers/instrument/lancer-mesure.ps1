$p = Start-Process powershell -PassThru -WindowStyle Hidden `
  -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','C:\dev\mesurer.ps1'
Write-Output ("pid=" + $p.Id)
Start-Sleep -Seconds 3
Write-Output ("mesure existe apres 3 s : " + (Test-Path 'C:\dev\mesure.txt'))
