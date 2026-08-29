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

from commun import (
    PORT_DEFAUT,
    adresse_ipv4_de,
    interface_de_route_par_defaut,
    lire_hote,
    lire_proxy_confiance,
)

RACINE = pathlib.Path(__file__).resolve().parents[1]

# PORT_DEFAUT (3445) et sa raison vivent désormais dans `commun.py`, seul
# endroit qui les porte — voir son commentaire. Repris ici tel quel par
# `lire_port()`, jamais recopié.
#
# Surchargeable par DESK_PORT pour que les tests (et un futur opérateur qui
# changerait de proxy) puissent poser une autre valeur ; le défaut reste
# celui de `commun.PORT_DEFAUT`.

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

    ✅ CORRIGÉ AU LOT 10A (29 août 2026, problème C) : cette fonction
    refusait CATÉGORIQUEMENT `auth_mode=pomerium`, au motif qu'aucune des
    quatre questions du wizard ne permet de déclarer un proxy de confiance.
    C'était vrai, mais rendait IMPOSSIBLE le choix explicite du propriétaire
    du dépôt pour la mise en service réelle (« PLATEFORME_AUTH=pomerium »,
    contre l'autre option qui lui était présentée). Le refus est levé : le
    proxy de confiance est désormais DÉRIVÉ, comme `PORT_DEFAUT` et
    `commun.HOTE_DEFAUT` — voir `commun.py::lire_proxy_confiance` pour le
    raisonnement complet et sa réserve (valeur RAISONNÉE, pas mesurée, à
    confirmer par le volet 10B). `resoudre()` l'inclut désormais dans les
    facts sous la clé `proxy_confiance`, et `hooks/install.py` l'écrit dans
    `PLATEFORME_PROXY_DE_CONFIANCE`.
    """
    mode = answers.get("auth_mode")
    if mode not in MODES_CONNUS:
        return (
            f"auth_mode inconnu : {mode!r} ; valeurs attendues "
            f"{' ou '.join(MODES_CONNUS)} — un repli silencieux ferait tourner "
            "un mode sous le nom de l'autre"
        )
    return None


def valider_vb_audio(answers: dict):
    """Rend None en succès, ou la phrase de refus sinon.

    🔴 RONDE DE CORRECTION 1 sur la tâche 6 (29 août 2026) : `hooks/vm.py::
    poser_vb_audio(armee=True)` lève `NotImplementedError` faute de charge
    VB-Audio fournie par ce package (licence personnelle seulement) — mais
    ce hook-là tourne dans `activate`, APRÈS que le compte administrateur
    et l'enrôlement de l'agent ont déjà été tentés (`hooks/activate.py`).
    Un opérateur qui coche la case du wizard (« Installer VB-Audio dans la
    VM (micro) ») obtiendrait donc une activation qui échoue à CHAQUE
    exécution, indéfiniment : aucun compte, aucun agent enrôlé. Le refus
    doit arriver ICI, dans `resolve`, avant qu'un octet touche le disque —
    exactement la doctrine de ce hook (voir le docstring de tête). La levée
    de `poser_vb_audio` reste en place par ailleurs : une défense en
    profondeur, pas un doublon — si ce refus était un jour contourné, le
    hook ne doit toujours pas prétendre avoir installé quoi que ce soit.
    """
    if not answers.get("vb_audio"):
        return None
    return (
        "vb_audio demandé, mais ce package ne fournit aucune charge "
        "VB-Audio : sa licence est personnelle seulement, et aucune tâche "
        "de ce lot n'en dépose dans l'arborescence que console construit. "
        "Laisser l'option décochée — le micro se signale alors de lui-même "
        "côté produit (mic: false dans le message ready, le bouton du "
        "navigateur ne paraît pas)."
    )


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
    emettre({"event": "progress", "pct": 10,
             "msg": "Vérification du mode d'authentification"})

    # 🔴 IL N'Y A PLUS DE PORTE « VM WINDOWS » ICI, ET C'EST LA CORRECTION
    # DE LA CRITIQUE DE LA REVUE FINALE DE BRANCHE (30 août 2026). Ce hook
    # portait `if not hw.get("vm_windows"): refuser(...)`. Trois faits, dont
    # chacun suffit à condamner cette porte :
    #
    #   ① AUCUN PRODUCTEUR DE CETTE CLÉ N'EXISTE. Le moteur passe à `resolve`
    #      EXACTEMENT ce que rend `installer/installer/common/hardware.py::
    #      detect_all()` — huit clés (`disks`, `ethernet`, `wifi`, `gpus`,
    #      `cpu`, `iommu`, `memory_mib`, `passthrough_candidates`), aucune
    #      nommée `vm_windows` — passé verbatim par `install-engine/run.py`
    #      (`hw = hardware.detect_all()`) à `steps/packages.py::plan_packages`
    #      puis à `run_resolve`, sans enrichissement. `hw.get("vm_windows")`
    #      rendait donc TOUJOURS `None`, ce hook refusait TOUJOURS, et
    #      `steps/packages.py` traduit un refus en `StepError` — c'est-à-dire
    #      que l'installation ENTIÈRE s'arrêtait, pas seulement `desk`. Le
    #      package ne pouvait pas s'installer, la seule chose qu'il existe
    #      pour faire.
    #   ② LE MOMENT REND LA CONDITION INSATISFIABLE. `plan_packages()` appelle
    #      `resolve` AVANT `partition()` (`installer/installer/packages/
    #      runner.py`, docstring de tête : « plan_packages() runs resolve
    #      BEFORE partition() »). À cet instant, le disque cible n'existe pas,
    #      donc le système que `console` va installer n'existe pas, donc la VM
    #      Windows que `console` provisionne ne peut pas exister. Aucun
    #      détecteur qu'on ajouterait au moteur ne changerait cela : la porte
    #      était fausse par construction, pas par oubli.
    #   ③ LA GARANTIE EXISTE DÉJÀ, PLUS TÔT ET PLUS FORTE, ET ELLE N'EST PAS
    #      LA NÔTRE. `nivuus-package.yaml` déclare `requires: packages:
    #      [console]`, et `plan_packages()` refuse par `missing_dependencies`
    #      AVANT le premier hook `resolve` et AVANT `partition()`. « La VM
    #      existera » est donc acquis par le manifeste ; le redire ici en
    #      interrogeant une clé inventée n'ajoutait rien et cassait tout.
    #
    # 🔴 LA PORTE N'EST PAS SUPPRIMÉE, ELLE MIGRE VERS `activate` — la seule
    # phase où la VM peut exister (après le redémarrage, sur le système
    # installé, avec le réseau). Voir `hooks/activate.py`, le bloc
    # « LA PORTE DE LA VM WINDOWS VIT ICI » : la VM y est éprouvée par un
    # ÉCHANGE WinRM RÉEL, jamais par une clé que personne ne produit.
    #
    # 🔴 CE QUE CE HOOK PEUT ENCORE LIRE DANS `hw` : rien qui ne figure dans
    # `detect_all()`. `tests/test_desk_contrat_hw.py` fige ce contrat et
    # rougit si une clé absente du producteur réapparaît ici.

    # --- Le mode d'authentification, et sa garde pomerium ------------------
    raison_auth = valider_auth_mode(answers)
    if raison_auth:
        refuser(raison_auth)
        return 0

    # --- VB-Audio : aucune charge fournie, le refus arrive ICI -------------
    raison_vb_audio = valider_vb_audio(answers)
    if raison_vb_audio:
        refuser(raison_vb_audio)
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

    # --- L'adresse d'écoute de la plateforme (jamais celle de TURN) --------
    # 🔴 DÉLIBÉRÉMENT UNE DÉRIVATION SÉPARÉE de `deriver_adresses_turn()`
    # ci-dessus : cette dernière résout l'interface de la route INTERNET
    # (publique, requise pour TURN) ; `lire_hote()` rend une adresse
    # INTERNE fixe (voir `commun.py::HOTE_DEFAUT` pour le bug réel que cette
    # séparation corrige — les deux étaient confondues avant le lot 10A).
    hote, raison_hote = lire_hote()
    if raison_hote:
        refuser(raison_hote)
        return 0

    # --- Le proxy de confiance (PLATEFORME_PROXY_DE_CONFIANCE) --------------
    # Voir `commun.py::lire_proxy_confiance` : une valeur RAISONNÉE, jamais
    # demandée, jamais absente. Exposée dans les facts quel que soit
    # auth_mode — elle ne nuit pas en mode motdepasse (elle y sert
    # seulement à faire croire X-Forwarded-For depuis cette adresse), et
    # c'est en mode pomerium qu'elle devient obligatoire côté service.
    proxy_confiance = lire_proxy_confiance()

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
            "hote": hote,
            "proxy_confiance": proxy_confiance,
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
