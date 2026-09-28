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
    """Parses `chemin` as KEY=VALUE, one per line. `{}` if the file does not
    exist — duplicated from the task 4 test parser rather than imported,
    same doctrine as the rest of this package: each hook stays
    runnable on its own.
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
    """Appends `KEY=VALUE` lines at the end of an environment file
    already in place, WITHOUT touching the lines already there, and closes the
    file back in mode 600 — the same mode `install.py::ecrire_env` gave
    it, and which it must keep: this file holds secrets.

    🔴 PRECONDITION, REQUIRED FROM THE CALLER: `chemin` MUST ALREADY EXIST
    (`hooks/activate.py::main()` refuses if `desk.env` is missing — see its
    comment). `write_text()` on an EXISTING file reuses the mode
    already in place, it does not recreate it with the process umask; it is
    ONLY on a NEW file that this pattern would open the
    write-then-`chmod` window that `install.py::ecrire_env` removed (commit
    `92cacfb`) by switching to `os.open(..., 0o600)`. This function never
    recreates this file from nothing — it is the guard in `main()`, not
    this function, that makes this precondition true.
    """
    corps = chemin.read_text(encoding="utf-8") if chemin.is_file() else ""
    if corps and not corps.endswith("\n"):
        corps += "\n"
    corps += "".join(f"{cle}={valeur}\n" for cle, valeur in nouvelles.items())
    chemin.write_text(corps, encoding="utf-8")
    os.chmod(chemin, stat.S_IRUSR | stat.S_IWUSR)  # 0o600, redundant if the
    # precondition already holds — but free, and a second line of
    # defence costs nothing.
