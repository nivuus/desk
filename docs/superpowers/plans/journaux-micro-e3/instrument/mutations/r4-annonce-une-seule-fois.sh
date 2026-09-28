# R4: the announcement only follows the FIRST transition -- a lifted refusal is
# never announced again. It is literally what E1's wrong comment
# ("PERMANENT condition") suggested was enough.
python3 - "$1" <<'PY'
import sys
p=sys.argv[1]
s=open(p,encoding='utf-8').read()
anc = "        if self.exclusivite_annoncee != Some(accepte) {"
n=s.count(anc); assert n==1, f"attendu 1, trouve {n}"
s=s.replace(anc, "        if self.exclusivite_annoncee.is_none() {")
open(p,'w',encoding='utf-8').write(s)
PY
