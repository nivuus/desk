#!/usr/bin/env python3
"""The install hook gives the platform the VM control socket of `console`.

`console` ships `nivuus-vm-control.socket` (`/run/nivuus/vm-control.sock`,
mode 0660, group `nivuus-vm`). The platform reads its path from
`PLATEFORME_SOCKET_VM` and its unit must join that group, otherwise the
channel is either disabled or refused at the first connection.

Run: python3 tests/test_desk_install_vm_control.py
"""
import pathlib
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RACINE / "hooks"))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from desk_install_fixtures import FACTS, appeler, lire_env, load_unit  # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    r = appeler(root, facts=FACTS)
    check("install: exit code 0", r.returncode, 0)

    env, _chemin = lire_env(root)
    check("desk.env sets the VM control socket",
          env.get("PLATEFORME_SOCKET_VM"), "/run/nivuus/vm-control.sock")

    ini = load_unit(root / "etc" / "systemd" / "system" / "desk-plateforme.service")
    check("the platform unit joins the group of the VM control socket",
          ini.get("Service", "SupplementaryGroups", fallback=""), "nivuus-vm")

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - install VM control socket tests passed")
