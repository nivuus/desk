#!/usr/bin/env python3
"""Tests du hook resolve du package desk.

Le hook est éprouvé par son VRAIE interface — un sous-processus nourri de
{"hw":…, "answers":…} sur stdin, qui répond en jsonl sur stdout — et non par
un import : c'est ainsi que le moteur l'appelle, et un package doit pouvoir
tourner sur une Debian qui n'a jamais vu ce moteur.

Run: python3 tests/test_desk_resolve.py
"""
import json
import os
import pathlib
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "resolve.py"

sys.path.insert(0, str(RACINE / "hooks"))
from commun import HOTE_DEFAUT, PROXY_DEFAUT  # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


def appeler(hw=None, answers=None, env=None):
    """Appelle le hook comme le moteur : stdin JSON, stdout jsonl."""
    contexte = json.dumps({"hw": hw or {}, "answers": answers or {}})
    r = subprocess.run([sys.executable, str(HOOK)], input=contexte,
                       capture_output=True, text=True, env=env)
    evenements = []
    for ligne in r.stdout.splitlines():
        ligne = ligne.strip()
        if not ligne:
            continue
        try:
            evenements.append(json.loads(ligne))
        except json.JSONDecodeError:
            pass          # le moteur relaie ces lignes en progress
    return r.returncode, evenements


REPONSES = {"admin_email": "a@b.c", "admin_password": "x",
            "auth_mode": "motdepasse", "vb_audio": False}


def refus(evenements):
    return [e for e in evenements if e.get("event") == "refuse"]


# 🔴 CE FICHIER NE POSE PLUS AUCUNE CLÉ `hw` — ET C'EST LA CORRECTION DE LA
# CRITIQUE DE LA REVUE FINALE DE BRANCHE (30 août 2026). Chaque appel
# ci-dessous posait une clé `vm_windows` vraie, et un scénario de tête
# éprouvait le refus sur la même clé fausse. Les deux étaient VERTS, et les
# deux étaient faux du monde réel : **aucun producteur de cette clé n'existe**
# (`installer/installer/common/hardware.py::detect_all()` en rend huit, aucune
# de ce nom), donc le hook refusait TOUJOURS chez le moteur — un refus que
# `steps/packages.py` traduit en `StepError`, c'est-à-dire l'arrêt de
# l'installation ENTIÈRE. La suite ne pouvait pas le voir parce qu'elle
# FABRIQUAIT elle-même le fait dont elle vérifiait la consommation.
#
# La porte a migré vers `hooks/activate.py` (la seule phase où la VM peut
# exister), et le contrat de `hw` est désormais figé par une suite dédiée :
# `tests/test_desk_contrat_hw.py`, qui lit les clés du PRODUCTEUR au lieu de
# les inventer. `hw={}` ci-dessous n'est pas une commodité : c'est ce que
# `resolve` doit savoir accepter.

# --- Le cas nominal : des faits, aucun refus ------------------------------
rc, ev = appeler(hw={}, answers=REPONSES)
check("hw sans aucune clé : aucun refus", refus(ev), [])
check("hw sans aucune clé : code de sortie 0", rc, 0)
faits = [e for e in ev if e.get("event") == "facts"]
check("hw sans aucune clé : un événement facts", len(faits), 1)
mesures = faits[0]["facts"] if faits else {}
check("les DEUX adresses TURN sont dérivées",
      all(k in mesures for k in ("turn_ecoute", "turn_relais")), True)

# 🔴 docker-compose.coturn.yml exige TURN_LISTENING_IP **et** TURN_RELAY_IP,
# toutes deux obligatoires : borner la seule écoute laisserait les allocations
# de relais sur toutes les interfaces — mesuré le 21 août 2026, 23 adresses
# distinctes dont l'adresse publique.

# --- L'adresse d'écoute de la plateforme (hote), et le proxy de confiance --
# 🔴 CORRIGE UN BUG RÉEL TROUVÉ AU LOT 10A (29 août 2026) : `hote` DOIT être
# DIFFÉRENTE de `turn_ecoute` — les deux étaient confondues avant ce
# correctif (voir commun.py::HOTE_DEFAUT), ce qui aurait fait écouter le
# service sur l'adresse PUBLIQUE dérivée pour TURN. Ce test aurait dû
# rougir avant le correctif (un contrôle qu'on n'a jamais vu rouge n'est
# pas un contrôle) : il ne le pouvait pas, faute d'assertion sur `hote` du
# tout — c'est précisément ce que cette addition ferme.
check("hote est présente et non vide", bool(mesures.get("hote")), True)
check("hote vaut le défaut fixe (jamais la route par défaut)",
      mesures.get("hote"), HOTE_DEFAUT)
check("hote n'est JAMAIS l'adresse TURN (les deux dérivations sont séparées)",
      mesures.get("hote") == mesures.get("turn_ecoute"), False)
check("proxy_confiance est présente et non vide",
      bool(mesures.get("proxy_confiance")), True)
check("proxy_confiance vaut le défaut (raisonné, voir commun.py)",
      mesures.get("proxy_confiance"), PROXY_DEFAUT)

# --- Le mode pomerium est ACCEPTÉ depuis le lot 10A (problème C) ----------
# Choisi explicitement par le propriétaire du dépôt pour la mise en service
# réelle. Le proxy de confiance est DÉRIVÉ (commun.py::lire_proxy_confiance),
# jamais demandé : aucun refus ne doit plus mordre ici.
rc, ev = appeler(hw={},
                 answers={**REPONSES, "auth_mode": "pomerium"})
check("pomerium : aucun refus (proxy dérivé, plus demandé)", refus(ev), [])
faits_pomerium = [e for e in ev if e.get("event") == "facts"]
mesures_pomerium = faits_pomerium[0]["facts"] if faits_pomerium else {}
check("pomerium : facts porte proxy_confiance",
      bool(mesures_pomerium.get("proxy_confiance")), True)

# --- Mais le refus MORD TOUJOURS si l'adresse dérivée est inutilisable ----
# 🔴 UN CONTRÔLE QU'ON N'A JAMAIS VU ROUGE N'EST PAS UN CONTRÔLE : ce
# scénario force `DESK_HOTE` vers une écoute universelle et vérifie que
# `resolve` refuse AVANT l'installation plutôt que de laisser
# `plateforme/src/config.ts::lireConfig` échouer plus tard sur un disque
# déjà partitionné.
env_hote_universelle = dict(os.environ)
env_hote_universelle["DESK_HOTE"] = "0.0.0.0"
rc, ev = appeler(hw={},
                 answers={**REPONSES, "auth_mode": "pomerium"},
                 env=env_hote_universelle)
r = refus(ev)
check("DESK_HOTE=0.0.0.0 : refus", len(r), 1)
check("DESK_HOTE=0.0.0.0 : le refus nomme l'écoute universelle",
      bool(r and "universelle" in r[0].get("reason", "").lower()), True)

# --- Une valeur de mode inconnue LÈVE, elle ne se replie pas -------------
rc, ev = appeler(hw={},
                 answers={**REPONSES, "auth_mode": "motdepass"})
check("mode inconnu : refus", len(refus(ev)), 1)

# --- vb_audio=true : REFUS avant l'installation, jamais un blocage APRÈS —
# ronde de correction 1 sur la tâche 6. Aucune charge VB-Audio n'est fournie
# par ce package (licence personnelle) : `poser_vb_audio(armee=True)` lève
# `NotImplementedError` dans `hooks/vm.py`, mais APRÈS que le compte et
# l'enrôlement de l'agent aient déjà été tentés (voir hooks/activate.py) —
# une activation qui ne passerait alors plus JAMAIS. Le refus doit arriver
# ICI, avant qu'un octet touche le disque.
rc, ev = appeler(hw={}, answers={**REPONSES, "vb_audio": True})
r = refus(ev)
check("vb_audio=true : refus", len(r), 1)
check("vb_audio=true : code de sortie 0", rc, 0)
check("vb_audio=true : la phrase nomme VB-Audio",
      bool(r and "VB-Audio" in r[0].get("reason", "")), True)
check("vb_audio=true : la phrase dit la licence personnelle",
      bool(r and "personnelle" in r[0].get("reason", "").lower()), True)
check("vb_audio=true : la phrase invite à décocher l'option",
      bool(r and "décoch" in r[0].get("reason", "").lower()), True)

# --- vb_audio=false (le défaut du wizard) : aucun refus lié à VB-Audio ----
rc, ev = appeler(hw={}, answers={**REPONSES, "vb_audio": False})
check("vb_audio=false : aucun refus", refus(ev), [])

# --- Une entrée mal formée est un refus, jamais une exception ------------
# Ronde de correction 1 (29 août 2026) : le hook laissait ces trois entrées
# lever telles quelles (JSONDecodeError ou AttributeError, code de sortie 1,
# trace complète) — l'invariant central de ce hook cassé par un chemin
# trivialement atteignable. Chaque contrôle éprouve les DEUX choses à la
# fois : le code de sortie 0 ET la présence d'un `refuse` portant une
# phrase — un contrôle qui ne vérifierait que le code de sortie passerait
# sur un hook devenu muet.
def appeler_brut(stdin_texte):
    """Comme appeler(), mais envoie stdin_texte TEL QUEL — pas du JSON
    ré-encodé — pour éprouver les entrées que json.dumps ne peut pas
    produire (JSON illisible, racine qui n'est pas un objet)."""
    r = subprocess.run([sys.executable, str(HOOK)], input=stdin_texte,
                       capture_output=True, text=True)
    evenements = []
    for ligne in r.stdout.splitlines():
        ligne = ligne.strip()
        if not ligne:
            continue
        try:
            evenements.append(json.loads(ligne))
        except json.JSONDecodeError:
            pass
    return r.returncode, evenements


for label, stdin_texte in [
    ("JSON illisible", "ceci nest pas du json"),
    ("racine qui n'est pas un objet", "[1,2,3]"),
    ("hw mal typé", json.dumps({"hw": "pas un dict", "answers": {}})),
]:
    rc, ev = appeler_brut(stdin_texte)
    r = refus(ev)
    check(f"{label} : code de sortie 0", rc, 0)
    check(f"{label} : un refus est émis", len(r), 1)
    check(f"{label} : le refus porte une phrase", bool(r and r[0].get("reason")), True)

# --- Le garde GÉNÉRIQUE : ce qu'AUCUN chemin n'a prévu -------------------
# 🔴 MINEURE #4 DE LA TÂCHE 3, RECOMMANDÉE AVANT FUSION PAR LE DOCUMENT DE
# RÉSULTATS ET CONFIRMÉE PAR LA REVUE FINALE DE BRANCHE. Les trois entrées
# ci-dessus tombent toutes dans les cas que `charger_contexte()` sait NOMMER
# — elles n'éprouvent donc PAS le `try/except Exception` de `main()`, qui est
# l'invariant central de ce hook (« un refus est une donnée, jamais une
# exception »). Le seul témoin de sa rouge était un `RecursionError` joué à
# la main par un relecteur, dans une session qui n'existe plus.
#
# Les DEUX entrées ci-dessous lèvent DANS `json.load` lui-même, chacune par
# une exception qui n'est PAS `json.JSONDecodeError` — donc hors de tout
# `except` nommé de `charger_contexte()` :
#   - des octets qui ne sont pas de l'UTF-8 : `UnicodeDecodeError`, levée
#     par le décodeur du flux AVANT que le moindre caractère JSON existe ;
#   - un entier littéral de plus de 4 300 chiffres : `ValueError` levée par
#     la conversion entière de CPython (limite `sys.set_int_max_str_digits`),
#     sur un JSON pourtant parfaitement BIEN FORMÉ.
# Aucune des deux n'est un cas que ce hook a anticipé, et c'est le point :
# un garde générique qui ne serait éprouvé que par des entrées qu'on lui a
# dictées ne prouverait rien de ce qu'il existe pour couvrir.
#
# ⚠️ CE QUE J'AI ESSAYÉ D'ABORD, ET QUI NE MORD PLUS : un JSON de profondeur
# excessive (le `RecursionError` du relecteur). Mesuré le 30 août 2026 sur
# cet interpréteur : `json.load` avale sans broncher une profondeur de
# 4 × `sys.getrecursionlimit()`, et l'entrée retombe alors dans le cas NOMMÉ
# « la racine n'est pas un objet ». Le test aurait été vert sans jamais
# atteindre le garde — exactement le patron « un contrôle qu'on n'a jamais vu
# rouge ». Il est remplacé, pas rafistolé.
def appeler_octets(donnees: bytes):
    """Comme `appeler_brut`, mais envoie des OCTETS bruts — nécessaire pour
    éprouver une entrée qui n'est pas décodable en UTF-8, ce que le mode
    texte de `subprocess` ne peut pas exprimer."""
    r = subprocess.run([sys.executable, str(HOOK)], input=donnees,
                       capture_output=True)
    evenements = []
    for ligne in r.stdout.decode("utf-8", "replace").splitlines():
        ligne = ligne.strip()
        if not ligne:
            continue
        try:
            evenements.append(json.loads(ligne))
        except json.JSONDecodeError:
            pass
    return r.returncode, evenements


for label, octets in [
    ("octets non décodables en UTF-8", b"\xff\xfe\x00{"),
    ("entier littéral de 5 000 chiffres", b'{"hw":' + b"1" * 5000 + b"}"),
]:
    rc, ev = appeler_octets(octets)
    r = refus(ev)
    check(f"garde générique ({label}) : code de sortie 0", rc, 0)
    check(f"garde générique ({label}) : un refus est émis", len(r), 1)
    check(f"garde générique ({label}) : le refus nomme l'imprévu",
          bool(r and "inattendue" in r[0].get("reason", "").lower()), True)

# Témoin négatif de CE contrôle : une entrée mal formée que le hook sait
# NOMMER ne doit PAS finir dans le garde générique — sinon l'assertion
# ci-dessus passerait pour n'importe quelle entrée et ne prouverait rien du
# chemin imprévu.
rc_temoin, ev_temoin = appeler_brut("[1,2,3]")
r_temoin = refus(ev_temoin)
check("témoin négatif : une entrée NOMMÉE ne passe pas par le garde générique",
      bool(r_temoin and "inattendue" not in r_temoin[0].get("reason", "").lower()),
      True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du hook resolve passés")
