#!/usr/bin/env python3
"""Tests de la compilation croisée de l'agent.

Ce que ces tests éprouvent : que le script REFUSE proprement quand son
outillage manque, et qu'il nomme ce qui manque. Ils n'éprouvent PAS que le
binaire fonctionne — voir la tâche 9 et la spec §4.3 : la compilation prouve
que l'agent se lie, jamais qu'il tourne.

Run: python3 tests/test_desk_build_croise.py
"""
import os
import pathlib
import subprocess
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = RACINE / "scripts" / "build-agent-croise.sh"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


check("le script existe", SCRIPT.is_file(), True)
check("le script est exécutable", os.access(SCRIPT, os.X_OK), True)

# Sans destination, il doit refuser et le DIRE — jamais écrire quelque part
# par défaut : un défaut ferait déposer un binaire de 20 Mio à un endroit que
# personne n'a demandé.
r = subprocess.run(["bash", str(SCRIPT)], capture_output=True, text=True)
check("sans destination : code non nul", r.returncode != 0, True)
check("sans destination : la raison est dite",
      "destination" in (r.stdout + r.stderr).lower(), True)

# Une cible rustup absente doit être nommée, pas laissée à cargo qui rendrait
# une erreur de compilation illisible.
with tempfile.TemporaryDirectory() as tmp:
    env = dict(os.environ, CIBLE_RUST="cible-qui-n-existe-pas")
    r = subprocess.run(["bash", str(SCRIPT), tmp],
                       capture_output=True, text=True, env=env)
    check("cible inconnue : code non nul", r.returncode != 0, True)
    check("cible inconnue : elle est nommée",
          "cible-qui-n-existe-pas" in (r.stdout + r.stderr), True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests de la compilation croisée passés")
