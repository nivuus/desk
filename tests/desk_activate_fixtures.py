#!/usr/bin/env python3
"""Fixtures shared by `tests/test_desk_activate.py`.

Extracted from the latter in correction round 1 on task 6: the review
noted that restoring the blank lines and the prose that a first
stylistic squeeze had cost took the test file over 500 lines —
and this repository forbids compressing code to avoid an extraction
("this repository paid for it twelve times"). This module carries the FIXTURES (fake
npm, fake systemctl, fake winrm_exec.py, installed root, calling the
hook); `test_desk_activate.py` carries the SCENARIOS and the assertions.
No fixture here has any value on its own: it is only exercised through the
scenarios that import it.

⚠️ This file is NOT named `test_*.py`: it is not a test suite
on its own, `python3 tests/desk_activate_fixtures.py` does nothing
useful — it is an imported module, never run directly.
"""
import configparser
import json
import os
import pathlib
import stat
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "activate.py"

REPONSES = {"admin_email": "ada@exemple.test", "admin_password": "hunter2hunter2",
            "auth_mode": "motdepasse", "vb_audio": False}

# The facts of resolve, as they REALLY arrive: merged
# into hw (installer/packages/runner.py::run_activate, line 337), never
# under a separate "facts" key.
# ⚠️ THE SIX KEYS, NOT FIVE (final branch review, 30 August 2026):
# `hote` and `proxy_confiance` entered the `facts` contract in batch
# 10A and this fixture had not followed them. No consequence — `activate`
# only reads `port` — but a fixture that describes an outdated contract ends up
# making a test pass for a shape the engine never sends.
# 🔴 `vm_repond` HAS DISAPPEARED FROM THE CONTRACT (same review): it was not a
# measured fact but a literal (`True` hardcoded in `hooks/resolve.py`), at a
# phase that can know nothing about the VM — see `hooks/activate.py` (head)
# and `tests/test_desk_contrat_hw.py` (guard ⑤, which refuses any return of
# this defect).
HW_WITH_FACTS = {"node_version": "24.9.0",
                  "turn_ecoute": "203.0.113.9", "turn_relais": "203.0.113.9",
                  "hote": "198.51.100.1", "proxy_confiance": "198.51.100.1",
                  "port": 9999}


# --- The fake npm: records each invocation, never touches a real
# Node nor a real database ---------------------------------------------------

FAUX_NPM = """#!/usr/bin/env python3
import json, os, sys

argv = sys.argv[1:]
stdin_data = sys.stdin.read()
with open({log!r}, "a", encoding="utf-8") as fh:
    fh.write(json.dumps({{"argv": ["npm", *argv], "cwd": os.getcwd(),
                          "stdin": stdin_data,
                          "env_base_url": os.environ.get("PLATEFORME_BASE_URL", ""),
                          }}) + "\\n")

if "admin:utilisateur" in argv:
    sys.stdout.write("u-test-0001\\n")
    sys.exit(0)
if "admin:agent" in argv:
    sys.stdout.write("vm_id=vm-test-uuid\\nprefixe=abcd\\n"
                      "AGENT_SECRET=test-secret-0123456789abcdef\\n")
    sys.exit(0)
if "admin:attribuer" in argv:
    # Failure arm (task 13): simulates the real refusal of attribuer-vm.ts
    # ('vm-deja-attribuee'), without ever touching a real database.
    if os.environ.get("FAUX_NPM_ATTRIBUER_ECHEC"):
        sys.stderr.write("the VM vm-test-uuid (windows) already belongs to "
                          "someone else - detach it first\\n")
        sys.exit(2)
    sys.stdout.write("vm=vm-test-uuid\\nnom=windows\\n"
                      "utilisateur=u-test-0001\\nemail=ada@exemple.test\\n")
    sys.exit(0)
sys.exit(1)
"""


def poser_faux_npm(bin_dir: pathlib.Path, log: pathlib.Path) -> None:
    script = bin_dir / "npm"
    script.write_text(FAUX_NPM.format(log=str(log)), encoding="utf-8")
    script.chmod(0o755)


# --- The fake winrm_exec.py (task 6): records each invocation,
# never reaches the VM nor the network. Exact contract of the real
# `console/guest/winrm_exec.py` (read in full, task 6): called
# `<script> {mode} <command>`, it prints its answer on stdout. Here,
# no answer: the empty output is enough for ProjFS to declare that no
# reboot is required, which is already exercised in detail by
# tests/test_desk_vm.py.
FAUX_WINRM_EXEC = """#!/usr/bin/env python3
import json, sys
with open({log!r}, "a", encoding="utf-8") as fh:
    fh.write(json.dumps({{"argv": sys.argv[1:]}}) + "\\n")
sys.exit(0)
"""


def poser_faux_winrm_exec(packages_dir: pathlib.Path, log: pathlib.Path) -> None:
    guest_dir = packages_dir / "console" / "guest"
    guest_dir.mkdir(parents=True, exist_ok=True)
    script = guest_dir / "winrm_exec.py"
    script.write_text(FAUX_WINRM_EXEC.format(log=str(log)), encoding="utf-8")
    script.chmod(0o755)


# --- The fake fetch_payload.py (task 7): a mere presence MARKER.
# `activate.py::chemin_agent_console()` only tests its existence
# (`is_file()`) — never runs it — so an empty content is enough to prove
# that "console is installed" without running anything of the real file.
def poser_faux_fetch_payload(packages_dir: pathlib.Path) -> None:
    guest_dir = packages_dir / "console" / "guest"
    guest_dir.mkdir(parents=True, exist_ok=True)
    (guest_dir / "fetch_payload.py").write_text("", encoding="utf-8")


# --- The fake scripts/build-agent-croise.sh (task 7): never the real one (it
# would build the real agent, ~40 s, cross toolchain). Exact contract of the
# real script (read in full): called `<script> <destination_dir>`, it
# drops `agent.exe` there itself. Here, a fixed FAKE content is enough: no
# acceptance of this batch judges the binary produced, only its LOCATION.
FAUX_BUILD_AGENT = """#!/usr/bin/env python3
import pathlib, sys
d = pathlib.Path(sys.argv[1])
d.mkdir(parents=True, exist_ok=True)
(d / "agent.exe").write_bytes(b"fake-test-agent-exe")
"""


def poser_faux_build_agent(chemin: pathlib.Path) -> None:
    chemin.write_text(FAUX_BUILD_AGENT, encoding="utf-8")
    chemin.chmod(0o755)


def poser_faux_systemctl(bin_dir: pathlib.Path, log: pathlib.Path) -> None:
    """A systemctl that acts on NOTHING: just a trace of its arguments,
    to prove it is never invoked under a test --root."""
    script = bin_dir / "systemctl"
    script.write_text(
        "#!/bin/sh\n"
        f'echo "$@" >> {log}\n'
        "exit 0\n",
        encoding="utf-8",
    )
    script.chmod(0o755)


def lire_commandes(log: pathlib.Path):
    if not log.exists():
        return []
    commandes = []
    for ligne in log.read_text(encoding="utf-8").splitlines():
        if ligne.strip():
            commandes.append(json.loads(ligne))
    return commandes


# --- Builds a root where `install` would already have run -------------------

def poser_racine_installee(root: pathlib.Path, contenu_env: dict = None) -> None:
    unite_dir = root / "etc" / "systemd" / "system"
    unite_dir.mkdir(parents=True, exist_ok=True)
    # Copies the REAL unit of task 4 — it is its WantedBy= that
    # decides under which .wants/ the link must live.
    (unite_dir / "desk-plateforme.service").write_text(
        (RACINE / "hooks" / "assets" / "desk-plateforme.service").read_text(encoding="utf-8"),
        encoding="utf-8",
    )

    plateforme_dir = root / "opt" / "nivuus" / "desk" / "plateforme"
    plateforme_dir.mkdir(parents=True, exist_ok=True)

    env_dir = root / "etc" / "nivuus"
    env_dir.mkdir(parents=True, exist_ok=True)
    values = contenu_env if contenu_env is not None else {
        "PLATEFORME_BASE": "sqlite",
        "PLATEFORME_BASE_URL": "/var/lib/nivuus-desk/plateforme.sqlite",
        "PLATEFORME_SECRET_JETON": "x" * 64,
    }
    corps = "\n".join(f"{k}={v}" for k, v in values.items()) + "\n"
    chemin_env = env_dir / "desk.env"
    chemin_env.write_text(corps, encoding="utf-8")
    os.chmod(chemin_env, stat.S_IRUSR | stat.S_IWUSR)


def appeler(root, bin_dir, hw=None, answers=None, root_arg=None, packages_dir=None):
    """Calls the hook like the engine (--phase, --root), PATH rewritten with
    bin_dir at its head, so that the npm/systemctl resolved are the fakes.

    `packages_dir` (task 6): where to point `NIVUUS_PACKAGES_DIR` for the
    resolution of `winrm_exec.py` AND, since task 7, of
    `fetch_payload.py`. By default (`None`), a WORKING fake
    `console/guest/winrm_exec.py` and a fake
    `console/guest/fetch_payload.py` (a mere marker) are built under
    `root` itself — otherwise every scenario that has nothing to do with
    ProjFS/VB-Audio/the agent.exe drop would fail on "console absent"
    before reaching the reason it really wants to exercise.
    `packages_dir=False` simulates `console` ABSENT (no file is created,
    for the dedicated scenarios).

    🔴 TASK 7 — `DESK_BUILD_AGENT_SCRIPT` is ALWAYS set (whatever
    `packages_dir` is) to a FAKE script that builds nothing: without it,
    `deposer_agent_console()`, called unconditionally by `main()`,
    would invoke the REAL `scripts/build-agent-croise.sh` in EVERY scenario
    of this file — the real build the task forbids in the
    tests.
    """
    contexte = {"hw": hw if hw is not None else HW_WITH_FACTS,
                "answers": answers if answers is not None else REPONSES}
    env = dict(os.environ)
    env["PATH"] = str(bin_dir) + os.pathsep + env.get("PATH", "")
    # Check ② wants to prove that PLATEFORME_BASE_URL reaches npm THROUGH THE
    # MERGE the hook performs from desk.env — never because the shell
    # running these tests happened to export it already.
    env.pop("PLATEFORME_BASE_URL", None)
    if packages_dir is None:
        packages_dir = root / "fake-packages-dir"
        poser_faux_winrm_exec(packages_dir, root / "winrm.log")
        poser_faux_fetch_payload(packages_dir)
    elif packages_dir is False:
        packages_dir = root / "console-absent-here"
    env["NIVUUS_PACKAGES_DIR"] = str(packages_dir)
    faux_build = root / "fake-build-agent.py"
    if not faux_build.is_file():
        poser_faux_build_agent(faux_build)
    env["DESK_BUILD_AGENT_SCRIPT"] = str(faux_build)
    cmd = [sys.executable, str(HOOK), "--phase", "activate",
           "--root", str(root_arg if root_arg is not None else root)]
    return subprocess.run(cmd, input=json.dumps(contexte), env=env,
                           capture_output=True, text=True)


def load_unit(path):
    parser = configparser.ConfigParser(strict=False, interpolation=None)
    parser.optionxform = str
    parser.read(path, encoding="utf-8")
    return parser


def lire_env(chemin):
    values = {}
    for ligne in chemin.read_text(encoding="utf-8").splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.startswith("#") or "=" not in ligne:
            continue
        cle, _, value = ligne.partition("=")
        values[cle] = value
    return values
