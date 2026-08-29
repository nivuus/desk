#!/usr/bin/env python3
"""Hook install du package desk : ce qui se pose sur l'hôte.

Protocole (voir `installer/packages/runner.py` du dépôt voisin, et
`hooks/resolve.py` de ce package) : lit {"hw":…, "answers":…} sur stdin,
écrit un objet JSON par ligne sur stdout. Contrairement à `resolve`, ce
hook n'a aucun canal `refuse` — une erreur ici se traduit par un code de
sortie non nul (`HookError` côté moteur), parce que `install` court APRÈS
que `resolve` a déjà validé la machine : un échec à ce stade est une
anomalie, pas une décision à motiver pour l'opérateur.

🔴 `install` NE REÇOIT PAS LES `facts` DE `resolve`, DANS LE MOTEUR RÉEL.
`installer/installer/install-engine/steps/packages.py::apply_packages`
appelle `run_install(manifest, hw, answers, target, emit)` — sans facts.
Seul `run_activate` les reçoit, mergées dans `hw`
(`packages/runner.py::run_activate`). Ce hook les accepte quand même, sous
une clé `facts` optionnelle du contexte stdin (défensif : un futur moteur,
ou ce fichier de tests, peut les fournir) et, à défaut, DÉRIVE lui-même les
mêmes valeurs — par la même méthode que `resolve.py`, dupliquée ici plutôt
qu'importée : chaque hook doit rester exécutable seul (voir le docstring de
`resolve.py` sur le protocole), sans dépendre d'un fichier voisin.

RACINE CIBLE : `--root` (défaut `/`), jamais une variable d'environnement.
C'est ce que le moteur envoie réellement
(`cmd += ["--root", root]` dans `packages/runner.py::_run_hook`), et c'est
la convention déjà éprouvée par `console/hooks/install.py` et son test
(`installer/console/tests/test_console_install.py`). Les CONTENUS écrits
dans les fichiers posés (unité systemd, `desk.env`) portent les chemins
RÉELS de la cible (`/opt/nivuus/desk/…`), jamais préfixés par cette racine
— seule leur PLACEMENT l'est, exactement comme pour les unités que
`console` dépose.
"""
import argparse
import json
import os
import pathlib
import re
import secrets
import shutil
import stat
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
ASSETS = pathlib.Path(__file__).resolve().parent / "assets"

# 🔴 LE PORT PAR DÉFAUT EST 3445, ET IL EST DÉRIVÉ, JAMAIS DEMANDÉ.
#
# /etc/pomerium/config.yaml porte une route `from: https://app.allanic.me`
# vers `to: http://127.0.0.1:3445` (relevé le 29 août 2026) : c'est le seul
# port qui fait marcher la route publique déjà en place. Dupliquée depuis
# `hooks/resolve.py::PORT_DEFAUT` (même valeur, même raison) plutôt
# qu'importée — voir le docstring de ce fichier.
PORT_DEFAUT = 3445

# Le port TURN standard, celui que docker-compose.coturn.yml pose par
# `--listening-port=3478` — la seule valeur qui fasse correspondre l'URL
# annoncée aux clients (TURN_URL) et le port sur lequel coturn écoute
# réellement.
PORT_TURN = 3478


def emettre(evenement: dict) -> None:
    print(json.dumps(evenement), flush=True)


# --- Dérivation de l'adresse d'écoute, dupliquée de resolve.py -------------
#
# PLATEFORME_HOTE, TURN_LISTENING_IP et TURN_RELAY_IP sont la MÊME adresse
# sur ce déploiement : celle de l'interface de la route IPv4 par défaut,
# joignable à la fois par Pomerium (réseau hôte) et par la VM Windows —
# vérifiée être 192.168.3.1 le 29 août 2026. Une seule dérivation, réutilisée
# trois fois plutôt que trois lectures indépendantes de `ip`.

def interface_de_route_par_defaut():
    """Le périphérique réseau de la route IPv4 par défaut, ou None."""
    try:
        r = subprocess.run(["ip", "-4", "route", "show", "default"],
                            capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0:
        return None
    for ligne in r.stdout.splitlines():
        correspond = re.search(r"\bdev\s+(\S+)", ligne)
        if correspond:
            return correspond.group(1)
    return None


def adresse_ipv4_de(interface: str):
    """La première adresse IPv4 portée par `interface`, ou None."""
    try:
        r = subprocess.run(["ip", "-4", "-o", "addr", "show", "dev", interface],
                            capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0:
        return None
    correspond = re.search(r"inet\s+(\d+\.\d+\.\d+\.\d+)", r.stdout)
    return correspond.group(1) if correspond else None


def deriver_adresse_hote() -> str:
    """L'adresse IPv4 de la route par défaut, ou lève RuntimeError.

    🔴 JAMAIS UNE ÉCOUTE UNIVERSELLE : cette fonction ne rend jamais
    '0.0.0.0'/'::'/'[::]'/'*' — elle échoue plutôt que d'inventer une valeur,
    exactement comme `resolve.py::deriver_adresses_turn`.
    """
    interface = interface_de_route_par_defaut()
    if not interface:
        raise RuntimeError(
            "aucune route IPv4 par défaut : impossible de dériver l'adresse "
            "sur laquelle la plateforme et coturn doivent écouter"
        )
    adresse = adresse_ipv4_de(interface)
    if not adresse:
        raise RuntimeError(
            f"aucune adresse IPv4 lisible sur l'interface {interface} (route "
            "par défaut) : impossible de dériver PLATEFORME_HOTE/TURN_*"
        )
    return adresse


# --- Fichiers ----------------------------------------------------------

def ecrire_secret() -> str:
    """Tire un secret au hasard — jamais demandé, jamais constant.

    🔴 `PLATEFORME_SECRET_JETON` n'a AUCUN défaut côté produit
    (`plateforme/src/config.ts::lireConfig`) : un défaut aléatoire À CHAQUE
    DÉMARRAGE invaliderait toutes les sessions à chaque redémarrage du
    service. Ce hook tire donc le secret UNE SEULE FOIS, à l'installation,
    et le persiste dans `desk.env` — jamais recalculé ensuite.
    `secrets.token_hex(32)` rend 64 caractères hexadécimaux, largement
    au-dessus du minimum de 32 que `LONGUEUR_SECRET_MIN` exige.
    """
    return secrets.token_hex(32)


def ecrire_env(chemin: pathlib.Path, valeurs: dict) -> None:
    """Écrit `chemin` en KEY=VALUE, un par ligne, et le pose en MODE 600.

    Mode 600 AVANT que le contenu n'y soit lu par quiconque d'autre que le
    créateur : `os.chmod` juste après l'écriture, sur un fichier neuf créé
    avec un umask qui peut être plus permissif.
    """
    chemin.parent.mkdir(parents=True, exist_ok=True)
    corps = "\n".join(f"{cle}={valeur}" for cle, valeur in valeurs.items()) + "\n"
    chemin.write_text(corps, encoding="utf-8")
    os.chmod(chemin, stat.S_IRUSR | stat.S_IWUSR)  # 0o600


def copier_arbre(source: pathlib.Path, destination: pathlib.Path,
                  exclure: tuple = ()) -> None:
    """Copie `source` sous `destination`, en écartant les noms d'`exclure`.

    `dirs_exist_ok=True` : une installation rejouée sur une racine déjà
    posée ne doit pas lever sur un répertoire déjà présent.
    """
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, destination, symlinks=True,
                     ignore=shutil.ignore_patterns(*exclure) if exclure else None,
                     dirs_exist_ok=True)


def ecrire_turnserver_conf(chemin: pathlib.Path, turn_ecoute: str,
                            turn_relais: str, secret: str) -> None:
    """Pose la configuration native du paquet Debian `coturn`.

    ⚠️ FORMAT NON VÉRIFIÉ SUR CETTE MACHINE — `coturn` n'y est pas installé
    (`dpkg -l coturn` n'y rend rien à la date d'écriture). Les directives
    ci-dessous reprennent, telles quelles, les options déjà vérifiées et
    commentées de `docker-compose.coturn.yml` (`--listening-ip`,
    `--relay-ip`, `--static-auth-secret`, etc.) : chaque option de coturn a,
    par construction du logiciel, une directive de fichier de configuration
    du même nom sans le préfixe `--`. C'est une extrapolation raisonnable,
    pas une mesure — à confirmer au premier `turnserver -c ce-fichier`
    réellement joué.

    🔴 POSÉE, PAS ARMÉE : ce fichier ne suffit pas à faire tourner coturn —
    `/etc/default/coturn` (TURNSERVER_ENABLED) n'est pas touché ici, par la
    même doctrine que l'unité desk-plateforme (voir son commentaire) :
    poser n'est pas armer. Aucune tâche de ce plan n'arme coturn ; c'est un
    legs nommé, pas un oubli — voir le rapport de cette tâche.
    """
    corps = f"""# turnserver.conf — posé par le hook install du package desk.
# Format extrapolé de docker-compose.coturn.yml, NON VÉRIFIÉ sur ce disque
# (coturn n'y est pas installé) — voir le docstring d'ecrire_turnserver_conf.
listening-port={PORT_TURN}
listening-ip={turn_ecoute}
relay-ip={turn_relais}
min-port=49160
max-port=49200
fingerprint
use-auth-secret
static-auth-secret={secret}
realm=nivuus
no-tls
no-dtls
no-cli
log-file=stdout
"""
    chemin.parent.mkdir(parents=True, exist_ok=True)
    chemin.write_text(corps, encoding="utf-8")
    # Ce fichier porte static-auth-secret en clair : même précaution que
    # desk.env, mode 600.
    os.chmod(chemin, stat.S_IRUSR | stat.S_IWUSR)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase")
    parser.add_argument("--root", default="/")
    args = parser.parse_args()
    root = pathlib.Path(args.root.rstrip("/") or "/")

    def sous(rel: str) -> pathlib.Path:
        return root / rel

    contexte = json.load(sys.stdin)
    answers = contexte.get("answers") or {}
    facts = contexte.get("facts") or {}

    auth_mode = answers.get("auth_mode")
    if not auth_mode:
        print("desk install : answers.auth_mode est requis et absent",
              file=sys.stderr)
        return 1
    # 🔴 PLATEFORME_AUTH vient de la réponse auth_mode, TELLE QUELLE, sans
    # repli : une valeur inconnue a déjà été refusée par resolve
    # (`valider_auth_mode`), et un repli silencieux ici ferait tourner un
    # mode sous le nom de l'autre.

    emettre({"event": "progress", "pct": 10,
             "msg": "Dérivation des adresses et du port"})

    try:
        turn_ecoute = facts.get("turn_ecoute") or deriver_adresse_hote()
        turn_relais = facts.get("turn_relais") or turn_ecoute
    except RuntimeError as exc:
        print(f"desk install : {exc}", file=sys.stderr)
        return 1
    # 🔴 PLATEFORME_HOTE NE DOIT JAMAIS ÊTRE UNIVERSELLE : en mode pomerium,
    # `lireConfig` refuse de démarrer sur 0.0.0.0/::/[::]/*. La valeur juste
    # sur cet hôte est 192.168.3.1 (interface de la route par défaut, la
    # même que TURN_ecoute) — jamais une valeur écrite en dur.
    hote_plateforme = turn_ecoute
    port = int(facts.get("port") or PORT_DEFAUT)

    secret_jeton = ecrire_secret()
    secret_turn = ecrire_secret()

    emettre({"event": "progress", "pct": 30, "msg": "Écriture de desk.env"})

    env = {
        "PLATEFORME_HOTE": hote_plateforme,
        "PLATEFORME_PORT": str(port),
        # sqlite persisté plutôt que ':memory:' (le défaut du produit) :
        # un service installé qui perd son état à chaque redémarrage n'a
        # aucune valeur. /var/lib/nivuus-desk est créé par StateDirectory=
        # dans l'unité systemd (voir hooks/assets/desk-plateforme.service).
        "PLATEFORME_BASE": "sqlite",
        "PLATEFORME_BASE_URL": "/var/lib/nivuus-desk/plateforme.sqlite",
        "PLATEFORME_SECRET_JETON": secret_jeton,
        # PLATEFORME_PAGE : ce qui rend nginx facultatif — la plateforme
        # sert elle-même la page bâtie depuis le 22 août 2026. Absente ou
        # vide, GET / rendrait 404 : elle ne se laisse donc jamais vide.
        "PLATEFORME_PAGE": "/opt/nivuus/desk/client/dist",
        "PLATEFORME_AUTH": auth_mode,
        # La configuration coturn côté PLATEFORME (pas côté serveur coturn,
        # voir ecrire_turnserver_conf) : `signaling/ice.ts::configurationIce`
        # exige les DEUX pour annoncer un relais aux pairs.
        "TURN_URL": f"turn:{turn_ecoute}:{PORT_TURN}",
        "TURN_SECRET": secret_turn,
    }
    ecrire_env(sous("etc/nivuus/desk.env"), env)

    emettre({"event": "progress", "pct": 55,
             "msg": "Copie de la plateforme et du client bâti"})

    # `donnees/` est un répertoire de DÉVELOPPEMENT (icônes et
    # téléversements accumulés par les recettes précédentes) : le copier
    # ferait naître une installation neuve avec le passé du poste de
    # développement. La plateforme le recrée elle-même au premier accès
    # (PLATEFORME_ICONES/PLATEFORME_TELEVERSEMENTS, tous deux à leur défaut
    # relatif ici, non posés dans desk.env).
    copier_arbre(RACINE / "plateforme", sous("opt/nivuus/desk/plateforme"),
                 exclure=("donnees",))
    copier_arbre(RACINE / "client" / "dist",
                 sous("opt/nivuus/desk/client/dist"))

    emettre({"event": "progress", "pct": 80, "msg": "Dépôt de l'unité systemd"})

    # POSÉE, PAS ARMÉE : voir le commentaire de tête de l'unité elle-même.
    unite_dest = sous("etc/systemd/system/desk-plateforme.service")
    unite_dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ASSETS / "desk-plateforme.service", unite_dest)
    os.chmod(unite_dest, 0o644)  # une unité est une DONNÉE, pas un programme

    emettre({"event": "progress", "pct": 95,
             "msg": "Configuration coturn"})
    ecrire_turnserver_conf(sous("etc/turnserver.conf"), turn_ecoute,
                            turn_relais, secret_turn)

    emettre({"event": "done"})
    return 0


if __name__ == "__main__":
    sys.exit(main())
