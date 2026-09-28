#!/usr/bin/env python3
"""Tests of the manifest and the wizard of the desk package.

Run: python3 tests/test_desk_manifeste.py
"""
import pathlib
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
INSTALLER = RACINE.parent / "installer"
sys.path.insert(0, str(INSTALLER / "installer"))

from packages.manifest import load_manifest          # noqa: E402
from packages.wizard import load_questions           # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


m = load_manifest(str(RACINE / "nivuus-package.yaml"))

check("the name is desk", m.name, "desk")
check("the tier is userspace", m.tier, "userspace")

# 🔴 The tier is not cosmetic: `userspace` FORBIDS declaring
# kernel-cmdline, modules and hugepages. console has already taken VFIO, the GPU and the
# NVMe; desk only adds a service. The guarantee is checked by the engine,
# not promised by us.
check("no kernel module", m.platform.modules, ())
check("no kernel command line", m.platform.kernel_cmdline, ())

# The hard dependency: desk must not be installable without console.
check("console is a prerequisite", m.packages, ("console",))

questions = load_questions(str(RACINE / "wizard.yaml"))
par_cle = {q.key: q for q in questions}

check("four questions, not one more", len(questions), 4)
check("the email of the initial account", par_cle["admin_email"].type, "texte")
check("the password is a secret", par_cle["admin_password"].type, "secret")
check("the authentication mode is a choice", par_cle["auth_mode"].type, "choix")
check("its two values", tuple(sorted(par_cle["auth_mode"].choices)),
      ("motdepasse", "pomerium"))

# 🔴 VB-Audio has a PERSONAL licence only: installing it is an option
# the operator arms, never a default.
check("vb_audio is a boolean", par_cle["vb_audio"].type, "bool")
check("vb_audio is DISARMED by default", par_cle["vb_audio"].default, False)

# The token secret is NOT asked for: it is drawn at random at installation.
# Asking for it means getting a short one chosen.
check("no question asks for the token secret",
      [q.key for q in questions if "jeton" in q.key or "token" in q.key], [])

# The release source `nivuus update` follows: removing or misspelling it
# would silently stop the package from ever being updated.
check("release source", m.source.github if m.source else None, "nivuus/desk")

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - manifest and wizard tests passed")
