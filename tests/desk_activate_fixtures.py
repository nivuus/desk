#!/usr/bin/env python3
"""Fixtures partagées par `tests/test_desk_activate.py`.

Extrait de ce dernier à la ronde de correction 1 sur la tâche 6 : la revue
a relevé que la restauration des lignes vides et de la prose qu'un premier
tassement stylistique avait coûtées faisait franchir 500 lignes au fichier
de tests — et ce dépôt interdit de comprimer pour éviter une extraction
(« ce dépôt l'a payé douze fois »). Ce module porte les FIXTURES (faux
npm, faux systemctl, faux winrm_exec.py, racine installée, l'appel du
hook) ; `test_desk_activate.py` porte les SCÉNARIOS et les assertions.
Aucune fixture d'ici n'a de valeur seule : elle ne s'exerce que par les
scénarios qui l'importent.

⚠️ Ce fichier ne s'appelle PAS `test_*.py` : il n'est pas une suite de
tests à lui seul, `python3 tests/desk_activate_fixtures.py` ne fait rien
d'utile — c'est un module importé, jamais exécuté directement.
"""
import configparser
import json
import os
import pathlib
import stat
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "activate.py"

REPONSES = {"admin_email": "ada@exemple.test", "admin_password": "hunter2hunter2",
            "auth_mode": "motdepasse", "vb_audio": False}

# Les facts de resolve, telles qu'elles arrivent RÉELLEMENT : fusionnées
# dans hw (installer/packages/runner.py::run_activate, ligne 337), jamais
# sous une clé "facts" séparée.
HW_AVEC_FACTS = {"vm_repond": True, "node_version": "24.9.0",
                  "turn_ecoute": "203.0.113.9", "turn_relais": "203.0.113.9",
                  "port": 9999}


# --- Le faux npm : enregistre chaque invocation, ne touche jamais un vrai
# Node ni une vraie base ---------------------------------------------------

FAUX_NPM = """#!/usr/bin/env python3
import json, os, sys

argv = sys.argv[1:]
stdin_data = sys.stdin.read()
with open({log!r}, "a", encoding="utf-8") as fh:
    fh.write(json.dumps({{"argv": ["npm", *argv], "cwd": os.getcwd(),
                          "stdin": stdin_data,
                          "env_base_url": os.environ.get("PLATEFORME_BASE_URL", ""),
                          }}) + "\\n")

if "admin:utilisateur" in argv:
    sys.stdout.write("u-test-0001\\n")
    sys.exit(0)
if "admin:agent" in argv:
    sys.stdout.write("vm_id=vm-test-uuid\\nprefixe=abcd\\n"
                      "AGENT_SECRET=secret-de-test-0123456789abcdef\\n")
    sys.exit(0)
sys.exit(1)
"""


def poser_faux_npm(bin_dir: pathlib.Path, log: pathlib.Path) -> None:
    script = bin_dir / "npm"
    script.write_text(FAUX_NPM.format(log=str(log)), encoding="utf-8")
    script.chmod(0o755)


# --- Le faux winrm_exec.py (tâche 6) : enregistre chaque invocation,
# n'atteint jamais la VM ni le réseau. Contrat exact du vrai
# `console/guest/winrm_exec.py` (lu intégralement, tâche 6) : appelé
# `<script> {mode} <commande>`, il imprime sa réponse sur stdout. Ici,
# aucune réponse : la sortie vide suffit à ce que ProjFS déclare qu'aucun
# redémarrage n'est requis, ce qui est déjà éprouvé en détail par
# tests/test_desk_vm.py.
FAUX_WINRM_EXEC = """#!/usr/bin/env python3
import json, sys
with open({log!r}, "a", encoding="utf-8") as fh:
    fh.write(json.dumps({{"argv": sys.argv[1:]}}) + "\\n")
sys.exit(0)
"""


def poser_faux_winrm_exec(packages_dir: pathlib.Path, log: pathlib.Path) -> None:
    guest_dir = packages_dir / "console" / "guest"
    guest_dir.mkdir(parents=True, exist_ok=True)
    script = guest_dir / "winrm_exec.py"
    script.write_text(FAUX_WINRM_EXEC.format(log=str(log)), encoding="utf-8")
    script.chmod(0o755)


# --- Le faux fetch_payload.py (tâche 7) : simple MARQUEUR de présence.
# `activate.py::chemin_agent_console()` ne fait que tester son existence
# (`is_file()`) — jamais l'exécuter — donc un contenu vide suffit à prouver
# que « console est installé » sans rien exécuter du vrai fichier.
def poser_faux_fetch_payload(packages_dir: pathlib.Path) -> None:
    guest_dir = packages_dir / "console" / "guest"
    guest_dir.mkdir(parents=True, exist_ok=True)
    (guest_dir / "fetch_payload.py").write_text("", encoding="utf-8")


# --- Le faux scripts/build-agent-croise.sh (tâche 7) : jamais le vrai (il
# compilerait l'agent réel, ~40 s, boîte à outils croisée). Contrat exact du
# vrai script (lu intégralement) : appelé `<script> <destination_dir>`, il y
# dépose lui-même `agent.exe`. Ici, un contenu FACTICE fixe suffit : aucune
# recette de ce lot ne juge le binaire produit, seulement son EMPLACEMENT.
FAUX_BUILD_AGENT = """#!/usr/bin/env python3
import pathlib, sys
d = pathlib.Path(sys.argv[1])
d.mkdir(parents=True, exist_ok=True)
(d / "agent.exe").write_bytes(b"faux-agent-exe-de-test")
"""


def poser_faux_build_agent(chemin: pathlib.Path) -> None:
    chemin.write_text(FAUX_BUILD_AGENT, encoding="utf-8")
    chemin.chmod(0o755)


def poser_faux_systemctl(bin_dir: pathlib.Path, log: pathlib.Path) -> None:
    """Un systemctl qui n'agit sur RIEN : juste une trace de ses arguments,
    pour prouver qu'il n'est jamais invoqué sous un --root de test."""
    script = bin_dir / "systemctl"
    script.write_text(
        "#!/bin/sh\n"
        f'echo "$@" >> {log}\n'
        "exit 0\n",
        encoding="utf-8",
    )
    script.chmod(0o755)


def lire_commandes(log: pathlib.Path):
    if not log.exists():
        return []
    commandes = []
    for ligne in log.read_text(encoding="utf-8").splitlines():
        if ligne.strip():
            commandes.append(json.loads(ligne))
    return commandes


# --- Fabrique une racine où `install` aurait déjà tourné -------------------

def poser_racine_installee(root: pathlib.Path, contenu_env: dict = None) -> None:
    unite_dir = root / "etc" / "systemd" / "system"
    unite_dir.mkdir(parents=True, exist_ok=True)
    # Copie l'unité RÉELLE de la tâche 4 — c'est elle dont le WantedBy=
    # décide sous quel .wants/ le lien doit vivre.
    (unite_dir / "desk-plateforme.service").write_text(
        (RACINE / "hooks" / "assets" / "desk-plateforme.service").read_text(encoding="utf-8"),
        encoding="utf-8",
    )

    plateforme_dir = root / "opt" / "nivuus" / "desk" / "plateforme"
    plateforme_dir.mkdir(parents=True, exist_ok=True)

    env_dir = root / "etc" / "nivuus"
    env_dir.mkdir(parents=True, exist_ok=True)
    valeurs = contenu_env if contenu_env is not None else {
        "PLATEFORME_BASE": "sqlite",
        "PLATEFORME_BASE_URL": "/var/lib/nivuus-desk/plateforme.sqlite",
        "PLATEFORME_SECRET_JETON": "x" * 64,
    }
    corps = "\n".join(f"{k}={v}" for k, v in valeurs.items()) + "\n"
    chemin_env = env_dir / "desk.env"
    chemin_env.write_text(corps, encoding="utf-8")
    os.chmod(chemin_env, stat.S_IRUSR | stat.S_IWUSR)


def appeler(root, bin_dir, hw=None, answers=None, root_arg=None, packages_dir=None):
    """Appelle le hook comme le moteur (--phase, --root), PATH réécrit vers
    bin_dir en tête, pour que npm/systemctl résolus soient les factices.

    `packages_dir` (tâche 6) : où pointer `NIVUUS_PACKAGES_DIR` pour la
    résolution de `winrm_exec.py` ET, depuis la tâche 7, de
    `fetch_payload.py`. Par défaut (`None`), un faux
    `console/guest/winrm_exec.py` FONCTIONNEL et un faux
    `console/guest/fetch_payload.py` (simple marqueur) sont fabriqués sous
    `root` lui-même — sans quoi tout scénario qui n'a rien à voir avec
    ProjFS/VB-Audio/le dépôt d'agent.exe échouerait sur « console absent »
    avant d'atteindre la raison qu'il veut réellement éprouver.
    `packages_dir=False` simule `console` ABSENT (aucun fichier n'est créé,
    pour les scénarios dédiés).

    🔴 TÂCHE 7 — `DESK_BUILD_AGENT_SCRIPT` est TOUJOURS posée (peu importe
    `packages_dir`) vers un script FACTICE qui ne compile rien : sans elle,
    `deposer_agent_console()`, appelé sans condition par `main()`,
    invoquerait le VRAI `scripts/build-agent-croise.sh` à CHAQUE scénario
    de ce fichier — la compilation réelle que la tâche interdit dans les
    tests.
    """
    contexte = {"hw": hw if hw is not None else HW_AVEC_FACTS,
                "answers": answers if answers is not None else REPONSES}
    env = dict(os.environ)
    env["PATH"] = str(bin_dir) + os.pathsep + env.get("PATH", "")
    # Le contrôle ② veut prouver que PLATEFORME_BASE_URL atteint npm PAR LA
    # FUSION que le hook fait depuis desk.env — jamais parce que le shell qui
    # fait tourner ces tests l'exportait déjà par accident.
    env.pop("PLATEFORME_BASE_URL", None)
    if packages_dir is None:
        packages_dir = root / "faux-packages-dir"
        poser_faux_winrm_exec(packages_dir, root / "winrm.log")
        poser_faux_fetch_payload(packages_dir)
    elif packages_dir is False:
        packages_dir = root / "console-absent-ici"
    env["NIVUUS_PACKAGES_DIR"] = str(packages_dir)
    faux_build = root / "faux-build-agent.py"
    if not faux_build.is_file():
        poser_faux_build_agent(faux_build)
    env["DESK_BUILD_AGENT_SCRIPT"] = str(faux_build)
    cmd = [sys.executable, str(HOOK), "--phase", "activate",
           "--root", str(root_arg if root_arg is not None else root)]
    return subprocess.run(cmd, input=json.dumps(contexte), env=env,
                           capture_output=True, text=True)


def load_unit(path):
    parser = configparser.ConfigParser(strict=False, interpolation=None)
    parser.optionxform = str
    parser.read(path, encoding="utf-8")
    return parser


def lire_env(chemin):
    valeurs = {}
    for ligne in chemin.read_text(encoding="utf-8").splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.startswith("#") or "=" not in ligne:
            continue
        cle, _, valeur = ligne.partition("=")
        valeurs[cle] = valeur
    return valeurs
