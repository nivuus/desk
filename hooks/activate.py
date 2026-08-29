#!/usr/bin/env python3
"""Hook activate du package desk : armer ce qu'`install` n'a fait que poser.

Protocole (voir `installer/packages/runner.py` du dépôt voisin, et les
docstrings de `resolve.py`/`install.py` de ce package) : lit
{"hw":…, "answers":…} sur stdin, écrit un objet JSON par ligne sur stdout.
Comme `install`, ce hook n'a AUCUN canal `refuse` — une erreur ici se
traduit par un code de sortie non nul (`HookError` côté moteur), parce
qu'`activate` court APRÈS que `resolve` a déjà validé la machine et
qu'`install` a déjà écrit sur le disque cible : un échec à ce stade est une
anomalie, pas une décision à motiver pour l'opérateur.

🔴 LES `facts` DE `resolve` ARRIVENT FUSIONNÉS DANS `hw`, JAMAIS DANS UNE
CLÉ `facts` SÉPARÉE. Vérifié dans `installer/packages/runner.py::run_activate`
(ligne 327-337) : `_run_hook(manifest, "activate",
merge_into_hw(hw, facts or {}), answers, emit=emit)` — sans paramètre `root`
non plus (`_run_hook`'s `root` a pour défaut `""`, donc AUCUN `--root` n'est
passé en production ; `argparse` ci-dessous lui donne quand même un défaut
`/`, comme `console/hooks/activate.py`, pour rester appelable de la même
façon par les tests). La règle de précédence est dans
`installer/packages/facts.py::merge_into_hw` : `hw` (la fraîche détection)
l'emporte sur un fait de même clé, un fait ne comble que ce que `hw` n'a pas
produit. Aucune clé de `hw` que `resolve.py` a mesurée (`vm_repond`,
`node_version`, `turn_ecoute`, `turn_relais`, `port`) n'a de détecteur
générique connu à ce jour, donc en pratique ces cinq clés survivent
toujours la fusion — mais ce hook les LIT dans `hw`, jamais dans un
`facts` frère, pour rester correct le jour où un détecteur générique les
produirait aussi.

🔴 L'ARMEMENT EST UN LIEN, JAMAIS `systemctl enable`. Même doctrine que
`console/hooks/activate.py` (le seul précédent de ce dépôt voisin) :
`systemctl` échoue EN SILENCE dans un environnement contraint — une
sous-commande de requête n'imprime rien —, donc un `enable` qui « rend la
main » ne dit rien. Un lien existe ou lève. `hooks/assets/
desk-plateforme.service` porte déjà ce commentaire depuis la tâche 4.

⚠️ DIFFÉRENCE DÉLIBÉRÉE AVEC LE PRÉCÉDENT `console` : le lien créé ici est
RELATIF (`../desk-plateforme.service`), pas absolu
(`/etc/systemd/system/…`). Un lien absolu ne se résout que quand le
`--root` employé EST `/` — ce que `console/hooks/activate.py` assume
explicitly (« The link target is an ABSOLUTE path in the running system's
namespace, not in the throwaway root »). Un lien relatif, lui, se résout
correctement des deux côtés : sous un `--root` de test comme sous le
`/` réel, `os.path.realpath()` retombe sur le même fichier — c'est ce qui
rend `os.path.exists(os.path.realpath(lien))` vérifiable EN TEST, sans
jamais toucher le systemd réel de la machine qui fait tourner ces tests.

🔴 TU N'ARMES RIEN SUR CETTE MACHINE. `daemon-reload` et `start` ne
s'exécutent QUE si `--root` vaut `/` — jamais sous un `--root` de test :
reloader/démarrer là driverait le systemd de l'INSTALLEUR (ou du banc de
test), pas celui de la cible. Les liens créés garantissent de toute façon
que le prochain démarrage réel est correct, avec ou sans ce geste
immédiat — même raisonnement que `console/hooks/activate.py::start_now`.

Le mot de passe du compte initial NE TRANSITE QUE PAR STDIN, jamais par
`argv` : voir `plateforme/src/admin/creer-utilisateur.ts`, qui REFUSE
explicitement un `--password`/`--mdp`/… sur la ligne de commande (`ps`
exposerait un secret passé en argv à tout utilisateur de la machine,
pendant toute la durée de l'appel, puis dans l'historique du shell).

🔴 `--vm` DE `admin:agent` SIGNIFIE DEUX CHOSES SELON LE MODE, LU DANS
`plateforme/src/admin/enroler-agent.ts` :
  - en mode ENRÔLEMENT (celui que ce hook emploie), `--vm <nom>` est le NOM
    d'affichage de la VM (`enroler-agent.ts:96` : « --vm <nom> est
    obligatoire » ; `enrolerLaVm` l'écrit tel quel dans la colonne `vm.nom`,
    l'identifiant `vm_id` étant un `randomUUID()` généré PAR la commande,
    jamais reçu) ;
  - en mode `--roter` (hors du périmètre de ce hook), `--vm <id>` est cette
    fois l'identifiant `vm_id` déjà attribué (voir son propre message de
    refus, `enroler-agent.ts:183-184` : « c'est `vm_id`, celui qu'`--vm
    <nom> --adresse <hôte>` a imprimé à l'enrôlement »).
  Ce hook n'enrôle qu'une seule fois, jamais ne fait tourner un secret :
  il n'emploie donc que le mode enrôlement, avec un NOM.
`--adresse` n'a AUCUN défaut côté commande (`enroler-agent.ts:112-114`) :
ce hook doit toujours lui en fournir une.

`AGENT_VM` — la variable que `scripts/run-agent.sh` pose sur l'agent Rust
(voir `agent/src/plateforme/identite.rs`) — est le `vm_id` (l'UUID) que la
plateforme vérifie contre `agent_enrole` (`plateforme/src/agents/
enrolement.ts::verifierEnrolement` interroge par `vm_id`), PAS le nom
d'affichage passé à `--vm` : c'est bien la ligne `vm_id=…` de la sortie de
`admin:agent` que ce hook recopie dans `AGENT_VM`, jamais le nom donné en
entrée.

⚠️ NI LE NOM NI L'ADRESSE DE LA VM WINDOWS NE SONT DES RÉPONSES DU WIZARD
(`wizard.yaml` n'en pose aucune question, à dessein — « Quatre questions,
et pas une de plus »), et aucun détecteur `hw` connu ne les produit non
plus (voir plus haut). `ADRESSE_VM_DEFAUT` reprend donc la valeur que
`console/guest/winrm_exec.py:37` emploie déjà comme défaut de `GUEST_IP`
(`192.168.3.2`) — une adresse RÉELLEMENT ÉPINGLÉE, pas devinée :
`console/guest/domain.py` fixe un bail DHCP statique sur cette adresse
pour l'adresse MAC de la VM Windows que `console` provisionne (desk exige
`console` en pré-requis, voir `nivuus-package.yaml`). Le nom d'affichage
`NOM_VM_DEFAUT`, lui, N'EST PAS UNE VALEUR MESURÉE : c'est un choix
arbitraire (une seule VM existe dans ce déploiement), documenté comme tel
et laissé surchargeable — voir le § Réserves du rapport de cette tâche.
"""
import argparse
import json
import os
import pathlib
import stat
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]

# --- Ce que la tâche 4 a posé, et que ce hook arme ou complète -----------
UNITE = "desk-plateforme.service"
UNIT_DIR = "etc/systemd/system"
# ⚠️ DOIT RESTER EN SYNC avec le `[Install] WantedBy=` de
# `hooks/assets/desk-plateforme.service` : c'est ce fichier qui décide sous
# quel `.target.wants/` le lien doit vivre pour que systemd le prenne en
# compte au démarrage `multi-user.target`. `tests/test_desk_activate.py`
# vérifie cette synchronisation en lisant l'unité elle-même.
WANTS_SUBDIR = "multi-user.target.wants"

ENV_RELATIF = "etc/nivuus/desk.env"
PLATEFORME_RELATIF = "opt/nivuus/desk/plateforme"

# Voir le docstring de tête pour la justification de ces deux défauts.
ADRESSE_VM_DEFAUT = "192.168.3.2"
NOM_VM_DEFAUT = "windows"


def emettre(evenement: dict) -> None:
    print(json.dumps(evenement), flush=True)


# --- Armement : un lien, jamais un `enable` -------------------------------

def armer_unite(root: pathlib.Path, unite: str) -> pathlib.Path:
    """Lie `unite` dans `<UNIT_DIR>/<WANTS_SUBDIR>/`. Idempotent.

    Lève `FileNotFoundError` si l'unité n'existe pas encore sous
    `<root>/<UNIT_DIR>/` : un lien mort serait pire qu'aucun lien, il se
    lirait comme armé alors qu'il ne l'est pas. C'est le bras que la tâche
    éprouve délibérément (voir le § Step 5 du plan).

    Le lien créé est RELATIF (`../<unite>`) — voir le docstring de tête
    pour pourquoi ce choix diffère du précédent `console`.
    """
    chemin_unite = root / UNIT_DIR / unite
    if not chemin_unite.is_file():
        raise FileNotFoundError(str(chemin_unite))

    wants_dir = root / UNIT_DIR / WANTS_SUBDIR
    wants_dir.mkdir(parents=True, exist_ok=True)
    lien = wants_dir / unite

    cible = os.path.relpath(chemin_unite, wants_dir)
    if lien.is_symlink() and os.readlink(lien) == cible:
        return lien
    if lien.exists() or lien.is_symlink():
        lien.unlink()  # un fichier RÉGULIER ici serait le bug, pas un état
    lien.symlink_to(cible)
    return lien


def demarrer_maintenant(unite: str) -> list:
    """`daemon-reload` puis `start <unite>`. Ne lève jamais.

    `systemctl` est légitimement inutilisable dans un environnement
    contraint (voir le docstring de tête), et le lien créé par
    `armer_unite` garantit de toute façon que le prochain démarrage réel
    est correct — donc chaque échec est seulement RAPPORTÉ, jamais fatal.
    N'est appelé par `main()` que lorsque `--root` vaut `/` : voir sa
    garde, juste avant l'appel.
    """
    echecs = []
    commandes = [["systemctl", "daemon-reload"], ["systemctl", "start", unite]]
    for commande in commandes:
        try:
            proc = subprocess.run(commande, capture_output=True, text=True)
        except OSError as exc:  # systemctl absent de ce PATH
            echecs.append(f"{' '.join(commande)} : {exc}")
            continue
        if proc.returncode != 0:
            detail = (proc.stderr or proc.stdout or "").strip()[:200]
            echecs.append(f"{' '.join(commande)} : {detail or proc.returncode}")
    return echecs


# --- Le fichier d'environnement : le lire pour la config, l'enrichir ------

def lire_env_fichier(chemin: pathlib.Path) -> dict:
    """Parse `chemin` en KEY=VALUE, un par ligne. `{}` si le fichier n'existe
    pas — dupliqué du parseur du test de la tâche 4 plutôt qu'importé,
    même doctrine que le reste de ce package : chaque hook reste
    exécutable seul.
    """
    valeurs = {}
    if not chemin.is_file():
        return valeurs
    for ligne in chemin.read_text(encoding="utf-8").splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.startswith("#") or "=" not in ligne:
            continue
        cle, _, valeur = ligne.partition("=")
        valeurs[cle] = valeur
    return valeurs


def ajouter_variables_env(chemin: pathlib.Path, nouvelles: dict) -> None:
    """Ajoute des lignes `KEY=VALUE` à la fin d'un fichier d'environnement
    déjà posé, SANS toucher aux lignes qui y sont déjà, et referme le
    fichier en mode 600 — le même mode que `install.py::ecrire_env` lui a
    donné, et qu'il doit garder : ce fichier porte des secrets.

    🔴 PRÉCONDITION, EXIGÉE PAR L'APPELANT : `chemin` DOIT DÉJÀ EXISTER
    (`main()` refuse plus haut si `desk.env` est absent — voir son
    commentaire). `write_text()` sur un fichier EXISTANT réutilise le mode
    déjà en place, il ne le recrée pas au umask du processus ; c'est
    SEULEMENT sur un fichier NEUF que ce patron ouvrirait la fenêtre
    écriture-puis-`chmod` qu'`install.py::ecrire_env` a éliminée (commit
    `92cacfb`) en passant à `os.open(..., 0o600)`. Cette fonction ne
    recrée jamais ce fichier depuis rien — c'est la garde de `main()`, pas
    elle, qui rend cette précondition vraie.
    """
    corps = chemin.read_text(encoding="utf-8") if chemin.is_file() else ""
    if corps and not corps.endswith("\n"):
        corps += "\n"
    corps += "".join(f"{cle}={valeur}\n" for cle, valeur in nouvelles.items())
    chemin.write_text(corps, encoding="utf-8")
    os.chmod(chemin, stat.S_IRUSR | stat.S_IWUSR)  # 0o600, redondant si la
    # précondition tient déjà — mais gratuit, et une seconde ligne de
    # défense ne coûte rien.


# --- Les deux commandes d'administration ----------------------------------

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
    STDIN — jamais sur l'argv (voir le docstring de tête). Rend
    `(identifiant, None)` en succès, `(None, raison)` sinon.
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
    ENRÔLEMENT (voir le docstring de tête sur les deux sens de `--vm`).

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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase")
    parser.add_argument("--root", default="/")
    args = parser.parse_args()
    root = pathlib.Path(args.root.rstrip("/") or "/")

    # Garde générique sur la forme de l'entrée : un JSON illisible, ou une
    # racine qui n'est pas un objet, ne doit jamais lever une trace Python
    # — la correction de la tâche 3 (un `main()` qui levait sur une entrée
    # malformée) s'applique tout aussi bien ici.
    try:
        contexte = json.load(sys.stdin)
    except json.JSONDecodeError as exc:
        print(f"desk activate: entree illisible, stdin n'est pas du JSON valide ({exc})",
              file=sys.stderr)
        return 1
    if not isinstance(contexte, dict):
        print("desk activate: entree malformee, la racine JSON doit etre un objet",
              file=sys.stderr)
        return 1
    hw = contexte.get("hw")
    hw = hw if isinstance(hw, dict) else {}
    answers = contexte.get("answers")
    answers = answers if isinstance(answers, dict) else {}

    # `hw` porte les facts de resolve, FUSIONNÉS (voir le docstring de
    # tête) : `port` en fait partie si aucun détecteur générique ne l'a
    # déjà produit sous ce nom. Purement informatif ici — le port lui-même
    # est déjà baké dans desk.env par `install.py` — mais c'est la preuve
    # que ce hook lit bien `hw`, jamais une clé `facts` qui n'existe pas à
    # ce niveau du protocole.
    port_attendu = hw.get("port")
    msg_armement = "Armement du service"
    if port_attendu:
        msg_armement += f" (port attendu : {port_attendu})"

    emettre({"event": "progress", "pct": 10, "msg": msg_armement})
    try:
        armer_unite(root, UNITE)
    except FileNotFoundError as exc:
        print(f"desk activate: unite absente, rien arme : {exc}", file=sys.stderr)
        return 1

    # Seulement sur la machine RÉELLE : voir le docstring de tête.
    if str(root) == "/":
        echecs = demarrer_maintenant(UNITE)
        if echecs:
            print("desk activate: unite liee mais non demarree ; l'armement "
                  "prendra effet au prochain redemarrage", file=sys.stderr)
            for item in echecs:
                print(f"  - {item}", file=sys.stderr)

    plateforme_dir = root / PLATEFORME_RELATIF
    if not plateforme_dir.is_dir():
        print(f"desk activate: {plateforme_dir} est absent ; le hook install "
              "ne semble pas avoir tourne sur cette racine", file=sys.stderr)
        return 1

    env_chemin = root / ENV_RELATIF

    # 🔴 CORRECTION, RONDE 1 — `desk.env` DOIT DÉJÀ EXISTER À CE STADE, ET
    # SON ABSENCE EST UN REFUS, JAMAIS UNE CRÉATION SILENCIEUSE. Deux
    # raisons, la seconde étant celle qui tranche :
    #   1. c'est le SYMPTÔME d'une installation partielle (`plateforme/`
    #      posé, `desk.env` jamais écrit ou supprimé depuis) — un problème
    #      plus grave qu'un simple fichier absent, qui mérite un refus nommé
    #      plutôt qu'un repli silencieux ;
    #   2. `ajouter_variables_env()` (plus bas) écrit par `write_text()` PUIS
    #      `chmod`, exactement le patron qu'`install.py::ecrire_env` a
    #      abandonné (commit `92cacfb`, « un secret jamais lisible entre
    #      creation et chmod ») pour un `os.open(..., 0o600)` atomique — un
    #      fichier NEUF naîtrait ici au umask du processus, brièvement lisible
    #      par quiconque avant que le `chmod` ne rattrape, et ce fichier va
    #      recevoir `AGENT_SECRET`. Refuser ici garantit que
    #      `ajouter_variables_env()` n'écrit JAMAIS que sur un fichier déjà en
    #      600 : `write_text()` sur un fichier EXISTANT ne touche pas à son
    #      mode, donc aucune fenêtre ne s'ouvre. `tests/test_desk_activate.py`
    #      éprouve ce refus (scénario dédié : `plateforme/` posé, `desk.env`
    #      absent).
    if not env_chemin.is_file():
        print(f"desk activate: {env_chemin} est absent ; le hook install ne "
              "semble pas avoir pose de fichier d'environnement sur cette "
              "racine (installation partielle, ou fichier supprime depuis) — "
              "rien n'est cree a sa place", file=sys.stderr)
        return 1

    env_deja_pose = lire_env_fichier(env_chemin)

    # 🔴 IDEMPOTENCE : une activation DÉJÀ ABOUTIE ne rejoue ni la création
    # du compte, ni l'enrôlement. `vm.nom` n'a AUCUNE contrainte d'unicité
    # (`plateforme/…/0001-schema.sql` : seule `utilisateur.email` est
    # `UNIQUE`) — un second `admin:agent` sur un rejeu créerait donc une VM
    # ORPHELINE supplémentaire à chaque appel, jamais un refus ; et un
    # second `admin:utilisateur` avec le MÊME courriel échouerait sur la
    # contrainte `UNIQUE` d'`utilisateur.email`. Sans cette garde, un rejeu
    # (retenté par le moteur, ou relancé à la main par un opérateur après un
    # premier succès) serait donc soit CORRUPTEUR (VMs orphelines qui
    # s'accumulent), soit condamné à échouer pour toujours. Même doctrine
    # que `console/hooks/activate.py::run_steps` (`step.already_done()`) :
    # une étape déjà aboutie se CONSTATE, elle ne se rejoue pas.
    #
    # ⚠️ CE QUE CETTE GARDE NE COUVRE PAS : un échec ENTRE la création du
    # compte et l'enrôlement (compte créé, `AGENT_VM`/`AGENT_SECRET` jamais
    # écrits) laisse un rejeu buter sur le courriel déjà pris — une
    # récupération PARTIELLE hors du périmètre de cette tâche, voir le
    # rapport, § Réserves.
    if env_deja_pose.get("AGENT_VM") and env_deja_pose.get("AGENT_SECRET"):
        emettre({"event": "progress", "pct": 100,
                 "msg": "desk : deja active (AGENT_VM/AGENT_SECRET deja "
                        "presents dans desk.env) ; compte et enrolement ignores"})
        emettre({"event": "done"})
        return 0

    email = str(answers.get("admin_email") or "")
    mot_de_passe = str(answers.get("admin_password") or "")
    if not email or not mot_de_passe:
        print("desk activate: answers.admin_email et answers.admin_password "
              "sont requis et absents", file=sys.stderr)
        return 1

    # `admin:utilisateur`/`admin:agent` lisent leur configuration (quelle
    # base, quel fichier SQLite…) dans `process.env` — voir
    # `plateforme/src/config.ts::lireConfig`. Ce ne sont PAS les variables
    # de ce processus Python : elles vivent dans desk.env, écrit par
    # `install.py`, et doivent donc être fusionnées dans l'environnement
    # transmis à `npm`, sans quoi les commandes d'administration
    # ouvriraient une base différente de celle que le service démarré va
    # réellement servir.
    env_npm = dict(os.environ)
    env_npm.update(env_deja_pose)

    emettre({"event": "progress", "pct": 40, "msg": "Creation du compte administrateur"})
    _identifiant, raison = creer_compte_admin(plateforme_dir, email, mot_de_passe, env_npm)
    if raison:
        print(f"desk activate: creation du compte refusee : {raison}", file=sys.stderr)
        return 1

    emettre({"event": "progress", "pct": 70, "msg": "Enrolement de l'agent"})
    nom_vm = os.environ.get("DESK_VM_NOM", NOM_VM_DEFAUT)
    adresse_vm = os.environ.get("GUEST_IP", ADRESSE_VM_DEFAUT)
    resultat, raison = enroler_agent_plateforme(plateforme_dir, nom_vm, adresse_vm, env_npm)
    if raison:
        print(f"desk activate: enrolement de l'agent refuse : {raison}", file=sys.stderr)
        return 1
    vm_id, secret = resultat

    ajouter_variables_env(env_chemin, {"AGENT_VM": vm_id, "AGENT_SECRET": secret})

    emettre({"event": "progress", "pct": 100,
             "msg": "desk : service arme, compte cree, agent enrole"})
    emettre({"event": "done"})
    return 0


if __name__ == "__main__":
    sys.exit(main())
