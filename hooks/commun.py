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
            "no default IPv4 route: cannot derive the address "
            "coturn must listen and relay on"
        )
    adresse = adresse_ipv4_de(interface)
    if not adresse:
        raise RuntimeError(
            f"no readable IPv4 address on interface {interface} (default "
            "route): cannot derive TURN_LISTENING_IP/"
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

# The four values that make a service listen on ALL interfaces —
# copied as is from `plateforme/src/config.ts::ECOUTES_UNIVERSELLES`.
ECOUTES_UNIVERSELLES = ("0.0.0.0", "::", "[::]", "*")


def _est_universelle(brut) -> bool:
    """True if `brut` is one of the four universal listen addresses.

    The ONLY place in this package that tests this membership: the three
    public validators below use it, none of them repeats the test.
    """
    return isinstance(brut, str) and brut.strip() in ECOUTES_UNIVERSELLES


def valider_hote(brut: str, origine: str = "DESK_HOTE"):
    """The ONLY check that refuses a universal listen address for
    `PLATEFORME_HOTE`, whatever the ORIGIN of `brut`.

    🔴 EXTRACTED FROM `lire_hote()` IN CORRECTION ROUND 1 (batch 10A,
    29 August 2026): the review showed that `install.py` called
    `lire_hote()` ONLY when `facts.get("hote")` was empty —
    `facts["hote"] = "0.0.0.0"` therefore went through WITHOUT ever meeting this
    check, returned code 0, and wrote `PLATEFORME_HOTE=0.0.0.0` into
    `desk.env`. Unreachable by the real engine TODAY (which passes
    no `facts` to `install`), but the head docstring of `install.py`
    itself says this channel exists for "a future engine, or this test
    file" — and the `facts` contract gained keys DURING this very
    batch. A guard that bites on only ONE of the two entry paths is
    not the guard this batch exists to put in place. `install.py` now calls
    THIS function on `facts.get("hote")` when it is present,
    EXACTLY as on the derived value — same check, whatever
    the origin.

    `origine` only feeds the refusal message ("DESK_HOTE" for the value
    derived by default, "facts["hote"]" for a value supplied by the
    engine) — never the logic: the guard is IDENTICAL in both cases.

    Returns `(hote, None)` on success, `(None, raison)` if `brut` is a
    universal listen address: a refusal HERE, never a service that starts and then
    exposes itself on every interface.
    """
    if _est_universelle(brut):
        return None, (
            f"{origine}={brut!r} is a universal listen address: PLATEFORME_HOTE "
            "must never be one (see plateforme/src/config.ts, the guard of "
            "the pomerium mode — and this package's doctrine, stricter: "
            "no universal listen address, in any mode)"
        )
    return brut, None


def lire_hote():
    """`HOTE_DEFAUT`, overridable by `DESK_HOTE` (tests, or a future
    change of network topology) — never derived from the default route,
    see the comment above. ALWAYS goes through `valider_hote` — see
    its docstring for why this is no longer an inline `if` here.

    Returns `(hote, None)` on success, `(None, raison)` if the retained value
    (default or overridden) is a universal listen address.
    """
    brut = os.environ.get("DESK_HOTE") or HOTE_DEFAUT
    return valider_hote(brut, origine="DESK_HOTE")


# --- PLATEFORME_PROXY_DE_CONFIANCE: hole C of batch 10A --------------------
#
# `plateforme/src/config.ts::lireConfig` refuses to start in
# `PLATEFORME_AUTH=pomerium` mode without `PLATEFORME_PROXY_DE_CONFIANCE` (otherwise
# the identity arrives in a plain `X-Pomerium-Claim-Email` header, that no
# signature checks). Before this fix, `wizard.yaml` asked NO
# question allowing it to be declared, and `hooks/resolve.py::valider_auth_mode`
# therefore flatly REFUSED `auth_mode=pomerium` — which made the mode
# explicitly chosen by the repository owner (batch 10A, 29 August 2026)
# IMPOSSIBLE to install.
#
# 🔴 THE CHOSEN PATH: THE SAME DOCTRINE AS `PORT_DEFAUT`/`HOTE_DEFAUT` —
# A DERIVED VALUE, NEVER A FIFTH WIZARD QUESTION. The operator
# has no information that would let them answer correctly: the
# right value depends on the topology of THIS machine (Pomerium runs in
# `network_mode: host`), not on a choice an operator would type in.
#
# ⚠️ THE VALUE IS NOT MEASURED, IT IS REASONED — AND THAT IS SAID HERE, NOT
# DISGUISED. Pomerium (host network) contacts `PLATEFORME_HOTE:PLATEFORME_PORT`,
# that is an address that is LOCAL to it (`192.168.3.1`, carried by
# `internalBridge`, not `127.0.0.1`). On Linux, a connection to an
# IPv4 address that already belongs to a local interface (and is not
# `127.0.0.1`) is routed through the local routing table and shows up
# on the server side with `remoteAddress` = THAT SAME ADDRESS (the kernel does not rewrite
# the source to `127.0.0.1` for a local non-loopback address) — hence
# `192.168.3.1`, the same value as `PLATEFORME_HOTE`. This is reasoning,
# NOT A MEASUREMENT: nothing was listening on the port at the time of this
# survey (29 August 2026), and Pomerium is not yet retargeted to
# this address (that is part 10B). **Part 10B MUST confirm this
# value through the announcement line of the platform startup
# (`trusted proxies retained=…`, see `plateforme/src/http/annonces.ts`)
# once its route points at `192.168.3.1:3445`.**
#
# Overridable by `DESK_PROXY_DE_CONFIANCE` (tests, or once 10B measures
# a different value).
PROXY_DEFAUT = HOTE_DEFAUT


def lire_proxy_confiance():
    """`PROXY_DEFAUT`, overridable by `DESK_PROXY_DE_CONFIANCE`. Never raises
    and never refuses: it is a REASONED value (see the comment
    above), never missing."""
    return os.environ.get("DESK_PROXY_DE_CONFIANCE") or PROXY_DEFAUT


# --- System-wide Node.js (batch 10A, 29 August 2026, problem A) -----------
#
# 🔴 THERE IS NO SYSTEM-WIDE NODE ON THIS MACHINE, AND
# `hooks/assets/desk-plateforme.service` CARRIED `ExecStart=/usr/bin/npm
# start` — A PATH THAT DOES NOT EXIST. Checked on 29 August 2026: neither
# `/usr/bin/node`, nor `/usr/bin/npm`, nor `/usr/local/bin/node`, no Debian
# `nodejs` package. `node`/`npm` are only zsh FUNCTIONS that source
# nvm from `$HOME/.nvm` (mode 700, root only) — neither systemd
# (`DynamicUser=yes`) nor a hook launched for another user can
# reach them, and a Python `subprocess.run(["node", ...])` never goes
# through an interactive shell function anyway.
#
# 🔴 THE REMEDY IS TO DEPLOY NODE, NOT TO DISGUISE THE DEFECT. A symbolic
# link `/usr/bin/npm` would have hidden the problem instead of closing it.
# This batch copies the nvm `v24.9.0` tree (the one that already satisfies
# `engines.node` of `plateforme/package.json`) to a SYSTEM location,
# readable by all (`a+rX`) — WITHOUT the global nvm packages unrelated to
# this repository (`bats`, `corepack`, `@github/copilot`, `@google/gemini-cli`:
# 237 MiB on their own, measured on 29 August 2026, for a need that boils down to
# `bin/node` and `lib/node_modules/npm/`). See the batch report for the
# exact deployment command.
#
# `NODE_BIN_DEFAUT` is the path of the DIRECTORY holding `node`/`npm`/`npx` —
# never the path of a lone binary — so that the systemd unit AND the administration
# hooks can derive from it BOTH `ExecStart`/`Environment=
# PATH=` and the absolute path of the `npm` invoked as a subprocess.
# Overridable by `DESK_NODE_BIN` (tests, or a future Node version).
NODE_BIN_DEFAUT = "/opt/nivuus/node/bin"


def lire_node_bin():
    """`NODE_BIN_DEFAUT`, surchargeable par `DESK_NODE_BIN`."""
    return os.environ.get("DESK_NODE_BIN") or NODE_BIN_DEFAUT


# --- What comes from the `facts` channel: validated, never taken on trust ----
#
# 🔴 MINOR #7 OF TASK 4, WHOSE REASON WAS REFUTED BY THIS VERY
# BRANCH. It was filed as "still owed" on the grounds that "`facts` has a
# fixed shape, produced by `resolve.py` of the same package and never by a
# third party". Batch 10A showed the opposite ON THE NEIGHBOURING KEY: `facts["hote"]`
# went through `install.py` without ever meeting the guard on universal listen
# addresses, code 0, `PLATEFORME_HOTE=0.0.0.0` written into `desk.env` — it took
# an Important finding to close it. The same reason was still written for
# `turn_ecoute`, `turn_relais`, `proxy_confiance` and `port`. The final branch
# review overturned it; these three functions are that overturning.
#
# ⚠️ WHAT THESE VALIDATORS DO NOT PROMISE: that the address is PRIVATE.
# The batch 10A ruling refused an RFC1918 allow list — it would create a
# second notion of "safe address", diverging from the product guard
# (`plateforme/src/config.ts::ECOUTES_UNIVERSELLES`, four literals), and
# would forbid legitimate deployments. This reservation stands, and it is
# named in the results document.


def valider_adresse_de_facts(brut, origine: str, role: str, consequence: str):
    """An address coming from `facts`: not empty, not of another type, not
    universal.

    Returns `(adresse, None)` on success, `(None, raison)` otherwise. `role` names what
    the address serves ("coturn", "PLATEFORME_PROXY_DE_CONFIANCE") — it
    only goes into the message, never into the logic.

    🔴 `consequence` EXISTS BECAUSE THIS VALIDATOR SERVES THREE ROLES
    (`turn_ecoute`, `turn_relais`, `proxy_confiance`), AND THE UNIVERSAL
    LISTEN MESSAGE IS NOT THE SAME FOR EACH — final branch
    review, 30 August 2026: the message was written FOR coturn ("relay
    allocations on every interface") and therefore gave a
    correct refusal with an OFF-TOPIC reason as soon as `role` was
    `PLATEFORME_PROXY_DE_CONFIANCE`, which relays nothing. `consequence` names
    what THIS precise role would break; the caller supplies it, this
    validator never guesses it.
    """
    if not isinstance(brut, str) or not brut.strip():
        return None, (
            f"{origine}={brut!r} is not a usable address for "
            f"{role}: a non-empty string is expected"
        )
    if _est_universelle(brut):
        return None, (
            f"{origine}={brut!r} is a universal listen address: {role} must "
            f"never be one — {consequence}"
        )
    return brut.strip(), None


def valider_port_de_facts(brut, origine: str = 'facts["port"]'):
    """A port coming from `facts`: an integer from 1 to 65535, never anything else.

    🔴 `int(facts.get("port"))` RAISED an uncaught `ValueError` on
    any non-numeric value — hence a Python traceback and an exit
    code 1 at the operator's, where `install.py` knows how to write a sentence
    everywhere else. A boolean is refused explicitly: `int(True)` is
    `1`, a perfectly valid and perfectly absurd port.
    """
    if isinstance(brut, bool):
        return None, f"{origine}={brut!r} is a boolean, not a port"
    if isinstance(brut, str):
        brut_nettoye = brut.strip()
        if not brut_nettoye.isdigit():
            return None, f"{origine}={brut!r} is not an integer"
        value = int(brut_nettoye)
    elif isinstance(brut, int):
        value = brut
    else:
        return None, f"{origine}={brut!r} is not an integer"
    if not 1 <= value <= 65535:
        return None, f"{origine}={value} is outside the range 1-65535"
    return value, None
