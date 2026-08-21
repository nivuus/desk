# R1 : retire `verifie_version` de la SEULE variante `MicState`.
# Ancré sur la syntaxe de la variante, jamais par sous-chaîne globale : la ligne
# `#[serde(rename = "v", deserialize_with = ...)]` existe DIX FOIS dans ce fichier.
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
