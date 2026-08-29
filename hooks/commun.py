"""Module partagé entre les hooks du package desk (`resolve.py`, `install.py`).

Extrait le 29 août 2026 (ronde de correction 1 sur la tâche 4) : la revue a
relevé `interface_de_route_par_defaut()` et `adresse_ipv4_de()` dupliquées
OCTET POUR OCTET entre les deux hooks, et `PORT_DEFAUT` dupliqué avec sa
raison en double.

⚠️ CE N'EST PAS UNE DÉPENDANCE HORS DU PACKAGE — l'argument « chaque hook
doit rester exécutable seul » (protocole décrit dans `resolve.py`) porte sur
l'absence de dépendance vers un AUTRE dépôt (le moteur, `installer/`), pas
sur l'absence de module frère À L'INTÉRIEUR du package : `console` (dépôt
voisin `installer/console/`) fait exactement cela pour ses propres hooks
(`hooks/install.py` importe `retro.py` et `guest_steps.py` depuis la racine
du package, via `HERE = os.path.dirname(...)` + `sys.path.insert(0, HERE)`).

Ici, aucun `sys.path.insert` n'est nécessaire : `commun.py` vit dans le même
répertoire (`hooks/`) que `resolve.py` et `install.py`, et Python ajoute déjà
automatiquement le répertoire du script exécuté en tête de `sys.path` — c'est
la même raison pour laquelle `resolve.py` importe déjà `guest_steps` sans
bricolage dans le package voisin. Un simple `import commun` suffit depuis
les deux hooks.
"""
import os
import re
import subprocess

# 🔴 LE PORT PAR DÉFAUT EST 3445, ET IL EST DÉRIVÉ, JAMAIS DEMANDÉ.
#
# /etc/pomerium/config.yaml porte une route `from: https://app.allanic.me`
# vers `to: http://127.0.0.1:3445` (relevé le 29 août 2026) : c'est le seul
# port qui fait marcher la route publique déjà en place. Ce n'est pas une
# cinquième question du wizard : l'opérateur n'a aucune information qui lui
# permettrait d'y répondre différemment sans casser la route Pomerium déjà
# en place. Cette raison ne vit désormais qu'ICI — ni `resolve.py` ni
# `install.py` ne la répètent, ils importent la valeur.
PORT_DEFAUT = 3445


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


# 🔴 L'ADRESSE D'ÉCOUTE DE LA PLATEFORME NE DOIT JAMAIS ÊTRE DÉRIVÉE DE LA
# ROUTE PAR DÉFAUT — BUG RÉEL TROUVÉ ET CORRIGÉ ICI (lot 10A, 29 août 2026).
#
# `interface_de_route_par_defaut()` + `adresse_ipv4_de()` ci-dessus résolvent
# l'interface de la route INTERNET (`ip -4 route show default`). Sur CETTE
# machine, mesuré le 29 août 2026 avec `/usr/bin/ip` (hors de tout alias de
# shell interactif, qui redéfinit `ip` en `myip && localip`) :
#
#     default via 193.253.160.3 dev ppp0 ...
#     ppp0: inet 90.87.35.18 peer 193.253.160.3/32 ...
#
# `ppp0` est une liaison PPPoE, et son adresse est PUBLIQUE. Avant ce
# correctif, `install.py` réutilisait CETTE MÊME dérivation pour
# `PLATEFORME_HOTE` (en la confondant avec l'adresse TURN) — ce qui aurait
# fait ÉCOUTER LE BUREAU DISTANT SUR L'ADRESSE PUBLIQUE, exposé à quiconque
# sur l'internet SANS passer par Pomerium. C'est un trou que la garde des
# écoutes universelles de `plateforme/src/config.ts` ne peut PAS attraper :
# 90.87.35.18 n'est ni `0.0.0.0` ni `::`, c'est une adresse ORDINAIRE — la
# garde ne mord que sur les quatre valeurs universelles, jamais sur « une
# adresse routable mais publique ».
#
# La dérivation route-par-défaut RESTE correcte pour
# TURN_LISTENING_IP/TURN_RELAY_IP (coturn DOIT être joignable depuis
# l'internet public pour servir de relais à des clients WebRTC derrière un
# NAT restrictif) : c'est le SEUL rôle qu'`interface_de_route_par_defaut()`
# doit garder. `PLATEFORME_HOTE` est une adresse DIFFÉRENTE, avec une
# contrainte DIFFÉRENTE : joignable par Pomerium (`network_mode: host`,
# même machine) ET par la VM Windows (192.168.3.2/24), JAMAIS par
# l'internet public.
#
# `192.168.3.1` (interface `internalBridge`) satisfait les deux : c'est
# l'adresse VÉRIFIÉE présente le 29 août 2026 (`ip -4 addr show`), sur le
# MÊME sous-réseau que la VM, et non universelle. Hardcodée ici, exactement
# comme `PORT_DEFAUT` ci-dessus et pour la même raison : l'opérateur n'a
# aucune information qui lui permettrait d'y répondre différemment sans
# casser soit la VM soit Pomerium — ce n'est pas une question de wizard,
# c'est une donnée de topologie réseau de CETTE appliance. Surchargeable par
# `DESK_HOTE` (tests, ou un futur changement de topologie).
HOTE_DEFAUT = "192.168.3.1"

# Les quatre valeurs qui font écouter un service sur TOUTES les interfaces —
# reprises telles quelles de `plateforme/src/config.ts::ECOUTES_UNIVERSELLES`.
ECOUTES_UNIVERSELLES = ("0.0.0.0", "::", "[::]", "*")


def lire_hote():
    """`HOTE_DEFAUT`, surchargeable par `DESK_HOTE` (tests, ou un futur
    changement de topologie réseau) — jamais dérivée de la route par défaut,
    voir le commentaire ci-dessus.

    Rend `(hote, None)` en succès, `(None, raison)` si la valeur retenue
    (défaut ou surchargée) est une écoute universelle : un refus ICI, jamais
    un service qui démarre puis s'expose sur toutes les interfaces.
    """
    brut = os.environ.get("DESK_HOTE") or HOTE_DEFAUT
    if brut.strip() in ECOUTES_UNIVERSELLES:
        return None, (
            f"DESK_HOTE={brut!r} est une écoute universelle : PLATEFORME_HOTE "
            "ne doit jamais l'être (voir plateforme/src/config.ts, la garde du "
            "mode pomerium — et la doctrine de ce package, plus stricte : "
            "aucune écoute universelle, dans aucun mode)"
        )
    return brut, None


# --- PLATEFORME_PROXY_DE_CONFIANCE : le trou C du lot 10A -------------------
#
# `plateforme/src/config.ts::lireConfig` refuse de démarrer en mode
# `PLATEFORME_AUTH=pomerium` sans `PLATEFORME_PROXY_DE_CONFIANCE` (l'identité
# arrive sinon dans un en-tête `X-Pomerium-Claim-Email` en clair, qu'aucune
# signature ne vérifie). Avant ce correctif, `wizard.yaml` ne posait AUCUNE
# question permettant de la déclarer, et `hooks/resolve.py::valider_auth_mode`
# REFUSAIT donc catégoriquement `auth_mode=pomerium` — ce qui rendait le mode
# choisi explicitement par le propriétaire du dépôt (lot 10A, 29 août 2026)
# IMPOSSIBLE à installer.
#
# 🔴 LE CHEMIN RETENU : LA MÊME DOCTRINE QUE `PORT_DEFAUT`/`HOTE_DEFAUT` —
# UNE VALEUR DÉRIVÉE, JAMAIS UNE CINQUIÈME QUESTION DU WIZARD. L'opérateur
# n'a aucune information qui lui permettrait de répondre correctement : la
# valeur juste dépend de la topologie de CETTE machine (Pomerium tourne en
# `network_mode: host`), pas d'un choix qu'un opérateur ferait au clavier.
#
# ⚠️ LA VALEUR N'EST PAS MESURÉE, ELLE EST RAISONNÉE — ET C'EST DIT ICI, PAS
# MAQUILLÉ. Pomerium (réseau hôte) contacte `PLATEFORME_HOTE:PLATEFORME_PORT`,
# c'est-à-dire une adresse qui lui est LOCALE (`192.168.3.1`, portée par
# `internalBridge`, pas `127.0.0.1`). Sous Linux, une connexion vers une
# adresse IPv4 qui appartient déjà à une interface locale (et n'est pas
# `127.0.0.1`) est acheminée par la table de routage locale et se présente
# côté serveur avec `remoteAddress` = CETTE MÊME ADRESSE (le noyau ne réécrit
# pas la source en `127.0.0.1` pour une adresse locale non-loopback) — donc
# `192.168.3.1`, la même valeur que `PLATEFORME_HOTE`. C'est un raisonnement,
# PAS UNE MESURE : rien n'écoutait sur le port au moment de cette
# reconnaissance (29 août 2026), et Pomerium n'est pas encore reciblé vers
# cette adresse (c'est le volet 10B). **Le volet 10B DOIT confirmer cette
# valeur par la ligne d'annonce du démarrage de la plateforme
# (`proxys de confiance retenus=…`, voir `plateforme/src/http/annonces.ts`)
# une fois sa route repointée vers `192.168.3.1:3445`.**
#
# Surchargeable par `DESK_PROXY_DE_CONFIANCE` (tests, ou une fois 10B mesure
# une valeur différente).
PROXY_DEFAUT = HOTE_DEFAUT


def lire_proxy_confiance():
    """`PROXY_DEFAUT`, surchargeable par `DESK_PROXY_DE_CONFIANCE`. Ne lève
    et ne refuse jamais : c'est une valeur RAISONNÉE (voir le commentaire
    ci-dessus), jamais absente."""
    return os.environ.get("DESK_PROXY_DE_CONFIANCE") or PROXY_DEFAUT


# --- Node.js à l'échelle du système (lot 10A, 29 août 2026, problème A) ----
#
# 🔴 IL N'Y A AUCUN NODE À L'ÉCHELLE DU SYSTÈME SUR CETTE MACHINE, ET
# `hooks/assets/desk-plateforme.service` PORTAIT `ExecStart=/usr/bin/npm
# start` — UN CHEMIN QUI N'EXISTE PAS. Vérifié le 29 août 2026 : ni
# `/usr/bin/node`, ni `/usr/bin/npm`, ni `/usr/local/bin/node`, aucun paquet
# Debian `nodejs`. `node`/`npm` ne sont que des FONCTIONS zsh qui sourcent
# nvm depuis `$HOME/.nvm` (mode 700, root seulement) — ni systemd
# (`DynamicUser=yes`) ni un hook lancé pour un autre utilisateur ne peuvent
# les atteindre, et un `subprocess.run(["node", ...])` en Python ne passe de
# toute façon jamais par une fonction de shell interactif.
#
# 🔴 LE REMÈDE EST DE DÉPLOYER NODE, PAS DE MAQUILLER LE DÉFAUT. Un lien
# symbolique `/usr/bin/npm` aurait caché le problème au lieu de le fermer.
# Ce lot copie l'arbre nvm `v24.9.0` (celui qui satisfait déjà
# `engines.node` de `plateforme/package.json`) vers un emplacement SYSTÈME,
# lisible par tous (`a+rX`) — SANS les paquets globaux nvm sans rapport avec
# ce dépôt (`bats`, `corepack`, `@github/copilot`, `@google/gemini-cli` : à
# eux seuls 237 Mio, mesurés le 29 août 2026, pour un besoin qui se résume à
# `bin/node` et `lib/node_modules/npm/`). Voir le rapport du lot pour la
# commande exacte de déploiement.
#
# `NODE_BIN_DEFAUT` est le chemin du RÉPERTOIRE portant `node`/`npm`/`npx` —
# jamais un chemin de binaire seul — pour que l'unité systemd ET les hooks
# d'administration puissent en dériver À LA FOIS `ExecStart`/`Environment=
# PATH=` et le chemin absolu de `npm` invoqué en sous-processus.
# Surchargeable par `DESK_NODE_BIN` (tests, ou une future version de Node).
NODE_BIN_DEFAUT = "/opt/nivuus/node/bin"


def lire_node_bin():
    """`NODE_BIN_DEFAUT`, surchargeable par `DESK_NODE_BIN`."""
    return os.environ.get("DESK_NODE_BIN") or NODE_BIN_DEFAUT
