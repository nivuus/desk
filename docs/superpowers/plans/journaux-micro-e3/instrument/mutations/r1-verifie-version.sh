# R1: removes `check_version` from the ONLY `MicState` variant.
# Anchored on the variant's syntax, never by a global substring: the line
# `#[serde(rename = "v", deserialize_with = ...)]` exists TEN TIMES in this file.
python3 - "$1" <<'PY'
import sys
p=sys.argv[1]
s=open(p,encoding='utf-8').read()
bloc = """    MicState {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,"""
n=s.count(bloc)
assert n==1, f"attendu 1 occurrence du bloc MicState, trouve {n}"
s=s.replace(bloc, """    MicState {
        #[serde(rename = "v")]
        version: u8,""")
open(p,'w',encoding='utf-8').write(s)
PY
