# R3 : l'emission passe de la TRANSITION a CHAQUE DEPOT.
python3 - "$1" <<'PY'
import sys
p=sys.argv[1]
s=open(p,encoding='utf-8').read()
anc = """        if self.exclusivite_annoncee != Some(accepte) {
            self.exclusivite_annoncee = Some(accepte);
            self.queue_control(proto::control::AgentControl::mic_state(accepte));
        }"""
n=s.count(anc); assert n==1, f"attendu 1, trouve {n}"
s=s.replace(anc, """        {
            self.exclusivite_annoncee = Some(accepte);
            self.queue_control(proto::control::AgentControl::mic_state(accepte));
        }""")
open(p,'w',encoding='utf-8').write(s)
PY
