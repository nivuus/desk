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
    """Tire un secret au hasard — jamais demandé, jamais constant.

    🔴 `PLATEFORME_SECRET_JETON` n'a AUCUN défaut côté produit
    (`plateforme/src/config.ts::lireConfig`) : un défaut aléatoire À CHAQUE
    DÉMARRAGE invaliderait toutes les sessions à chaque redémarrage du
    service. Le hook tire donc le secret UNE SEULE FOIS, à l'installation,
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
