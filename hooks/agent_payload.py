#!/usr/bin/env python3
"""Where `console` looks for `agent.exe`, never guessed: cross build
and drop of the binary, task 7 (2026-08-29).

Extracted from `hooks/activate.py` by task 13 (2026-08-29), for the SAME
reason as `hooks/administration.py` and `hooks/env_file.py` (see their
own head docstrings): `activate.py` had reached 509 lines after
adding the assignment of the VM to the administrator account (task 13,
a gap found in production), and this repository forbids compressing code to avoid
an extraction ("this repository paid for it twelve times"). This module therefore
carries, VERBATIM, what task 7 had written: the same body, the same docstring,
only moved in a DEDICATED commit, BEFORE the one that adds the assignment.

Like `hooks/vm.py`, `hooks/administration.py` and `hooks/env_file.py`, this
module IS NOT a hook runnable on its own (no `--phase`/stdin JSON):
`hooks/activate.py` imports it (`from agent_payload import chemin_agent_console,
construire_agent_reel, deposer_agent_console`) — Python automatically adds
the directory of the LAUNCHED script (`hooks/`) to `sys.path`, so this import
resolves without any extra manipulation, neither here nor in the caller. `tests/
test_desk_payload.py` still reaches them through `activate.chemin_agent_console`
etc.: a name imported BY NAME into `activate.py` becomes an attribute of the
`activate` module just like a locally defined name — no test change
was needed for this extraction.

🔴 Not `<drivers_dir>/agent/agent.exe` (derived from `guest_workdir`, an answer OF
`console` that this hook never receives — `activate_cli.py:107-108` only indexes
the answers of the calling package; `/etc/nivuus/packages.json` is
absent on this machine anyway). The REAL, fixed path that `console` reads
itself: `console/guest/fetch_payload.py:61` —
`PACKAGED_AGENT_EXE = Path(__file__).resolve().parent/"payload"/"agent"
/"agent.exe"`, a VENDORED file (see `console/guest/payload/agent/
README.md`) that `install_packaged_agent()` copies to `<drivers_dir>/
agent/agent.exe` on every `fetch_payload.py`. This hook refreshes THAT
source, found under `<NIVUUS_PACKAGES_DIR>/console/` (same convention
as `hooks/vm.py::chemin_winrm_exec`, whose constant is imported here).
⚠️ CAVEAT: `console` activates BEFORE `desk` (dependency) — the VERY
FIRST provisioning consumes the image already committed; this drop only
guarantees FUTURE rebuilds. See the report of task 7,
§ Caveats.
"""
import os
import pathlib
import subprocess

from vm import NIVUUS_PACKAGES_DIR_DEFAUT

RACINE = pathlib.Path(__file__).resolve().parents[1]

AGENT_EXE_CONSOLE_RELATIF = pathlib.Path("console") / "guest" / "payload" / "agent" / "agent.exe"
FETCH_PAYLOAD_RELATIF = pathlib.Path("console") / "guest" / "fetch_payload.py"


def chemin_agent_console() -> pathlib.Path:
    """Targets `<NIVUUS_PACKAGES_DIR>/console/guest/payload/agent/agent.exe`
    (reread by `fetch_payload.py:61`); raises `FileNotFoundError`, naming the
    path, if `console/guest/fetch_payload.py` is absent — a silent
    failure would be indistinguishable from a no-op (doctrine of
    `hooks/vm.py::chemin_winrm_exec`)."""
    packages_dir = os.environ.get("NIVUUS_PACKAGES_DIR", NIVUUS_PACKAGES_DIR_DEFAUT)
    fetch_payload = pathlib.Path(packages_dir) / FETCH_PAYLOAD_RELATIF
    if not fetch_payload.is_file():
        raise FileNotFoundError(
            f"fetch_payload.py not found at the expected location: {fetch_payload} "
            "(is the console package installed? NIVUUS_PACKAGES_DIR="
            f"{packages_dir!r} - see hooks/activate.py for the convention)"
        )
    return pathlib.Path(packages_dir) / AGENT_EXE_CONSOLE_RELATIF


def construire_agent_reel(destination: pathlib.Path) -> None:
    """Invokes `scripts/build-agent-croise.sh <destination.parent>` (a
    DIRECTORY, never a file name); raises `RuntimeError` (the script's
    output) on a non-zero code. Path overridable through
    `DESK_BUILD_AGENT_SCRIPT` (never in production) so that tests never
    compile the real agent (~40 s) — see `tests/test_desk_payload.py`.
    """
    defaut = RACINE / "scripts" / "build-agent-croise.sh"
    script = pathlib.Path(os.environ.get("DESK_BUILD_AGENT_SCRIPT") or defaut)
    proc = subprocess.run([str(script), str(destination.parent)],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(f"{script} failed (code {proc.returncode}): {detail or 'no output'}")


def deposer_agent_console(construire=None) -> pathlib.Path:
    """Builds `agent.exe` and drops it where `console` looks for it.
    `construire`: fake in tests, `construire_agent_reel` by default.
    🔴 Checks AFTERWARDS that the file exists (raises otherwise): a mute
    `construire` would be a silent success ("a check never
    seen red is not a check", paid for on `AUDIO_FAUTE_LECTURE`).
    """
    cible = chemin_agent_console()
    construire = construire or construire_agent_reel
    cible.parent.mkdir(parents=True, exist_ok=True)
    construire(cible)
    if not cible.is_file():
        raise RuntimeError(f"construire() returned but {cible} is absent: no agent.exe was dropped")
    return cible
