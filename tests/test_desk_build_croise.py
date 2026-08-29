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
import shutil
import subprocess
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = RACINE / "scripts" / "build-agent-croise.sh"

# Chemin ABSOLU de bash : le troisième bloc ci-dessous construit un PATH
# bac à sable pour le processus enfant, et un "bash" sans slash y serait
# lui-même introuvable — c'est le PATH de l'ENFANT que subprocess consulte
# pour résoudre argv[0], pas celui de ce script.
BASH = shutil.which("bash") or "/bin/bash"

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

# Un éditeur de liens mingw absent doit être nommé, pas laissé à un échec de
# lien cargo illisible. ⚠️ Sur cet hôte, /bin est un lien symbolique vers
# /usr/bin : un PATH qui retire seulement /usr/bin laisse
# x86_64-w64-mingw32-gcc joignable via /bin, et ce bloc passerait pour la
# MAUVAISE raison — incapable de rougir. Le PATH ci-dessous est un bac à
# sable construit outil par outil (jamais un répertoire entier retiré) :
# seuls les exécutables dont le script a besoin (rustup, cargo — son propre
# lien vers rustup —, grep, mkdir, cp, stat, dirname) y sont joignables, et
# x86_64-w64-mingw32-gcc ne l'est nulle part.
with tempfile.TemporaryDirectory() as tmp, \
     tempfile.TemporaryDirectory() as bac_a_sable:
    outils_necessaires = ("grep", "mkdir", "cp", "stat", "dirname")
    for outil in outils_necessaires:
        chemin_reel = shutil.which(outil)
        assert chemin_reel, f"outil requis introuvable sur cet hôte : {outil}"
        os.symlink(chemin_reel, os.path.join(bac_a_sable, outil))

    rustup_reel = shutil.which("rustup")
    assert rustup_reel, "rustup introuvable sur cet hôte"
    repertoire_rustup = os.path.dirname(rustup_reel)

    env = dict(os.environ, PATH=f"{bac_a_sable}:{repertoire_rustup}")
    r = subprocess.run([BASH, str(SCRIPT), tmp],
                       capture_output=True, text=True, env=env)
    check("éditeur de liens absent : code non nul", r.returncode != 0, True)
    check("éditeur de liens absent : il est nommé",
          "x86_64-w64-mingw32-gcc" in (r.stdout + r.stderr), True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests de la compilation croisée passés")
