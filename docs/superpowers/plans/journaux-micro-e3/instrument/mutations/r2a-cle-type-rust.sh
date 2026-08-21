# R2a : la cle `type` de MicState passe de `mic-state` a `micstate`, cote RUST.
#
# ⚠️ Le geste choisi est un `#[serde(rename)]` sur la variante, PAS un renommage
# de la variante elle-meme : renommer l'identifiant Rust casserait `redaction.rs`
# et `transport/controle.rs`, et la rouge rougirait alors sur une ERREUR DE
# COMPILATION -- c'est-a-dire pour la mauvaise raison. Ce qu'on veut faire
# tomber est l'assertion de FORME DE FIL, et elle seule.
python3 - "$1" <<'PY'
import sys
p=sys.argv[1]
s=open(p,encoding='utf-8').read()
anc = """    MicState {
        #[serde(rename = "v", deserialize_with = "verifie_version")]"""
n=s.count(anc)
assert n==1, f"attendu 1, trouve {n}"
s=s.replace(anc, """    #[serde(rename = "micstate")]
    MicState {
        #[serde(rename = "v", deserialize_with = "verifie_version")]""")
open(p,'w',encoding='utf-8').write(s)
PY
