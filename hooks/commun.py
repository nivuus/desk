"""Module shared between the THREE hooks of the desk package.

⚠️ The header said "(`resolve.py`, `install.py`)". It has been FALSE since
batch 10A: `activate.py` uses it too, through `administration.py`
(`from commun import lire_node_bin`) — flagged by the final branch review,
August 30th, 2026, and fixed here rather than left to age.

Extracted on August 29th, 2026 (correction round 1 on task 4): the review
flagged `interface_de_route_par_defaut()` and `adresse_ipv4_de()` duplicated
BYTE FOR BYTE between the two hooks, and `PORT_DEFAUT` duplicated with its
reason twice.

⚠️ IT IS NOT A DEPENDENCY OUTSIDE THE PACKAGE — the argument "each hook
must stay runnable alone" (protocol described in `resolve.py`) bears on
the absence of a dependency on ANOTHER repository (the engine, `installer/`), not
on the absence of a sibling module INSIDE the package: `console` (the
neighbouring repository `installer/console/`) does exactly that for its own hooks
(`hooks/install.py` imports `retro.py` and `guest_steps.py` from the package
root, via `HERE = os.path.dirname(...)` + `sys.path.insert(0, HERE)`).

Here, no `sys.path.insert` is needed: `commun.py` lives in the same
directory (`hooks/`) as `resolve.py` and `install.py`, and Python already
automatically adds the executed script's directory at the head of `sys.path` — it is
the same reason `resolve.py` already imports `guest_steps` without
tinkering in the neighbouring package. A simple `import commun` is enough from
both hooks.
"""
import os
import re
import subprocess

# 🔴 THE DEFAULT PORT IS 3445, AND IT IS DERIVED, NEVER ASKED.
#
# /etc/pomerium/config.yaml carries a route `from: https://app.allanic.me`
# to `to: http://127.0.0.1:3445` (read on August 29th, 2026): it is the only
# port that makes the public route already in place work. It is not a
# fifth wizard question: the operator has no information that would
# let them answer differently without breaking the Pomerium route already
# in place. This reason now only lives HERE — neither `resolve.py` nor
# `install.py` repeat it, they import the value.
PORT_DEFAUT = 3445


def interface_de_route_par_defaut():
    """The network device of the default IPv4 route, or None."""
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
    """The first IPv4 address carried by `interface`, or None."""
    try:
        r = subprocess.run(["ip", "-4", "-o", "addr", "show", "dev", interface],
                            capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0:
        return None
    correspond = re.search(r"inet\s+(\d+\.\d+\.\d+\.\d+)", r.stdout)
    return correspond.group(1) if correspond else None


# --- Deriving the TURN address (PUBLIC) ------------------------------
#
# ⚠️ MOVED HERE FROM `install.py` ON AUGUST 30TH, 2026, in a dedicated commit:
# the file fell back to 494 lines out of 500 after the final review's
# guards, a margin the next workstream would immediately lose again. No
# behaviour change; `install.py` now imports it, as it
# already imported the two functions it uses.
#
# 🔴 FIXED IN BATCH 10A (August 29th, 2026): THIS BLOCK WAS CALLED
# `deriver_adresse_hote()` AND ITS COMMENT ASSERTED THAT PLATEFORME_HOTE,
# TURN_LISTENING_IP AND TURN_RELAY_IP WERE THE SAME ADDRESS. IT WAS FALSE,
# AND IT WAS A REAL BUG, NOT A COMMENT INACCURACY: measured on
# August 29th, 2026 with `/usr/bin/ip` (outside any shell alias), the interface
# of the DEFAULT IPv4 route on this machine is `ppp0` (PPPoE), whose
# address is PUBLIC (90.87.35.18) — not `internalBridge`
# (192.168.3.1). `install.py` therefore set `PLATEFORME_HOTE=90.87.35.18`,
# exposing the remote desktop on the public internet WITHOUT Pomerium in front of it,
# a hole the universal-listen guard of `config.ts` CANNOT
# catch (90.87.35.18 is not one of the four universal values).
#
# This block now derives ONLY the TURN address (public, by
# construction: coturn must be reachable from the internet by
# WebRTC clients behind a restrictive NAT — it is the ONLY legitimate role
# of the default route here). `PLATEFORME_HOTE` is derived separately,
# by `commun.lire_hote()` (a FIXED, internal address, never the default
# route) — see its comment for the complete detail of the bug and the
# fix.
#
# `interface_de_route_par_defaut()` and `adresse_ipv4_de()` come from
# `commun.py` (imported at the top of the file): they were duplicated
# byte for byte with `resolve.py` before correction round 1.

def deriver_adresse_turn() -> str:
    """The PUBLIC IPv4 address of the default route, or raises RuntimeError.

    ⚠️ NEVER use this function for `PLATEFORME_HOTE` — see the
    comment above. It ONLY serves TURN_LISTENING_IP/TURN_RELAY_IP.

    🔴 NEVER A UNIVERSAL LISTEN: this function never returns
    '0.0.0.0'/'::'/'[::]'/'*' — it fails rather than inventing a value,
    exactly like `resolve.py::deriver_adresses_turn`.
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


# 🔴 THE PLATFORM'S LISTEN ADDRESS MUST NEVER BE DERIVED FROM THE
# DEFAULT ROUTE — A REAL BUG FOUND AND FIXED HERE (batch 10A, August 29th, 2026).
#
# `interface_de_route_par_defaut()` + `adresse_ipv4_de()` above resolve
# the interface of the INTERNET route (`ip -4 route show default`). On THIS
# machine, measured on August 29th, 2026 with `/usr/bin/ip` (outside any
# interactive shell alias, which redefines `ip` as `myip && localip`):
#
#     default via 193.253.160.3 dev ppp0 ...
#     ppp0: inet 90.87.35.18 peer 193.253.160.3/32 ...
#
# `ppp0` is a PPPoE link, and its address is PUBLIC. Before this
# fix, `install.py` reused THIS SAME derivation for
# `PLATEFORME_HOTE` (confusing it with the TURN address) — which would have
# made THE REMOTE DESKTOP LISTEN ON THE PUBLIC ADDRESS, exposed to anyone
# on the internet WITHOUT going through Pomerium. It is a hole the universal-listen
# guard of `plateforme/src/config.ts` CANNOT catch:
# 90.87.35.18 is neither `0.0.0.0` nor `::`, it is an ORDINARY address — the
# guard only bites on the four universal values, never on "a
# routable but public address".
#
# The default-route derivation STAYS correct for
# TURN_LISTENING_IP/TURN_RELAY_IP (coturn MUST be reachable from the
# public internet to serve as a relay for WebRTC clients behind a
# restrictive NAT): it is the ONLY role `interface_de_route_par_defaut()`
# must keep. `PLATEFORME_HOTE` is a DIFFERENT address, with a
# DIFFERENT constraint: reachable by Pomerium (`network_mode: host`,
# same machine) AND by the Windows VM (192.168.3.2/24), NEVER by
# the public internet.
#
# `192.168.3.1` (interface `internalBridge`) satisfies both: it is
# the address VERIFIED present on August 29th, 2026 (`ip -4 addr show`), on the
# SAME subnet as the VM, and not universal. Hard-coded here, exactly
# like `PORT_DEFAUT` above and for the same reason: the operator has
# no information that would let them answer differently without
# breaking either the VM or Pomerium — it is not a wizard question,
# it is a network topology fact of THIS appliance. Overridable through
# `DESK_HOTE` (tests, or a future topology change).
HOTE_DEFAUT = "192.168.3.1"

# Les quatre valeurs qui font écouter un service sur TOUTES les interfaces —
# reprises telles quelles de `plateforme/src/config.ts::ECOUTES_UNIVERSELLES`.
ECOUTES_UNIVERSELLES = ("0.0.0.0", "::", "[::]", "*")


def _est_universelle(brut) -> bool:
    """Vrai si `brut` est l'une des quatre écoutes universelles.

    Le SEUL endroit de ce package qui teste cette appartenance : les trois
    validateurs publics ci-dessous s'en servent, aucun ne redit le test.
    """
    return isinstance(brut, str) and brut.strip() in ECOUTES_UNIVERSELLES


def valider_hote(brut: str, origine: str = "DESK_HOTE"):
    """Le SEUL contrôle qui refuse une écoute universelle pour
    `PLATEFORME_HOTE`, quelle que soit la PROVENANCE de `brut`.

    🔴 EXTRAIT DE `lire_hote()` EN RONDE DE CORRECTION 1 (lot 10A,
    29 août 2026) : la revue a démontré qu'`install.py` appelait
    `lire_hote()` SEULEMENT quand `facts.get("hote")` était vide —
    `facts["hote"] = "0.0.0.0"` traversait donc SANS jamais rencontrer ce
    contrôle, rendait code 0, et écrivait `PLATEFORME_HOTE=0.0.0.0` dans
    `desk.env`. Inatteignable par le moteur réel AUJOURD'HUI (qui ne passe
    aucun `facts` à `install`), mais le docstring de tête d'`install.py`
    dit lui-même que ce canal existe pour « un futur moteur, ou ce fichier
    de tests » — et le contrat de `facts` a gagné des clés PENDANT ce lot
    même. Un garde qui ne mord que sur UN des deux chemins d'entrée n'est
    pas le garde que ce lot existe pour poser. `install.py` appelle
    désormais CETTE fonction sur `facts.get("hote")` s'il est présent,
    EXACTEMENT comme sur la valeur dérivée — même contrôle, quelle que soit
    la provenance.

    `origine` ne sert qu'au message de refus (« DESK_HOTE » pour la valeur
    dérivée par défaut, « facts["hote"] » pour une valeur fournie par le
    moteur) — jamais à la logique : la garde est IDENTIQUE dans les deux cas.

    Rend `(hote, None)` en succès, `(None, raison)` si `brut` est une
    écoute universelle : un refus ICI, jamais un service qui démarre puis
    s'expose sur toutes les interfaces.
    """
    if _est_universelle(brut):
        return None, (
            f"{origine}={brut!r} est une écoute universelle : PLATEFORME_HOTE "
            "ne doit jamais l'être (voir plateforme/src/config.ts, la garde du "
            "mode pomerium — et la doctrine de ce package, plus stricte : "
            "aucune écoute universelle, dans aucun mode)"
        )
    return brut, None


def lire_hote():
    """`HOTE_DEFAUT`, surchargeable par `DESK_HOTE` (tests, ou un futur
    changement de topologie réseau) — jamais dérivée de la route par défaut,
    voir le commentaire ci-dessus. Passe TOUJOURS par `valider_hote` — voir
    son docstring pour pourquoi ce n'est plus un `if` inline ici.

    Rend `(hote, None)` en succès, `(None, raison)` si la valeur retenue
    (défaut ou surchargée) est une écoute universelle.
    """
    brut = os.environ.get("DESK_HOTE") or HOTE_DEFAUT
    return valider_hote(brut, origine="DESK_HOTE")


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


# --- Ce qui vient du canal `facts` : validé, jamais cru sur parole ---------
#
# 🔴 MINEURE #7 DE LA TÂCHE 4, DONT LA RAISON A ÉTÉ RÉFUTÉE PAR CETTE BRANCHE
# ELLE-MÊME. Elle était classée « reste due » au motif que « `facts` a une
# forme fixe, produite par `resolve.py` du même package et jamais par un
# tiers ». Le lot 10A a démontré l'inverse SUR LA CLÉ VOISINE : `facts["hote"]`
# traversait `install.py` sans jamais rencontrer la garde des écoutes
# universelles, code 0, `PLATEFORME_HOTE=0.0.0.0` écrit dans `desk.env` — il a
# fallu une Importante pour le fermer. La même raison restait écrite pour
# `turn_ecoute`, `turn_relais`, `proxy_confiance` et `port`. La revue finale
# de branche l'a renversée ; ces trois fonctions sont ce renversement.
#
# ⚠️ CE QUE CES VALIDATEURS NE PROMETTENT PAS : que l'adresse soit PRIVÉE.
# Le ruling du lot 10A a refusé une liste blanche RFC1918 — elle créerait une
# seconde notion d'« adresse sûre », divergente de la garde du produit
# (`plateforme/src/config.ts::ECOUTES_UNIVERSELLES`, quatre littéraux), et
# interdirait des déploiements légitimes. Cette réserve tient, et elle est
# nommée au document de résultats.


def valider_adresse_de_facts(brut, origine: str, role: str, consequence: str):
    """Une adresse venue de `facts` : ni vide, ni d'un autre type, ni
    universelle.

    Rend `(adresse, None)` en succès, `(None, raison)` sinon. `role` nomme ce
    que l'adresse sert (« coturn », « PLATEFORME_PROXY_DE_CONFIANCE ») — il
    n'entre que dans le message, jamais dans la logique.

    🔴 `consequence` EXISTE PARCE QUE CE VALIDATEUR SERT TROIS RÔLES
    (`turn_ecoute`, `turn_relais`, `proxy_confiance`), ET LE MESSAGE
    D'ÉCOUTE UNIVERSELLE N'EST PAS LE MÊME POUR CHACUN — revue finale de
    branche, 30 août 2026 : le message était rédigé POUR coturn (« les
    allocations de relais sur toutes les interfaces ») et rendait donc un
    refus juste avec un motif HORS SUJET dès que `role` valait
    `PLATEFORME_PROXY_DE_CONFIANCE`, qui ne relaie rien. `consequence` nomme
    ce que CE rôle précis romprait ; l'appelant la fournit, jamais ce
    validateur ne la devine.
    """
    if not isinstance(brut, str) or not brut.strip():
        return None, (
            f"{origine}={brut!r} n'est pas une adresse exploitable pour "
            f"{role} : une chaîne non vide est attendue"
        )
    if _est_universelle(brut):
        return None, (
            f"{origine}={brut!r} est une écoute universelle : {role} ne doit "
            f"jamais l'être — {consequence}"
        )
    return brut.strip(), None


def valider_port_de_facts(brut, origine: str = 'facts["port"]'):
    """Un port venu de `facts` : un entier de 1 à 65535, jamais autre chose.

    🔴 `int(facts.get("port"))` LEVAIT une `ValueError` non rattrapée sur
    n'importe quelle valeur non numérique — donc une trace Python et un code
    de sortie 1 chez l'opérateur, là où `install.py` sait écrire une phrase
    partout ailleurs. Un booléen est refusé explicitement : `int(True)` vaut
    `1`, un port parfaitement valide et parfaitement absurde.
    """
    if isinstance(brut, bool):
        return None, f"{origine}={brut!r} est un booléen, pas un port"
    if isinstance(brut, str):
        brut_nettoye = brut.strip()
        if not brut_nettoye.isdigit():
            return None, f"{origine}={brut!r} n'est pas un entier"
        valeur = int(brut_nettoye)
    elif isinstance(brut, int):
        valeur = brut
    else:
        return None, f"{origine}={brut!r} n'est pas un entier"
    if not 1 <= valeur <= 65535:
        return None, f"{origine}={valeur} est hors de la plage 1-65535"
    return valeur, None
