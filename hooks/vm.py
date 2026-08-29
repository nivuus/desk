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

# --- Où vit winrm_exec.py, résolu jamais supposé --------------------------

NIVUUS_PACKAGES_DIR_DEFAUT = "/opt/nivuus-packages"
WINRM_EXEC_RELATIF = pathlib.Path("console") / "guest" / "winrm_exec.py"


def chemin_winrm_exec() -> pathlib.Path:
    """Résout le chemin de `winrm_exec.py`, sans jamais le deviner.

    Lit `NIVUUS_PACKAGES_DIR` (défaut `/opt/nivuus-packages` — voir le
    docstring de tête pour d'où vient cette convention), vise
    `<packages_dir>/console/guest/winrm_exec.py`, et LÈVE
    `FileNotFoundError`, NOMMANT le chemin cherché, si ce fichier n'existe
    pas : un package qui échoue en silence sur un contrat inter-packages
    est indiscernable d'un package qui n'a rien à faire.
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
    """L'exécuteur RÉEL — jamais employé par `tests/test_desk_vm.py`, sauf
    pour sa propre ROUGE, qui rate avant tout accès réseau (voir plus
    haut).

    Résout `winrm_exec.py` (ce qui lève AVANT tout appel réseau si
    `console` n'est pas installé), puis invoque
    `winrm_exec.py <mode> <commande>` en sous-processus. Rend la sortie
    standard, dépouillée de ses espaces de bord ; lève `RuntimeError` sur
    un échec de transport ou une commande distante en erreur (le fichier
    de mot de passe absent, la VM injoignable, `RestartNeeded` non
    lisible...).
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


# --- ProjFS : le redémarrage se CONSTATE et se DIT, il ne se prend pas ----

@dataclass(frozen=True)
class EtatProjFS:
    """Ce que `poser_projfs` rapporte à l'appelant (`activate.py`)."""
    redemarrage_requis: bool


# La feature ProjFS EXIGE un redémarrage pour devenir active, mais
# `-NoRestart` empêche `Enable-WindowsOptionalFeature` de le PRENDRE
# lui-même : redémarrer la VM d'un opérateur sans le lui demander est un
# effet de bord qu'aucune installation ne doit prendre (voir le docstring
# de tête). La ligne `Write-Output 'RestartNeeded'` n'est émise QUE si la
# propriété `RestartNeeded` de l'objet rendu est vraie — c'est ce jeton,
# et lui seul, que `poser_projfs` cherche dans la sortie : une sortie vide
# veut dire « aucun redémarrage requis », jamais une réponse ambiguë à
# interpréter.
COMMANDE_PROJFS = (
    "$r = Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS "
    "-NoRestart; if ($r.RestartNeeded) { Write-Output 'RestartNeeded' }"
)


def poser_projfs(executer=None) -> EtatProjFS:
    """Active la fonctionnalité optionnelle Client-ProjFS dans la VM, SANS
    redémarrer (`-NoRestart`), et RAPPORTE si un redémarrage est requis —
    elle ne le déclenche jamais (voir le docstring de tête).

    `executer(mode, commande) -> str` : en test, un exécuteur factice
    (`faux_winrm_rendant`, voir `tests/test_desk_vm.py`) ; par défaut,
    `executer_winrm_reel`, qui invoque réellement `winrm_exec.py` — donc
    jamais exercé par ce module tant qu'un `executer` factice est fourni.
    """
    executer = executer or executer_winrm_reel
    sortie = executer("ps", COMMANDE_PROJFS)
    return EtatProjFS(redemarrage_requis="RestartNeeded" in sortie)


# --- VB-Audio : licence PERSONNELLE seulement -----------------------------

def poser_vb_audio(armee: bool = False, executer=None) -> None:
    """Pose VB-Audio dans la VM SI `armee` (le défaut du wizard, `vb_audio`
    dans `wizard.yaml`, est faux).

    🔴 DÉSARMÉ (le cas nominal), RIEN NE PART VERS LA VM — pas seulement
    « rien ne s'installe » : `executer` n'est JAMAIS invoqué, et
    `tests/test_desk_vm.py` éprouve que la liste des commandes vues par
    l'exécuteur factice reste vide, pas qu'une installation a été sautée.

    ⚠️ ARMÉ, CE LOT NE POSE AUCUN PAYLOAD VB-AUDIO — ce n'est pas un
    oubli : aucune tâche de ce lot (voir l'index des tâches,
    `.superpowers/sdd/2026-08-29-package-nivuus/task-*-brief.md`) ne
    dépose de pilote VB-Audio dans l'arborescence que `console` construit
    (`console/guest/fetch_payload.py` : `agent/`, `virtio/`, `steam/`,
    `sudovda/`, `winfsp/`, `apollo/`, `nvidia/` — AUCUNE entrée
    `vb-audio`), et aucun chemin d'installateur silencieux n'est documenté
    nulle part dans ce dépôt ni dans `installer/`. Fabriquer une commande
    d'installation ici serait deviner un contrat qui n'existe pas — la
    consigne explicite de cette tâche est de ne jamais deviner un tel
    contrat (voir le chemin de `winrm_exec.py` ci-dessus, traité de la
    même façon). Lever ICI, fort et nommé, est le choix cohérent avec le
    reste de ce dépôt : un mécanisme qui échouerait en silence sur une
    demande explicite de l'opérateur (`vb_audio: true`) serait indiscernable
    d'un mécanisme qui n'a rien à faire. C'est un point ouvert, à trancher
    par le propriétaire du dépôt quand un payload VB-Audio existera — voir
    le rapport de cette tâche, § Réserves.
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
