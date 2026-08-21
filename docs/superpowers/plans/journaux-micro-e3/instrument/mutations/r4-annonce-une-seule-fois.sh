# R4 : l'annonce ne suit QUE la premiere transition -- un refus leve n'est
# jamais reannonce. C'est litteralement ce que le commentaire faux de E1
# ("condition PERMANENTE") laissait croire suffisant.
python3 - "$1" <<'PY'
import sys
p=sys.argv[1]
s=open(p,encoding='utf-8').read()
anc = "        if self.exclusivite_annoncee != Some(accepte) {"
n=s.count(anc); assert n==1, f"attendu 1, trouve {n}"
s=s.replace(anc, "        if self.exclusivite_annoncee.is_none() {")
open(p,'w',encoding='utf-8').write(s)
PY
