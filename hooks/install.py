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
    """The root of the SOURCE repository (what is copied to the target).

    Overridable by `DESK_SOURCE_RACINE` (tests only — see
    `tests/test_desk_install.py`, the missing node_modules scenario): the
    real engine, like all the other hooks of this package, needs
    no default other than the root of the cloned repository.
    """
    brut = os.environ.get("DESK_SOURCE_RACINE")
    if brut:
        return pathlib.Path(brut)
    return pathlib.Path(__file__).resolve().parents[1]


RACINE = _racine_source()
ASSETS = pathlib.Path(__file__).resolve().parent / "assets"

# PORT_DEFAUT (3445) and its reason live in `commun.py`, the only place that
# holds them now — see its comment.

def emettre(evenement: dict) -> None:
    print(json.dumps(evenement), flush=True)


# --- The PRE-FLIGHT: everything missing is said BEFORE a secret is written --
#
# 🔴 IMPORTANT FINDING OF THE FINAL BRANCH REVIEW (30 August 2026), SHOWN BY
# EXECUTION. `client/dist/` and `plateforme/node_modules/` are BOTH
# gitignored: a freshly cloned repository, or one packaged by a chain that
# never ran `npm install` nor `npm run build`, carries neither. Yet the
# copy of `client/dist` came BEFORE the `tsx` guard, so this hook
# gave a PYTHON TRACEBACK (`FileNotFoundError` raised from
# `shutil.copytree`) where it knows how to write a sentence everywhere else — and
# it gave it AFTER having already written `desk.env` WITH ITS TWO FRESHLY
# DRAWN SECRETS. An aborted `install` therefore left an orphan secrets
# file, which minor #9 ("a replayed install silently reforges the secret")
# half described without seeing that half.
#
# 🔴 THE PRE-FLIGHT ENUMERATES, IT DOES NOT STOP AT THE FIRST GAP. An operator
# who is told "`client/dist` is missing", builds it, and is then told
# "`node_modules` is missing" has paid two round trips where one
# was enough. It returns the WHOLE list.
#
# ⚠️ THIS PRE-FLIGHT DOES NOT REPLACE THE AFTER-COPY GUARDS (`proto/ts/
# plateforme.ts` found, `node_modules/.bin/tsx` found): those
# prove that the COPY succeeded, this one that the SOURCE exists. The two
# can fail independently — a partial copy on a full disk
# only shows up through the latter.

def raisons_de_pre_vol(racine_source: pathlib.Path, prefixe_node) -> list:
    """The (possibly empty) list of reasons to refuse BEFORE writing.

    `prefixe_node` is the resolved Node prefix, or None; its own failure
    reason is passed through `raison_node` in `main()`.
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
    # wrongly typed `facts` and `answers` gave an `AttributeError` on the
    # first `.get()` — a traceback where this hook writes sentences.
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
    # 🔴 PLATEFORME_AUTH comes from the auth_mode answer, AS IS, without
    # fallback: an unknown value was already refused by resolve
    # (`valider_auth_mode`), and a silent fallback here would run one
    # mode under the name of the other.

    emettre({"event": "progress", "pct": 10,
             "msg": "Dérivation des adresses et du port"})

    # 🔴 MINOR #7, OVERTURNED BY THE FINAL BRANCH REVIEW: a value
    # coming from `facts` goes through the SAME validator as a derived value —
    # see `commun.py`, § "What comes from the facts channel", for the refutation
    # that requires it. Without this, `facts["turn_ecoute"]="0.0.0.0"` made
    # coturn listen and relay on ALL interfaces, silently.
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

    # 🔴 PLATEFORME_HOTE MUST NEVER BE UNIVERSAL, AND MUST NEVER
    # BE THE TURN ADDRESS (see the comment of `deriver_adresse_turn`:
    # mixing the two up exposed the service on the PUBLIC address). Derived
    # separately, by `commun.lire_hote()` — a FIXED and internal address,
    # never the default route.
    #
    # 🔴 CORRECTION ROUND 1 (29 August 2026): THE REVIEW SHOWED THAT THE
    # PREVIOUS FORM — `lire_hote()` called ONLY when
    # `facts.get("hote")` was empty — let `facts["hote"] = "0.0.0.0"`
    # go through WITHOUT ever meeting `ECOUTES_UNIVERSELLES`: code 0,
    # `PLATEFORME_HOTE=0.0.0.0` written into `desk.env`. Unreachable by the
    # real engine TODAY (it passes no `facts` to `install`), but
    # this hook accepts that channel precisely for "a future engine, or this
    # test file" (see the head docstring) — and the `facts` contract
    # gained keys DURING this very batch. `valider_hote()` is
    # now called on THE RETAINED VALUE, whatever its
    # origin: `facts["hote"]` if present, `commun.lire_hote()`
    # (which itself calls `valider_hote`) otherwise — a SINGLE check, two
    # entry paths, never one without the other.
    brut_hote = facts.get("hote")
    if brut_hote:
        hote_plateforme, raison_hote = valider_hote(brut_hote, origine='facts["hote"]')
    else:
        hote_plateforme, raison_hote = lire_hote()
    if raison_hote:
        print(f"desk install : {raison_hote}", file=sys.stderr)
        return 1

    # PLATEFORME_PROXY_DE_CONFIANCE — see `commun.py::lire_proxy_confiance`
    # for the full reasoning (a DERIVED value, never asked for).
    # Written whatever auth_mode is: it does no harm in password mode
    # (there it only serves to trust X-Forwarded-For from this
    # address), and becomes mandatory on the service side in pomerium mode.
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

    # --- THE PRE-FLIGHT, BEFORE THE FIRST SECRET -----------------------------
    # See the comment of `raisons_de_pre_vol`: everything missing is said
    # HERE, before a single byte of `desk.env` — hence before a secret — touches
    # the disk.
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

    # 🔴 FIXED ON 2026-09-08: install WAS NOT IDEMPOTENT — these two
    # secrets were drawn UNCONDITIONALLY on every run, in
    # direct contradiction with the `ecrire_secret` docstring ("drawn ONLY
    # ONCE, never recomputed"), which described an invariant the
    # code did not hold. The release plan relies on an update that
    # REPLAYS install IN PLACE: without this fix, every update of
    # desk would have silently rotated PLATEFORME_SECRET_JETON
    # (invalidating all open sessions) and TURN_SECRET (breaking
    # ongoing coturn authentication). `lire_secret_persiste` reads back the
    # `desk.env` ALREADY IN PLACE on the TARGET root (never the source
    # root); it only returns a value if it is really
    # reusable — missing file, missing key, empty or blank value
    # all count as "nothing to reuse", never as an error
    # (see its own docstring) — and a secret is only drawn when there
    # is nothing to reuse.
    #
    # 🔴 REVIEW OF 2026-09-08: A `desk.env` PRESENT BUT UNREADABLE IS
    # NOT "NOTHING TO REUSE" — IT IS A FAILURE. `lire_secret_persiste`
    # catches ONLY `FileNotFoundError` (a TOCTOU race, equivalent to
    # "the file never existed"); any other `OSError`
    # (permissions skewed by a partial migration, disk error, …)
    # bubbles up to here. Catching it higher up and drawing a new secret
    # anyway would redo EXACTLY the bug this fix corrects, through
    # a narrower door. The choice is therefore a LOUD REFUSAL, never a
    # warning that would let the install go on: a warning
    # one can ignore protects nothing, and rotating the session
    # token/the TURN secret because a file was momentarily
    # unreadable is worse than refusing to install.
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
        # persisted sqlite rather than ':memory:' (the product default):
        # an installed service that loses its state at every restart has
        # no value. /var/lib/nivuus-desk is created by StateDirectory=
        # in the systemd unit (see hooks/assets/desk-plateforme.service).
        "PLATEFORME_BASE": "sqlite",
        "PLATEFORME_BASE_URL": "/var/lib/nivuus-desk/plateforme.sqlite",
        "PLATEFORME_SECRET_JETON": secret_jeton,
        # PLATEFORME_PAGE: what makes nginx optional — the platform
        # serves the built page itself since 22 August 2026. Missing or
        # empty, GET / would return 404: so it is never left empty.
        "PLATEFORME_PAGE": "/opt/nivuus/desk/client/dist",
        "PLATEFORME_AUTH": auth_mode,
        # See the comment of `proxy_confiance` above: mandatory
        # in pomerium mode, harmless in password mode.
        "PLATEFORME_PROXY_DE_CONFIANCE": proxy_confiance,
        # 🔴 SET EXPLICITLY, AND IT IS MANDATORY (correction round
        # 1, task 4): their product default (`donnees/icones`,
        # `donnees/televersements`, relative to `WorkingDirectory`) would land
        # under `/opt/nivuus/desk/plateforme`, a path that `DynamicUser=yes`
        # makes READ ONLY (implicit `ProtectSystem=strict` — see
        # `hooks/assets/desk-plateforme.service`). Without this pair,
        # icon and upload handling would fail with `EROFS` on
        # first use. `/var/lib/nivuus-desk` is the ONLY directory
        # writable by the service: it is the one `StateDirectory=
        # nivuus-desk` creates and owns, the same as `PLATEFORME_BASE_URL`
        # above.
        "PLATEFORME_ICONES": "/var/lib/nivuus-desk/icones",
        "PLATEFORME_TELEVERSEMENTS": "/var/lib/nivuus-desk/televersements",
        # The coturn configuration on the PLATFORM side (not the coturn server side,
        # see ecrire_turnserver_conf): `signaling/ice.ts::configurationIce`
        # requires BOTH to announce a relay to peers.
        "TURN_URL": f"turn:{turn_ecoute}:{PORT_TURN}",
        "TURN_SECRET": secret_turn,
    }
    ecrire_env(sous("etc/nivuus/desk.env"), env)

    emettre({"event": "progress", "pct": 55,
             "msg": "Copie de la plateforme et du client bâti"})

    # `donnees/` is a DEVELOPMENT directory (icons and
    # uploads piled up by previous acceptance runs): copying it
    # would give birth to a new installation carrying the past of the development
    # workstation. The platform recreates it itself on first access —
    # under `/var/lib/nivuus-desk`, NOT under its relative default: see
    # PLATEFORME_ICONES/PLATEFORME_TELEVERSEMENTS above, and why the
    # default would break under DynamicUser.
    copier_arbre(RACINE / "plateforme", sous("opt/nivuus/desk/plateforme"),
                 exclure=("donnees",))
    copier_arbre(RACINE / "client" / "dist",
                 sous("opt/nivuus/desk/client/dist"))

    # 🔴 REAL FINDING OF BATCH 10A (29 August 2026), BY STARTING THE REAL
    # SERVICE: `proto/ts/` WAS NOT COPIED AT ALL, AND THE SERVICE DOES NOT
    # START WITHOUT IT. `plateforme/src/agents/canal.ts` (and twenty
    # other files of `plateforme/src/`) imports `../../../proto/ts/…`
    # — a RELATIVE path that assumes `proto/ts/` is a SIBLING of
    # `plateforme/`, exactly as in this development repository.
    # `tsx` transpiles ON THE FLY (unlike `tsc`, which would only have
    # rejected the module at type-check): without the SOURCE files of
    # `proto/ts/` deployed at the same relative place, `npm start` fails at
    # the very instant the first module importing it is loaded
    # (`ERR_MODULE_NOT_FOUND`, measured on this real service). Copied WITHOUT its
    # `*.test.ts` files (never run by the service, only by
    # `vitest` in development).
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

    # See `rendre_lisible_par_tous`: needed so that the ephemeral UID of
    # `DynamicUser=yes` can at least TRAVERSE `/opt/nivuus/desk/…` —
    # set on `sous("opt/nivuus/desk")`, one level ABOVE both copies,
    # to also cover that parent directory itself (created by the first
    # `copier_arbre` through `destination.parent.mkdir`, under the umask of the
    # process running `install`, never guaranteed world-traversable).
    rendre_lisible_par_tous(sous("opt/nivuus/desk"))

    # 🔴 PROBLEM B OF BATCH 10A: `npm start` REQUIRES `node_modules`, AND NOTHING
    # GUARANTEED IT. `copier_arbre()` above copies all of `plateforme/`
    # (only `donnees/` is excluded), so `node_modules` IS copied in practice
    # AS SOON AS IT IS PRESENT on the source side — checked on 29 August 2026: 56
    # packages, the relative link `node_modules/.bin/tsx` still resolves
    # correctly under the copied root. But that is only a SIDE EFFECT
    # of copying the whole directory, never a guarantee: a freshly
    # cloned repository (or one packaged by a pipeline that never ran
    # `npm install`) would copy a `plateforme/` WITHOUT `node_modules`, and
    # the installation would "succeed" anyway — the service would only
    # find out at its first start, `ExecStart` failing for lack of
    # `tsx`. This guard closes the hole: it checks the REAL presence
    # of the binary that `npm start` needs (`node_modules/.bin/tsx`,
    # never a mere non-emptiness test of the directory, which would let
    # a partial `node_modules` through), and REFUSES rather than letting an
    # inert service be laid down without saying so.
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

    # 🔴 THE NODE RUNTIME, DEPLOYED AND NO LONGER MERELY ASSUMED (final branch
    # review, 30 August 2026). `NODE_BIN_DEFAUT` pointed at a directory that
    # NOTHING in this repository created: the tree present on the development
    # machine had been copied there by hand during batch 10A, and the
    # command only lived in a gitignored report. See
    # `hooks/depot_node.py` for the full reasoning and for exactly what is
    # deployed. The target prefix is the PARENT of the `bin/` that
    # `lire_node_bin()` returns, so that the two cannot diverge.
    emettre({"event": "progress", "pct": 70,
             "msg": "Dépôt du runtime Node (node, npm, npx)"})
    prefixe_cible = pathlib.PurePosixPath(lire_node_bin()).parent
    deposer_node(prefixe_node, sous(str(prefixe_cible).lstrip("/")))

    emettre({"event": "progress", "pct": 80, "msg": "Dépôt de l'unité systemd"})

    # 🔴 PROBLEM A OF BATCH 10A: THE UNIT CARRIED `ExecStart=/usr/bin/npm
    # start`, A PATH THAT EXISTS ON NO DEBIAN WITHOUT THE `nodejs` PACKAGE.
    # See `commun.py::lire_node_bin` for the full diagnosis and the
    # deployment decision. The file `assets/desk-plateforme.service`
    # is now a TEMPLATE carrying the `__NODE_BIN__` token (both
    # in `ExecStart=` and in `Environment=PATH=…`, so that `npm`
    # itself — a `#!/usr/bin/env node` script — finds `node` when the
    # kernel resolves its interpreter): the substitution below is
    # TEXTUAL, done once and for all at install time, never a
    # variable expansion on the systemd side (which does not expand the executed
    # program itself). Written with `os.open(..., 0o644)` rather than
    # `shutil.copy2`: this file is no longer a verbatim copy.
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

    # LAID DOWN, NOT ARMED: see the head comment of the unit itself.
    unite_dest = sous("etc/systemd/system/desk-plateforme.service")
    unite_dest.parent.mkdir(parents=True, exist_ok=True)
    unite_dest.write_text(contenu_unite, encoding="utf-8")
    os.chmod(unite_dest, 0o644)  # a unit is DATA, not a program

    emettre({"event": "progress", "pct": 95,
             "msg": "Configuration coturn"})
    ecrire_turnserver_conf(sous("etc/turnserver.conf"), turn_ecoute,
                            turn_relais, secret_turn)

    emettre({"event": "done"})
    return 0


if __name__ == "__main__":
    sys.exit(main())
