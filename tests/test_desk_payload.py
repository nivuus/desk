#!/usr/bin/env python3
"""Tests of task 7: dropping `agent.exe` where `console` looks for it.

`console/guest/fetch_payload.py:61` defines
`PACKAGED_AGENT_EXE = Path(__file__).resolve().parent / "payload" / "agent"
/ "agent.exe"` — a FIXED path, vendored in the `console` repository itself
(see `console/guest/payload/agent/README.md`), found under
`<NIVUUS_PACKAGES_DIR>/console/` (same convention as
`hooks/vm.py::chemin_winrm_exec`). It is this target, and NOWHERE
else, that `activate.py::chemin_agent_console()`/`deposer_agent_console()`
aim at. It is NOT `<drivers_dir>/agent/agent.exe` (the build root
of the ISO): that path depends on the wizard answer
`guest_workdir` OF `console`, which this hook never receives (see the
head docstring of `activate.py` for the detail of this elimination).

🔴 NO REAL BUILD HERE. The unit tests inject a fake Python
`construire` directly into `deposer_agent_console()`;
the end-to-end test (subprocess) uses `DESK_BUILD_AGENT_SCRIPT`
(set by `desk_activate_fixtures.appeler()`) to replace
`scripts/build-agent-croise.sh` with a script that builds nothing — never
the real script, which would take ~40 s and require a cross toolchain.

Run: python3 tests/test_desk_payload.py
"""
import importlib.util
import os
import pathlib
import sys
import tempfile

from desk_activate_fixtures import (
    HOOK,
    appeler,
    poser_faux_fetch_payload,
    poser_faux_npm,
    poser_faux_winrm_exec,
    poser_racine_installee,
)

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# `hooks/activate.py` does `from vm import ...` / `from env_file import
# ...` / `from administration import ...`: an import by PATH
# (spec_from_file_location) does not go through the mechanism that
# automatically adds the script's directory to sys.path (precedent of
# test_desk_activate.py, task 6).
sys.path.insert(0, str(HOOK.parent))
spec = importlib.util.spec_from_file_location("desk_activate_payload", HOOK)
activate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(activate)


class _EnvTemporaire:
    """Sets an environment variable for the duration of the `with` block, and
    restores it exactly to its previous value (or absence). Avoids
    polluting the following tests with the `NIVUUS_PACKAGES_DIR` of a previous
    scenario."""

    def __init__(self, **values):
        self.values = values
        self.anciennes = {}

    def __enter__(self):
        for cle, value in self.values.items():
            self.anciennes[cle] = os.environ.get(cle)
            os.environ[cle] = value
        return self

    def __exit__(self, *_exc):
        for cle, ancienne in self.anciennes.items():
            if ancienne is None:
                os.environ.pop(cle, None)
            else:
                os.environ[cle] = ancienne


# === chemin_agent_console(): resolution, the name and the subdirectory ====
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    packages_dir = root / "packages"
    poser_faux_winrm_exec(packages_dir, root / "winrm.log")
    poser_faux_fetch_payload(packages_dir)

    with _EnvTemporaire(NIVUUS_PACKAGES_DIR=str(packages_dir)):
        cible = activate.chemin_agent_console()

    check("resolved file name", cible.name, "agent.exe")
    check("resolved subdirectory (REQUIRED_BINARIES contract)", cible.parent.name, "agent")
    check("resolved full path",
          cible,
          packages_dir / "console" / "guest" / "payload" / "agent" / "agent.exe")


# === RED of the inter-package contract: console absent (fetch_payload.py
# absent) -> chemin_agent_console() RAISES, never an invented path ===
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    packages_dir = root / "packages-without-console"

    with _EnvTemporaire(NIVUUS_PACKAGES_DIR=str(packages_dir)):
        a_leve = False
        try:
            activate.chemin_agent_console()
        except FileNotFoundError as exc:
            a_leve = True
            message = str(exc)

    check("chemin_agent_console() raises if fetch_payload.py is absent", a_leve, True)
    check("the message names fetch_payload.py", "fetch_payload.py" in message, True)


# === deposer_agent_console(): the dropped file IS agent.exe/agent/ =====
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    packages_dir = root / "packages"
    poser_faux_winrm_exec(packages_dir, root / "winrm.log")
    poser_faux_fetch_payload(packages_dir)

    appels = []

    def faux_construire(destination):
        appels.append(destination)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(b"fake-test-binary")

    with _EnvTemporaire(NIVUUS_PACKAGES_DIR=str(packages_dir)):
        cible = activate.deposer_agent_console(construire=faux_construire)

    check("a single call to construire", len(appels), 1)
    check("the dropped file name is agent.exe (REQUIRED_BINARIES contract)",
          cible.name, "agent.exe")
    check("the subdirectory is indeed 'agent'", cible.parent.name, "agent")
    check("the file really exists on disk", cible.is_file(), True)
    check("the content is the one dropped by construire()",
          cible.read_bytes(), b"fake-test-binary")


# === RED: a construire() that drops nothing -> RuntimeError, never a
# silent success ("a check never seen red is not
# a check") ============================================================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    packages_dir = root / "packages"
    poser_faux_winrm_exec(packages_dir, root / "winrm.log")
    poser_faux_fetch_payload(packages_dir)

    def construire_muet(_destination):
        pass  # never creates the file: simulates a silently no-op script

    with _EnvTemporaire(NIVUUS_PACKAGES_DIR=str(packages_dir)):
        a_leve = False
        try:
            activate.deposer_agent_console(construire=construire_muet)
        except RuntimeError:
            a_leve = True

    check("deposer_agent_console() raises if construire() dropped nothing", a_leve, True)


# === construire_agent_reel(): the REAL subprocess, but a FAKE
# script (DESK_BUILD_AGENT_SCRIPT) — never the real build-agent-croise.sh ====
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    script = root / "fake-build.py"
    script.write_text(
        "#!/usr/bin/env python3\n"
        "import pathlib, sys\n"
        "d = pathlib.Path(sys.argv[1])\n"
        "d.mkdir(parents=True, exist_ok=True)\n"
        "(d / 'agent.exe').write_bytes(b'fake-test-pe32')\n",
        encoding="utf-8")
    script.chmod(0o755)

    with _EnvTemporaire(DESK_BUILD_AGENT_SCRIPT=str(script)):
        cible = root / "cible" / "agent" / "agent.exe"
        activate.construire_agent_reel(cible)

    check("construire_agent_reel() invokes the fake script and drops the file",
          cible.is_file(), True)
    check("the content does come from the fake script",
          cible.read_bytes(), b"fake-test-pe32")

# --- RED: the script fails (non-zero code) -> RuntimeError, carrying its
# output -----------------------------------------------------------------
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    script = root / "fake-build-fails.sh"
    script.write_text("#!/bin/sh\necho 'simulated build error' >&2\nexit 1\n",
                       encoding="utf-8")
    script.chmod(0o755)

    with _EnvTemporaire(DESK_BUILD_AGENT_SCRIPT=str(script)):
        a_leve = False
        try:
            activate.construire_agent_reel(root / "cible" / "agent" / "agent.exe")
        except RuntimeError as exc:
            a_leve = True
            detail = str(exc)

    check("construire_agent_reel() raises if the script fails", a_leve, True)
    check("the message names the script's error", "simulated build error" in detail, True)


# === The COMPLETE hook, as a subprocess: console PARTIALLY present
# (winrm_exec.py present, fetch_payload.py absent) -> clean refusal, AFTER
# ProjFS/VB-Audio (task 6) succeeded ================================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    packages_dir = root / "incomplete-packages"
    poser_faux_winrm_exec(packages_dir, root / "winrm.log")  # PARTIAL console

    r = appeler(root, bin_dir, packages_dir=packages_dir)
    check("complete hook: clean refusal if fetch_payload.py is absent (rc != 0)",
          r.returncode != 0, True)
    check("the refusal names fetch_payload.py",
          "fetch_payload.py" in (r.stderr or ""), True)
    check("no Python traceback reaches the operator",
          "Traceback" in (r.stderr or ""), False)
    check("no npm command launched (the refusal precedes the account creation)",
          (root / "npm.log").exists(), False)
    # ProjFS must still have succeeded before this refusal (a single call, its
    # own): the proof that the order does put the drop AFTER task 6.
    check("ProjFS still ran before the refusal (WinRM path present)",
          (root / "winrm.log").exists(), True)


# === The COMPLETE hook, as a subprocess: end-to-end success, with the
# FAKE build script set by default by appeler() =====================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir)
    check("complete hook: end-to-end success (rc == 0)", r.returncode, 0)
    if r.returncode != 0:
        failures.append(f"hook stderr: {r.stderr!r}")
    packages_dir_defaut = root / "fake-packages-dir"
    cible = (packages_dir_defaut / "console" / "guest" / "payload" / "agent"
             / "agent.exe")
    check("agent.exe is indeed dropped at the expected path", cible.is_file(), True)


if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - agent.exe drop tests (task 7) passed")
