#!/usr/bin/env python3
"""Hook activate du package desk : armer ce qu'`install` n'a fait que poser.

Protocole (voir `installer/packages/runner.py` du dépôt voisin, et les
docstrings de `resolve.py`/`install.py` de ce package) : lit
{"hw":…, "answers":…} sur stdin, écrit un objet JSON par ligne sur stdout.
Comme `install`, ce hook n'a AUCUN canal `refuse` — une erreur ici se
traduit par un code de sortie non nul (`HookError` côté moteur), parce
qu'`activate` court APRÈS que `resolve` a déjà validé la machine et
qu'`install` a déjà écrit sur le disque cible : un échec à ce stade est une
anomalie, pas une décision à motiver pour l'opérateur.

🔴 LES `facts` DE `resolve` ARRIVENT FUSIONNÉS DANS `hw`, JAMAIS DANS UNE
CLÉ `facts` SÉPARÉE. Vérifié dans `installer/packages/runner.py::run_activate`
(ligne 327-337) : `_run_hook(manifest, "activate",
merge_into_hw(hw, facts or {}), answers, emit=emit)` — sans paramètre `root`
non plus (`_run_hook`'s `root` a pour défaut `""`, donc AUCUN `--root` n'est
passé en production ; `argparse` ci-dessous lui donne quand même un défaut
`/`, comme `console/hooks/activate.py`, pour rester appelable de la même
façon par les tests). La règle de précédence est dans
`installer/packages/facts.py::merge_into_hw` : `hw` (la fraîche détection)
l'emporte sur un fait de même clé, un fait ne comble que ce que `hw` n'a pas
produit. Aucune clé de `hw` que `resolve.py` a mesurée (`node_version`,
`turn_ecoute`, `turn_relais`, `port`) n'a de détecteur générique connu à ce
jour, donc en pratique ces quatre clés survivent toujours la fusion — mais
ce hook les LIT dans `hw`, jamais dans un `facts` frère, pour rester correct
le jour où un détecteur générique les produirait aussi.

🔴 CORRECTION DE LA REVUE FINALE DE BRANCHE (30 août 2026) : cette liste
portait aussi `vm_repond`, une clé que `resolve.py` n'a JAMAIS mesurée —
elle valait `True` EN DUR dans le dict d'émission, à la phase même dont ce
lot a établi qu'elle ne peut rien savoir de la VM (`resolve` court avant
`partition()` ; voir le commentaire de tête de `resolve.py::resoudre`). La
clé a été retirée des `facts` : aucun consommateur ne la lisait, ni dans ce
dépôt (`grep -rn vm_repond hooks/`, avant retrait, ne rendait que
`resolve.py` et les fixtures de test) ni dans le dépôt voisin `installer/`.
`tests/test_desk_contrat_hw.py` (garde ⑤) refuse désormais tout fait futur
qui referait la même chose : une valeur écrite en dur dans le dict
d'émission de `resolve.py`, plutôt que dérivée d'une mesure.

🔴 L'ARMEMENT EST UN LIEN, JAMAIS `systemctl enable`. Même doctrine que
`console/hooks/activate.py` (le seul précédent de ce dépôt voisin) :
`systemctl` échoue EN SILENCE dans un environnement contraint — une
sous-commande de requête n'imprime rien —, donc un `enable` qui « rend la
main » ne dit rien. Un lien existe ou lève. `hooks/assets/
desk-plateforme.service` porte déjà ce commentaire depuis la tâche 4.

⚠️ DIFFÉRENCE DÉLIBÉRÉE AVEC LE PRÉCÉDENT `console` : le lien créé ici est
RELATIF (`../desk-plateforme.service`), pas absolu
(`/etc/systemd/system/…`). Un lien absolu ne se résout que quand le
`--root` employé EST `/` — ce que `console/hooks/activate.py` assume
explicitly (« The link target is an ABSOLUTE path in the running system's
namespace, not in the throwaway root »). Un lien relatif, lui, se résout
correctement des deux côtés : sous un `--root` de test comme sous le
`/` réel, `os.path.realpath()` retombe sur le même fichier — c'est ce qui
rend `os.path.exists(os.path.realpath(lien))` vérifiable EN TEST, sans
jamais toucher le systemd réel de la machine qui fait tourner ces tests.

🔴 TU N'ARMES RIEN SUR CETTE MACHINE. `daemon-reload` et `start` ne
s'exécutent QUE si `--root` vaut `/` — jamais sous un `--root` de test :
reloader/démarrer là driverait le systemd de l'INSTALLEUR (ou du banc de
test), pas celui de la cible. Les liens créés garantissent de toute façon
que le prochain démarrage réel est correct, avec ou sans ce geste
immédiat — même raisonnement que `console/hooks/activate.py::start_now`.

Le mot de passe du compte initial NE TRANSITE QUE PAR STDIN, jamais par
`argv` : voir `plateforme/src/admin/creer-utilisateur.ts`, qui REFUSE
explicitement un `--password`/`--mdp`/… sur la ligne de commande (`ps`
exposerait un secret passé en argv à tout utilisateur de la machine,
pendant toute la durée de l'appel, puis dans l'historique du shell).

🔴 `--vm` DE `admin:agent` SIGNIFIE DEUX CHOSES SELON LE MODE, LU DANS
`plateforme/src/admin/enroler-agent.ts` :
  - en mode ENRÔLEMENT (celui que ce hook emploie), `--vm <nom>` est le NOM
    d'affichage de la VM (`enroler-agent.ts:96` : « --vm <nom> est
    obligatoire » ; `enrolerLaVm` l'écrit tel quel dans la colonne `vm.nom`,
    l'identifiant `vm_id` étant un `randomUUID()` généré PAR la commande,
    jamais reçu) ;
  - en mode `--roter` (hors du périmètre de ce hook), `--vm <id>` est cette
    fois l'identifiant `vm_id` déjà attribué (voir son propre message de
    refus, `enroler-agent.ts:183-184` : « c'est `vm_id`, celui qu'`--vm
    <nom> --adresse <hôte>` a imprimé à l'enrôlement »).
  Ce hook n'enrôle qu'une seule fois, jamais ne fait tourner un secret :
  il n'emploie donc que le mode enrôlement, avec un NOM.
`--adresse` n'a AUCUN défaut côté commande (`enroler-agent.ts:112-114`) :
ce hook doit toujours lui en fournir une.

`AGENT_VM` — la variable que `scripts/run-agent.sh` pose sur l'agent Rust
(voir `agent/src/plateforme/identite.rs`) — est le `vm_id` (l'UUID) que la
plateforme vérifie contre `agent_enrole` (`plateforme/src/agents/
enrolement.ts::verifierEnrolement` interroge par `vm_id`), PAS le nom
d'affichage passé à `--vm` : c'est bien la ligne `vm_id=…` de la sortie de
`admin:agent` que ce hook recopie dans `AGENT_VM`, jamais le nom donné en
entrée.

⚠️ NI LE NOM NI L'ADRESSE DE LA VM WINDOWS NE SONT DES RÉPONSES DU WIZARD
(`wizard.yaml` n'en pose aucune question, à dessein — « Quatre questions,
et pas une de plus »), et aucun détecteur `hw` connu ne les produit non
plus (voir plus haut). `ADRESSE_VM_DEFAUT` reprend donc la valeur que
`console/guest/winrm_exec.py:37` emploie déjà comme défaut de `GUEST_IP`
(`192.168.3.2`) — une adresse RÉELLEMENT ÉPINGLÉE, pas devinée :
`console/guest/domain.py` fixe un bail DHCP statique sur cette adresse
pour l'adresse MAC de la VM Windows que `console` provisionne (desk exige
`console` en pré-requis, voir `nivuus-package.yaml`). Le nom d'affichage
`NOM_VM_DEFAUT`, lui, N'EST PAS UNE VALEUR MESURÉE : c'est un choix
arbitraire (une seule VM existe dans ce déploiement), documenté comme tel
et laissé surchargeable — voir le § Réserves du rapport de cette tâche.
"""
import argparse
import json
import os
import pathlib
import subprocess
import sys

from administration import assign_vm_to_user, create_admin_account, enroler_agent_plateforme
from agent_payload import chemin_agent_console, construire_agent_reel, deposer_agent_console  # noqa: F401
from env_file import add_env_variables, read_env_file
from vm import poser_projfs, poser_vb_audio

# --- What task 4 set up, and what this hook arms or completes -----------
UNITE = "desk-plateforme.service"
UNIT_DIR = "etc/systemd/system"
# ⚠️ MUST STAY IN SYNC with the `[Install] WantedBy=` of
# `hooks/assets/desk-plateforme.service`: it is that file that decides under
# which `.target.wants/` the link must live for systemd to take it into
# account at `multi-user.target` startup. `tests/test_desk_activate.py`
# checks this synchronisation by reading the unit itself.
WANTS_SUBDIR = "multi-user.target.wants"

ENV_RELATIF = "etc/nivuus/desk.env"
PLATEFORME_RELATIF = "opt/nivuus/desk/plateforme"

# See the top docstring for the justification of these two defaults.
ADRESSE_VM_DEFAUT = "192.168.3.2"
NOM_VM_DEFAUT = "windows"


def emettre(evenement: dict) -> None:
    print(json.dumps(evenement), flush=True)


# --- Arming: a link, never an `enable` -------------------------------

def armer_unite(root: pathlib.Path, unite: str) -> pathlib.Path:
    """Links `unite` into `<UNIT_DIR>/<WANTS_SUBDIR>/`. Idempotent.

    Raises `FileNotFoundError` if the unit does not exist yet under
    `<root>/<UNIT_DIR>/`: a dead link would be worse than no link, it would
    read as armed while it is not. It is the arm the task
    deliberately tests (see the Step 5 § of the plan).

    The link created is RELATIVE (`../<unite>`) — see the top docstring
    for why this choice differs from the `console` precedent.
    """
    chemin_unite = root / UNIT_DIR / unite
    if not chemin_unite.is_file():
        raise FileNotFoundError(str(chemin_unite))

    wants_dir = root / UNIT_DIR / WANTS_SUBDIR
    wants_dir.mkdir(parents=True, exist_ok=True)
    lien = wants_dir / unite

    cible = os.path.relpath(chemin_unite, wants_dir)
    if lien.is_symlink() and os.readlink(lien) == cible:
        return lien
    if lien.exists() or lien.is_symlink():
        lien.unlink()  # a REGULAR file here would be the bug, not a state
    lien.symlink_to(cible)
    return lien


def start_now(unite: str) -> list:
    """`daemon-reload` then `start <unite>`. Never raises.

    `systemctl` is legitimately unusable in a constrained
    environment (see the top docstring), and the link created by
    `armer_unite` guarantees anyway that the next real boot
    is correct — so each failure is only REPORTED, never fatal.
    Only called by `main()` when `--root` is `/`: see its
    guard, right before the call.
    """
    echecs = []
    commandes = [["systemctl", "daemon-reload"], ["systemctl", "start", unite]]
    for commande in commandes:
        try:
            proc = subprocess.run(commande, capture_output=True, text=True)
        except OSError as exc:  # systemctl absent from this PATH
            echecs.append(f"{' '.join(commande)} : {exc}")
            continue
        if proc.returncode != 0:
            detail = (proc.stderr or proc.stdout or "").strip()[:200]
            echecs.append(f"{' '.join(commande)} : {detail or proc.returncode}")
    return echecs


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase")
    parser.add_argument("--root", default="/")
    args = parser.parse_args()
    root = pathlib.Path(args.root.rstrip("/") or "/")

    # Generic guard on the shape of the input: an unreadable JSON, or a
    # root that is not an object, must never raise a Python traceback
    # — task 3's fix (a `main()` that raised on a malformed
    # input) applies just as well here.
    try:
        contexte = json.load(sys.stdin)
    except json.JSONDecodeError as exc:
        print(f"desk activate: entree illisible, stdin n'est pas du JSON valide ({exc})",
              file=sys.stderr)
        return 1
    if not isinstance(contexte, dict):
        print("desk activate: entree malformee, la racine JSON doit etre un objet",
              file=sys.stderr)
        return 1
    hw = contexte.get("hw")
    hw = hw if isinstance(hw, dict) else {}
    answers = contexte.get("answers")
    answers = answers if isinstance(answers, dict) else {}

    # `hw` carries resolve's facts, MERGED (see the top
    # docstring): `port` is among them if no generic detector has
    # already produced it under that name. Purely informative here — the port itself
    # is already baked into desk.env by `install.py` — but it is the proof
    # that this hook does read `hw`, never a `facts` key that does not exist at
    # this level of the protocol.
    port_attendu = hw.get("port")
    msg_armement = "Armement du service"
    if port_attendu:
        msg_armement += f" (port attendu : {port_attendu})"

    emettre({"event": "progress", "pct": 10, "msg": msg_armement})
    try:
        armer_unite(root, UNITE)
    except FileNotFoundError as exc:
        print(f"desk activate: unite absente, rien arme : {exc}", file=sys.stderr)
        return 1

    # Only on the REAL machine: see the top docstring.
    if str(root) == "/":
        echecs = start_now(UNITE)
        if echecs:
            print("desk activate: unite liee mais non demarree ; l'armement "
                  "prendra effet au prochain redemarrage", file=sys.stderr)
            for item in echecs:
                print(f"  - {item}", file=sys.stderr)

    # 🔴 TASK 6 — WHAT `desk` SETS UP IN THE VM, THROUGH THE WINRM PATH OF
    # `console`. Placed HERE, BEFORE the rest (account, enrolment), and never
    # behind the idempotence guard below (`AGENT_VM`/`AGENT_SECRET`):
    # these two setups are independent of the state of `plateforme/`
    # (`Enable-WindowsOptionalFeature` is natively idempotent — replaying
    # it breaks nothing), and gating them behind the account's idempotence
    # would make a first failure HERE, occurring AFTER an already
    # successful enrolment, never be retried by a later replay.
    #
    # 🔴 THE WINDOWS VM GATE LIVES HERE, SINCE THE FINAL BRANCH REVIEW
    # (August 30th, 2026), AND NO LONGER IN `resolve`. `hooks/resolve.py` carried
    # `if not hw.get("vm_windows"): refuser(...)`: a key that NO
    # producer of the engine sets (`common/hardware.py::detect_all()` returns
    # eight, none of that name), tested at a phase — before `partition()` — where
    # the target disk, hence the system, hence the VM, do not exist yet.
    # See the matching comment block in `resolve.py` for the
    # three complete reasons.
    #
    # HERE, the VM can exist: `activate` runs after the reboot, on the
    # installed system, with the network, and `console` — a HARD prerequisite of the
    # manifest, hence activated BEFORE `desk` — has provisioned it. The gate is
    # therefore a MEASUREMENT, never a key read: the first WinRM exchange
    # below IS the test. If it fails, it is not "ProjFS could not
    # be enabled", it is "the VM is not there", and the message says so.
    #
    # ⚠️ WHY NOT A SEPARATE, READ-ONLY PROBE BEFORE THIS ONE:
    # it would cost a second WinRM round trip that could say nothing
    # more than the first — and its success criterion (a non-empty output)
    # is not reliable, `winrm_exec.py` legitimately returning an empty output
    # for a command that prints nothing. A check that cannot
    # distinguish its two values is not a check: we keep the one that
    # can.
    emettre({"event": "progress", "pct": 25,
             "msg": "Verification de la VM Windows, puis pose de ProjFS "
                    "(chemin WinRM de console)"})
    try:
        etat_projfs = poser_projfs()
    except (FileNotFoundError, RuntimeError) as exc:
        print("desk activate: la VM Windows ne repond pas au chemin WinRM de "
              "console — desk orchestre un bureau distant Windows en WebRTC "
              "et n'a rien a faire sans elle (console la provisionne, voir le "
              "pre-requis 'console' du manifeste ; c'est ICI, apres le "
              "redemarrage, que cette condition est eprouvee, jamais dans "
              f"resolve, ou la VM ne peut pas encore exister) : {exc}",
              file=sys.stderr)
        return 1
    # The reboot is NOTED and SAID HERE, it is never taken: see
    # hooks/vm.py::poser_projfs. An operator reading this message knows a
    # reboot of the VM remains their responsibility.
    if etat_projfs.redemarrage_requis:
        print("desk activate: ProjFS active dans la VM ; un redemarrage de "
              "la VM est requis pour qu'il prenne effet (non declenche "
              "automatiquement)", file=sys.stderr)

    try:
        poser_vb_audio(armee=bool(answers.get("vb_audio", False)))
    except (FileNotFoundError, RuntimeError, NotImplementedError) as exc:
        print(f"desk activate: pose de VB-Audio dans la VM refusee : {exc}",
              file=sys.stderr)
        return 1

    # Task 7 — dropping agent.exe (see chemin_agent_console() above);
    # independent of plateforme/, never behind the idempotence guard.
    emettre({"event": "progress", "pct": 30,
             "msg": "Depot de agent.exe pour console (build croise)"})
    try:
        deposer_agent_console()
    except (FileNotFoundError, RuntimeError, OSError) as exc:
        print(f"desk activate: depot de agent.exe pour console refuse : {exc}",
              file=sys.stderr)
        return 1

    plateforme_dir = root / PLATEFORME_RELATIF
    if not plateforme_dir.is_dir():
        print(f"desk activate: {plateforme_dir} est absent ; le hook install "
              "ne semble pas avoir tourne sur cette racine", file=sys.stderr)
        return 1

    env_chemin = root / ENV_RELATIF

    # 🔴 FIX, ROUND 1 — `desk.env` MUST ALREADY EXIST AT THIS STAGE, AND
    # ITS ABSENCE IS A REFUSAL, NEVER A SILENT CREATION. Two
    # reasons, the second being the one that settles it:
    #   1. it is the SYMPTOM of a partial installation (`plateforme/`
    #      set up, `desk.env` never written or deleted since) — a problem
    #      more serious than a mere missing file, which deserves a named refusal
    #      rather than a silent fallback;
    #   2. `add_env_variables()` (below) writes through `write_text()` THEN
    #      `chmod`, exactly the pattern `install.py::write_env`
    #      abandoned (commit `92cacfb`, "a secret never readable between
    #      creation and chmod") for an atomic `os.open(..., 0o600)` — a
    #      NEW file would be born here with the process umask, briefly readable
    #      by anyone before the `chmod` catches up, and this file is about to
    #      receive `AGENT_SECRET`. Refusing here guarantees that
    #      `add_env_variables()` only EVER writes to a file already at
    #      600: `write_text()` on an EXISTING file does not touch its
    #      mode, so no window opens. `tests/test_desk_activate.py`
    #      tests this refusal (dedicated scenario: `plateforme/` set up, `desk.env`
    #      absent).
    if not env_chemin.is_file():
        print(f"desk activate: {env_chemin} est absent ; le hook install ne "
              "semble pas avoir pose de fichier d'environnement sur cette "
              "racine (installation partielle, ou fichier supprime depuis) — "
              "rien n'est cree a sa place", file=sys.stderr)
        return 1

    env_deja_pose = read_env_file(env_chemin)

    # 🔴 IDEMPOTENCE: an ALREADY SUCCESSFUL activation replays neither the creation
    # of the account, nor the enrolment. `vm.nom` has NO uniqueness constraint
    # (`plateforme/…/0001-schema.sql`: only `user.email` is
    # `UNIQUE`) — a second `admin:agent` on a replay would therefore create one more
    # ORPHAN VM at each call, never a refusal; and a
    # second `admin:user` with the SAME email would fail on the
    # `UNIQUE` constraint of `user.email`. Without this guard, a replay
    # (retried by the engine, or relaunched by hand by an operator after a
    # first success) would therefore be either CORRUPTING (orphan VMs piling
    # up), or doomed to fail forever. Same doctrine
    # as `console/hooks/activate.py::run_steps` (`step.already_done()`):
    # an already completed step is NOTED, it is not replayed.
    #
    # ⚠️ WHAT THIS GUARD DOES NOT COVER: a failure BETWEEN the creation of the
    # account and the enrolment (account created, `AGENT_VM`/`AGENT_SECRET` never
    # written) makes a replay stumble on the already taken email — a
    # PARTIAL recovery outside the scope of this task, see the
    # report, § Reservations.
    if env_deja_pose.get("AGENT_VM") and env_deja_pose.get("AGENT_SECRET"):
        emettre({"event": "progress", "pct": 100,
                 "msg": "desk : deja active (AGENT_VM/AGENT_SECRET deja "
                        "presents dans desk.env) ; compte et enrolement ignores"})
        emettre({"event": "done"})
        return 0

    email = str(answers.get("admin_email") or "")
    mot_de_passe = str(answers.get("admin_password") or "")
    if not email or not mot_de_passe:
        print("desk activate: answers.admin_email et answers.admin_password "
              "sont requis et absents", file=sys.stderr)
        return 1

    # `admin:user`/`admin:agent` read their configuration (which
    # database, which SQLite file…) from `process.env` — see
    # `plateforme/src/config.ts::lireConfig`. These are NOT the variables
    # of this Python process: they live in desk.env, written by
    # `install.py`, and must therefore be merged into the environment
    # passed to `npm`, otherwise the administration commands
    # would open a database different from the one the started service will
    # really serve.
    env_npm = dict(os.environ)
    env_npm.update(env_deja_pose)

    emettre({"event": "progress", "pct": 40, "msg": "Creation du compte administrateur"})
    _identifiant, raison = create_admin_account(plateforme_dir, email, mot_de_passe, env_npm)
    if raison:
        print(f"desk activate: creation du compte refusee : {raison}", file=sys.stderr)
        return 1

    emettre({"event": "progress", "pct": 70, "msg": "Enrolement de l'agent"})
    nom_vm = os.environ.get("DESK_VM_NOM", NOM_VM_DEFAUT)
    adresse_vm = os.environ.get("GUEST_IP", ADRESSE_VM_DEFAUT)
    result, raison = enroler_agent_plateforme(plateforme_dir, nom_vm, adresse_vm, env_npm)
    if raison:
        print(f"desk activate: enrolement de l'agent refuse : {raison}", file=sys.stderr)
        return 1
    vm_id, secret = result

    add_env_variables(env_chemin, {"AGENT_VM": vm_id, "AGENT_SECRET": secret})

    # 🔴 TASK 13 — A HOLE FOUND IN PRODUCTION ON AUGUST 29TH, 2026: `enroler_agent_
    # plateforme()` (hence `admin:agent`, hence `enrolerLaVm`) does
    # `INSERT INTO vm(id, nom, adresse)` WITHOUT ever passing a user —
    # `vm.utilisateur_id` stayed NULL, and `plateforme/src/http/
    # routes-applications.ts` had announced it IN SO MANY WORDS since before this
    # fix: "as long as no VM is assigned, EVERY AUTHENTICATED USER
    # SEES ALL THE VMS: it is NOT an isolation" — and the
    # symptom MEASURED at the owner's was the mirror of that same cell
    # left NULL: `GET /vm` returned `{"vms":[]}` for an account that was
    # indeed created, hence an empty hub. Repaired on the running instance by hand
    # (`npm run admin:attribuer -- --email … --vm windows`); what follows is
    # what was missing so that a FRESH installation never again needs
    # that manual repair.
    #
    # 🔴 PLACEMENT: NEITHER IN THE IDEMPOTENCE SHORT-CIRCUIT ABOVE, NOR OUTSIDE
    # ANY GUARD — the two obvious traps, and neither of them works:
    #   - IN the short-circuit (which only runs ON REACTIVATION, once
    #     `AGENT_VM`/`AGENT_SECRET` are already written): the NORMAL pass —
    #     the one that runs ONLY ONCE per package, at activation — would
    #     NEVER reach it. It is exactly the hole found in
    #     production: an installation that never replays activation never
    #     catches up on its own.
    #   - OUTSIDE any guard (replayed at EACH call, short-circuit included):
    #     `admin:attribuer` IS NOT idempotent for a replay — the orchestrator
    #     refuses `vm-deja-attribuee` as soon as `utilisateur_id` is no longer NULL,
    #     EVEN for reassignment to the same user
    #     (`inventaire-statique.ts::attribuer`, read ①). Replaying at each
    #     reactivation would therefore make EVERY reactivation fail after the
    #     first success, on a refusal that says nothing false but
    #     is not a failure either.
    #   It therefore lives HERE, as the CONTINUATION of the normal path — protected
    #   by THE SAME guard as the account and the enrolment just above (the
    #   short-circuit skips it just as much as them): it runs only
    #   once, in the pass that has just created that account and enrolled
    #   that agent — never in the following reactivations.
    #
    # ⚠️ WHAT THIS PLACEMENT DOES NOT COVER, SAME LIMIT AS THE IDEMPOTENCE
    # GUARD ABOVE (see its own comment): a failure HERE
    # occurs AFTER `AGENT_VM`/`AGENT_SECRET` are already written (previous
    # line) — a reactivation will therefore short-circuit from now on BEFORE
    # reaching this assignment, without ever retrying it. It is the
    # LEAST bad choice of the two possible orders: writing these two variables
    # AFTER the assignment would instead reopen, on this same failure, the REPLAY of
    # `admin:agent` — which, for its part, creates one more ORPHAN VM at each
    # call (see the idempotence guard's comment above): a
    # worse regression than the one being fixed here. An assignment missed at
    # this point stays diagnosable (the message below names the cause)
    # and repairable BY HAND through this same `npm run admin:attribuer` — it is
    # exactly how the real instance of August 29th, 2026 was repaired.
    emettre({"event": "progress", "pct": 90,
             "msg": "Attribution de la VM au compte administrateur"})
    _sortie_attribution, raison = assign_vm_to_user(plateforme_dir, email, vm_id, env_npm)
    if raison:
        print(f"desk activate: attribution de la VM refusee : {raison}", file=sys.stderr)
        return 1

    emettre({"event": "progress", "pct": 100,
             "msg": "desk : service arme, compte cree, agent enrole, VM attribuee"})
    emettre({"event": "done"})
    return 0


if __name__ == "__main__":
    sys.exit(main())
