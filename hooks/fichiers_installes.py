#!/usr/bin/env python3
"""Les fichiers de configuration que `install` POSE, et les secrets qu'il tire.

Extrait de `hooks/install.py` le 30 août 2026, dans un commit DÉDIÉ et
AVANT tout ajout — jamais en comprimant après coup. Motif : la revue finale
de branche exige d'`install.py` deux gardes de plus (un pré-vol qui refuse
avant qu'un secret soit écrit, et la pose du runtime Node), et le fichier
était à 454 lignes sur 500. Ce dépôt a payé douze fois la compression
rétroactive ; la règle est « EXTRAIRE, JAMAIS COMPRIMER, dans une tâche
dédiée AVANT celle qui ajoute ».

Comme `hooks/commun.py`, `hooks/depot_arbre.py` et `hooks/vm.py`, ce module
N'EST PAS un hook exécutable seul (pas de `--phase`, pas de stdin JSON) :
`hooks/install.py` l'importe, et Python ajoute automatiquement le répertoire
du script LANCÉ (`hooks/`) en tête de `sys.path`, donc l'import résout sans
manipulation supplémentaire.

⚠️ EXTRACTION VERBATIM : les trois fonctions et la constante ci-dessous sont
déplacées SANS UNE MODIFICATION DE COMPORTEMENT — leurs docstrings, qui
portent la raison de chaque choix (le mode 0600 atomique, le format coturn
extrapolé), voyagent avec elles. Seuls les déictiques « ci-dessus » /
« ce fichier » qui pointaient vers `install.py` ont été renommés pour ne pas
mentir à leur nouvel emplacement.
"""
import os
import pathlib
import secrets

# Le port TURN standard, celui que docker-compose.coturn.yml pose par
# `--listening-port=3478` — la seule valeur qui fasse correspondre l'URL
# annoncée aux clients (TURN_URL) et le port sur lequel coturn écoute
# réellement.
PORT_TURN = 3478


def ecrire_secret() -> str:
    """Tire un secret FRAIS au hasard — jamais demandé, jamais constant.

    Le mot « FRAIS » est délibéré : cette fonction, à elle seule, tire un
    NOUVEAU secret à CHAQUE appel, sans exception — c'est le comportement
    voulu la toute première fois qu'un secret est nécessaire (aucun n'existe
    encore à réutiliser). `secrets.token_hex(32)` rend 64 caractères
    hexadécimaux, largement au-dessus du minimum de 32 que
    `LONGUEUR_SECRET_MIN` exige.

    🔴 CETTE FONCTION SEULE NE PORTE PAS L'INVARIANT « tiré une seule fois,
    jamais recalculé » — un docstring antérieur le prétendait ICI, à tort
    (bug réel, trouvé et corrigé le 2026-09-08 : `install.py` appelait cette
    fonction sans condition à chaque exécution, donc deux installations sur
    la même racine tiraient deux secrets DIFFÉRENTS — `PLATEFORME_SECRET_JETON`
    compris — et invalidaient silencieusement toutes les sessions ainsi que
    l'authentification coturn à chaque REPLAY d'install, exactement le
    chemin qu'emprunte l'update de ce plan). `PLATEFORME_SECRET_JETON` n'a
    d'ailleurs AUCUN défaut côté produit
    (`plateforme/src/config.ts::lireConfig`) : c'est précisément pourquoi un
    secret qui change à chaque exécution du hook est un défaut, jamais une
    fonctionnalité.

    L'invariant est maintenant porté par l'APPELANT, jamais par cette
    fonction : `install.py` doit d'abord tenter `lire_secret_persiste()` sur
    le `desk.env` déjà en place à la racine CIBLE, et n'appeler
    `ecrire_secret()` que si rien n'y est réutilisable.
    """
    return secrets.token_hex(32)


def lire_secret_persiste(chemin_env: pathlib.Path, cle: str) -> str | None:
    """Relit un secret déjà écrit par une installation antérieure de `desk.env`.

    C'est CETTE fonction, appelée par `install.py` AVANT `ecrire_secret()`,
    qui porte réellement l'invariant « tiré une seule fois, jamais
    recalculé » que le docstring d'`ecrire_secret` décrivait sans
    l'appliquer (bug corrigé le 2026-09-08 — voir son propre docstring).

    Trois cas se traitent comme « rien à réutiliser », jamais comme une
    erreur qui ferait échouer l'install pour une raison qui n'a rien à voir
    avec l'installation elle-même — dans TOUS les trois, `return None`,
    silencieusement :
      - `chemin_env` n'existe pas (premier install sur cette racine) ;
      - `chemin_env` a disparu ENTRE le test d'existence et la lecture — une
        vraie course, mais qui revient au même cas que ci-dessus : rien à
        lire, jamais une panne à signaler ;
      - `cle` est présente mais sa valeur est vide, ou faite uniquement
        d'espaces (un fichier tronqué ou modifié à la main), ou `chemin_env`
        existe sans porter `cle` (une installation antérieure d'une version
        qui n'écrivait pas encore cette clé).

    🔴 UN QUATRIÈME CAS EST DÉLIBÉRÉMENT *EXCLU* DE CETTE LISTE, ET C'EST
    LE POINT DE CE CORRECTIF (2026-09-08, revue) : `chemin_env` EXISTE mais
    ne peut PAS être lu — permissions faussées par une migration partielle,
    erreur disque, tout ce qui n'est pas « le fichier n'est simplement pas
    là ». Un `except OSError` qui avalait CE cas-là aussi referait
    EXACTEMENT le bug que ce module corrige, par une porte plus étroite :
    un `desk.env` présent mais momentanément illisible ferait tirer un
    secret NEUF EN SILENCE — la même rotation silencieuse du bug d'origine,
    juste déclenchée différemment. Cette fonction ne l'avale donc PAS :
    seule `FileNotFoundError` (la course TOCTOU ci-dessus) est rattrapée ;
    toute AUTRE `OSError` (`PermissionError`, une erreur disque, …) se
    propage à l'appelant tel quel. `install.py` la rattrape à son tour et
    REFUSE l'installation plutôt que d'en tirer un secret halluciné — voir
    son propre commentaire à l'appel.
    """
    if not chemin_env.is_file():
        return None
    try:
        contenu = chemin_env.read_text(encoding="utf-8")
    except FileNotFoundError:
        return None
    for ligne in contenu.splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.startswith("#") or "=" not in ligne:
            continue
        cle_ligne, _, valeur = ligne.partition("=")
        if cle_ligne.strip() != cle:
            continue
        valeur = valeur.strip()
        return valeur or None
    return None


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
    legs nommé, pas un oubli — voir le document de résultats du chantier.
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
