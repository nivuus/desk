#!/usr/bin/env python3
"""Tests de `hooks/administration.py::lancer_npm` — le problème A du lot
10A (29 août 2026) touché ici : `npm` invoqué par son seul nom dépend du
PATH du processus appelant, qui ne contient `node`/`npm` que par accident
sur un poste de développement où nvm est sourcé dans le shell interactif.

Ce fichier éprouve DIRECTEMENT `lancer_npm` (import, pas sous-processus du
hook complet — `tests/test_desk_activate.py` couvre déjà le hook entier au
travers d'un scénario complet) : c'est la fonction la plus étroite qui
porte cette responsabilité, et le module n'est PAS un hook exécutable seul
(voir son propre docstring de tête).

Run: python3 tests/test_desk_administration.py
"""
import importlib.util
import os
import pathlib
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOKS = RACINE / "hooks"

sys.path.insert(0, str(HOOKS))
from commun import NODE_BIN_DEFAUT  # noqa: E402

spec = importlib.util.spec_from_file_location("administration", HOOKS / "administration.py")
administration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(administration)

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# --- lancer_npm APPONDE NODE_BIN_DEFAUT en fin de PATH, sans l'imposer ------
with tempfile.TemporaryDirectory() as tmp:
    cwd = pathlib.Path(tmp)
    env_sans_node = {"PATH": "/un/chemin/qui/ne/contient/pas/node"}
    code, out, err = administration.lancer_npm(cwd, "admin:utilisateur", [],
                                                env_sans_node, entree="x\n")
    # npm est introuvable (aucun vrai npm sur ce PATH factice, et
    # NODE_BIN_DEFAUT n'existe pas forcement sur la machine de test) : ce
    # test ne juge PAS le code de retour de npm, seulement que lancer_npm
    # ne LÈVE jamais (le contrat documenté : "Rend toujours (code, stdout,
    # stderr), jamais ne lève").
    check("lancer_npm ne leve jamais meme sans node dans le PATH fourni",
          isinstance(code, int), True)

# --- Un `npm` factice DÉJÀ EN TÊTE DU PATH continue de gagner --------------
# 🔴 C'EST LE CONTRÔLE QUI PROUVE QUE L'APPONDAGE NE COURT-CIRCUITE RIEN :
# `tests/desk_activate_fixtures.py::appeler` pose son PROPRE faux npm en
# TÊTE de PATH, et si `lancer_npm` le préfixait au lieu de l'apponder, ce
# faux npm ne serait plus jamais trouvé.
with tempfile.TemporaryDirectory() as tmp:
    cwd = pathlib.Path(tmp)
    bin_dir = pathlib.Path(tmp) / "faux-bin"
    bin_dir.mkdir()
    faux_npm = bin_dir / "npm"
    marqueur = pathlib.Path(tmp) / "vu.txt"
    faux_npm.write_text(
        "#!/usr/bin/env python3\n"
        "import pathlib, sys\n"
        f"pathlib.Path({str(marqueur)!r}).write_text('vu')\n"
        "sys.exit(0)\n",
        encoding="utf-8",
    )
    faux_npm.chmod(0o755)
    # `/usr/bin` reste nécessaire : le faux npm est un script
    # `#!/usr/bin/env python3`, et c'est `/usr/bin/env` qui doit être
    # trouvé pour que le noyau puisse même lancer python3.
    env = {"PATH": str(bin_dir) + os.pathsep + "/usr/bin:/bin"}
    code, out, err = administration.lancer_npm(cwd, "admin:utilisateur", [], env)
    check("le faux npm en tete de PATH est bien invoque (code 0)", code, 0)
    check("le faux npm en tete de PATH a bien tourne (marqueur pose)",
          marqueur.is_file(), True)

# --- NODE_BIN_DEFAUT est bien apponde, jamais absent du PATH final ---------
with tempfile.TemporaryDirectory() as tmp:
    cwd = pathlib.Path(tmp)
    marqueur_path = pathlib.Path(tmp) / "path_vu.txt"
    bin_dir = pathlib.Path(tmp) / "faux-bin"
    bin_dir.mkdir()
    faux_npm = bin_dir / "npm"
    faux_npm.write_text(
        "#!/usr/bin/env python3\n"
        "import os, pathlib\n"
        f"pathlib.Path({str(marqueur_path)!r}).write_text(os.environ.get('PATH', ''))\n",
        encoding="utf-8",
    )
    faux_npm.chmod(0o755)
    env = {"PATH": str(bin_dir) + os.pathsep + "/usr/bin:/bin"}
    administration.lancer_npm(cwd, "admin:utilisateur", [], env)
    path_vu = marqueur_path.read_text(encoding="utf-8") if marqueur_path.is_file() else ""
    check("NODE_BIN_DEFAUT est present dans le PATH transmis au sous-processus",
          NODE_BIN_DEFAUT in path_vu.split(os.pathsep), True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - tests de hooks/administration.py passés")
