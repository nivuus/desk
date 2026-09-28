#!/usr/bin/env python3
"""Les deux commandes d'administration `npm` que `hooks/activate.py` invoque
pour armer la plateforme : création du compte administrateur, enrôlement de
l'agent Windows.

Extrait de `hooks/activate.py` par la tâche 7 (2026-08-29), AVANT que cette
dernière n'ajoute le dépôt de `agent.exe` pour `console` : `activate.py`
pesait 479 lignes sur un plafond de 500, et cet ajout l'aurait fait
franchir. Ce dépôt interdit de comprimer pour éviter une extraction
(« ce dépôt l'a payé douze fois ») — l'extraction est jouée D'ABORD, dans son
propre commit, sans changement de comportement : les mêmes fonctions, le
même corps, le même docstring, seulement déplacés.

Comme `hooks/vm.py` (tâche 6), ce module N'EST PAS un hook exécutable seul
(pas de `--phase`/stdin JSON) : `hooks/activate.py` l'importe
(`from administration import creer_compte_admin, enroler_agent_plateforme`),
exactement comme il importe déjà `vm.py` — Python ajoute automatiquement le
répertoire du SCRIPT LANCÉ (`hooks/`) à `sys.path`, donc cet import résout
sans manipulation supplémentaire, ni ici ni chez l'appelant. Les tests le
chargent par chemin de fichier explicite (`importlib`), comme
`tests/test_desk_vm.py` le fait déjà pour `vm.py`, s'ils veulent l'éprouver
seul ; en pratique, ce module est déjà entièrement couvert au travers du
hook complet par `tests/test_desk_activate.py` (Scénario 1), qui n'a pas
changé de comportement.
"""
import os
import pathlib
import subprocess

from commun import lire_node_bin


def lancer_npm(cwd: pathlib.Path, sous_commande: str, arguments: list,
               env: dict, entree=None):
    """`npm run <sous_commande> -- <arguments>`, CWD=`cwd`, ENV=`env`.

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
    commande = ["npm", "run", sous_commande, "--", *arguments]
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
        return 127, "", f"impossible de lancer {' '.join(commande)} : {exc}"
    return proc.returncode, proc.stdout, proc.stderr


def creer_compte_admin(plateforme_dir: pathlib.Path, email: str,
                        mot_de_passe: str, env: dict):
    """`npm run admin:utilisateur -- --email <email>`, password on
    STDIN — never in argv (see the top docstring of `activate.py`).
    Returns `(identifier, None)` on success, `(None, reason)` otherwise.
    """
    code, out, err = lancer_npm(plateforme_dir, "admin:utilisateur",
                                 ["--email", email], env,
                                 entree=f"{mot_de_passe}\n")
    if code != 0:
        return None, (err or out).strip() or f"code de sortie {code}"
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
        return None, (err or out).strip() or f"code de sortie {code}"

    valeurs = {}
    for ligne in out.splitlines():
        if "=" in ligne:
            cle, _, valeur = ligne.partition("=")
            valeurs[cle.strip()] = valeur.strip()
    vm_id = valeurs.get("vm_id")
    secret = valeurs.get("AGENT_SECRET")
    if not vm_id or not secret:
        return None, f"sortie d'enrolement illisible (vm_id/AGENT_SECRET absents) : {out!r}"
    return (vm_id, secret), None


def attribuer_vm_a_utilisateur(plateforme_dir: pathlib.Path, email: str,
                                vm_id: str, env: dict):
    """`npm run admin:attribuer -- --email <email> --vm <vm_id>` (task 13,
    a hole found in production on August 29th, 2026 — see the comment of
    `activate.py::main()` for the why and the placement).

    Neither `email` nor `vm_id` are secrets: no need for stdin here,
    unlike `creer_compte_admin`. `attribuer-vm.ts` REFUSES
    anyway any flag that would pass a secret through argv
    (`DRAPEAUX_INTERDITS`) — this command uses none.

    Returns `(output, None)` on success, `(None, reason)` otherwise. `output` carries
    `vm=…\\nnom=…\\nutilisateur=…\\nemail=…\\n` (see `attribuer-vm.ts::appliquer`),
    ignored by the caller: only failure counts here.
    """
    code, out, err = lancer_npm(plateforme_dir, "admin:attribuer",
                                 ["--email", email, "--vm", vm_id], env)
    if code != 0:
        return None, (err or out).strip() or f"code de sortie {code}"
    return out.strip(), None
