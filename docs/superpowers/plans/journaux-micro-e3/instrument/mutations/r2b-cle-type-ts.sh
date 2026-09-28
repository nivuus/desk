# R2b: THE SAME mutation, on the TYPESCRIPT side -- the `type` key of `MicStateMessage`
# goes from 'mic-state' to 'micstate', in the interface AND in `ALL_AGENT`.
python3 - "$1" <<'PY'
import sys
p=sys.argv[1]
s=open(p,encoding='utf-8').read()
n=s.count("    type: 'mic-state';")
assert n==1, f"interface : attendu 1, trouve {n}"
s=s.replace("    type: 'mic-state';", "    type: 'micstate';",1)
n=s.count("    'mic-state': true,")
assert n==1, f"TOUS_AGENT : attendu 1, trouve {n}"
s=s.replace("    'mic-state': true,", "    micstate: true,",1)
open(p,'w',encoding='utf-8').write(s)
PY
