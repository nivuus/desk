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
from desk_install_fixtures import FACTS, appeler, poser_faux_node_source  # noqa: E402

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

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - install replay stands on the dropped Node runtime")
