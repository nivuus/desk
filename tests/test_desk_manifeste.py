#!/usr/bin/env python3
"""Tests du manifeste et du wizard du package desk.

Run: python3 tests/test_desk_manifeste.py
"""
import pathlib
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
INSTALLER = RACINE.parent / "installer"
sys.path.insert(0, str(INSTALLER / "installer"))

from packages.manifest import load_manifest          # noqa: E402
from packages.wizard import load_questions           # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


m = load_manifest(str(RACINE / "nivuus-package.yaml"))

check("le nom est desk", m.name, "desk")
check("le tier est userspace", m.tier, "userspace")

# 🔴 Le tier n'est pas cosmétique : `userspace` INTERDIT de déclarer
# kernel-cmdline, modules et hugepages. console a déjà pris VFIO, le GPU et le
# NVMe ; desk n'ajoute qu'un service. La garantie est vérifiée par le moteur,
# pas promise par nous.
check("aucun module noyau", m.platform.modules, ())
check("aucune ligne de commande noyau", m.platform.kernel_cmdline, ())

# La dépendance dure : desk ne doit pas pouvoir s'installer sans console.
check("console est un pré-requis", m.packages, ("console",))

questions = load_questions(str(RACINE / "wizard.yaml"))
par_cle = {q.key: q for q in questions}

check("quatre questions, pas une de plus", len(questions), 4)
check("le courriel du compte initial", par_cle["admin_email"].type, "texte")
check("le mot de passe est un secret", par_cle["admin_password"].type, "secret")
check("le mode d'authentification est un choix", par_cle["auth_mode"].type, "choix")
check("ses deux valeurs", tuple(sorted(par_cle["auth_mode"].choices)),
      ("motdepasse", "pomerium"))

# 🔴 VB-Audio a une licence PERSONNELLE seulement : sa pose est une option
# que l'opérateur arme, jamais un défaut.
check("vb_audio est un booléen", par_cle["vb_audio"].type, "bool")
check("vb_audio est DÉSARMÉ par défaut", par_cle["vb_audio"].default, False)

# Le secret de jeton n'est PAS demandé : il se tire au sort à l'installation.
# Le demander, c'est le faire choisir court.
check("aucune question ne demande le secret de jeton",
      [q.key for q in questions if "jeton" in q.key or "token" in q.key], [])

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests du manifeste et du wizard passés")
