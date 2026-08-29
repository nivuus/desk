#!/usr/bin/env python3
"""Lecture et enrichissement de `etc/nivuus/desk.env`, le fichier
d'environnement que `hooks/install.py` pose et que `hooks/activate.py`
complète (`AGENT_VM`, `AGENT_SECRET`) une fois l'enrôlement réussi.

Extrait de `hooks/activate.py` par la tâche 7 (2026-08-29), pour la MÊME
raison que `hooks/administration.py` (voir son propre docstring de tête) :
faire de la place, dans un commit SÉPARÉ et SANS changement de
comportement, avant que la tâche n'ajoute le dépôt de `agent.exe` pour
`console`. Importé comme `vm.py` et `administration.py` — Python ajoute le
répertoire du script LANCÉ à `sys.path`, aucune manipulation nécessaire.
"""
import os
import pathlib
import stat


def lire_env_fichier(chemin: pathlib.Path) -> dict:
    """Parse `chemin` en KEY=VALUE, un par ligne. `{}` si le fichier n'existe
    pas — dupliqué du parseur du test de la tâche 4 plutôt qu'importé,
    même doctrine que le reste de ce package : chaque hook reste
    exécutable seul.
    """
    valeurs = {}
    if not chemin.is_file():
        return valeurs
    for ligne in chemin.read_text(encoding="utf-8").splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.startswith("#") or "=" not in ligne:
            continue
        cle, _, valeur = ligne.partition("=")
        valeurs[cle] = valeur
    return valeurs


def ajouter_variables_env(chemin: pathlib.Path, nouvelles: dict) -> None:
    """Ajoute des lignes `KEY=VALUE` à la fin d'un fichier d'environnement
    déjà posé, SANS toucher aux lignes qui y sont déjà, et referme le
    fichier en mode 600 — le même mode que `install.py::ecrire_env` lui a
    donné, et qu'il doit garder : ce fichier porte des secrets.

    🔴 PRÉCONDITION, EXIGÉE PAR L'APPELANT : `chemin` DOIT DÉJÀ EXISTER
    (`hooks/activate.py::main()` refuse si `desk.env` est absent — voir son
    commentaire). `write_text()` sur un fichier EXISTANT réutilise le mode
    déjà en place, il ne le recrée pas au umask du processus ; c'est
    SEULEMENT sur un fichier NEUF que ce patron ouvrirait la fenêtre
    écriture-puis-`chmod` qu'`install.py::ecrire_env` a éliminée (commit
    `92cacfb`) en passant à `os.open(..., 0o600)`. Cette fonction ne
    recrée jamais ce fichier depuis rien — c'est la garde de `main()`, pas
    elle, qui rend cette précondition vraie.
    """
    corps = chemin.read_text(encoding="utf-8") if chemin.is_file() else ""
    if corps and not corps.endswith("\n"):
        corps += "\n"
    corps += "".join(f"{cle}={valeur}\n" for cle, valeur in nouvelles.items())
    chemin.write_text(corps, encoding="utf-8")
    os.chmod(chemin, stat.S_IRUSR | stat.S_IWUSR)  # 0o600, redondant si la
    # précondition tient déjà — mais gratuit, et une seconde ligne de
    # défense ne coûte rien.
