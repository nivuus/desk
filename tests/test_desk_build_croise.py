#!/usr/bin/env python3
"""Tests of the agent cross build.

What these tests exercise: that the script REFUSES cleanly when its
tooling is missing, and that it names what is missing. They do NOT exercise that the
binary works — see task 9 and the spec §4.3: the build proves
that the agent links, never that it runs.

Run: python3 tests/test_desk_build_croise.py
"""
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = RACINE / "scripts" / "build-agent-croise.sh"

# ABSOLUTE path of bash: the third block below builds a sandbox PATH
# for the child process, and a "bash" without a slash would itself be
# unreachable there — it is the CHILD's PATH that subprocess consults
# to resolve argv[0], not this script's.
BASH = shutil.which("bash") or "/bin/bash"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


check("the script exists", SCRIPT.is_file(), True)
check("the script is executable", os.access(SCRIPT, os.X_OK), True)

# Without a destination, it must refuse and SAY so — never write somewhere
# by default: a default would drop a 20 MiB binary at a place
# nobody asked for.
r = subprocess.run(["bash", str(SCRIPT)], capture_output=True, text=True)
check("no destination: non-zero code", r.returncode != 0, True)
check("no destination: the reason is stated",
      "destination" in (r.stdout + r.stderr).lower(), True)

# A missing rustup target must be named, not left to cargo, which would return
# an unreadable build error.
with tempfile.TemporaryDirectory() as tmp:
    env = dict(os.environ, CIBLE_RUST="target-that-does-not-exist")
    r = subprocess.run(["bash", str(SCRIPT), tmp],
                       capture_output=True, text=True, env=env)
    check("unknown target: non-zero code", r.returncode != 0, True)
    check("unknown target: it is named",
          "target-that-does-not-exist" in (r.stdout + r.stderr), True)

# A missing mingw linker must be named, not left to an unreadable cargo
# link failure. ⚠️ On this host, /bin is a symbolic link to
# /usr/bin: a PATH that only removes /usr/bin leaves
# x86_64-w64-mingw32-gcc reachable through /bin, and this block would pass for the
# WRONG reason — unable to go red. The PATH below is a sandbox
# built tool by tool (never a whole directory removed):
# only the executables the script needs (rustup, cargo — its own
# link to rustup —, grep, mkdir, cp, stat, dirname) are reachable there, and
# x86_64-w64-mingw32-gcc is reachable nowhere.
with tempfile.TemporaryDirectory() as tmp, \
     tempfile.TemporaryDirectory() as bac_a_sable:
    outils_necessaires = ("grep", "mkdir", "cp", "stat", "dirname")
    for outil in outils_necessaires:
        chemin_reel = shutil.which(outil)
        assert chemin_reel, f"required tool not found on this host: {outil}"
        os.symlink(chemin_reel, os.path.join(bac_a_sable, outil))

    rustup_reel = shutil.which("rustup")
    assert rustup_reel, "rustup not found on this host"
    repertoire_rustup = os.path.dirname(rustup_reel)

    env = dict(os.environ, PATH=f"{bac_a_sable}:{repertoire_rustup}")
    r = subprocess.run([BASH, str(SCRIPT), tmp],
                       capture_output=True, text=True, env=env)
    check("linker missing: non-zero code", r.returncode != 0, True)
    check("linker missing: it is named",
          "x86_64-w64-mingw32-gcc" in (r.stdout + r.stderr), True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - cross build tests passed")
