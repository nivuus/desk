#!/usr/bin/env python3
"""Le runtime Node.js sur la cible : ce que le lot 10A avait posé À LA MAIN.

🔴 CE MODULE FERME UN TROU NOMMÉ PAR LA REVUE FINALE DE BRANCHE (30 août
2026). `hooks/assets/desk-plateforme.service` lance
`__NODE_BIN__/npm start`, et `commun.py::NODE_BIN_DEFAUT` pointe ce jeton
vers `/opt/nivuus/node/bin` — mais AUCUN hook ne déposait quoi que ce soit à
cet emplacement. L'arbre qui s'y trouve sur la machine de développement a
été copié à la main pendant le lot 10A, et la commande ne vivait que dans un
rapport gitignoré. Une installation neuve aurait donc posé un service
structurellement incapable de démarrer, et `install.py` — qui REFUSE avec
une phrase quand `node_modules/.bin/tsx` manque, et quand
`proto/ts/plateforme.ts` manque — n'aurait rien dit du tout sur celui-ci.

**Tranché : le hook POSE Node, il ne se contente pas de refuser.** Les deux
issues que la revue laissait ouvertes ne sont pas équivalentes :

  - refuser SEULEMENT aurait reproduit la Critique de cette même revue —
    une porte qu'aucune installation ne peut franchir, puisque rien, nulle
    part, ne provisionne Node sur la cible. Un package qui refuse toujours
    ne s'installe jamais ;
  - poser est possible, et sans deviner : `hooks/resolve.py` a DÉJÀ validé
    le `node` de la machine qui exécute les hooks contre la borne
    `engines.node` que `plateforme/package.json` déclare. C'est ce
    `node`-là qu'on dépose. **Effet de bord recherché** : la version que
    `resolve` a validée devient la version que le service exécute — ce qui
    ferme au passage la mineure #6 de la tâche 3 (« le contrôle de Node
    interroge la machine qui exécute le hook, jamais la cible »). Les deux
    machines n'étaient pas la même ; désormais le runtime voyage de l'une
    à l'autre, donc la mesure porte sur ce qui tournera.

Ce qui est déposé, et rien de plus — la forme EXACTE relevée sur l'arbre
posé à la main le 29 août 2026 (`ls -la /opt/nivuus/node/bin`,
`ls /opt/nivuus/node/lib/node_modules`) :

    <préfixe>/bin/node              (le binaire, ~124 Mio)
    <préfixe>/bin/npm  -> ../lib/node_modules/npm/bin/npm-cli.js
    <préfixe>/bin/npx  -> ../lib/node_modules/npm/bin/npx-cli.js
    <préfixe>/lib/node_modules/npm  (~20 Mio)

⚠️ LES PAQUETS GLOBAUX SANS RAPPORT AVEC CE DÉPÔT NE SONT PAS COPIÉS
(`bats`, `corepack`, `@github/copilot`, `@google/gemini-cli` : 237 Mio à eux
seuls, mesurés au lot 10A). Le besoin se résume à `bin/node` et à `npm`.

⚠️ LES DEUX LIENS SONT RECRÉÉS COMME LIENS, jamais suivis : leur cible est
RELATIVE et pointe à l'intérieur de l'arbre déposé, donc elle se résout
correctement à la destination. Les suivre déposerait deux copies du même
script sous un nom qui prétendrait être `npm`.

Comme `commun.py`, `depot_arbre.py` et `vm.py`, ce module n'est pas un hook
exécutable seul : `hooks/install.py` l'importe.
"""
import os
import pathlib
import shutil
import subprocess

from depot_arbre import copier_arbre, rendre_lisible_par_tous

# Les trois exécutables attendus sous `<préfixe>/bin`, et le seul paquet
# global qui voyage. Énoncés ici plutôt que devinés à l'énumération : un
# `bin/` d'nvm porte aussi les paquets globaux de son propriétaire.
EXECUTABLES = ("node", "npm", "npx")
PAQUET_GLOBAL = pathlib.Path("lib") / "node_modules" / "npm"


def racine_node_source():
    """Le préfixe Node à déposer. Rend `(chemin, None)` ou `(None, raison)`.

    Surchargeable par `DESK_NODE_SOURCE` — pour les tests (qui posent un
    arbre factice de quelques octets plutôt que de recopier 144 Mio à chaque
    scénario), et pour un opérateur qui voudrait déposer un autre runtime que
    celui qui exécute le hook.

    À défaut, le préfixe est DÉRIVÉ de l'interpréteur lui-même :
    `process.execPath` rend `<préfixe>/bin/node`, donc `parents[1]` est le
    préfixe. Jamais `command -v node` : sur cette machine, `node` est une
    FONCTION zsh qui source nvm, et un `subprocess` ne passe de toute façon
    jamais par une fonction de shell interactif — c'est le diagnostic du lot
    10A (voir `commun.py::NODE_BIN_DEFAUT`).
    """
    brut = os.environ.get("DESK_NODE_SOURCE")
    if brut:
        prefixe = pathlib.Path(brut)
    else:
        try:
            r = subprocess.run(
                ["node", "-e", "process.stdout.write(process.execPath)"],
                capture_output=True, text=True, timeout=30)
        except (OSError, subprocess.TimeoutExpired) as exc:
            return None, (
                f"node est introuvable ou muet sur cette machine ({exc}) : "
                "impossible de déposer un runtime Node sur la cible, alors "
                "que l'unité systemd de desk lance npm"
            )
        if r.returncode != 0 or not r.stdout.strip():
            return None, (
                "`node -e process.execPath` n'a rien rendu d'exploitable "
                f"(code {r.returncode}) : impossible de localiser le runtime "
                "Node à déposer sur la cible"
            )
        prefixe = pathlib.Path(r.stdout.strip()).resolve().parents[1]

    manquants = [nom for nom in ("bin/node",) + (str(PAQUET_GLOBAL),)
                 if not (prefixe / nom).exists()]
    if manquants:
        return None, (
            f"le runtime Node source {prefixe} est incomplet : "
            f"{', '.join(manquants)} manque(nt). Un service desk sans "
            "`node` ni `npm` déposés ne peut pas démarrer (voir "
            "hooks/assets/desk-plateforme.service::ExecStart)."
        )
    return prefixe, None


def deposer_node(prefixe: pathlib.Path, destination: pathlib.Path) -> None:
    """Copie le runtime de `prefixe` vers `destination` (le PRÉFIXE cible,
    pas son `bin/`), et le rend lisible par tous.

    `rendre_lisible_par_tous` est indispensable et pas cosmétique :
    `DynamicUser=yes` fait tourner le service sous un UID éphémère qui
    n'appartient à aucun groupe partagé avec les fichiers déposés par root —
    c'est le bug réel du lot 10A, déjà payé sur `/opt/nivuus/desk` (voir
    `depot_arbre.py::rendre_lisible_par_tous`). Un `bin/node` en `rwxr-x---`
    donnerait un `ExecStart` qui échoue avant la première ligne de
    JavaScript.
    """
    bin_dest = destination / "bin"
    if bin_dest.exists() or bin_dest.is_symlink():
        shutil.rmtree(bin_dest)
    bin_dest.mkdir(parents=True)
    for nom in EXECUTABLES:
        source = prefixe / "bin" / nom
        if not source.exists() and not source.is_symlink():
            continue          # `npx` peut légitimement manquer d'un runtime
        cible = bin_dest / nom
        if source.is_symlink():
            os.symlink(os.readlink(source), cible)
        else:
            shutil.copy2(source, cible)
    copier_arbre(prefixe / PAQUET_GLOBAL, destination / PAQUET_GLOBAL)
    rendre_lisible_par_tous(destination)
