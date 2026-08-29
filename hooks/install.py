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
import sys

from commun import (
    PORT_DEFAUT,
    adresse_ipv4_de,
    interface_de_route_par_defaut,
    lire_hote,
    lire_node_bin,
    lire_proxy_confiance,
)
from depot_arbre import copier_arbre, rendre_lisible_par_tous


def _racine_source() -> pathlib.Path:
    """La racine du dépôt SOURCE (ce qui est copié vers la cible).

    Surchargeable par `DESK_SOURCE_RACINE` (tests seuls — voir
    `tests/test_desk_install.py`, scénario du nœud_modules absent) : le
    moteur réel, comme tous les autres hooks de ce package, n'a besoin
    d'aucun défaut différent de la racine du dépôt cloné.
    """
    brut = os.environ.get("DESK_SOURCE_RACINE")
    if brut:
        return pathlib.Path(brut)
    return pathlib.Path(__file__).resolve().parents[1]


RACINE = _racine_source()
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


# --- Dérivation des adresses TURN (PUBLIQUES), partagée avec resolve.py ---
#
# 🔴 CORRIGÉ AU LOT 10A (29 août 2026) : CE BLOC S'APPELAIT
# `deriver_adresse_hote()` ET SON COMMENTAIRE AFFIRMAIT QUE PLATEFORME_HOTE,
# TURN_LISTENING_IP ET TURN_RELAY_IP ÉTAIENT LA MÊME ADRESSE. C'ÉTAIT FAUX,
# ET C'ÉTAIT UN BUG RÉEL, PAS UNE IMPRÉCISION DE COMMENTAIRE : mesuré le
# 29 août 2026 avec `/usr/bin/ip` (hors de tout alias de shell), l'interface
# de la route IPv4 PAR DÉFAUT sur cette machine est `ppp0` (PPPoE), dont
# l'adresse est PUBLIQUE (90.87.35.18) — pas `internalBridge`
# (192.168.3.1). `install.py` posait donc `PLATEFORME_HOTE=90.87.35.18`,
# exposant le bureau distant sur l'internet public SANS Pomerium devant lui,
# un trou que la garde des écoutes universelles de `config.ts` ne peut PAS
# attraper (90.87.35.18 n'est pas une des quatre valeurs universelles).
#
# Ce bloc dérive désormais UNIQUEMENT l'adresse TURN (publique, par
# construction : coturn doit être joignable depuis l'internet par des
# clients WebRTC derrière un NAT restrictif — c'est le SEUL rôle légitime
# de la route par défaut ici). `PLATEFORME_HOTE` est dérivée séparément,
# par `commun.lire_hote()` (adresse FIXE, interne, jamais la route par
# défaut) — voir son commentaire pour le détail complet du bug et du
# correctif.
#
# `interface_de_route_par_defaut()` et `adresse_ipv4_de()` viennent de
# `commun.py` (importées en tête de fichier) : elles étaient dupliquées
# octet pour octet avec `resolve.py` avant la ronde de correction 1.

def deriver_adresse_turn() -> str:
    """L'adresse IPv4 PUBLIQUE de la route par défaut, ou lève RuntimeError.

    ⚠️ NE JAMAIS employer cette fonction pour `PLATEFORME_HOTE` — voir le
    commentaire ci-dessus. Elle ne sert QUE TURN_LISTENING_IP/TURN_RELAY_IP.

    🔴 JAMAIS UNE ÉCOUTE UNIVERSELLE : cette fonction ne rend jamais
    '0.0.0.0'/'::'/'[::]'/'*' — elle échoue plutôt que d'inventer une valeur,
    exactement comme `resolve.py::deriver_adresses_turn`.
    """
    interface = interface_de_route_par_defaut()
    if not interface:
        raise RuntimeError(
            "aucune route IPv4 par défaut : impossible de dériver l'adresse "
            "sur laquelle coturn doit écouter et relayer"
        )
    adresse = adresse_ipv4_de(interface)
    if not adresse:
        raise RuntimeError(
            f"aucune adresse IPv4 lisible sur l'interface {interface} (route "
            "par défaut) : impossible de dériver TURN_LISTENING_IP/"
            "TURN_RELAY_IP"
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
        turn_ecoute = facts.get("turn_ecoute") or deriver_adresse_turn()
        turn_relais = facts.get("turn_relais") or turn_ecoute
    except RuntimeError as exc:
        print(f"desk install : {exc}", file=sys.stderr)
        return 1

    # 🔴 PLATEFORME_HOTE NE DOIT JAMAIS ÊTRE UNIVERSELLE, ET NE DOIT JAMAIS
    # ÊTRE L'ADRESSE TURN (voir le commentaire de `deriver_adresse_turn` :
    # confondre les deux exposait le service sur l'adresse PUBLIQUE). Dérivée
    # séparément, par `commun.lire_hote()` — une adresse FIXE et interne,
    # jamais la route par défaut.
    hote_plateforme, raison_hote = facts.get("hote"), None
    if not hote_plateforme:
        hote_plateforme, raison_hote = lire_hote()
    if raison_hote:
        print(f"desk install : {raison_hote}", file=sys.stderr)
        return 1

    # PLATEFORME_PROXY_DE_CONFIANCE — voir `commun.py::lire_proxy_confiance`
    # pour le raisonnement complet (une valeur DÉRIVÉE, jamais demandée).
    # Écrite quel que soit auth_mode : elle ne nuit pas en mode motdepasse
    # (elle y sert seulement à faire croire X-Forwarded-For depuis cette
    # adresse), et devient obligatoire côté service en mode pomerium.
    proxy_confiance = facts.get("proxy_confiance") or lire_proxy_confiance()

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
        # Voir le commentaire de `proxy_confiance` ci-dessus : obligatoire
        # en mode pomerium, inoffensive en mode motdepasse.
        "PLATEFORME_PROXY_DE_CONFIANCE": proxy_confiance,
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

    # 🔴 TROUVAILLE RÉELLE DU LOT 10A (29 août 2026), EN LANÇANT LE VRAI
    # SERVICE : `proto/ts/` N'ÉTAIT PAS COPIÉ DU TOUT, ET LE SERVICE NE
    # DÉMARRE PAS SANS LUI. `plateforme/src/agents/canal.ts` (et vingt
    # autres fichiers de `plateforme/src/`) importe `../../../proto/ts/…`
    # — un chemin RELATIF qui suppose que `proto/ts/` est un FRÈRE de
    # `plateforme/`, exactement comme dans ce dépôt de développement.
    # `tsx` transpile à la VOLÉE (contrairement à `tsc`, qui n'aurait
    # rejeté le module qu'au type-check) : sans les fichiers SOURCE de
    # `proto/ts/` déployés au même endroit relatif, `npm start` échoue à
    # l'instant même où le premier module qui l'importe est chargé
    # (`ERR_MODULE_NOT_FOUND`, mesuré sur ce service réel). Copié SANS ses
    # fichiers `*.test.ts` (jamais exécutés par le service, seulement par
    # `vitest` en développement).
    copier_arbre(RACINE / "proto" / "ts", sous("opt/nivuus/desk/proto/ts"),
                 exclure=("*.test.ts",))
    proto_plateforme_ts = sous("opt/nivuus/desk/proto/ts/plateforme.ts")
    if not proto_plateforme_ts.is_file():
        print(
            f"desk install : {proto_plateforme_ts} est absent apres la copie "
            "de proto/ts ; le service ne demarrera pas "
            "(ERR_MODULE_NOT_FOUND sur '../../../proto/ts/plateforme').",
            file=sys.stderr,
        )
        return 1

    # Voir `rendre_lisible_par_tous` : nécessaire pour que l'UID éphémère de
    # `DynamicUser=yes` puisse seulement TRAVERSER `/opt/nivuus/desk/…` —
    # posé sur `sous("opt/nivuus/desk")`, un cran AU-DESSUS des deux copies,
    # pour couvrir aussi ce répertoire parent lui-même (créé par le premier
    # `copier_arbre` via `destination.parent.mkdir`, sous l'umask du
    # processus qui exécute `install`, jamais garanti world-traversable).
    rendre_lisible_par_tous(sous("opt/nivuus/desk"))

    # 🔴 PROBLÈME B DU LOT 10A : `npm start` EXIGE `node_modules`, ET RIEN NE
    # LE GARANTISSAIT. `copier_arbre()` ci-dessus copie tout `plateforme/`
    # (seul `donnees/` est exclu), donc `node_modules` EST copié en pratique
    # DÈS QU'IL EST PRÉSENT côté source — vérifié le 29 août 2026 : 56
    # paquets, le lien relatif `node_modules/.bin/tsx` se résout encore
    # correctement sous la racine copiée. Mais ce n'est qu'un EFFET DE BORD
    # de la copie du répertoire entier, jamais une garantie : un dépôt
    # fraîchement cloné (ou empaqueté par une pipeline qui n'a jamais lancé
    # `npm install`) copierait un `plateforme/` SANS `node_modules`, et
    # l'installation « réussirait » quand même — le service ne le
    # découvrirait qu'à son premier démarrage, `ExecStart` échouant faute de
    # trouver `tsx`. Cette garde ferme le trou : elle vérifie la présence
    # RÉELLE du binaire dont `npm start` a besoin (`node_modules/.bin/tsx`,
    # jamais un simple test de non-vacuité du répertoire, qui laisserait
    # passer un `node_modules` partiel), et REFUSE plutôt que de laisser un
    # service inerte être posé sans le dire.
    tsx_bin = sous("opt/nivuus/desk/plateforme/node_modules/.bin/tsx")
    if not tsx_bin.exists():
        print(
            f"desk install : {tsx_bin} est absent ; `npm start` "
            "(= `tsx src/index.ts`, voir plateforme/package.json) ne pourra "
            "pas demarrer. Executer `npm install` dans plateforme/ AVANT "
            "d'empaqueter/d'installer ce depot.",
            file=sys.stderr,
        )
        return 1

    emettre({"event": "progress", "pct": 80, "msg": "Dépôt de l'unité systemd"})

    # 🔴 PROBLÈME A DU LOT 10A : L'UNITÉ PORTAIT `ExecStart=/usr/bin/npm
    # start`, UN CHEMIN QUI N'EXISTE SUR AUCUNE DEBIAN SANS PAQUET `nodejs`.
    # Voir `commun.py::lire_node_bin` pour le diagnostic complet et la
    # décision de déploiement. Le fichier `assets/desk-plateforme.service`
    # est désormais un GABARIT portant le jeton `__NODE_BIN__` (à la fois
    # dans `ExecStart=` et dans `Environment=PATH=…`, pour que `npm`
    # lui-même — un script `#!/usr/bin/env node` — retrouve `node` quand le
    # noyau résout son interpréteur) : la substitution ci-dessous est
    # TEXTUELLE, faite une fois pour toutes à l'installation, jamais une
    # expansion de variable côté systemd (qui n'expanse pas le programme
    # exécuté lui-même). Écrit avec `os.open(..., 0o644)` plutôt que
    # `shutil.copy2` : ce fichier n'est plus une copie verbatim.
    node_bin = lire_node_bin()
    gabarit_unite = (ASSETS / "desk-plateforme.service").read_text(encoding="utf-8")
    if "__NODE_BIN__" not in gabarit_unite:
        print(
            "desk install : hooks/assets/desk-plateforme.service ne porte "
            "plus le jeton __NODE_BIN__ ; le gabarit a-t-il change de forme "
            "sans que install.py ne suive ?",
            file=sys.stderr,
        )
        return 1
    contenu_unite = gabarit_unite.replace("__NODE_BIN__", node_bin)

    # POSÉE, PAS ARMÉE : voir le commentaire de tête de l'unité elle-même.
    unite_dest = sous("etc/systemd/system/desk-plateforme.service")
    unite_dest.parent.mkdir(parents=True, exist_ok=True)
    unite_dest.write_text(contenu_unite, encoding="utf-8")
    os.chmod(unite_dest, 0o644)  # une unité est une DONNÉE, pas un programme

    emettre({"event": "progress", "pct": 95,
             "msg": "Configuration coturn"})
    ecrire_turnserver_conf(sous("etc/turnserver.conf"), turn_ecoute,
                            turn_relais, secret_turn)

    emettre({"event": "done"})
    return 0


if __name__ == "__main__":
    sys.exit(main())
