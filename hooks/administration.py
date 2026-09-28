#!/usr/bin/env python3
"""The two `npm` administration commands that `hooks/activate.py` invokes
to arm the platform: creation of the administrator account, enrolment of
the Windows agent.

Extracted from `hooks/activate.py` by task 7 (2026-08-29), BEFORE that task
added the drop of `agent.exe` for `console`: `activate.py`
weighed 479 lines against a ceiling of 500, and that addition would have made it
cross it. This repository forbids compressing code to avoid an extraction
("this repository paid for it twelve times") — the extraction is played FIRST, in its
own commit, with no behaviour change: the same functions, the
same body, the same docstring, only moved.

Like `hooks/vm.py` (task 6), this module IS NOT a hook runnable on its own
(no `--phase`/stdin JSON): `hooks/activate.py` imports it
(`from administration import create_admin_account, enroler_agent_plateforme`),
exactly as it already imports `vm.py` — Python automatically adds the
directory of the LAUNCHED SCRIPT (`hooks/`) to `sys.path`, so this import
resolves without any extra manipulation, neither here nor in the caller. Tests
load it by explicit file path (`importlib`), as
`tests/test_desk_vm.py` already does for `vm.py`, if they want to exercise it
alone; in practice, this module is already fully covered through the
complete hook by `tests/test_desk_activate.py` (Scenario 1), whose
behaviour did not change.
"""
import os
import pathlib
import subprocess

from commun import lire_node_bin


def lancer_npm(cwd: pathlib.Path, subcommand: str, arguments: list,
               env: dict, entree=None):
    """`npm run <subcommand> -- <arguments>`, CWD=`cwd`, ENV=`env`.

    Always returns `(code, stdout, stderr)`, never raises: `npm` absent
    from `PATH`, or a `cwd` that does not exist (installation did not run),
    would both raise an `OSError` at the level of `subprocess.run` itself
    — a failure path the nominal code (an `npm` present, a
    command that refuses politely) does not cover, and that would become an
    uncaught Python traceback without this guard.

    🔴 PROBLEM A OF BATCH 10A (August 29th, 2026): `npm` invoked by its name alone
    depends on what the CALLING PROCESS's PATH already contains — true by
    accident on this development machine (nvm is sourced there in the
    interactive shell), FALSE in general for a `python3 hooks/activate.py` launched
    by the real installation engine, without an nvm PATH. `commun.lire_node_bin()`
    (see its comment for the complete diagnosis) is therefore APPENDED at the
    end of PATH — never at the head, so as never to short-circuit an `npm`
    the caller would have deliberately placed earlier in PATH (it is
    exactly what `tests/desk_activate_fixtures.py::appeler` does, whose
    fake `npm` must keep being found first).
    """
    commande = ["npm", "run", subcommand, "--", *arguments]
    env_complet = dict(env)
    node_bin = lire_node_bin()
    chemin_actuel = env_complet.get("PATH", "")
    composants = chemin_actuel.split(os.pathsep) if chemin_actuel else []
    if node_bin not in composants:
        env_complet["PATH"] = (
            f"{chemin_actuel}{os.pathsep}{node_bin}" if chemin_actuel else node_bin
        )
    try:
        proc = subprocess.run(commande, cwd=str(cwd), env=env_complet,
                               input=entree, capture_output=True, text=True)
    except OSError as exc:
        return 127, "", f"cannot launch {' '.join(commande)}: {exc}"
    return proc.returncode, proc.stdout, proc.stderr


def create_admin_account(plateforme_dir: pathlib.Path, email: str,
                        mot_de_passe: str, env: dict):
    """`npm run admin:utilisateur -- --email <email>`, password on  # policy: allow-fr - npm script name
    STDIN — never in argv (see the top docstring of `activate.py`).
    Returns `(identifier, None)` on success, `(None, reason)` otherwise.
    """
    code, out, err = lancer_npm(plateforme_dir, "admin:utilisateur",
                                 ["--email", email], env,
                                 entree=f"{mot_de_passe}\n")
    if code != 0:
        return None, (err or out).strip() or f"exit code {code}"
    return out.strip(), None


def enroler_agent_plateforme(plateforme_dir: pathlib.Path, nom_vm: str,
                              adresse_vm: str, env: dict):
    """`npm run admin:agent -- --vm <nom_vm> --adresse <adresse_vm>` — ENROLMENT
    mode (see the top docstring of `activate.py` on the two meanings
    of `--vm`).

    Returns `((vm_id, secret), None)` on success, `(None, reason)` otherwise.
    """
    code, out, err = lancer_npm(plateforme_dir, "admin:agent",
                                 ["--vm", nom_vm, "--adresse", adresse_vm], env)
    if code != 0:
        return None, (err or out).strip() or f"exit code {code}"

    values = {}
    for ligne in out.splitlines():
        if "=" in ligne:
            cle, _, value = ligne.partition("=")
            values[cle.strip()] = value.strip()
    vm_id = values.get("vm_id")
    secret = values.get("AGENT_SECRET")
    if not vm_id or not secret:
        return None, f"unreadable enrolment output (vm_id/AGENT_SECRET absent): {out!r}"
    return (vm_id, secret), None


def assign_vm_to_user(plateforme_dir: pathlib.Path, email: str,
                                vm_id: str, env: dict):
    """`npm run admin:attribuer -- --email <email> --vm <vm_id>` (task 13,
    a hole found in production on August 29th, 2026 — see the comment of
    `activate.py::main()` for the why and the placement).

    Neither `email` nor `vm_id` are secrets: no need for stdin here,
    unlike `create_admin_account`. `attribuer-vm.ts` REFUSES
    anyway any flag that would pass a secret through argv
    (`DRAPEAUX_INTERDITS`) — this command uses none.

    Returns `(output, None)` on success, `(None, reason)` otherwise. `output` carries
    `vm=…\\nnom=…\\nutilisateur=…\\nemail=…\\n` (see `attribuer-vm.ts::appliquer`),
    ignored by the caller: only failure counts here.
    """
    code, out, err = lancer_npm(plateforme_dir, "admin:attribuer",
                                 ["--email", email, "--vm", vm_id], env)
    if code != 0:
        return None, (err or out).strip() or f"exit code {code}"
    return out.strip(), None
