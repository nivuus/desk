#!/usr/bin/env python3
"""Reading and extending `etc/nivuus/desk.env`, the environment
file that `hooks/install.py` lays down and that `hooks/activate.py`
completes (`AGENT_VM`, `AGENT_SECRET`) once enrolment has succeeded.

Extracted from `hooks/activate.py` by task 7 (2026-08-29), for the SAME
reason as `hooks/administration.py` (see its own head docstring):
making room, in a SEPARATE commit and WITHOUT any behaviour change,
before the task adds the drop of `agent.exe` for
`console`. Imported like `vm.py` and `administration.py` — Python adds the
directory of the LAUNCHED script to `sys.path`, no manipulation needed.
"""
import os
import pathlib
import stat

# The variables `activate` ADDS to the file `install` wrote: its proof that
# the agent is enrolled (its guard against re-enrolling - and re-creating the
# administrator account - on a replay). `install` must carry them over when
# it rewrites the file at an update, or a replay loses that proof, re-runs
# the enrolment, mints a new AGENT_SECRET the agent in the VM does not hold
# and trips on the account that already exists (measured 2026-10-03, first
# `nivuus update desk`: "UNIQUE constraint failed: utilisateur.email").  # policy: allow-fr - real table name
ACTIVATE_KEYS = ("AGENT_VM", "AGENT_SECRET")


def activate_variables(chemin: pathlib.Path) -> dict:
    """The ACTIVATE_KEYS present in `chemin`, `{}` when none or no file."""
    present = read_env_file(chemin)
    return {cle: present[cle] for cle in ACTIVATE_KEYS if present.get(cle)}



def read_env_file(chemin: pathlib.Path) -> dict:
    """Parses `chemin` as KEY=VALUE, one per line. `{}` if the file does not
    exist — duplicated from the task 4 test parser rather than imported,
    same doctrine as the rest of this package: each hook stays
    runnable on its own.
    """
    values = {}
    if not chemin.is_file():
        return values
    for ligne in chemin.read_text(encoding="utf-8").splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.startswith("#") or "=" not in ligne:
            continue
        cle, _, value = ligne.partition("=")
        values[cle] = value
    return values


def add_env_variables(chemin: pathlib.Path, nouvelles: dict) -> None:
    """Appends `KEY=VALUE` lines at the end of an environment file
    already in place, WITHOUT touching the lines already there, and closes the
    file back in mode 600 — the same mode `install.py::write_env` gave
    it, and which it must keep: this file holds secrets.

    🔴 PRECONDITION, REQUIRED FROM THE CALLER: `chemin` MUST ALREADY EXIST
    (`hooks/activate.py::main()` refuses if `desk.env` is missing — see its
    comment). `write_text()` on an EXISTING file reuses the mode
    already in place, it does not recreate it with the process umask; it is
    ONLY on a NEW file that this pattern would open the
    write-then-`chmod` window that `install.py::write_env` removed (commit
    `92cacfb`) by switching to `os.open(..., 0o600)`. This function never
    recreates this file from nothing — it is the guard in `main()`, not
    this function, that makes this precondition true.
    """
    corps = chemin.read_text(encoding="utf-8") if chemin.is_file() else ""
    if corps and not corps.endswith("\n"):
        corps += "\n"
    corps += "".join(f"{cle}={value}\n" for cle, value in nouvelles.items())
    chemin.write_text(corps, encoding="utf-8")
    os.chmod(chemin, stat.S_IRUSR | stat.S_IWUSR)  # 0o600, redundant if the
    # precondition already holds — but free, and a second line of
    # defence costs nothing.
