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
import pathlib
import subprocess


def lancer_npm(cwd: pathlib.Path, sous_commande: str, arguments: list,
               env: dict, entree=None):
    """`npm run <sous_commande> -- <arguments>`, CWD=`cwd`, ENV=`env`.

    Rend toujours `(code, stdout, stderr)`, jamais ne lève : `npm` absent
    du `PATH`, ou `cwd` qui n'existe pas (l'installation n'a pas tourné),
    lèveraient tous deux un `OSError` au niveau de `subprocess.run` lui-même
    — un chemin d'échec que le code nominal (un `npm` présent, une
    commande qui refuse poliment) ne couvre pas, et qui deviendrait une
    trace Python non rattrapée sans cette garde.
    """
    commande = ["npm", "run", sous_commande, "--", *arguments]
    try:
        proc = subprocess.run(commande, cwd=str(cwd), env=env, input=entree,
                               capture_output=True, text=True)
    except OSError as exc:
        return 127, "", f"impossible de lancer {' '.join(commande)} : {exc}"
    return proc.returncode, proc.stdout, proc.stderr


def creer_compte_admin(plateforme_dir: pathlib.Path, email: str,
                        mot_de_passe: str, env: dict):
    """`npm run admin:utilisateur -- --email <email>`, mot de passe sur
    STDIN — jamais sur l'argv (voir le docstring de tête d'`activate.py`).
    Rend `(identifiant, None)` en succès, `(None, raison)` sinon.
    """
    code, out, err = lancer_npm(plateforme_dir, "admin:utilisateur",
                                 ["--email", email], env,
                                 entree=f"{mot_de_passe}\n")
    if code != 0:
        return None, (err or out).strip() or f"code de sortie {code}"
    return out.strip(), None


def enroler_agent_plateforme(plateforme_dir: pathlib.Path, nom_vm: str,
                              adresse_vm: str, env: dict):
    """`npm run admin:agent -- --vm <nom_vm> --adresse <adresse_vm>` — mode
    ENRÔLEMENT (voir le docstring de tête d'`activate.py` sur les deux sens
    de `--vm`).

    Rend `((vm_id, secret), None)` en succès, `(None, raison)` sinon.
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
