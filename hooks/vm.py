#!/usr/bin/env python3
"""Ce que `desk` pose DANS la VM Windows, par le chemin WinRM de `console`.

La VM cible n'est plus une machine de développement : c'est une appliance
provisionnée par le package voisin `console`
(`/home/mallanic/Projects/Nivuus/packages/installer/console/`). Deux
dépendances de `desk` y sont absentes par choix de `console` — ProjFS
(désactivé) et VB-Audio (jamais posé, licence personnelle) — et la décision
du propriétaire du dépôt est que `desk` les pose LUI-MÊME, par le chemin
WinRM que `console` expose déjà, cohérente avec le principe que `console`
énonce pour ses propres dépendances : « un package porte ses dépendances ».
Légitime parce que le manifeste déclare `requires: packages: [console]`
(`nivuus-package.yaml`).

🔴 CE MODULE N'ÉCRIT RIEN SUR LA VM QUAND IL EST IMPORTÉ OU TESTÉ. Les deux
fonctions publiques (`poser_projfs`, `poser_vb_audio`) prennent un
paramètre `executer` : en test, un exécuteur FACTIQUE ; en production,
`executer_winrm_reel` par défaut, qui invoque réellement `winrm_exec.py`.

⚠️ CE PARAGRAPHE A DIT UNE CHOSE FAUSSE JUSQU'AU 30 AOÛT 2026 : « [ce
fichier] n'injecte jamais autre chose qu'un exécuteur factice, sauf pour SA
PROPRE ROUGE, qui n'atteint de toute façon jamais le réseau ». C'est faux
depuis la ronde 1 de la tâche 6 — `tests/test_desk_vm.py` porte DEUX
scénarios (A et B) qui passent délibérément par `executer_winrm_reel`, donc
par un vrai `subprocess.run`, pour éprouver son bras d'échec et son
round-trip. Ce qu'ils exécutent est un FAUX `winrm_exec.py` de quatre lignes
posé sous un `NIVUUS_PACKAGES_DIR` temporaire : le sous-processus est réel,
la VM et le réseau ne le sont jamais. Le correctif était bon ; ce docstring
ne l'avait pas suivi (relevé par la revue finale de branche).

--- Par où on apprend le chemin de `winrm_exec.py` (jamais supposé) -------

Établi en LISANT deux fichiers du dépôt voisin `installer/` (aucun n'a été
deviné) :

  - `installer/installer/packages/discovery.py:22` —
    `PACKAGES_DIR = os.environ.get("NIVUUS_PACKAGES_DIR", "/opt/nivuus-packages")`
    — c'est la variable par laquelle le MOTEUR lui-même découvre les
    manifestes de packages installés ; `apply_packages()` copie chaque
    package sélectionné, `console` compris, sous
    `<PACKAGES_DIR>/<nom_du_package>/`, une fois pour toutes, à
    l'installation, et ne le retire jamais (voir
    `installer/installer/README.md:57,156` et
    `installer/docs/superpowers/specs/2026-08-27-decoupage-installer-console-design.md:268`).
  - `installer/console/host/guest-ready-watch.py:113,155-162` — un script
    du MÊME dépôt voisin, confronté au MÊME besoin (appeler un outil de
    `console` depuis un autre point du système, une fois `console`
    installé), résout DÉJÀ
    `WINRM_EXEC = "/opt/nivuus-packages/console/guest/winrm_exec.py"` par
    ce chemin — et son propre commentaire dit pourquoi : « `apply_packages()`
    copies the whole package tree there once, at install time, and never
    removes it — so [ce fichier] is reliably at this path on any machine
    where this script itself is running ».

Ce module REPREND cette convention plutôt que d'en inventer une nouvelle :
`NIVUUS_PACKAGES_DIR` (défaut `/opt/nivuus-packages`), sous-chemin
`console/guest/winrm_exec.py`. Sur la machine qui fait tourner ces tests,
`/opt/nivuus-packages` n'existe pas (confirmé : `ls /opt/nivuus-packages`
rend « Aucun fichier ou dossier de ce nom ») — le vrai fichier de
développement vit sous
`/home/mallanic/Projects/Nivuus/packages/installer/console/guest/winrm_exec.py`,
un arbre entièrement différent. C'est pourquoi `NIVUUS_PACKAGES_DIR` doit
être surchargeable : les tests le font, jamais le code de production, qui
n'a besoin d'aucun défaut différent puisqu'à l'installation réelle le
défaut `/opt/nivuus-packages` sera correct.

--- Le contrat exact de `winrm_exec.py`, lu intégralement -----------------

`installer/console/guest/winrm_exec.py` : `Usage: winrm_exec.py {cmd|ps}
<command...>`, transport `pywinrm` en NTLM (Basic est refusé par le
guest), mot de passe lu depuis `GUEST_PASS_FILE` (jamais l'argv). Le code
de sortie du processus est celui de la commande distante
(`result.status_code`), la sortie standard est imprimée telle quelle.
"""
import os
import pathlib
import subprocess
import sys
from dataclasses import dataclass

# --- Where winrm_exec.py lives, resolved never assumed ----------------------

NIVUUS_PACKAGES_DIR_DEFAUT = "/opt/nivuus-packages"
WINRM_EXEC_RELATIF = pathlib.Path("console") / "guest" / "winrm_exec.py"


def chemin_winrm_exec() -> pathlib.Path:
    """Resolves the path of `winrm_exec.py`, without ever guessing it.

    Reads `NIVUUS_PACKAGES_DIR` (default `/opt/nivuus-packages` — see the
    head docstring for where this convention comes from), targets
    `<packages_dir>/console/guest/winrm_exec.py`, and RAISES
    `FileNotFoundError`, NAMING the searched path, if this file does not
    exist: a package that fails silently on an inter-package contract
    cannot be told apart from a package that has nothing to do.
    """
    packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR", NIVUUS_PACKAGES_DIR_DEFAUT)
    chemin = pathlib.Path(packages_dir) / WINRM_EXEC_RELATIF
    if not chemin.is_file():
        raise FileNotFoundError(
            f"winrm_exec.py introuvable a l'emplacement attendu : {chemin} "
            "(le package console est-il installe ? NIVUUS_PACKAGES_DIR="
            f"{packages_dir!r} - voir hooks/vm.py pour la convention)"
        )
    return chemin


def executer_winrm_reel(mode: str, commande: str) -> str:
    """The REAL executor — never used by `tests/test_desk_vm.py`, except
    for its own RED, which misses before any network access (see
    above).

    Resolves `winrm_exec.py` (which raises BEFORE any network call if
    `console` is not installed), then invokes
    `winrm_exec.py <mode> <command>` as a subprocess. Returns the standard
    output, stripped of its edge whitespace; raises `RuntimeError` on
    a transport failure or a remote command in error (the password
    file missing, the VM unreachable, `RestartNeeded` not
    readable...).
    """
    chemin = chemin_winrm_exec()
    proc = subprocess.run(
        [sys.executable, str(chemin), mode, commande],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(
            f"winrm_exec.py {mode} a echoue (code {proc.returncode}) : "
            f"{detail or 'aucune sortie'}"
        )
    return proc.stdout.strip()


# --- ProjFS: the reboot is NOTED and SAID, it is not taken ----------------

@dataclass(frozen=True)
class EtatProjFS:
    """What `poser_projfs` reports to the caller (`activate.py`)."""
    redemarrage_requis: bool


# The ProjFS feature REQUIRES a reboot to become active, but
# `-NoRestart` prevents `Enable-WindowsOptionalFeature` from TAKING it
# itself: rebooting an operator's VM without asking is a side
# effect no installation may take (see the head
# docstring). The line `Write-Output 'RestartNeeded'` is emitted ONLY if the
# `RestartNeeded` property of the returned object is true — it is this token,
# and it alone, that `poser_projfs` looks for in the output: an empty output
# means "no reboot required", never an ambiguous answer to
# interpret.
COMMANDE_PROJFS = (
    "$r = Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS "
    "-NoRestart; if ($r.RestartNeeded) { Write-Output 'RestartNeeded' }"
)


def poser_projfs(executer=None) -> EtatProjFS:
    """Enables the Client-ProjFS optional feature in the VM, WITHOUT
    rebooting (`-NoRestart`), and REPORTS whether a reboot is required —
    it never triggers it (see the head docstring).

    `executer(mode, commande) -> str`: in tests, a fake executor
    (`faux_winrm_rendant`, see `tests/test_desk_vm.py`); by default,
    `executer_winrm_reel`, which really invokes `winrm_exec.py` — hence
    never exercised by this module as long as a fake `executer` is supplied.
    """
    executer = executer or executer_winrm_reel
    sortie = executer("ps", COMMANDE_PROJFS)
    return EtatProjFS(redemarrage_requis="RestartNeeded" in sortie)


# --- VB-Audio: PERSONAL licence only -----------------------------------

def poser_vb_audio(armee: bool = False, executer=None) -> None:
    """Installs VB-Audio in the VM IF `armee` (the wizard default, `vb_audio`
    in `wizard.yaml`, is false).

    🔴 DISARMED (the nominal case), NOTHING GOES TO THE VM — not only
    "nothing gets installed": `executer` is NEVER invoked, and
    `tests/test_desk_vm.py` proves that the list of commands seen by
    the fake executor stays empty, not that an installation was skipped.

    ⚠️ ARMED, THIS BATCH LAYS DOWN NO VB-AUDIO PAYLOAD — this is not an
    oversight: no task of this batch (see the task index,
    `.superpowers/sdd/2026-08-29-package-nivuus/task-*-brief.md`)
    drops a VB-Audio driver into the tree that `console` builds
    (`console/guest/fetch_payload.py`: `agent/`, `virtio/`, `steam/`,
    `sudovda/`, `winfsp/`, `apollo/`, `nvidia/` — NO
    `vb-audio` entry), and no silent installer path is documented
    anywhere in this repository nor in `installer/`. Making up an
    install command here would be guessing a contract that does not exist — the
    explicit instruction of this task is to never guess such a
    contract (see the path of `winrm_exec.py` above, handled the
    same way). Raising HERE, loud and named, is the choice consistent with the
    rest of this repository: a mechanism that would fail silently on an
    explicit operator request (`vb_audio: true`) could not be told apart
    from a mechanism that has nothing to do. It is an open point, to be settled
    by the repository owner when a VB-Audio payload exists — see
    the report of this task, § Reservations.
    """
    if not armee:
        return
    raise NotImplementedError(
        "vb_audio est arme (vb_audio=true) mais aucun payload VB-Audio "
        "n'existe encore dans l'arborescence que console construit "
        "(fetch_payload.py) : aucune tache de ce lot ne le depose, et "
        "inventer un chemin d'installateur serait deviner un contrat qui "
        "n'existe pas. Voir hooks/vm.py::poser_vb_audio pour le detail."
    )
