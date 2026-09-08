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
import sys

from commun import (
    PORT_DEFAUT,
    deriver_adresse_turn,
    lire_hote,
    lire_node_bin,
    lire_proxy_confiance,
    valider_adresse_de_facts,
    valider_hote,
    valider_port_de_facts,
)
from depot_arbre import copier_arbre, rendre_lisible_par_tous
from depot_node import deposer_node, racine_node_source
from fichiers_installes import (
    PORT_TURN,
    ecrire_env,
    ecrire_secret,
    ecrire_turnserver_conf,
    lire_secret_persiste,
)


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

def emettre(evenement: dict) -> None:
    print(json.dumps(evenement), flush=True)


# --- Le PRÉ-VOL : tout ce qui manque se dit AVANT qu'un secret soit écrit --
#
# 🔴 IMPORTANTE DE LA REVUE FINALE DE BRANCHE (30 août 2026), DÉMONTRÉE PAR
# EXÉCUTION. `client/dist/` et `plateforme/node_modules/` sont TOUS DEUX
# gitignorés : un dépôt fraîchement cloné, ou empaqueté par une chaîne qui
# n'a jamais lancé `npm install` ni `npm run build`, n'en porte aucun. Or la
# copie de `client/dist` arrivait AVANT le garde `tsx`, si bien que ce hook
# rendait une TRACE PYTHON (`FileNotFoundError` remontée de
# `shutil.copytree`) là où il sait écrire une phrase partout ailleurs — et
# il la rendait APRÈS avoir déjà écrit `desk.env` AVEC SES DEUX SECRETS
# FRAÎCHEMENT TIRÉS. Un `install` avorté laissait donc un fichier de secrets
# orphelin, ce que la mineure #9 (« un install rejoué reforge le secret en
# silence ») décrivait à moitié sans voir cette moitié-là.
#
# 🔴 LE PRÉ-VOL ÉNUMÈRE, IL NE S'ARRÊTE PAS AU PREMIER MANQUE. Un opérateur
# à qui l'on dit « `client/dist` manque », qui le bâtit, puis à qui l'on dit
# « `node_modules` manque » a payé deux allers-retours là où un seul
# suffisait. Il rend la liste ENTIÈRE.
#
# ⚠️ CE PRÉ-VOL NE REMPLACE PAS LES GARDES D'APRÈS-COPIE (`proto/ts/
# plateforme.ts` retrouvé, `node_modules/.bin/tsx` retrouvé) : ceux-là
# éprouvent que la COPIE a abouti, celui-ci que la SOURCE existe. Les deux
# peuvent échouer indépendamment — une copie partielle sur un disque plein
# ne se voit que par les seconds.

def raisons_de_pre_vol(racine_source: pathlib.Path, prefixe_node) -> list:
    """La liste (éventuellement vide) des raisons de refuser AVANT d'écrire.

    `prefixe_node` est le préfixe Node résolu, ou None ; sa propre raison
    d'échec est passée par `raison_node` dans `main()`.
    """
    raisons = []
    exigences = [
        (racine_source / "plateforme" / "node_modules" / ".bin" / "tsx",
         "`npm start` vaut `tsx src/index.ts` (plateforme/package.json) : "
         "lancer `npm install` dans plateforme/ avant d'empaqueter ou "
         "d'installer ce dépôt"),
        (racine_source / "client" / "dist",
         "la plateforme sert elle-même la page bâtie (PLATEFORME_PAGE) : "
         "lancer `npm run build` dans client/ avant d'empaqueter ou "
         "d'installer ce dépôt — ce répertoire est gitignoré, un clone frais "
         "ne le porte jamais"),
        (racine_source / "proto" / "ts" / "plateforme.ts",
         "plateforme/src importe `../../../proto/ts/…` à l'exécution : sans "
         "ces sources, `npm start` échoue en ERR_MODULE_NOT_FOUND"),
    ]
    for chemin, pourquoi in exigences:
        if not chemin.exists():
            raisons.append(f"{chemin} est absent — {pourquoi}")
    if prefixe_node is None:
        raisons.append("aucun runtime Node à déposer (voir la raison "
                       "ci-dessus)")
    return raisons


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
    # `facts` et `answers` mal typés donnaient un `AttributeError` sur le
    # premier `.get()` — une trace là où ce hook écrit des phrases.
    for nom, valeur in (("answers", answers), ("facts", facts)):
        if not isinstance(valeur, dict):
            print(f"desk install : {nom} doit etre un objet, recu "
                  f"{type(valeur).__name__}", file=sys.stderr)
            return 1

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

    # 🔴 MINEURE #7, RENVERSÉE PAR LA REVUE FINALE DE BRANCHE : une valeur
    # venue de `facts` passe par le MÊME validateur qu'une valeur dérivée —
    # voir `commun.py`, § « Ce qui vient du canal facts », pour la réfutation
    # qui l'impose. Sans cela, `facts["turn_ecoute"]="0.0.0.0"` faisait
    # écouter et relayer coturn sur TOUTES les interfaces, en silence.
    adresses_turn = {}
    for cle in ("turn_ecoute", "turn_relais"):
        brut = facts.get(cle)
        if brut is None:
            continue
        adresses_turn[cle], raison = valider_adresse_de_facts(
            brut, f'facts["{cle}"]', "coturn (TURN_LISTENING_IP/TURN_RELAY_IP)",
            "borner la seule écoute laisserait de surcroît les allocations "
            "de relais sur toutes les interfaces (mesuré le 21 août 2026 : "
            "23 adresses distinctes, dont l'adresse publique)")
        if raison:
            print(f"desk install : {raison}", file=sys.stderr)
            return 1
    if "turn_ecoute" not in adresses_turn:
        try:
            adresses_turn["turn_ecoute"] = deriver_adresse_turn()
        except RuntimeError as exc:
            print(f"desk install : {exc}", file=sys.stderr)
            return 1
    turn_ecoute = adresses_turn["turn_ecoute"]
    turn_relais = adresses_turn.get("turn_relais") or turn_ecoute

    # 🔴 PLATEFORME_HOTE NE DOIT JAMAIS ÊTRE UNIVERSELLE, ET NE DOIT JAMAIS
    # ÊTRE L'ADRESSE TURN (voir le commentaire de `deriver_adresse_turn` :
    # confondre les deux exposait le service sur l'adresse PUBLIQUE). Dérivée
    # séparément, par `commun.lire_hote()` — une adresse FIXE et interne,
    # jamais la route par défaut.
    #
    # 🔴 RONDE DE CORRECTION 1 (29 août 2026) : LA REVUE A DÉMONTRÉ QUE LA
    # FORME PRÉCÉDENTE — `lire_hote()` appelée SEULEMENT quand
    # `facts.get("hote")` était vide — laissait `facts["hote"] = "0.0.0.0"`
    # traverser SANS jamais rencontrer `ECOUTES_UNIVERSELLES` : code 0,
    # `PLATEFORME_HOTE=0.0.0.0` écrit dans `desk.env`. Inatteignable par le
    # moteur réel AUJOURD'HUI (il ne passe aucun `facts` à `install`), mais
    # ce hook accepte ce canal précisément pour « un futur moteur, ou ce
    # fichier de tests » (voir le docstring de tête) — et le contrat de
    # `facts` a gagné des clés PENDANT ce lot même. `valider_hote()` est
    # désormais appelée sur LA VALEUR RETENUE, quelle que soit sa
    # provenance : `facts["hote"]` s'il est présent, `commun.lire_hote()`
    # (qui appelle elle-même `valider_hote`) sinon — un SEUL contrôle, deux
    # chemins d'entrée, jamais l'un sans l'autre.
    brut_hote = facts.get("hote")
    if brut_hote:
        hote_plateforme, raison_hote = valider_hote(brut_hote, origine='facts["hote"]')
    else:
        hote_plateforme, raison_hote = lire_hote()
    if raison_hote:
        print(f"desk install : {raison_hote}", file=sys.stderr)
        return 1

    # PLATEFORME_PROXY_DE_CONFIANCE — voir `commun.py::lire_proxy_confiance`
    # pour le raisonnement complet (une valeur DÉRIVÉE, jamais demandée).
    # Écrite quel que soit auth_mode : elle ne nuit pas en mode motdepasse
    # (elle y sert seulement à faire croire X-Forwarded-For depuis cette
    # adresse), et devient obligatoire côté service en mode pomerium.
    brut_proxy = facts.get("proxy_confiance")
    if brut_proxy is None:
        proxy_confiance = lire_proxy_confiance()
    else:
        proxy_confiance, raison_proxy = valider_adresse_de_facts(
            brut_proxy, 'facts["proxy_confiance"]',
            "PLATEFORME_PROXY_DE_CONFIANCE",
            "`pairDeConfiance` (plateforme/src/http/adresse-source.ts) compare "
            "cette valeur à l'adresse RÉELLE d'un pair (`req.socket."
            "remoteAddress`), jamais à une interface d'écoute : aucun pair "
            "ne se présente jamais sous l'une de ces quatre valeurs, donc la "
            "poser ici ne fait QUE casser la garde du mode pomerium (personne "
            "n'y correspondra jamais), sans rien ouvrir")
        if raison_proxy:
            print(f"desk install : {raison_proxy}", file=sys.stderr)
            return 1

    brut_port = facts.get("port")
    if brut_port is None:
        port = PORT_DEFAUT
    else:
        port, raison_port = valider_port_de_facts(brut_port)
        if raison_port:
            print(f"desk install : {raison_port}", file=sys.stderr)
            return 1

    # --- LE PRÉ-VOL, AVANT LE PREMIER SECRET ------------------------------
    # Voir le commentaire de `raisons_de_pre_vol` : tout ce qui manque se dit
    # ICI, avant qu'un octet de `desk.env` — donc avant qu'un secret — touche
    # le disque.
    prefixe_node, raison_node = racine_node_source()
    raisons = raisons_de_pre_vol(RACINE, prefixe_node)
    if raison_node:
        raisons = [r for r in raisons if not r.startswith("aucun runtime")]
        raisons.append(raison_node)
    if raisons:
        print("desk install : refus AVANT toute ecriture (aucun secret n'a "
              "ete tire, desk.env n'existe pas) :", file=sys.stderr)
        for raison in raisons:
            print(f"  - {raison}", file=sys.stderr)
        return 1

    # 🔴 CORRIGÉ LE 2026-09-08 : install N'ÉTAIT PAS IDEMPOTENT — ces deux
    # secrets étaient tirés SANS CONDITION à chaque exécution, en
    # contradiction directe avec le docstring d'`ecrire_secret` (« tiré UNE
    # SEULE FOIS, jamais recalculé »), qui décrivait un invariant que le
    # code ne tenait pas. Le plan de release repose sur un update qui
    # REJOUE install EN PLACE : sans ce correctif, chaque mise à jour de
    # desk aurait silencieusement fait tourner PLATEFORME_SECRET_JETON
    # (invalidant toutes les sessions ouvertes) et TURN_SECRET (cassant
    # l'authentification coturn en cours). `lire_secret_persiste` relit le
    # `desk.env` DÉJÀ EN PLACE sur la racine CIBLE (jamais la racine
    # source) ; elle ne renvoie une valeur que si elle est réellement
    # réutilisable — fichier absent, clé absente, valeur vide ou blanche
    # comptent tous comme « rien à réutiliser », jamais comme une erreur
    # (voir son propre docstring) — et un secret n'est tiré que quand il
    # n'y a rien à réutiliser.
    #
    # 🔴 REVUE DU 2026-09-08 : UN `desk.env` PRÉSENT MAIS ILLISIBLE N'EST
    # PAS « RIEN À RÉUTILISER » — C'EST UNE PANNE. `lire_secret_persiste`
    # ne rattrape QUE `FileNotFoundError` (une course TOCTOU, équivalente à
    # « le fichier n'a jamais existé ») ; toute autre `OSError`
    # (permissions faussées par une migration partielle, erreur disque, …)
    # remonte jusqu'ici. La rattraper plus haut et tirer un secret neuf
    # quand même referait EXACTEMENT le bug que ce correctif corrige, par
    # une porte plus étroite. Le choix est donc un REFUS BRUYANT, jamais un
    # avertissement qui laisserait l'install continuer : un avertissement
    # qu'on peut ignorer ne protège rien, et faire tourner le jeton de
    # session/le secret TURN parce qu'un fichier était momentanément
    # illisible est pire que refuser d'installer.
    env_existant = sous("etc/nivuus/desk.env")
    try:
        jeton_existant = lire_secret_persiste(env_existant, "PLATEFORME_SECRET_JETON")
        turn_existant = lire_secret_persiste(env_existant, "TURN_SECRET")
    except OSError as exc:
        print(f"desk install : {env_existant} existe mais n'a pas pu etre "
              f"lu ({exc}) - refus AVANT de tirer un secret neuf, pour ne "
              "pas faire tourner PLATEFORME_SECRET_JETON/TURN_SECRET en "
              "silence pendant une panne de lecture passagere.",
              file=sys.stderr)
        return 1
    secret_jeton = jeton_existant or ecrire_secret()
    secret_turn = turn_existant or ecrire_secret()

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

    # 🔴 LE RUNTIME NODE, DÉPOSÉ ET PLUS SEULEMENT SUPPOSÉ (revue finale de
    # branche, 30 août 2026). `NODE_BIN_DEFAUT` désignait un répertoire que
    # RIEN dans ce dépôt ne créait : l'arbre présent sur la machine de
    # développement y avait été copié à la main pendant le lot 10A, et la
    # commande ne vivait que dans un rapport gitignoré. Voir
    # `hooks/depot_node.py` pour le raisonnement complet et pour ce qui est
    # déposé exactement. Le préfixe cible est le PARENT du `bin/` que
    # `lire_node_bin()` rend, pour que les deux ne puissent pas diverger.
    emettre({"event": "progress", "pct": 70,
             "msg": "Dépôt du runtime Node (node, npm, npx)"})
    prefixe_cible = pathlib.PurePosixPath(lire_node_bin()).parent
    deposer_node(prefixe_node, sous(str(prefixe_cible).lstrip("/")))

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
