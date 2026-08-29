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
mêmes valeurs — par les fonctions de `commun.py` (module FRÈRE, voir son
docstring), partagées avec `resolve.py` plutôt que dupliquées : la ronde de
correction 1 sur cette tâche a extrait `interface_de_route_par_defaut()`,
`adresse_ipv4_de()` et `PORT_DEFAUT`, relevés identiques octet pour octet
entre les deux hooks.

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
import secrets
import shutil
import sys

from commun import PORT_DEFAUT, adresse_ipv4_de, interface_de_route_par_defaut

RACINE = pathlib.Path(__file__).resolve().parents[1]
ASSETS = pathlib.Path(__file__).resolve().parent / "assets"

# PORT_DEFAUT (3445) et sa raison vivent dans `commun.py`, seul endroit qui
# les porte désormais — voir son commentaire.

# Le port TURN standard, celui que docker-compose.coturn.yml pose par
# `--listening-port=3478` — la seule valeur qui fasse correspondre l'URL
# annoncée aux clients (TURN_URL) et le port sur lequel coturn écoute
# réellement.
PORT_TURN = 3478


def emettre(evenement: dict) -> None:
    print(json.dumps(evenement), flush=True)


# --- Dérivation de l'adresse d'écoute, partagée avec resolve.py -----------
#
# PLATEFORME_HOTE, TURN_LISTENING_IP et TURN_RELAY_IP sont la MÊME adresse
# sur ce déploiement : celle de l'interface de la route IPv4 par défaut,
# joignable à la fois par Pomerium (réseau hôte) et par la VM Windows —
# vérifiée être 192.168.3.1 le 29 août 2026. Une seule dérivation, réutilisée
# trois fois plutôt que trois lectures indépendantes de `ip`.
#
# `interface_de_route_par_defaut()` et `adresse_ipv4_de()` viennent de
# `commun.py` (importées en tête de fichier) : elles étaient dupliquées
# octet pour octet avec `resolve.py` avant la ronde de correction 1.

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
    """Écrit `chemin` en KEY=VALUE, un par ligne, DÉJÀ CRÉÉ en mode 600.

    🔴 CRÉÉ EN 0600, JAMAIS ÉCRIT PUIS `chmod`É APRÈS COUP (ronde de
    correction 1, tâche 4) : entre un `write_text` et un `os.chmod`
    ultérieur, le fichier existe brièvement au mode par défaut du `umask`
    du processus (644 dans le cas le plus courant) — une fenêtre
    d'exposition réelle pour un fichier qui porte `PLATEFORME_SECRET_JETON`
    en clair. `os.open(..., mode=0o600)` pose la permission ATOMIQUEMENT à
    la création : le `mode` d'un `open(2)` avec `O_CREAT` est toujours
    masqué par le `umask` du processus (qui ne peut que RETIRER des bits,
    jamais en ajouter), donc le résultat est au plus 0600, jamais plus
    permissif — il n'existe aucun instant où le fichier est lisible par
    autrui.
    """
    chemin.parent.mkdir(parents=True, exist_ok=True)
    corps = "\n".join(f"{cle}={valeur}" for cle, valeur in valeurs.items()) + "\n"
    descripteur = os.open(chemin, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descripteur, "w", encoding="utf-8") as fh:
        fh.write(corps)


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
    # Même précaution que `ecrire_env` (voir son docstring) : ce fichier
    # porte `static-auth-secret` en clair, et une fenêtre write-puis-chmod
    # y serait PIRE que sur desk.env — créé déjà en 0600, jamais chmod après
    # coup.
    descripteur = os.open(chemin, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descripteur, "w", encoding="utf-8") as fh:
        fh.write(corps)


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
        # 🔴 POSÉES EXPLICITEMENT, ET C'EST OBLIGATOIRE (ronde de correction
        # 1, tâche 4) : leur défaut produit (`donnees/icones`,
        # `donnees/televersements`, relatifs à `WorkingDirectory`) tomberait
        # sous `/opt/nivuus/desk/plateforme`, un chemin que `DynamicUser=yes`
        # rend EN LECTURE SEULE (`ProtectSystem=strict` implicite — voir
        # `hooks/assets/desk-plateforme.service`). Sans cette paire,
        # la gestion d'icônes et de téléversements échouerait en `EROFS` au
        # premier usage. `/var/lib/nivuus-desk` est le SEUL répertoire
        # inscriptible par le service : c'est celui que `StateDirectory=
        # nivuus-desk` crée et possède, le même que `PLATEFORME_BASE_URL`
        # ci-dessus.
        "PLATEFORME_ICONES": "/var/lib/nivuus-desk/icones",
        "PLATEFORME_TELEVERSEMENTS": "/var/lib/nivuus-desk/televersements",
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
    # développement. La plateforme le recrée elle-même au premier accès —
    # sous `/var/lib/nivuus-desk`, PAS sous son défaut relatif : voir
    # PLATEFORME_ICONES/PLATEFORME_TELEVERSEMENTS ci-dessus, et pourquoi le
    # défaut casserait sous DynamicUser.
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
