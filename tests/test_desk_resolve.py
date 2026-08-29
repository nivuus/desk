#!/usr/bin/env python3
"""Tests du hook resolve du package desk.

Le hook est éprouvé par son VRAIE interface — un sous-processus nourri de
{"hw":…, "answers":…} sur stdin, qui répond en jsonl sur stdout — et non par
un import : c'est ainsi que le moteur l'appelle, et un package doit pouvoir
tourner sur une Debian qui n'a jamais vu ce moteur.

Run: python3 tests/test_desk_resolve.py
"""
import json
import pathlib
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "resolve.py"

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

# --- Le mode pomerium fait mordre ses gardes AVANT l'installation --------
rc, ev = appeler(hw={"vm_windows": True},
                 answers={**REPONSES, "auth_mode": "pomerium"})
r = refus(ev)
check("pomerium sans proxy déclaré : refus", len(r), 1)
check("pomerium : le refus nomme le proxy de confiance",
      bool(r and "proxy" in r[0]["reason"].lower()), True)

# --- Une valeur de mode inconnue LÈVE, elle ne se replie pas -------------
rc, ev = appeler(hw={"vm_windows": True},
                 answers={**REPONSES, "auth_mode": "motdepass"})
check("mode inconnu : refus", len(refus(ev)), 1)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du hook resolve passés")
