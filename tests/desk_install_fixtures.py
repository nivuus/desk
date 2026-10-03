#!/usr/bin/env python3
"""Fixtures shared by `tests/test_desk_install.py`.

Extracted from the latter on 30 August 2026, in a DEDICATED commit and BEFORE the
final branch review added its scenarios there (the pre-flight that refuses before
any secret, the drop of the Node runtime): the file was at 465 lines out of
500. This repository forbids compressing code to avoid an extraction — exact
precedent: `tests/desk_activate_fixtures.py`, extracted for the same reason in
task 6.

This module carries the CONSTANTS (the wizard answers, the facts) and the
HELPERS (calling the hook, reading `desk.env` and a systemd unit,
building a minimal source root); `test_desk_install.py` carries the
SCENARIOS and the assertions. No fixture here has any value on its own.

⚠️ This file is NOT named `test_*.py`: it is not a suite, and the
`Makefile` therefore does not discover it — same convention, and same reason, as
`desk_activate_fixtures.py`.
"""
import configparser
import json
import os
import pathlib
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "install.py"


REPONSES = {"admin_email": "a@b.c", "admin_password": "hunter2hunter2",
            "auth_mode": "motdepasse", "vb_audio": False}

# The facts that task 3 (resolve) measures and that the engine WOULD HAND BACK to
# activate at first boot — see `installer/installer/install-engine/
# steps/packages.py::apply_packages`: `run_install` does NOT receive these facts
# (only `run_activate` receives them, merged into `hw`). install.py therefore
# cannot depend on them to work in the real engine; it accepts them
# here as a defensive fallback (if the contract changes one day, or for this test),
# and DERIVES them itself otherwise — exactly what it does in the two calls
# below, one WITH facts, the other WITHOUT.
# 🔴 `hote` and `proxy_confiance` are DELIBERATELY DIFFERENT from
# `turn_ecoute` below: that is what makes the test able to detect
# a regression towards the real bug fixed in batch 10A (29 August 2026) — before
# that fix, `install.py` set `PLATEFORME_HOTE = turn_ecoute`, which
# would have made the service listen on the address derived for TURN (public,
# on the real machine) rather than on an internal address.
# 🔴 `vm_repond` HAS DISAPPEARED FROM THE CONTRACT (final branch review, 30 August
# 2026): it was not a measured fact but a literal (`True` hardcoded
# in `hooks/resolve.py`), at a phase that can know nothing about the VM.
# See `tests/test_desk_contrat_hw.py` (guard ⑤).
FACTS = {"node_version": "24.9.0",
         "turn_ecoute": "203.0.113.9", "turn_relais": "203.0.113.9",
         "hote": "198.51.100.1", "proxy_confiance": "198.51.100.1",
         "port": 9999}


def poser_faux_node_source(racine: pathlib.Path) -> pathlib.Path:
    """A FAKE Node prefix: `bin/node`, `bin/npm` (a REAL relative
    link, like the real tree), and `lib/node_modules/npm/`.

    ⚠️ WHY A FAKE BY DEFAULT, AND NOT THE REAL ONE: the real runtime weighs
    144 MiB (measured on 29 August 2026 on `/opt/nivuus/node`), and every
    scenario of this suite would copy it. The dedicated scenario
    "installation 7" below uses, for its part, the REAL runtime of this
    machine — it is the one that exercises the production path, the others need
    not pay for it again.
    """
    prefixe = racine / "fake-node"
    (prefixe / "bin").mkdir(parents=True, exist_ok=True)
    (prefixe / "bin" / "node").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    (prefixe / "bin" / "node").chmod(0o755)
    npm_cli = prefixe / "lib" / "node_modules" / "npm" / "bin"
    npm_cli.mkdir(parents=True, exist_ok=True)
    (npm_cli / "npm-cli.js").write_text("// fake\n", encoding="utf-8")
    for nom, cible in (("npm", "../lib/node_modules/npm/bin/npm-cli.js"),):
        lien = prefixe / "bin" / nom
        if not lien.is_symlink():
            lien.symlink_to(cible)
    return prefixe


def poser_faux_node_bavard(prefixe: pathlib.Path, version: str) -> pathlib.Path:
    """Turns the fake `bin/node` of `prefixe` into one that ANSWERS: its
    own absolute path to `-e process.stdout.write(process.execPath)` (what
    `depot_node._prefixe_sur_le_path` asks) and `v<version>` to
    `--version` (what `borne_node.version_de` asks). Returns its `bin/`,
    to put on a PATH or to leave where a first install dropped it."""
    node = prefixe / "bin" / "node"
    node.write_text(
        "#!/bin/sh\n"
        "case \"$1\" in\n"
        f"  --version) echo v{version} ;;\n"
        f"  -e) printf '%s' '{node}' ;;\n"
        "esac\n", encoding="utf-8")
    node.chmod(0o755)
    return prefixe / "bin"


def appeler(root, hw=None, answers=None, facts=None, env=None,
            node_source=True):
    """Calls the hook like the engine: --phase/--root, stdin JSON.

    `node_source` (30 August 2026): `True` sets `DESK_NODE_SOURCE` to a
    FAKE Node prefix built under `root` (see
    `poser_faux_node_source`); `False` lets the hook derive the REAL
    runtime of this machine; a string sets it as is (refusal
    scenarios)."""
    # `hw` EMPTY by default: the engine sends `detect_all()` verbatim, and
    # `install.py` reads nothing from it — see tests/test_desk_contrat_hw.py, which
    # freezes this contract. It carried `{"vm_windows": True}`, a key no
    # producer sets (Critical finding of the final branch review).
    contexte = {"hw": hw if hw is not None else {},
                "answers": answers if answers is not None else REPONSES}
    if facts is not None:
        contexte["facts"] = facts
    if node_source is not False:
        env = dict(os.environ) if env is None else env
        env["DESK_NODE_SOURCE"] = (
            str(poser_faux_node_source(pathlib.Path(root)))
            if node_source is True else str(node_source))
    r = subprocess.run(
        [sys.executable, str(HOOK), "--phase", "install", "--root", str(root)],
        input=json.dumps(contexte), capture_output=True, text=True, env=env)
    return r


def lire_env(racine):
    """Parses `etc/nivuus/desk.env` (KEY=VALUE, one line per variable)."""
    chemin = pathlib.Path(racine) / "etc" / "nivuus" / "desk.env"
    values = {}
    for ligne in chemin.read_text(encoding="utf-8").splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.startswith("#") or "=" not in ligne:
            continue
        cle, _, value = ligne.partition("=")
        values[cle] = value
    return values, chemin


def poser_source_minimale(racine: pathlib.Path) -> None:
    """Builds a minimal SOURCE root (`plateforme/`, `client/dist/`,
    `proto/ts/`) that `DESK_SOURCE_RACINE` can point to — used
    by the scenarios that exercise `install` WITHOUT going through the real repository.

    🔴 `proto/ts/plateforme.ts` MUST EXIST: since batch 10A
    (29 August 2026, real finding), `install.py` also copies `proto/ts/`
    and REFUSES if `plateforme.ts` is not found there afterwards (see
    `hooks/install.py::main`) — without this file, these scenarios would all refuse
    for a reason they do not intend to exercise.
    """
    (racine / "plateforme").mkdir(parents=True, exist_ok=True)
    (racine / "plateforme" / "package.json").write_text("{}", encoding="utf-8")
    (racine / "client" / "dist").mkdir(parents=True, exist_ok=True)
    (racine / "client" / "dist" / "index.html").write_text(
        "<html></html>", encoding="utf-8")
    (racine / "proto" / "ts").mkdir(parents=True, exist_ok=True)
    (racine / "proto" / "ts" / "plateforme.ts").write_text(
        "export {};\n", encoding="utf-8")


def load_unit(path):
    """Parses a systemd unit file (INI, case-sensitive keys).

    Same pattern as `console/tests/test_console_install.py::load_unit`:
    `strict=False` (systemd tolerates a repeated key, the last one wins) and
    `optionxform=str` (systemd is case-sensitive, configparser
    lowercases by default).
    """
    parser = configparser.ConfigParser(strict=False, interpolation=None)
    parser.optionxform = str
    parser.read(path, encoding="utf-8")
    return parser
