$p = Start-Process powershell -PassThru -WindowStyle Hidden `
  -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','C:\dev\rouge-i.ps1'
Write-Output ("pid=" + $p.Id)
