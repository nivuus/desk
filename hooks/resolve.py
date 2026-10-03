#!/usr/bin/env python3
"""Resolve hook of the desk package.

🔴 THIS HOOK RUNS BEFORE partition(): at the moment the engine calls it, the
target disk does not exist yet. That is what gives the refusal its value — it
reaches the wizard, never an already wiped disk. See the
`packages.runner` module of the engine (sibling repository `installer`): resolve is
strictly read-only BY CONVENTION, not by sandbox — nothing
technically prevents a write, but the engine never reads or uses
anything this hook would have written.

A REFUSAL IS DATA, NEVER AN EXCEPTION. Every path that can fail
here ends with a `{"event":"refuse","reason":"…"}` event and an exit
code of 0 — never an uncaught exception: a traceback gives the operator
an exit code they cannot act on, a sentence gives them a
reason they can read before their disk is touched.

Protocol (see `installer/packages/runner.py` in the sibling repository): reads
{"hw":…, "answers":…} on stdin, writes one JSON object per line on stdout.
This hook never emits a `platform` event: desk's tier is
`userspace` (see `nivuus-package.yaml`), which forbids kernel-cmdline,
modules and hugepages — desk has nothing to put on the kernel command line.
"""
import json
import os
import pathlib
import re
import subprocess
import sys

from borne_node import lire_borne_node, parser_borne, verifier_version, version_de  # noqa: F401
from commun import (
    PORT_DEFAUT,
    adresse_ipv4_de,
    interface_de_route_par_defaut,
    lire_hote,
    lire_proxy_confiance,
)

RACINE = pathlib.Path(__file__).resolve().parents[1]

# PORT_DEFAUT (3445) and its reason now live in `commun.py`, the only
# place that holds them — see its comment. Reused here as is by
# `lire_port()`, never copied.
#
# Overridable by DESK_PORT so that the tests (and a future operator who
# would switch proxies) can set another value; the default stays
# that of `commun.PORT_DEFAUT`.

MODES_CONNUS = ("motdepasse", "pomerium")


def emettre(evenement: dict) -> None:
    print(json.dumps(evenement), flush=True)


def refuser(raison: str) -> None:
    emettre({"event": "refuse", "reason": raison})


def version_node_locale():
    """`node --version` of the machine running THIS hook, or None.

    resolve runs before the reboot into the target: what it can test is
    the node of the machine that actually runs it (the host of the wizard,
    or that of the tests) - never that of a target not yet installed. The
    target's own runtime is measured later, by `depot_node.py` at a replay,
    against the same bound (`borne_node.py`).
    """
    return version_de()


def valider_node():
    """Returns (version, None) on success, (None, raison) otherwise."""
    borne_brute, raison = lire_borne_node()
    if raison:
        return None, raison
    version = version_node_locale()
    if version is None:
        return None, "node is not found on this machine; plateforme/ requires it"
    return verifier_version(version, borne_brute)


def deriver_adresses_turn():
    """The two addresses of docker-compose.coturn.yml, derived from the interfaces.

    🔴 `docker-compose.coturn.yml` requires TURN_LISTENING_IP **and**
    TURN_RELAY_IP, both mandatory, otherwise coturn binds to ALL
    interfaces (measured on 21 August 2026: 23 distinct addresses, including
    the public address). Both are derived from the same interface — that
    of the default IPv4 route — because this deployment runs coturn in
    `network_mode: host` on a machine with a single public interface:
    the address we listen from and the one we relay from are the
    same physical address. If either cannot be established,
    REFUSE rather than invent a value.

    Returns (turn_ecoute, turn_relais, None) on success, or
    (None, None, raison) otherwise.
    """
    interface = interface_de_route_par_defaut()
    if not interface:
        return None, None, (
            "no default IPv4 route: cannot derive the interface "
            "coturn must listen and relay on"
        )
    adresse = adresse_ipv4_de(interface)
    if not adresse:
        return None, None, (
            f"no readable IPv4 address on interface {interface} (default "
            "route): cannot derive TURN_LISTENING_IP/TURN_RELAY_IP"
        )
    return adresse, adresse, None


def lire_port():
    """PORT_DEFAUT, overridable by DESK_PORT (for the tests only)."""
    brut = os.environ.get("DESK_PORT")
    if not brut:
        return PORT_DEFAUT, None
    try:
        return int(brut), None
    except ValueError:
        return None, f"DESK_PORT={brut!r} is not an integer"


def valider_auth_mode(answers: dict):
    """Returns None on success, or the refusal sentence otherwise.

    🔴 An unknown mode RAISES (on the service side, `PLATEFORME_AUTH` outside
    `motdepasse`/`pomerium` raises in `plateforme/src/config.ts`): here,
    a silent fallback to `motdepasse` would run one mode under the name
    of the other, and one of the two directions is an opening. So refusal, never
    a fallback.

    ✅ FIXED IN BATCH 10A (29 August 2026, problem C): this function
    flatly REFUSED `auth_mode=pomerium`, on the grounds that none of the
    four wizard questions allows declaring a trusted proxy.
    That was true, but made IMPOSSIBLE the explicit choice of the repository
    owner for the real commissioning ("PLATEFORME_AUTH=pomerium",
    against the other option offered to them). The refusal is lifted: the
    trusted proxy is now DERIVED, like `PORT_DEFAUT` and
    `commun.HOTE_DEFAUT` — see `commun.py::lire_proxy_confiance` for the
    full reasoning and its reservation (a REASONED value, not measured, to
    be confirmed by part 10B). `resoudre()` now includes it in the
    facts under the key `proxy_confiance`, and `hooks/install.py` writes it into
    `PLATEFORME_PROXY_DE_CONFIANCE`.
    """
    mode = answers.get("auth_mode")
    if mode not in MODES_CONNUS:
        return (
            f"unknown auth_mode: {mode!r}; expected values "
            f"{' or '.join(MODES_CONNUS)} — a silent fallback would run "
            "one mode under the name of the other"
        )
    return None


def valider_vb_audio(answers: dict):
    """Returns None on success, or the refusal sentence otherwise.

    🔴 CORRECTION ROUND 1 on task 6 (29 August 2026): `hooks/vm.py::
    poser_vb_audio(armee=True)` raises `NotImplementedError` for lack of a
    VB-Audio payload supplied by this package (personal licence only) — but
    that hook runs in `activate`, AFTER the administrator account
    and the agent enrolment have already been attempted (`hooks/activate.py`).
    An operator who ticks the wizard box ("Install VB-Audio in the
    VM (microphone)") would therefore get an activation that fails on EVERY
    run, forever: no account, no enrolled agent. The refusal
    must come HERE, in `resolve`, before a single byte touches the disk —
    exactly the doctrine of this hook (see the head docstring). The raise
    in `poser_vb_audio` stays in place as well: defence in
    depth, not a duplicate — if this refusal were ever bypassed, the
    hook must still not claim to have installed anything.
    """
    if not answers.get("vb_audio"):
        return None
    return (
        "vb_audio requested, but this package supplies no VB-Audio "
        "payload: its licence is personal only, and no task "
        "of this batch drops one into the tree that console builds. "
        "Leave the option unticked — the microphone then reports itself "
        "on the product side (mic: false in the ready message, the browser "
        "button does not appear)."
    )


def charger_contexte():
    """Reads and validates `{"hw":…, "answers":…}` on stdin.

    Returns (hw, answers, None) on success, or (None, None, raison) on the two
    malformed input shapes we know how to NAME: unreadable JSON, or a
    root / `hw` / `answers` that are not objects. Naming these two cases
    rather than letting them fall into the generic guard of `main()` saves
    the operator a round trip: the sentence says PRECISELY what is wrong,
    not only that an exception happened.
    """
    try:
        contexte = json.load(sys.stdin)
    except json.JSONDecodeError as exc:
        return None, None, f"unreadable input: stdin is not valid JSON ({exc})"
    if not isinstance(contexte, dict):
        return None, None, (
            "malformed input: the JSON root must be an object carrying "
            f"'hw' and 'answers', got {type(contexte).__name__}"
        )
    hw = contexte.get("hw")
    if hw is None:
        hw = {}
    elif not isinstance(hw, dict):
        return None, None, f"malformed input: 'hw' must be an object, got {type(hw).__name__}"
    answers = contexte.get("answers")
    if answers is None:
        answers = {}
    elif not isinstance(answers, dict):
        return None, None, (
            f"malformed input: 'answers' must be an object, got {type(answers).__name__}"
        )
    return hw, answers, None


def resoudre(hw: dict, answers: dict) -> int:
    """The body of the hook, once `hw`/`answers` are guaranteed to be objects.

    Isolated from `main()` so that the generic guard of `main()` also wraps
    this function: any exception that NO path below
    anticipated becomes a refusal there again, never a traceback for the operator.
    """
    emettre({"event": "progress", "pct": 10,
             "msg": "Checking the authentication mode"})

    # 🔴 THERE IS NO "WINDOWS VM" GATE HERE ANY MORE, AND THAT IS THE FIX
    # FOR THE CRITICAL FINDING OF THE FINAL BRANCH REVIEW (30 August 2026). This hook
    # carried `if not hw.get("vm_windows"): refuser(...)`. Three facts, each
    # of which is enough to condemn that gate:
    #
    #   ① NO PRODUCER OF THIS KEY EXISTS. The engine passes to `resolve`
    #      EXACTLY what `installer/installer/common/hardware.py::
    #      detect_all()` returns — eight keys (`disks`, `ethernet`, `wifi`, `gpus`,
    #      `cpu`, `iommu`, `memory_mib`, `passthrough_candidates`), none
    #      named `vm_windows` — passed verbatim by `install-engine/run.py`
    #      (`hw = hardware.detect_all()`) to `steps/packages.py::plan_packages`
    #      then to `run_resolve`, without enrichment. `hw.get("vm_windows")`
    #      therefore ALWAYS returned `None`, this hook ALWAYS refused, and
    #      `steps/packages.py` turns a refusal into a `StepError` — meaning
    #      that the WHOLE installation stopped, not only `desk`. The
    #      package could not install itself, the one thing it exists
    #      to do.
    #   ② THE TIMING MAKES THE CONDITION UNSATISFIABLE. `plan_packages()` calls
    #      `resolve` BEFORE `partition()` (`installer/installer/packages/
    #      runner.py`, head docstring: "plan_packages() runs resolve
    #      BEFORE partition()"). At that instant, the target disk does not exist,
    #      so the system `console` will install does not exist, so the Windows
    #      VM that `console` provisions cannot exist. No
    #      detector added to the engine would change that: the gate
    #      was wrong by construction, not by oversight.
    #   ③ THE GUARANTEE ALREADY EXISTS, EARLIER AND STRONGER, AND IT IS NOT
    #      OURS. `nivuus-package.yaml` declares `requires: packages:
    #      [console]`, and `plan_packages()` refuses through `missing_dependencies`
    #      BEFORE the first `resolve` hook and BEFORE `partition()`. "The VM
    #      will exist" is therefore settled by the manifest; saying it again here by
    #      querying an invented key added nothing and broke everything.
    #
    # 🔴 THE GATE IS NOT REMOVED, IT MOVES TO `activate` — the only
    # phase where the VM can exist (after the reboot, on the installed
    # system, with the network). See `hooks/activate.py`, the block
    # "THE WINDOWS VM GATE LIVES HERE": the VM is tested there through a
    # REAL WinRM EXCHANGE, never through a key that nobody produces.
    #
    # 🔴 WHAT THIS HOOK CAN STILL READ IN `hw`: nothing that is not in
    # `detect_all()`. `tests/test_desk_contrat_hw.py` freezes this contract and
    # turns red if a key missing from the producer reappears here.

    # --- The authentication mode, and its pomerium guard --------------------
    raison_auth = valider_auth_mode(answers)
    if raison_auth:
        refuser(raison_auth)
        return 0

    # --- VB-Audio: no payload supplied, the refusal comes HERE ---------------
    raison_vb_audio = valider_vb_audio(answers)
    if raison_vb_audio:
        refuser(raison_vb_audio)
        return 0

    emettre({"event": "progress", "pct": 40, "msg": "Checking node"})

    # --- Node, against the bound DECLARED by plateforme/package.json ----------
    version_node, raison_node = valider_node()
    if raison_node:
        refuser(raison_node)
        return 0

    emettre({"event": "progress", "pct": 70, "msg": "Deriving the TURN addresses"})

    # --- The two TURN addresses, derived, never asked for ------------------
    turn_ecoute, turn_relais, raison_turn = deriver_adresses_turn()
    if raison_turn:
        refuser(raison_turn)
        return 0

    # --- The listen address of the platform (never the TURN one) ------------
    # 🔴 DELIBERATELY A DERIVATION SEPARATE from `deriver_adresses_turn()`
    # above: the latter resolves the interface of the INTERNET route
    # (public, required for TURN); `lire_hote()` returns a fixed INTERNAL
    # address (see `commun.py::HOTE_DEFAUT` for the real bug this
    # separation fixes — the two were mixed up before batch 10A).
    hote, raison_hote = lire_hote()
    if raison_hote:
        refuser(raison_hote)
        return 0

    # --- The trusted proxy (PLATEFORME_PROXY_DE_CONFIANCE) ------------------
    # See `commun.py::lire_proxy_confiance`: a REASONED value, never
    # asked for, never missing. Exposed in the facts whatever
    # auth_mode is — it does no harm in `motdepasse` mode (there it only
    # serves to trust X-Forwarded-For from this address), and
    # it is in pomerium mode that it becomes mandatory on the service side.
    proxy_confiance = lire_proxy_confiance()

    port, raison_port = lire_port()
    if raison_port:
        refuser(raison_port)
        return 0

    emettre({"event": "progress", "pct": 90, "msg": "Facts resolved"})
    emettre({
        "event": "facts",
        "facts": {
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
    # sys.argv is deliberately ignored: the real engine calls this hook with
    # `--phase resolve` (see installer/packages/runner.py), but its test
    # calls it without any argument. The only thing that matters is stdin —
    # so the hook answers both calls without having to tell which one
    # it is.
    #
    # 🔴 GENERIC GUARD, CORRECTION ROUND 1 (29 August 2026): the first
    # version let `json.load` and the `.get()` access on a wrongly typed `hw`/`answers`
    # raise as is — measured: unreadable JSON, a root that
    # is not an object, or `hw` being a string instead of a dict
    # all three gave a Python traceback and an exit code 1,
    # breaking the central invariant of this hook. `charger_contexte()` names the
    # two cases we can tell apart (unreadable JSON; wrongly typed `hw`/`answers`);
    # THIS `try` covers everything else — what no path in
    # `resoudre()` anticipated. Both are needed together: the generic
    # guard alone would return an "unexpected error" on a case we know how to
    # name, costing the operator a round trip; the named cases alone
    # would let through everything we did not foresee.
    try:
        hw, answers, raison = charger_contexte()
        if raison:
            refuser(raison)
            return 0
        return resoudre(hw, answers)
    except Exception as exc:  # noqa: BLE001 — this is the generic guard itself
        refuser(f"unexpected error in resolve: {exc}")
        return 0


if __name__ == "__main__":
    sys.exit(main())
