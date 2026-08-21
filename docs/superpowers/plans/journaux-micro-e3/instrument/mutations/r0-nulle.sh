# ROUGE 0 : réécrit un commentaire À L'IDENTIQUE. Le fichier ne change pas.
# Le harnais DOIT refuser de jouer.
f="$1"
sed -i 's|^//! Messages du canal de contrôle (fiable, ordonné, faible débit).$|//! Messages du canal de contrôle (fiable, ordonné, faible débit).|' "$f"
