#!/usr/bin/env python3
"""What `desk` sets up INSIDE the Windows VM, through `console`'s WinRM path.

The target VM is no longer a development machine: it is an appliance
provisioned by the sibling package `console`
(`/home/mallanic/Projects/Nivuus/packages/installer/console/`). Two
dependencies of `desk` are absent there by `console`'s choice — ProjFS
(disabled) and VB-Audio (never installed, personal licence) — and the decision
of the repository owner is that `desk` sets them up ITSELF, through the
WinRM path that `console` already exposes, consistent with the principle that `console`
states for its own dependencies: "a package carries its dependencies".
Legitimate because the manifest declares `requires: packages: [console]`
(`nivuus-package.yaml`).

🔴 THIS MODULE WRITES NOTHING TO THE VM WHEN IT IS IMPORTED OR TESTED. The two
public functions (`poser_projfs`, `poser_vb_audio`) take an
`executer` parameter: in tests, a FAKE executor; in production,
`executer_winrm_reel` by default, which really invokes `winrm_exec.py`.

⚠️ THIS PARAGRAPH SAID SOMETHING WRONG UNTIL 30 AUGUST 2026: "[this
file] never injects anything but a fake executor, except for ITS
OWN RED, which never reaches the network anyway". That has been wrong
since round 1 of task 6 — `tests/test_desk_vm.py` carries TWO
scenarios (A and B) that deliberately go through `executer_winrm_reel`, hence
through a real `subprocess.run`, to exercise its failure arm and its
round trip. What they run is a FAKE four-line `winrm_exec.py`
laid down under a temporary `NIVUUS_PACKAGES_DIR`: the subprocess is real,
the VM and the network never are. The fix was right; this docstring
had not followed it (spotted by the final branch review).

--- How the path of `winrm_exec.py` is learnt (never assumed) -------

Established by READING two files of the sibling repository `installer/` (none was
guessed):

  - `installer/installer/packages/discovery.py:22` —
    `PACKAGES_DIR = os.environ.get("NIVUUS_PACKAGES_DIR", "/opt/nivuus-packages")`
    — it is the variable through which the ENGINE itself discovers the
    manifests of installed packages; `apply_packages()` copies each
    selected package, `console` included, under
    `<PACKAGES_DIR>/<package_name>/`, once and for all, at
    installation, and never removes it (see
    `installer/installer/README.md:57,156` and
    `installer/docs/superpowers/specs/2026-08-27-decoupage-installer-console-design.md:268`).
  - `installer/console/host/guest-ready-watch.py:113,155-162` — a script
    of the SAME sibling repository, facing the SAME need (calling a tool of
    `console` from another point of the system, once `console` is
    installed), ALREADY resolves
    `WINRM_EXEC = "/opt/nivuus-packages/console/guest/winrm_exec.py"` through
    this path — and its own comment says why: "`apply_packages()`
    copies the whole package tree there once, at install time, and never
    removes it — so [this file] is reliably at this path on any machine
    where this script itself is running".

This module TAKES UP that convention rather than inventing a new one:
`NIVUUS_PACKAGES_DIR` (default `/opt/nivuus-packages`), sub-path
`console/guest/winrm_exec.py`. On the machine running these tests,
`/opt/nivuus-packages` does not exist (confirmed: `ls /opt/nivuus-packages`
returns "No such file or directory") — the real development
file lives under
`/home/mallanic/Projects/Nivuus/packages/installer/console/guest/winrm_exec.py`,
an entirely different tree. That is why `NIVUUS_PACKAGES_DIR` must
be overridable: the tests do it, never the production code, which
needs no different default since at real installation the
default `/opt/nivuus-packages` will be correct.

--- The exact contract of `winrm_exec.py`, read in full -----------------

`installer/console/guest/winrm_exec.py`: `Usage: winrm_exec.py {cmd|ps}
<command...>`, `pywinrm` transport over NTLM (Basic is refused by the
guest), password read from `GUEST_PASS_FILE` (never argv). The exit code
of the process is that of the remote command
(`result.status_code`), standard output is printed as is.
"""
import os
import pathlib
import subprocess
import sys
from dataclasses import dataclass

# --- Where winrm_exec.py lives, resolved never assumed ----------------------

NIVUUS_PACKAGES_DIR_DEFAUT = "/opt/nivuus-packages"
WINRM_EXEC_RELATIF = pathlib.Path("console") / "guest" / "winrm_exec.py"


def chemin_winrm_exec() -> pathlib.Path:
    """Resolves the path of `winrm_exec.py`, without ever guessing it.

    Reads `NIVUUS_PACKAGES_DIR` (default `/opt/nivuus-packages` — see the
    head docstring for where this convention comes from), targets
    `<packages_dir>/console/guest/winrm_exec.py`, and RAISES
    `FileNotFoundError`, NAMING the searched path, if this file does not
    exist: a package that fails silently on an inter-package contract
    cannot be told apart from a package that has nothing to do.
    """
    packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR", NIVUUS_PACKAGES_DIR_DEFAUT)
    chemin = pathlib.Path(packages_dir) / WINRM_EXEC_RELATIF
    if not chemin.is_file():
        raise FileNotFoundError(
            f"winrm_exec.py not found at the expected location: {chemin} "
            "(is the console package installed? NIVUUS_PACKAGES_DIR="
            f"{packages_dir!r} - see hooks/vm.py for the convention)"
        )
    return chemin


def executer_winrm_reel(mode: str, commande: str) -> str:
    """The REAL executor — never used by `tests/test_desk_vm.py`, except
    for its own RED, which misses before any network access (see
    above).

    Resolves `winrm_exec.py` (which raises BEFORE any network call if
    `console` is not installed), then invokes
    `winrm_exec.py <mode> <command>` as a subprocess. Returns the standard
    output, stripped of its edge whitespace; raises `RuntimeError` on
    a transport failure or a remote command in error (the password
    file missing, the VM unreachable, `RestartNeeded` not
    readable...).
    """
    chemin = chemin_winrm_exec()
    proc = subprocess.run(
        [sys.executable, str(chemin), mode, commande],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(
            f"winrm_exec.py {mode} failed (code {proc.returncode}): "
            f"{detail or 'no output'}"
        )
    return proc.stdout.strip()


# --- ProjFS: the reboot is NOTED and SAID, it is not taken ----------------

@dataclass(frozen=True)
class EtatProjFS:
    """What `poser_projfs` reports to the caller (`activate.py`)."""
    redemarrage_requis: bool


# The ProjFS feature REQUIRES a reboot to become active, but
# `-NoRestart` prevents `Enable-WindowsOptionalFeature` from TAKING it
# itself: rebooting an operator's VM without asking is a side
# effect no installation may take (see the head
# docstring). The line `Write-Output 'RestartNeeded'` is emitted ONLY if the
# `RestartNeeded` property of the returned object is true — it is this token,
# and it alone, that `poser_projfs` looks for in the output: an empty output
# means "no reboot required", never an ambiguous answer to
# interpret.
COMMANDE_PROJFS = (
    "$r = Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS "
    "-NoRestart; if ($r.RestartNeeded) { Write-Output 'RestartNeeded' }"
)


def poser_projfs(executer=None) -> EtatProjFS:
    """Enables the Client-ProjFS optional feature in the VM, WITHOUT
    rebooting (`-NoRestart`), and REPORTS whether a reboot is required —
    it never triggers it (see the head docstring).

    `executer(mode, commande) -> str`: in tests, a fake executor
    (`faux_winrm_rendant`, see `tests/test_desk_vm.py`); by default,
    `executer_winrm_reel`, which really invokes `winrm_exec.py` — hence
    never exercised by this module as long as a fake `executer` is supplied.
    """
    executer = executer or executer_winrm_reel
    sortie = executer("ps", COMMANDE_PROJFS)
    return EtatProjFS(redemarrage_requis="RestartNeeded" in sortie)


# --- VB-Audio: PERSONAL licence only -----------------------------------

def poser_vb_audio(armee: bool = False, executer=None) -> None:
    """Installs VB-Audio in the VM IF `armee` (the wizard default, `vb_audio`
    in `wizard.yaml`, is false).

    🔴 DISARMED (the nominal case), NOTHING GOES TO THE VM — not only
    "nothing gets installed": `executer` is NEVER invoked, and
    `tests/test_desk_vm.py` proves that the list of commands seen by
    the fake executor stays empty, not that an installation was skipped.

    ⚠️ ARMED, THIS BATCH LAYS DOWN NO VB-AUDIO PAYLOAD — this is not an
    oversight: no task of this batch (see the task index,
    `.superpowers/sdd/2026-08-29-package-nivuus/task-*-brief.md`)
    drops a VB-Audio driver into the tree that `console` builds
    (`console/guest/fetch_payload.py`: `agent/`, `virtio/`, `steam/`,
    `sudovda/`, `winfsp/`, `apollo/`, `nvidia/` — NO
    `vb-audio` entry), and no silent installer path is documented
    anywhere in this repository nor in `installer/`. Making up an
    install command here would be guessing a contract that does not exist — the
    explicit instruction of this task is to never guess such a
    contract (see the path of `winrm_exec.py` above, handled the
    same way). Raising HERE, loud and named, is the choice consistent with the
    rest of this repository: a mechanism that would fail silently on an
    explicit operator request (`vb_audio: true`) could not be told apart
    from a mechanism that has nothing to do. It is an open point, to be settled
    by the repository owner when a VB-Audio payload exists — see
    the report of this task, § Reservations.
    """
    if not armee:
        return
    raise NotImplementedError(
        "vb_audio is armed (vb_audio=true) but no VB-Audio payload "
        "exists yet in the tree that console builds "
        "(fetch_payload.py): no task of this batch drops one, and "
        "making up an installer path would be guessing a contract that "
        "does not exist. See hooks/vm.py::poser_vb_audio for the detail."
    )
