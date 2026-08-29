#!/usr/bin/env python3
"""Hook resolve du package desk.

🔴 CE HOOK COURT AVANT partition() : à l'instant où le moteur l'appelle, le
disque cible n'existe pas encore. C'est ce qui donne sa valeur au refus — il
arrive à l'assistant, jamais sur un disque déjà effacé. Voir le module
`packages.runner` du moteur (dépôt voisin `installer`) : resolve est
strictement en lecture seule PAR CONVENTION, pas par bac à sable — rien
n'empêche techniquement une écriture, mais le moteur ne lit et n'utilise
jamais rien que ce hook aurait écrit.

UN REFUS EST UNE DONNÉE, JAMAIS UNE EXCEPTION. Tout chemin qui peut échouer
ici finit par un événement `{"event":"refuse","reason":"…"}` et un code de
sortie 0 — jamais une exception non rattrapée : une trace donne à l'opérateur
un code de sortie qu'il ne peut pas exploiter, une phrase lui donne une
raison qu'il peut lire avant que son disque soit touché.

Protocole (voir `installer/packages/runner.py` du dépôt voisin) : lit
{"hw":…, "answers":…} sur stdin, écrit un objet JSON par ligne sur stdout.
Ce hook n'émet jamais d'événement `platform` : le tier de desk est
`userspace` (voir `nivuus-package.yaml`), qui interdit kernel-cmdline,
modules et hugepages — desk n'a rien à poser sur la ligne de commande noyau.
"""
import json
import os
import pathlib
import re
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]

# 🔴 LE PORT EST 3445, ET IL EST DÉRIVÉ, JAMAIS DEMANDÉ.
#
# /etc/pomerium/config.yaml porte une route `from: https://app.allanic.me`
# vers `to: http://127.0.0.1:3445` (relevé le 29 août 2026) : c'est le port
# que la plateforme doit prendre pour que l'adresse publique serve — aucun
# autre choix ne fait marcher l'existant. Ce n'est pas une cinquième question
# du wizard : l'opérateur n'a aucune information qui lui permettrait d'y
# répondre différemment sans casser la route Pomerium déjà en place.
#
# Surchargeable par DESK_PORT pour que les tests (et un futur opérateur qui
# changerait de proxy) puissent poser une autre valeur ; le défaut reste 3445.
PORT_DEFAUT = 3445

MODES_CONNUS = ("motdepasse", "pomerium")


def emettre(evenement: dict) -> None:
    print(json.dumps(evenement), flush=True)


def refuser(raison: str) -> None:
    emettre({"event": "refuse", "reason": raison})


def lire_borne_node():
    """Relit `engines.node` dans plateforme/package.json.

    🔴 RELUE DANS LE FICHIER, JAMAIS RECOPIÉE DEPUIS UN PLAN : un intervalle
    recopié survit à la réalité qu'il décrivait — le naufrage du « 487 » que
    CLAUDE.md nomme, appliqué ici d'avance.

    Rend (borne, None) en succès, ou (None, raison) si le fichier est
    illisible ou ne déclare pas `engines.node` — un chemin d'échec comme un
    autre, qui se traduit en refus, jamais en exception.
    """
    chemin = RACINE / "plateforme" / "package.json"
    try:
        contenu = json.loads(chemin.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return None, f"impossible de lire {chemin} : {exc}"
    borne = (contenu.get("engines") or {}).get("node")
    if not borne or not isinstance(borne, str):
        return None, f"{chemin} ne déclare aucune borne engines.node exploitable"
    return borne, None


def parser_borne(borne: str):
    """Parse une borne à deux clauses ('>=A.B.C <X.Y.Z') en deux tuples.

    Ne comprend QUE les deux opérateurs réellement présents dans
    `plateforme/package.json` (`>=` et `<`) : un format plus riche que celui
    qu'on a mesuré n'a pas à être deviné, il doit faire échouer le parsing —
    et donc refuser, jamais planter.
    """
    mini = maxi = None
    for clause in borne.split():
        correspond = re.match(r"^(>=|<)(\d+)\.(\d+)\.(\d+)$", clause)
        if not correspond:
            return None
        operateur, a, b, c = correspond.groups()
        valeur = (int(a), int(b), int(c))
        if operateur == ">=":
            mini = valeur
        else:
            maxi = valeur
    if mini is None or maxi is None:
        return None
    return mini, maxi


def version_node_locale():
    """`node --version` de la machine qui exécute CE hook, ou None.

    resolve tourne avant le redémarrage vers la cible : ce qu'il peut
    éprouver est le node de la machine qui l'exécute réellement (l'hôte de
    l'assistant, ou celui des tests) — jamais celui d'une cible pas encore
    installée.
    """
    try:
        r = subprocess.run(["node", "--version"], capture_output=True,
                            text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0:
        return None
    return r.stdout.strip().lstrip("v")


def valider_node():
    """Rend (version, None) en succès, (None, raison) sinon."""
    borne_brute, raison = lire_borne_node()
    if raison:
        return None, raison
    bornes = parser_borne(borne_brute)
    if bornes is None:
        return None, f"borne engines.node illisible : {borne_brute!r}"
    mini, maxi = bornes
    version = version_node_locale()
    if version is None:
        return None, "node est introuvable sur cette machine ; plateforme/ l'exige"
    try:
        version_tuple = tuple(int(x) for x in version.split(".")[:3])
    except ValueError:
        return None, f"version de node illisible : {version!r}"
    if not (mini <= version_tuple < maxi):
        return None, (
            f"node {version} ne satisfait pas engines.node={borne_brute!r} "
            "déclaré par plateforme/package.json"
        )
    return version, None


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


def deriver_adresses_turn():
    """Les deux adresses de docker-compose.coturn.yml, dérivées des interfaces.

    🔴 `docker-compose.coturn.yml` exige TURN_LISTENING_IP **et**
    TURN_RELAY_IP, toutes deux obligatoires, sans quoi coturn se lie à TOUTES
    les interfaces (mesuré le 21 août 2026 : 23 adresses distinctes, dont
    l'adresse publique). Les deux sont dérivées de la même interface — celle
    de la route IPv4 par défaut — parce que ce déploiement porte coturn en
    `network_mode: host` sur une machine à interface publique unique :
    l'adresse d'où l'on écoute et celle depuis laquelle on relaie sont la
    même adresse physique. Si l'une des deux ne peut pas être établie,
    REFUSER plutôt qu'inventer une valeur.

    Rend (turn_ecoute, turn_relais, None) en succès, ou
    (None, None, raison) sinon.
    """
    interface = interface_de_route_par_defaut()
    if not interface:
        return None, None, (
            "aucune route IPv4 par défaut : impossible de dériver l'interface "
            "sur laquelle coturn doit écouter et relayer"
        )
    adresse = adresse_ipv4_de(interface)
    if not adresse:
        return None, None, (
            f"aucune adresse IPv4 lisible sur l'interface {interface} (route "
            "par défaut) : impossible de dériver TURN_LISTENING_IP/TURN_RELAY_IP"
        )
    return adresse, adresse, None


def lire_port():
    """PORT_DEFAUT, surchargeable par DESK_PORT (pour les tests seuls)."""
    brut = os.environ.get("DESK_PORT")
    if not brut:
        return PORT_DEFAUT, None
    try:
        return int(brut), None
    except ValueError:
        return None, f"DESK_PORT={brut!r} n'est pas un entier"


def valider_auth_mode(answers: dict):
    """Rend None en succès, ou la phrase de refus sinon.

    🔴 Un mode inconnu LÈVE (côté service, `PLATEFORME_AUTH` en dehors de
    `motdepasse`/`pomerium` lève dans `plateforme/src/config.ts`) : ici,
    un repli silencieux sur `motdepasse` ferait tourner un mode sous le nom
    de l'autre, et l'un des deux sens est une ouverture. Donc refus, jamais
    de repli.
    """
    mode = answers.get("auth_mode")
    if mode not in MODES_CONNUS:
        return (
            f"auth_mode inconnu : {mode!r} ; valeurs attendues "
            f"{' ou '.join(MODES_CONNUS)} — un repli silencieux ferait tourner "
            "un mode sous le nom de l'autre"
        )
    if mode == "pomerium":
        # Le wizard de la tâche 2 pose exactement quatre questions, et aucune
        # ne permet de déclarer un proxy de confiance. Or
        # `plateforme/src/config.ts::lireConfig` refuse de démarrer en mode
        # `pomerium` sans PLATEFORME_PROXY_DE_CONFIANCE (l'identité arrive
        # alors dans un en-tête `X-Pomerium-Claim-Email` en clair, qu'aucune
        # signature ne vérifie) : sans ce hook, l'opérateur ne l'apprendrait
        # qu'APRÈS l'installation, la plateforme refusant de démarrer sur un
        # disque déjà partitionné. Autant refuser ici, avant.
        return (
            "auth_mode=pomerium demandé, mais ce wizard ne permet de déclarer "
            "aucun proxy de confiance : plateforme/src/config.ts::lireConfig "
            "refuse de démarrer sans PLATEFORME_PROXY_DE_CONFIANCE en mode "
            "pomerium (l'identité arriverait dans un en-tête en clair "
            "qu'aucune signature ne vérifie), et l'opérateur ne l'apprendrait "
            "qu'après l'installation, sur un disque déjà partitionné. "
            "Choisir auth_mode=motdepasse."
        )
    return None


def charger_contexte():
    """Lit et valide `{"hw":…, "answers":…}` sur stdin.

    Rend (hw, answers, None) en succès, ou (None, None, raison) sur les deux
    formes d'entrée mal formée qu'on sait NOMMER : un JSON illisible, ou une
    racine / `hw` / `answers` qui ne sont pas des objets. Nommer ces deux cas
    plutôt que de les laisser tomber dans le garde générique de `main()` évite
    à l'opérateur un aller-retour : la phrase dit PRÉCISÉMENT ce qui cloche,
    pas seulement qu'une exception a eu lieu.
    """
    try:
        contexte = json.load(sys.stdin)
    except json.JSONDecodeError as exc:
        return None, None, f"entrée illisible : stdin n'est pas du JSON valide ({exc})"
    if not isinstance(contexte, dict):
        return None, None, (
            "entrée malformée : la racine JSON doit être un objet portant "
            f"'hw' et 'answers', reçu {type(contexte).__name__}"
        )
    hw = contexte.get("hw")
    if hw is None:
        hw = {}
    elif not isinstance(hw, dict):
        return None, None, f"entrée malformée : 'hw' doit être un objet, reçu {type(hw).__name__}"
    answers = contexte.get("answers")
    if answers is None:
        answers = {}
    elif not isinstance(answers, dict):
        return None, None, (
            f"entrée malformée : 'answers' doit être un objet, reçu {type(answers).__name__}"
        )
    return hw, answers, None


def resoudre(hw: dict, answers: dict) -> int:
    """Le corps du hook, une fois `hw`/`answers` garantis être des objets.

    Isolé de `main()` pour que le garde générique de `main()` enveloppe
    aussi cette fonction : toute exception qu'AUCUN chemin ci-dessous n'a
    prévue redevient un refus là-bas, jamais une trace pour l'opérateur.
    """
    emettre({"event": "progress", "pct": 10, "msg": "Vérification de la VM Windows"})

    # --- La VM d'abord : sans elle, rien de ce que desk orchestre n'existe --
    if not hw.get("vm_windows"):
        refuser(
            "aucune VM Windows détectée ou répondante : desk orchestre un "
            "bureau distant Windows en WebRTC, et n'a rien à faire sans elle "
            "(console la provisionne — voir le pré-requis 'console' du "
            "manifeste)"
        )
        return 0

    # --- Le mode d'authentification, et sa garde pomerium ------------------
    raison_auth = valider_auth_mode(answers)
    if raison_auth:
        refuser(raison_auth)
        return 0

    emettre({"event": "progress", "pct": 40, "msg": "Vérification de node"})

    # --- Node, contre la borne DÉCLARÉE par plateforme/package.json --------
    version_node, raison_node = valider_node()
    if raison_node:
        refuser(raison_node)
        return 0

    emettre({"event": "progress", "pct": 70, "msg": "Dérivation des adresses TURN"})

    # --- Les deux adresses TURN, dérivées, jamais demandées ----------------
    turn_ecoute, turn_relais, raison_turn = deriver_adresses_turn()
    if raison_turn:
        refuser(raison_turn)
        return 0

    port, raison_port = lire_port()
    if raison_port:
        refuser(raison_port)
        return 0

    emettre({"event": "progress", "pct": 90, "msg": "Faits résolus"})
    emettre({
        "event": "facts",
        "facts": {
            "vm_repond": True,
            "node_version": version_node,
            "turn_ecoute": turn_ecoute,
            "turn_relais": turn_relais,
            "port": port,
        },
    })
    emettre({"event": "done"})
    return 0


def main() -> int:
    # sys.argv est délibérément ignoré : le moteur réel appelle ce hook avec
    # `--phase resolve` (voir installer/packages/runner.py), mais son test
    # l'appelle sans aucun argument. La seule chose qui compte est stdin —
    # ainsi le hook répond aux deux appels sans avoir à distinguer lequel
    # c'est.
    #
    # 🔴 GARDE GÉNÉRIQUE, RONDE DE CORRECTION 1 (29 août 2026) : la première
    # version laissait `json.load` et l'accès `.get()` sur un `hw`/`answers`
    # mal typé lever tels quels — mesuré : un JSON illisible, une racine qui
    # n'est pas un objet, ou `hw` valant une chaîne au lieu d'un dict
    # donnaient tous les trois une trace Python et un code de sortie 1,
    # cassant l'invariant central de ce hook. `charger_contexte()` nomme les
    # deux cas qu'on sait distinguer (JSON illisible ; `hw`/`answers` mal
    # typés) ; CE `try` couvre tout le reste — ce qu'aucun chemin de
    # `resoudre()` n'a prévu. Les deux sont nécessaires ensemble : le garde
    # générique seul rendrait un « erreur inattendue » sur un cas qu'on sait
    # nommer, coûtant un aller-retour à l'opérateur ; les cas nommés seuls
    # laisseraient passer tout ce qu'on n'a pas anticipé.
    try:
        hw, answers, raison = charger_contexte()
        if raison:
            refuser(raison)
            return 0
        return resoudre(hw, answers)
    except Exception as exc:  # noqa: BLE001 — c'est le garde générique lui-même
        refuser(f"erreur inattendue dans resolve : {exc}")
        return 0


if __name__ == "__main__":
    sys.exit(main())
