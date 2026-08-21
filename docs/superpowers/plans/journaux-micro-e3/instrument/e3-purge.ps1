$env:Path += ';C:\Users\Administrateur\.cargo\bin'
Set-Location C:\dev
cargo clean --release -p proto -p agent 2>&1 | Out-String
'PURGE FAITE'
