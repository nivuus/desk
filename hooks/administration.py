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

    Rend toujours `(code, stdout, stderr)`, jamais ne lève : `npm` absent
    du `PATH`, ou `cwd` qui n'existe pas (l'installation n'a pas tourné),
    lèveraient tous deux un `OSError` au niveau de `subprocess.run` lui-même
    — un chemin d'échec que le code nominal (un `npm` présent, une
    commande qui refuse poliment) ne couvre pas, et qui deviendrait une
    trace Python non rattrapée sans cette garde.

    🔴 PROBLÈME A DU LOT 10A (29 août 2026) : `npm` invoqué par son seul nom
    dépend de ce que le PATH du PROCESSUS APPELANT contient déjà — vrai par
    accident sur ce poste de développement (nvm y est sourcé dans le shell
    interactif), FAUX en général pour un `python3 hooks/activate.py` lancé
    par le vrai moteur d'installation, sans PATH nvm. `commun.lire_node_bin()`
    (voir son commentaire pour le diagnostic complet) est donc APPONDU en
    fin de PATH — jamais en tête, pour ne jamais court-circuiter un `npm`
    que l'appelant aurait délibérément placé plus tôt dans le PATH (c'est
    exactement ce que fait `tests/desk_activate_fixtures.py::appeler`, dont
    le faux `npm` factice doit continuer à être trouvé en premier).
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


def attribuer_vm_a_utilisateur(plateforme_dir: pathlib.Path, email: str,
                                vm_id: str, env: dict):
    """`npm run admin:attribuer -- --email <email> --vm <vm_id>` (tâche 13,
    trou trouvé en production le 29 août 2026 — voir le commentaire
    d'`activate.py::main()` pour le pourquoi et le placement).

    Ni `email` ni `vm_id` ne sont des secrets : aucun besoin de stdin ici,
    à la différence de `creer_compte_admin`. `attribuer-vm.ts` REFUSE de
    toute façon tout drapeau qui ferait passer un secret par l'argv
    (`DRAPEAUX_INTERDITS`) — cette commande n'en emploie aucun.

    Rend `(sortie, None)` en succès, `(None, raison)` sinon. `sortie` porte
    `vm=…\\nnom=…\\nutilisateur=…\\nemail=…\\n` (voir `attribuer-vm.ts::appliquer`),
    ignorée par l'appelant : seul l'échec compte ici.
    """
    code, out, err = lancer_npm(plateforme_dir, "admin:attribuer",
                                 ["--email", email, "--vm", vm_id], env)
    if code != 0:
        return None, (err or out).strip() or f"code de sortie {code}"
    return out.strip(), None
