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

# The standard TURN port, the one docker-compose.coturn.yml sets through
# `--listening-port=3478` — the only value that makes the URL
# announced to clients (TURN_URL) match the port coturn actually
# listens on.
PORT_TURN = 3478


def ecrire_secret() -> str:
    """Draws a FRESH random secret — never asked for, never constant.

    The word "FRESH" is deliberate: this function, on its own, draws a
    NEW secret on EVERY call, without exception — that is the behaviour
    wanted the very first time a secret is needed (none exists
    yet to reuse). `secrets.token_hex(32)` returns 64 hexadecimal
    characters, well above the minimum of 32 that
    `LONGUEUR_SECRET_MIN` demands.

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
    `ecrire_secret()` only if nothing there is reusable.
    """
    return secrets.token_hex(32)


def lire_secret_persiste(chemin_env: pathlib.Path, cle: str) -> str | None:
    """Reads back a secret already written by an earlier installation of `desk.env`.

    It is THIS function, called by `install.py` BEFORE `ecrire_secret()`,
    that really carries the invariant "drawn only once, never
    recomputed" that the `ecrire_secret` docstring described without
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
        cle_ligne, _, valeur = ligne.partition("=")
        if cle_ligne.strip() != cle:
            continue
        valeur = valeur.strip()
        return valeur or None
    return None


def ecrire_env(chemin: pathlib.Path, valeurs: dict) -> None:
    """Writes `chemin` as KEY=VALUE, one per line, ALREADY CREATED in mode 600.

    🔴 CREATED AS 0600, NEVER WRITTEN THEN `chmod`ED AFTERWARDS (correction
    round 1, task 4): between a `write_text` and a later
    `os.chmod`, the file briefly exists with the default mode of the process
    `umask` (644 in the most common case) — a real exposure
    window for a file that holds `PLATEFORME_SECRET_JETON`
    in plain text. `os.open(..., mode=0o600)` sets the permission ATOMICALLY at
    creation: the `mode` of an `open(2)` with `O_CREAT` is always
    masked by the process `umask` (which can only REMOVE bits,
    never add them), so the result is at most 0600, never more
    permissive — there is no instant where the file is readable by
    others.
    """
    chemin.parent.mkdir(parents=True, exist_ok=True)
    corps = "\n".join(f"{cle}={valeur}" for cle, valeur in valeurs.items()) + "\n"
    descripteur = os.open(chemin, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descripteur, "w", encoding="utf-8") as fh:
        fh.write(corps)


def ecrire_turnserver_conf(chemin: pathlib.Path, turn_ecoute: str,
                            turn_relais: str, secret: str) -> None:
    """Lays down the native configuration of the Debian `coturn` package.

    ⚠️ FORMAT NOT VERIFIED ON THIS MACHINE — `coturn` is not installed on it
    (`dpkg -l coturn` returns nothing there at the time of writing). The directives
    below reuse, as is, the options already verified and
    commented in `docker-compose.coturn.yml` (`--listening-ip`,
    `--relay-ip`, `--static-auth-secret`, etc.): each coturn option has,
    by construction of the software, a configuration file directive
    of the same name without the `--` prefix. This is a reasonable extrapolation,
    not a measurement — to be confirmed at the first `turnserver -c this-file`
    actually run.

    🔴 LAID DOWN, NOT ARMED: this file is not enough to make coturn run —
    `/etc/default/coturn` (TURNSERVER_ENABLED) is not touched here, by the
    same doctrine as the desk-plateforme unit (see its comment):
    laying down is not arming. No task of this plan arms coturn; it is a
    named legacy, not an oversight — see the results document of the project.
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
    # Same precaution as `ecrire_env` (see its docstring): this file
    # holds `static-auth-secret` in plain text, and a write-then-chmod window
    # would be WORSE there than on desk.env — created already as 0600, never chmod
    # afterwards.
    descripteur = os.open(chemin, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descripteur, "w", encoding="utf-8") as fh:
        fh.write(corps)
