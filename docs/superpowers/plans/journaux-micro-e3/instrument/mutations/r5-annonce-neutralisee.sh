# R5 : `annoncerExclusivite` neutralisee -- le corps devient inerte.
python3 - "$1" <<'PY'
import sys
p=sys.argv[1]
s=open(p,encoding='utf-8').read()
anc = "            if (micro.etat() !== 'actif') return;"
n=s.count(anc); assert n==1, f"attendu 1, trouve {n}"
s=s.replace(anc, "            if (micro.etat() !== 'actif') return;\n            if (granted || !granted) return; // MUTATION R5 : inerte")
open(p,'w',encoding='utf-8').write(s)
PY
