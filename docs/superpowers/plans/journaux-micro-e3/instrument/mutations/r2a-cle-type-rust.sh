# R2a: MicState's `type` key goes from `mic-state` to `micstate`, on the RUST side.
#
# ⚠️ The chosen gesture is a `#[serde(rename)]` on the variant, NOT a renaming
# of the variant itself: renaming the Rust identifier would break `redaction.rs`
# and `transport/controle.rs`, and the red run would then turn red on a
# COMPILATION ERROR -- that is, for the wrong reason. What we want to bring
# down is the WIRE SHAPE assertion, and it alone.
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
