#!/usr/bin/env python3
"""The configuration files that `install` LAYS DOWN, and the secrets it draws.

Extracted from `hooks/install.py` on 30 August 2026, in a DEDICATED commit and
BEFORE any addition — never by compressing afterwards. Reason: the final branch
review requires two more guards of `install.py` (a pre-flight that refuses
before a secret is written, and the drop of the Node runtime), and the file
was at 454 lines out of 500. This repository paid twelve times for retroactive
compression; the rule is "EXTRACT, NEVER COMPRESS, in a task
dedicated to it BEFORE the one that adds".

Like `hooks/commun.py`, `hooks/depot_arbre.py` and `hooks/vm.py`, this module
IS NOT a hook runnable on its own (no `--phase`, no stdin JSON):
`hooks/install.py` imports it, and Python automatically adds the directory
of the LAUNCHED script (`hooks/`) at the head of `sys.path`, so the import resolves without
any extra manipulation.

⚠️ VERBATIM EXTRACTION: the three functions and the constant below are
moved WITHOUT A SINGLE BEHAVIOUR CHANGE — their docstrings, which
carry the reason for each choice (the atomic 0600 mode, the extrapolated coturn
format), travel with them. Only the deictics "above" /
"this file" that pointed to `install.py` were renamed so as not to
lie in their new location.
"""
import pathlib
import secrets
import stat

from atomic_file import replace_file
from turn_peers import peer_restriction_lines

# The standard TURN port, the one docker-compose.coturn.yml sets through
# `--listening-port=3478` — the only value that makes the URL
# announced to clients (TURN_URL) match the port coturn actually
# listens on.
PORT_TURN = 3478


def write_secret() -> str:
    """Draws a FRESH random secret — never asked for, never constant.

    The word "FRESH" is deliberate: this function, on its own, draws a
    NEW secret on EVERY call, without exception — that is the behaviour
    wanted the very first time a secret is needed (none exists
    yet to reuse). `secrets.token_hex(32)` returns 64 hexadecimal
    characters, well above the minimum of 32 that
    `MIN_SECRET_LENGTH` demands.

    🔴 THIS FUNCTION ALONE DOES NOT CARRY THE INVARIANT "drawn only once,
    never recomputed" — an earlier docstring claimed it HERE, wrongly
    (real bug, found and fixed on 2026-09-08: `install.py` called this
    function unconditionally on every run, so two installations on
    the same root drew two DIFFERENT secrets — `PLATEFORME_SECRET_JETON`
    included — and silently invalidated all sessions as well as
    coturn authentication on every install REPLAY, exactly the
    path the update of this plan takes). `PLATEFORME_SECRET_JETON` has
    moreover NO default on the product side
    (`plateforme/src/config.ts::lireConfig`): that is precisely why a
    secret that changes on every hook run is a defect, never a
    feature.

    The invariant is now carried by the CALLER, never by this
    function: `install.py` must first try `lire_secret_persiste()` on
    the `desk.env` already in place at the TARGET root, and call
    `write_secret()` only if nothing there is reusable.
    """
    return secrets.token_hex(32)


def lire_secret_persiste(chemin_env: pathlib.Path, cle: str) -> str | None:
    """Reads back a secret already written by an earlier installation of `desk.env`.

    It is THIS function, called by `install.py` BEFORE `write_secret()`,
    that really carries the invariant "drawn only once, never
    recomputed" that the `write_secret` docstring described without
    enforcing it (bug fixed on 2026-09-08 — see its own docstring).

    Three cases are treated as "nothing to reuse", never as an
    error that would make the install fail for a reason unrelated to
    the installation itself — in ALL three, `return None`,
    silently:
      - `chemin_env` does not exist (first install on this root);
      - `chemin_env` vanished BETWEEN the existence test and the read — a
        real race, but one that comes back to the same case as above: nothing to
        read, never a failure to report;
      - `cle` is present but its value is empty, or made only of
        spaces (a truncated or hand-edited file), or `chemin_env`
        exists without carrying `cle` (an earlier installation of a version
        that did not write this key yet).

    🔴 A FOURTH CASE IS DELIBERATELY *EXCLUDED* FROM THIS LIST, AND THAT IS
    THE POINT OF THIS FIX (2026-09-08, review): `chemin_env` EXISTS but
    can NOT be read — permissions skewed by a partial migration,
    disk error, anything that is not "the file is simply not
    there". An `except OSError` that swallowed THAT case too would redo
    EXACTLY the bug this module fixes, through a narrower door:
    a `desk.env` present but momentarily unreadable would draw a
    NEW secret SILENTLY — the same silent rotation as the original bug,
    just triggered differently. This function therefore does NOT swallow it:
    only `FileNotFoundError` (the TOCTOU race above) is caught;
    any OTHER `OSError` (`PermissionError`, a disk error, …) propagates
    to the caller as is. `install.py` catches it in turn and
    REFUSES the installation rather than drawing a hallucinated secret — see
    its own comment at the call site.
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
        cle_ligne, _, value = ligne.partition("=")
        if cle_ligne.strip() != cle:
            continue
        value = value.strip()
        return value or None
    return None


def write_env(chemin: pathlib.Path, values: dict) -> None:
    """Writes `chemin` as KEY=VALUE, one per line, owner-only (0600).

    The file holds `PLATEFORME_SECRET_JETON` in plain text. It is REPLACED
    atomically by `atomic_file.replace_file` (see its docstring): the path
    always holds either the complete previous file or the complete new one,
    the content is never readable by "other" at any instant, and the final
    mode is 0600 whatever the mode of the file it replaces — `O_CREAT`'s
    mode, used here before, only applied to a NEW file.
    """
    corps = "\n".join(f"{cle}={value}" for cle, value in values.items()) + "\n"
    replace_file(chemin, corps, 0o600)


def write_turnserver_conf(chemin: pathlib.Path, turn_ecoute: str,
                            turn_relais: str, secret: str,
                            vm_address: str) -> None:
    """Lays down the native configuration of the Debian `coturn` package.

    The directive names and their syntax were checked against the coturn
    4.6.1 sources, the version Debian trixie ships (4.6.1-2):
    `examples/etc/turnserver.conf` and the option table of
    `src/apps/relay/mainrelay.c`. The file has NOT been run by a coturn on
    this machine (`coturn` is not installed on it) — the first
    `turnserver -c this-file` actually run remains the check of record.

    The relay's peers are confined by `turn_peers.peer_restriction_lines`:
    every non-public destination and the relay's own addresses are denied,
    and only `vm_address` — the controlled machine (`commun.
    read_vm_address`) — is allowed back. An invalid `vm_address` raises
    `ValueError` BEFORE anything is written: coturn would skip the entry it
    cannot parse and start anyway.

    🔴 LAID DOWN, NOT ARMED BY THIS HOOK — BUT THE DISTRIBUTION ARMS IT.
    Nothing here enables or starts coturn. However the Debian package, which
    the manifest's `apt:` list installs, enables and starts `coturn.service`
    at installation (`debian/rules` runs `dh_installsystemd` with its
    defaults), and that unit runs `turnserver -c /etc/turnserver.conf`
    directly: it does not read `/etc/default/coturn` (TURNSERVER_ENABLED is
    only read by the SysV init script). On a systemd host the relay
    therefore runs with this file as soon as both are in place.
    """
    restrictions = "\n".join(
        peer_restriction_lines(vm_address, (turn_ecoute, turn_relais)))
    corps = f"""# turnserver.conf — laid down by the install hook of the desk package.
# Directives checked against the coturn 4.6.1 sources (Debian trixie), not yet
# run by a coturn on this disk — see the docstring of write_turnserver_conf.
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
# Peers: every non-public destination and the relay's own addresses are
# denied; only the controlled machine is allowed (hooks/turn_peers.py).
{restrictions}
"""
    # This file holds `static-auth-secret` in plain text: replaced atomically
    # like `write_env`, never readable by "other". See `turnserver_conf_owner`
    # for the group the coturn daemon needs.
    mode, gid = turnserver_conf_owner(chemin)
    replace_file(chemin, corps, mode, gid)


def turnserver_conf_owner(chemin: pathlib.Path) -> tuple:
    """The (mode, gid) the replaced `turnserver.conf` must carry.

    The Debian `coturn` package (4.6.1-2, trixie: `debian/coturn.postinst`)
    registers `dpkg-statoverride root turnserver 640 /etc/turnserver.conf`,
    and its unit runs the daemon as `User=turnserver`/`Group=turnserver`:
    the daemon reads its configuration through the GROUP read bit. Replacing
    the inode would drop that group and lock the daemon out of its own
    configuration, so when the existing file grants group read, its group
    and that bit are kept (0640). Otherwise — no file yet, or a file without
    group read — the new file is owner-only (0600), returned with no group
    change. "Other" never gets anything in either case.
    """
    try:
        existing = chemin.stat()
    except FileNotFoundError:
        return 0o600, None
    if existing.st_mode & stat.S_IRGRP:
        return 0o640, existing.st_gid
    return 0o600, None
