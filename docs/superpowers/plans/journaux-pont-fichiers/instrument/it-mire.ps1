$chrome = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
$udd = 'C:\dev\udd-mire-f1'
Start-Process -FilePath $chrome -ArgumentList @(
  '--app=http://192.168.3.1:5399/mire.html',
  "--user-data-dir=$udd",
  '--no-first-run','--no-default-browser-check',
  '--window-size=1280,720','--window-position=100,100'
)
