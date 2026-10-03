"""REPLAYING install WITHOUT `node` ON THE PATH: the dropped runtime stands in.

`nivuus update desk` replays the install hook on a target whose only Node
is the one its first install dropped under `/opt/nivuus/node` — nowhere on
PATH, by construction (an engine-installed target has no nvm; measured on
2026-10-03, a plain root PATH on the reference machine has no `node`
either). Before this scenario existed, `racine_node_source()` asked PATH
only, so every update refused with "node is missing or silent" and the
updater marked desk failed.

A success scenario, hence not in tests/test_desk_install_gardes.py (its
selection rule: refusals only); not in tests/test_desk_install.py either,
which sits at its 500-line ceiling. Same fixtures as both.

Run: python3 tests/test_desk_install_replay.py
"""
import os
import pathlib
import shutil
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RACINE / "hooks"))
sys.path.insert(0, str(RACINE / "tests"))

from commun import lire_node_bin  # noqa: E402
from env_file import ACTIVATE_KEYS, add_env_variables, read_env_file  # noqa: E402
from desk_install_fixtures import (  # noqa: E402
    FACTS, appeler, poser_faux_node_bavard, poser_faux_node_source,
)

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


with tempfile.TemporaryDirectory() as tmp, tempfile.TemporaryDirectory() as vide:
    root = pathlib.Path(tmp)
    # What a first install leaves behind: a complete runtime at the target
    # prefix (the parent of lire_node_bin()), here the fixtures' fake one.
    faux = poser_faux_node_source(root)
    depose = root / lire_node_bin().lstrip("/")
    depose = depose.parent
    shutil.copytree(faux, depose, symlinks=True)
    shutil.rmtree(faux)
    # The fixtures' fake node answers nothing; the dropped one must answer
    # `--version` with something engines.node accepts (FACTS carries a
    # version resolve already validated against that very bound).
    poser_faux_node_bavard(depose, FACTS["node_version"])
    node_avant = (depose / "bin" / "node").read_bytes()
    lien_avant = os.readlink(depose / "bin" / "npm")

    env = {k: v for k, v in os.environ.items() if k != "DESK_NODE_SOURCE"}
    env["PATH"] = vide  # no `node` resolves: the replay case
    r = appeler(root, facts=FACTS, env=env, node_source=False)
    check("replay without node on PATH: exit code 0", r.returncode, 0)
    check("replay: no Python traceback", "Traceback" in (r.stderr or ""), False)
    check("replay: desk.env is written",
          (root / "etc" / "nivuus" / "desk.env").is_file(), True)
    # The dropped runtime is the source AND the destination: it must come
    # out of the replay byte for byte - a copy onto itself would first
    # delete bin/ and then find nothing to read.
    check("replay: bin/node is intact",
          (depose / "bin" / "node").read_bytes(), node_avant)
    check("replay: the npm link is intact and still relative",
          os.readlink(depose / "bin" / "npm"), lien_avant)
    check("replay: the npm package is still there",
          (depose / "lib" / "node_modules" / "npm" / "bin" / "npm-cli.js").is_file(),
          True)

# A `node` on the PATH that this package's engines.node REFUSES must not be
# preferred over a dropped runtime that satisfies it: the PATH comes first
# only when it is compatible (review of #21).
with tempfile.TemporaryDirectory() as tmp, tempfile.TemporaryDirectory() as autre:
    root = pathlib.Path(tmp)
    faux = poser_faux_node_source(root)
    depose = (root / lire_node_bin().lstrip("/")).parent
    shutil.copytree(faux, depose, symlinks=True)
    shutil.rmtree(faux)
    poser_faux_node_bavard(depose, FACTS["node_version"])
    node_avant = (depose / "bin" / "node").read_bytes()
    vieux = poser_faux_node_source(pathlib.Path(autre))
    bin_vieux = poser_faux_node_bavard(vieux, "0.10.0")

    env = {k: v for k, v in os.environ.items() if k != "DESK_NODE_SOURCE"}
    env["PATH"] = str(bin_vieux)
    r = appeler(root, facts=FACTS, env=env, node_source=False)
    check("incompatible node on PATH, compatible runtime dropped: exit code 0",
          r.returncode, 0)
    check("incompatible node on PATH: the dropped runtime is kept, byte for byte",
          (depose / "bin" / "node").read_bytes(), node_avant)
    check("incompatible node on PATH: desk.env is written",
          (root / "etc" / "nivuus" / "desk.env").is_file(), True)

# REPLAYING install KEEPS WHAT activate ADDED. activate appends AGENT_VM and
# AGENT_SECRET to desk.env once the agent is enrolled, and skips enrolment
# (and the administrator account) on a replay when both are there. install
# rewrote the file from its own keys only, so an update lost that proof:
# measured 2026-10-03 on the first `nivuus update desk`, activate then
# tripped on "UNIQUE constraint failed: utilisateur.email" - and, had it
# not, would have minted a new AGENT_SECRET the agent in the VM does not
# hold. A first install, with no file yet, must still write no AGENT_* key.
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    r = appeler(root, facts=FACTS)
    check("activate keys, first pass: exit code 0", r.returncode, 0)
    env_chemin = root / "etc" / "nivuus" / "desk.env"
    premier = read_env_file(env_chemin)
    check("first pass: no AGENT_* key is invented",
          [cle for cle in ACTIVATE_KEYS if cle in premier], [])
    add_env_variables(env_chemin, dict(zip(ACTIVATE_KEYS, ("vm-1", "s3cret"))))
    r = appeler(root, facts=FACTS)
    check("activate keys, replay: exit code 0", r.returncode, 0)
    second = read_env_file(env_chemin)
    check("replay: AGENT_VM survives the rewrite", second.get("AGENT_VM"), "vm-1")
    check("replay: AGENT_SECRET survives the rewrite", second.get("AGENT_SECRET"), "s3cret")
    check("replay: PLATEFORME_SECRET_JETON is still the first one",
          second.get("PLATEFORME_SECRET_JETON"), premier.get("PLATEFORME_SECRET_JETON"))
    check("replay: the file stays 0600", oct(env_chemin.stat().st_mode & 0o777), "0o600")

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - install replay tests passed")
