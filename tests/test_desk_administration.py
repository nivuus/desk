#!/usr/bin/env python3
"""Tests of `hooks/administration.py::lancer_npm` — problem A of batch
10A (29 August 2026) addressed here: `npm` invoked by its name alone depends on the
PATH of the calling process, which only contains `node`/`npm` by accident
on a development machine where nvm is sourced in the interactive shell.

This file exercises `lancer_npm` DIRECTLY (import, not a subprocess of the
complete hook — `tests/test_desk_activate.py` already covers the whole hook through
a complete scenario): it is the narrowest function that
carries this responsibility, and the module is NOT a hook runnable on its own
(see its own head docstring).

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


# --- lancer_npm APPENDS NODE_BIN_DEFAUT at the end of PATH, without imposing it ------
with tempfile.TemporaryDirectory() as tmp:
    cwd = pathlib.Path(tmp)
    env_sans_node = {"PATH": "/a/path/that/does/not/contain/node"}
    code, out, err = administration.lancer_npm(cwd, "admin:utilisateur", [],
                                                env_sans_node, entree="x\n")
    # npm is not found (no real npm on this fake PATH, and
    # NODE_BIN_DEFAUT does not necessarily exist on the test machine): this
    # test does NOT judge npm's return code, only that lancer_npm
    # NEVER raises (the documented contract: "Always returns (code, stdout,
    # stderr), never raises").
    check("lancer_npm never raises even without node in the given PATH",
          isinstance(code, int), True)

# --- A fake `npm` ALREADY AT THE HEAD OF PATH keeps winning --------------
# 🔴 THIS IS THE CHECK THAT PROVES THE APPENDING SHORT-CIRCUITS NOTHING:
# `tests/desk_activate_fixtures.py::appeler` puts its OWN fake npm at the
# HEAD of PATH, and if `lancer_npm` prefixed it instead of appending it, that
# fake npm would never be found again.
with tempfile.TemporaryDirectory() as tmp:
    cwd = pathlib.Path(tmp)
    bin_dir = pathlib.Path(tmp) / "fake-bin"
    bin_dir.mkdir()
    faux_npm = bin_dir / "npm"
    marqueur = pathlib.Path(tmp) / "seen.txt"
    faux_npm.write_text(
        "#!/usr/bin/env python3\n"
        "import pathlib, sys\n"
        f"pathlib.Path({str(marqueur)!r}).write_text('seen')\n"
        "sys.exit(0)\n",
        encoding="utf-8",
    )
    faux_npm.chmod(0o755)
    # `/usr/bin` stays necessary: the fake npm is a
    # `#!/usr/bin/env python3` script, and `/usr/bin/env` must be
    # found for the kernel to even launch python3.
    env = {"PATH": str(bin_dir) + os.pathsep + "/usr/bin:/bin"}
    code, out, err = administration.lancer_npm(cwd, "admin:utilisateur", [], env)
    check("the fake npm at the head of PATH is indeed invoked (code 0)", code, 0)
    check("the fake npm at the head of PATH did run (marker written)",
          marqueur.is_file(), True)

# --- NODE_BIN_DEFAUT is indeed appended, never absent from the final PATH ---------
with tempfile.TemporaryDirectory() as tmp:
    cwd = pathlib.Path(tmp)
    marqueur_path = pathlib.Path(tmp) / "path_seen.txt"
    bin_dir = pathlib.Path(tmp) / "fake-bin"
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
    check("NODE_BIN_DEFAUT is present in the PATH passed to the subprocess",
          NODE_BIN_DEFAUT in path_vu.split(os.pathsep), True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - hooks/administration.py tests passed")
