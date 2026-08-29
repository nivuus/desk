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


# --- Le refus est une DONNÉE, jamais une exception -----------------------
# Sans VM, le hook doit REFUSER avec une phrase, et sortir 0 : un code non nul
# donne à l'opérateur une trace sur laquelle il ne peut rien agir.
rc, ev = appeler(hw={"vm_windows": False}, answers=REPONSES)
r = refus(ev)
check("sans VM : un refus est émis", len(r), 1)
check("sans VM : le refus porte une phrase", bool(r and r[0].get("reason")), True)
check("sans VM : la phrase nomme la VM",
      bool(r and "vm" in r[0]["reason"].lower()), True)
check("sans VM : code de sortie 0", rc, 0)

# --- Le cas nominal : des faits, aucun refus ------------------------------
rc, ev = appeler(hw={"vm_windows": True}, answers=REPONSES)
check("avec VM : aucun refus", refus(ev), [])
check("avec VM : code de sortie 0", rc, 0)
faits = [e for e in ev if e.get("event") == "facts"]
check("avec VM : un événement facts", len(faits), 1)
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
rc, ev = appeler(hw={"vm_windows": True},
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
rc, ev = appeler(hw={"vm_windows": True},
                 answers={**REPONSES, "auth_mode": "pomerium"},
                 env=env_hote_universelle)
r = refus(ev)
check("DESK_HOTE=0.0.0.0 : refus", len(r), 1)
check("DESK_HOTE=0.0.0.0 : le refus nomme l'écoute universelle",
      bool(r and "universelle" in r[0].get("reason", "").lower()), True)

# --- Une valeur de mode inconnue LÈVE, elle ne se replie pas -------------
rc, ev = appeler(hw={"vm_windows": True},
                 answers={**REPONSES, "auth_mode": "motdepass"})
check("mode inconnu : refus", len(refus(ev)), 1)

# --- vb_audio=true : REFUS avant l'installation, jamais un blocage APRÈS —
# ronde de correction 1 sur la tâche 6. Aucune charge VB-Audio n'est fournie
# par ce package (licence personnelle) : `poser_vb_audio(armee=True)` lève
# `NotImplementedError` dans `hooks/vm.py`, mais APRÈS que le compte et
# l'enrôlement de l'agent aient déjà été tentés (voir hooks/activate.py) —
# une activation qui ne passerait alors plus JAMAIS. Le refus doit arriver
# ICI, avant qu'un octet touche le disque.
rc, ev = appeler(hw={"vm_windows": True}, answers={**REPONSES, "vb_audio": True})
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
rc, ev = appeler(hw={"vm_windows": True}, answers={**REPONSES, "vb_audio": False})
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

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du hook resolve passés")
