#!/usr/bin/env python3
"""Où `console` va chercher `agent.exe`, jamais deviné : construction croisée
et dépôt du binaire, tâche 7 (2026-08-29).

Extrait de `hooks/activate.py` par la tâche 13 (2026-08-29), pour la MÊME
raison que `hooks/administration.py` et `hooks/env_fichier.py` (voir leurs
propres docstrings de tête) : `activate.py` atteignait 509 lignes après
l'ajout de l'attribution de la VM au compte administrateur (tâche 13,
trou trouvé en production), et ce dépôt interdit de comprimer pour éviter
une extraction (« ce dépôt l'a payé douze fois »). Ce module porte donc,
VERBATIM, ce que la tâche 7 avait écrit : le même corps, le même docstring,
seulement déplacés dans un commit DÉDIÉ, AVANT celui qui ajoute l'attribution.

Comme `hooks/vm.py`, `hooks/administration.py` et `hooks/env_fichier.py`, ce
module N'EST PAS un hook exécutable seul (pas de `--phase`/stdin JSON) :
`hooks/activate.py` l'importe (`from agent_payload import chemin_agent_console,
construire_agent_reel, deposer_agent_console`) — Python ajoute automatiquement
le répertoire du script LANCÉ (`hooks/`) à `sys.path`, donc cet import résout
sans manipulation supplémentaire, ni ici ni chez l'appelant. `tests/
test_desk_payload.py` continue de les atteindre par `activate.chemin_agent_console`
etc. : un nom importé PAR NOM dans `activate.py` devient un attribut du module
`activate` au même titre qu'un nom défini localement — aucun changement de
test n'était nécessaire pour cette extraction.

🔴 Pas `<drivers_dir>/agent/agent.exe` (dérivé de `guest_workdir`, réponse DE
`console` que ce hook ne reçoit jamais — `activate_cli.py:107-108` n'indexe
que les réponses du package appelant ; `/etc/nivuus/packages.json` est
d'ailleurs absent sur cette machine). Le chemin RÉEL, fixe, que `console` lit
lui-même : `console/guest/fetch_payload.py:61` —
`PACKAGED_AGENT_EXE = Path(__file__).resolve().parent/"payload"/"agent"
/"agent.exe"`, un fichier VENDORISÉ (voir `console/guest/payload/agent/
README.md`) qu'`install_packaged_agent()` recopie vers `<drivers_dir>/
agent/agent.exe` à chaque `fetch_payload.py`. Ce hook rafraîchit CETTE
source, retrouvée sous `<NIVUUS_PACKAGES_DIR>/console/` (même convention
que `hooks/vm.py::chemin_winrm_exec`, sa constante importée ici).
⚠️ RÉSERVE : `console` s'active AVANT `desk` (dépendance) — le TOUT
PREMIER provisionnement consomme l'image déjà committée ; ce dépôt ne
garantit que les RECONSTRUCTIONS futures. Voir le rapport de la tâche 7,
§ Réserves.
"""
import os
import pathlib
import subprocess

from vm import NIVUUS_PACKAGES_DIR_DEFAUT

RACINE = pathlib.Path(__file__).resolve().parents[1]

AGENT_EXE_CONSOLE_RELATIF = pathlib.Path("console") / "guest" / "payload" / "agent" / "agent.exe"
FETCH_PAYLOAD_RELATIF = pathlib.Path("console") / "guest" / "fetch_payload.py"


def chemin_agent_console() -> pathlib.Path:
    """Targets `<NIVUUS_PACKAGES_DIR>/console/guest/payload/agent/agent.exe`
    (reread by `fetch_payload.py:61`); raises `FileNotFoundError`, naming the
    path, if `console/guest/fetch_payload.py` is absent — a silent
    failure would be indistinguishable from a no-op (doctrine of
    `hooks/vm.py::chemin_winrm_exec`)."""
    packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR", NIVUUS_PACKAGES_DIR_DEFAUT)
    fetch_payload = pathlib.Path(packages_dir) / FETCH_PAYLOAD_RELATIF
    if not fetch_payload.is_file():
        raise FileNotFoundError(
            f"fetch_payload.py introuvable a l'emplacement attendu : {fetch_payload} "
            "(le package console est-il installe ? NIVUUS_PACKAGES_DIR="
            f"{packages_dir!r} - voir hooks/activate.py pour la convention)"
        )
    return pathlib.Path(packages_dir) / AGENT_EXE_CONSOLE_RELATIF


def construire_agent_reel(destination: pathlib.Path) -> None:
    """Invokes `scripts/build-agent-croise.sh <destination.parent>` (a
    DIRECTORY, never a file name); raises `RuntimeError` (the script's
    output) on a non-zero code. Path overridable through
    `DESK_BUILD_AGENT_SCRIPT` (never in production) so that tests never
    compile the real agent (~40 s) — see `tests/test_desk_payload.py`.
    """
    defaut = RACINE / "scripts" / "build-agent-croise.sh"
    script = pathlib.Path(os.environ.get("DESK_BUILD_AGENT_SCRIPT") or defaut)
    proc = subprocess.run([str(script), str(destination.parent)],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(f"{script} a echoue (code {proc.returncode}) : {detail or 'aucune sortie'}")


def deposer_agent_console(construire=None) -> pathlib.Path:
    """Builds `agent.exe` and drops it where `console` looks for it.
    `construire`: fake in tests, `construire_agent_reel` by default.
    🔴 Checks AFTERWARDS that the file exists (raises otherwise): a mute
    `construire` would be a silent success ("a check never
    seen red is not a check", paid for on `AUDIO_FAUTE_LECTURE`).
    """
    cible = chemin_agent_console()
    construire = construire or construire_agent_reel
    cible.parent.mkdir(parents=True, exist_ok=True)
    construire(cible)
    if not cible.is_file():
        raise RuntimeError(f"construire() a rendu la main mais {cible} est absent : aucun agent.exe n'a ete depose")
    return cible
