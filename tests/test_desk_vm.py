#!/usr/bin/env python3
"""Tests of the `hooks/vm.py` module: what `desk` sets up INSIDE the Windows VM,
through `console`'s WinRM path (ProjFS, VB-Audio).

🔴 NO REAL WINRM CALL IS MADE HERE, AND NO PACKET LEAVES THIS
MACHINE. Most behaviour tests inject a FAKE executor
(`faux_winrm_rendant`), which short-circuits every subprocess.

⚠️ BUT "NO `subprocess.run` IS REACHED" WOULD BE FALSE, and this header
said so until 30 August 2026 (spotted by the final branch review): the
scenarios A and B at the bottom of this file DELIBERATELY go through
`executer_winrm_reel`, hence through a real `subprocess.run`, to exercise its
failure arm and its round trip — two paths no fake executor
can cover. What they launch is a FAKE four-line `winrm_exec.py`,
laid down under a temporary `NIVUUS_PACKAGES_DIR`: the subprocess is real,
the VM and the network never are.

`hooks/vm.py` is not a hook runnable on its own (no `--phase`/stdin
JSON): it is a module imported by `hooks/activate.py`, just like
`hooks/commun.py`. It is therefore loaded here by explicit file path
(`importlib`), as the RED of `test_desk_activate.py` already does to
call an internal function of the hook directly.

Run: python3 tests/test_desk_vm.py
"""
import importlib.util
import os
import pathlib
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
MODULE_PATH = RACINE / "hooks" / "vm.py"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


def charger_module():
    spec = importlib.util.spec_from_file_location("desk_vm", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


vm = charger_module()


# --- The fake executor: records each command, never touches the
# VM nor the network -----------------------------------------------------------

class FauxWinRM:
    def __init__(self, sortie=""):
        self.sortie = sortie
        self.commandes = []

    def __call__(self, mode, commande):
        self.commandes.append(commande)
        return self.sortie


def faux_winrm_rendant(sortie=""):
    return FauxWinRM(sortie)


# === ProjFS: the reboot is NOTED and SAID, it is not taken =====

# --- Arm 1: the VM answers that a reboot is required --------------------
faux = faux_winrm_rendant("RestartNeeded")
etat = vm.poser_projfs(executer=faux)
check("the required reboot is reported", etat.redemarrage_requis, True)
check("no reboot was requested",
      any("Restart-Computer" in c for c in faux.commandes), False)
check("a single command reached the executor", len(faux.commandes), 1)
check("the command does enable Client-ProjFS",
      "Client-ProjFS" in faux.commandes[0], True)
check("the command carries -NoRestart (the reboot is NEVER taken)",
      "-NoRestart" in faux.commandes[0], True)

# --- Arm 2: the VM answers that no reboot is required ---------------
faux2 = faux_winrm_rendant("")
etat2 = vm.poser_projfs(executer=faux2)
check("no reboot required when the VM does not report one",
      etat2.redemarrage_requis, False)
check("no reboot was requested (arm 2 either)",
      any("Restart-Computer" in c for c in faux2.commandes), False)


# === VB-Audio: PERSONAL licence only ==============================

# --- Disarmed (the wizard default): NOTHING goes to the VM ---------------
faux3 = faux_winrm_rendant("")
vm.poser_vb_audio(armee=False, executer=faux3)
check("disarmed: no command reaches the VM", faux3.commandes, [])

# --- Disarmed by default (without passing `armee` at all) ----------------------
faux4 = faux_winrm_rendant("")
vm.poser_vb_audio(executer=faux4)
check("disarmed by default (armee omitted): no command reaches the VM",
      faux4.commandes, [])

# --- Armed: no payload exists (see hooks/vm.py::poser_vb_audio) — the
# batch raises LOUDLY rather than guessing an installer that does not exist.
# Still NO command must reach the executor before this refusal. ---
faux5 = faux_winrm_rendant("")
a_leve_vb = False
try:
    vm.poser_vb_audio(armee=True, executer=faux5)
except NotImplementedError:
    a_leve_vb = True
check("armed without payload: raises loudly rather than inventing an installer",
      a_leve_vb, True)
check("armed without payload: no command still reached the VM",
      faux5.commandes, [])


# === The RED — Step 5: the inter-package contract must be seen missing ====
# A `NIVUUS_PACKAGES_DIR` that does NOT carry `console/guest/winrm_exec.py`
# must make `poser_projfs()` raise (real executor, not injected) with an
# exception NAMING the searched path — never a silent network call,
# never a generic Python traceback. It is the arm that tells "console
# absent" apart from "nothing to do".
ancien_packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR")
try:
    with tempfile.TemporaryDirectory() as tmp:
        os.environ["NIVUUS_PACKAGES_DIR"] = tmp
        chemin_attendu = str(pathlib.Path(tmp) / "console" / "guest" / "winrm_exec.py")

        a_leve = False
        message = ""
        try:
            vm.poser_projfs()  # executer=None: goes through the REAL executor
        except FileNotFoundError as exc:
            a_leve = True
            message = str(exc)
        check("RED: winrm_exec.py absent makes poser_projfs() raise",
              a_leve, True)
        check("RED: the exception names the searched path",
              chemin_attendu in message, True)
        print(f"RED (real output): {message}")
finally:
    if ancien_packages_dir is None:
        os.environ.pop("NIVUUS_PACKAGES_DIR", None)
    else:
        os.environ["NIVUUS_PACKAGES_DIR"] = ancien_packages_dir


# --- Positive control of the resolution: winrm_exec.py PRESENT under
# NIVUUS_PACKAGES_DIR is found without raising, at the exact location --------
ancien_packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR")
try:
    with tempfile.TemporaryDirectory() as tmp:
        guest_dir = pathlib.Path(tmp) / "console" / "guest"
        guest_dir.mkdir(parents=True)
        (guest_dir / "winrm_exec.py").write_text("# fake\n", encoding="utf-8")
        os.environ["NIVUUS_PACKAGES_DIR"] = tmp

        chemin = vm.chemin_winrm_exec()
        check("a present winrm_exec.py is resolved without raising",
              str(chemin), str(guest_dir / "winrm_exec.py"))
finally:
    if ancien_packages_dir is None:
        os.environ.pop("NIVUUS_PACKAGES_DIR", None)
    else:
        os.environ["NIVUUS_PACKAGES_DIR"] = ancien_packages_dir


# === Correction round 1: the REAL failure arm of `executer_winrm_reel` =
# 🔴 Until then, NO test went through the real subprocess: all the
# behaviour tests above inject `FauxWinRM`, which short-circuits
# `executer_winrm_reel` (and hence its `subprocess.run`) entirely. The
# `if proc.returncode != 0: raise RuntimeError(...)` arm of `hooks/vm.py` was
# therefore exercised by NOTHING — a check never seen red is not a
# check. The two scenarios below lay down a REAL
# `console/guest/winrm_exec.py` file (a standalone Python script, never the VM) and
# call `poser_projfs()` WITHOUT an injected executor (`executer=None`), so
# that `executer_winrm_reel` — hence `subprocess.run` — is really exercised.

def poser_script_winrm_reel(packages_dir: pathlib.Path, corps: str) -> None:
    """A REAL `console/guest/winrm_exec.py` file (never the VM): a
    standalone Python script whose `corps` is the whole content."""
    guest_dir = packages_dir / "console" / "guest"
    guest_dir.mkdir(parents=True, exist_ok=True)
    script = guest_dir / "winrm_exec.py"
    script.write_text(corps, encoding="utf-8")
    script.chmod(0o755)


# --- A: the remote command FAILS (non-zero exit code, message on
# stderr) — `executer_winrm_reel` must raise `RuntimeError`, and the MESSAGE
# of that exception must carry the remote reason, not merely raise. --
ancien_packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR")
try:
    with tempfile.TemporaryDirectory() as tmp:
        packages_dir = pathlib.Path(tmp)
        poser_script_winrm_reel(packages_dir, (
            "#!/usr/bin/env python3\n"
            "import sys\n"
            # ⚠️ THE TEXT SAYS IT IS FAKE, AND THAT IS DELIBERATE: the
            # previous version printed "error: cannot reach guest at
            # 192.168.3.2:5985: timeout", which `make test` displayed as is
            # in NORMAL output — the reviewer of the final review
            # believed, reading it, that the suite had touched the VM. A bench
            # text that cannot be mistaken for a real failure costs
            # one line.
            "sys.stderr.write('FAKE test winrm_exec.py: simulated failure, "
            "no VM contacted\\n')\n"
            "sys.exit(1)\n"
        ))
        os.environ["NIVUUS_PACKAGES_DIR"] = str(packages_dir)

        a_leve = False
        message = ""
        try:
            vm.poser_projfs()  # executer=None: goes through the REAL executer_winrm_reel
        except RuntimeError as exc:
            a_leve = True
            message = str(exc)
        check("real executor, failure: RuntimeError raised", a_leve, True)
        check("real executor, failure: the message names the mode and the code",
              "winrm_exec.py ps failed (code 1)" in message, True)
        check("real executor, failure: the message carries the remote reason",
              "simulated failure" in message, True)
        print(f"RED (real executor, failure): {message}")
finally:
    if ancien_packages_dir is None:
        os.environ.pop("NIVUUS_PACKAGES_DIR", None)
    else:
        os.environ["NIVUUS_PACKAGES_DIR"] = ancien_packages_dir


# --- B (symmetric, promoted from Minor): the remote command SUCCEEDS
# emitting `RestartNeeded` — the real round trip stdout -> redemarrage_requis
# is thus covered directly, not only indirectly through FauxWinRM. --
ancien_packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR")
try:
    with tempfile.TemporaryDirectory() as tmp:
        packages_dir = pathlib.Path(tmp)
        poser_script_winrm_reel(packages_dir, (
            "#!/usr/bin/env python3\n"
            "import sys\n"
            "sys.stdout.write('RestartNeeded\\n')\n"
            "sys.exit(0)\n"
        ))
        os.environ["NIVUUS_PACKAGES_DIR"] = str(packages_dir)

        etat = vm.poser_projfs()  # executer=None: goes through the REAL executer_winrm_reel
        check("real executor, success: round trip stdout -> redemarrage_requis",
              etat.redemarrage_requis, True)
finally:
    if ancien_packages_dir is None:
        os.environ.pop("NIVUUS_PACKAGES_DIR", None)
    else:
        os.environ["NIVUUS_PACKAGES_DIR"] = ancien_packages_dir


if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - vm module tests passed")
