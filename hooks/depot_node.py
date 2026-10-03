#!/usr/bin/env python3
"""The Node.js runtime on the target: what batch 10A had laid down BY HAND.

🔴 THIS MODULE CLOSES A HOLE NAMED BY THE FINAL BRANCH REVIEW (30 August
2026). `hooks/assets/desk-plateforme.service` launches
`__NODE_BIN__/npm start`, and `commun.py::NODE_BIN_DEFAUT` points that token
to `/opt/nivuus/node/bin` — but NO hook dropped anything at
that location. The tree found there on the development machine had
been copied by hand during batch 10A, and the command only lived in a
gitignored report. A fresh installation would therefore have laid down a service
structurally unable to start, and `install.py` — which REFUSES with
a sentence when `node_modules/.bin/tsx` is missing, and when
`proto/ts/plateforme.ts` is missing — would have said nothing at all about this one.

**Settled: the hook LAYS DOWN Node, it does not merely refuse.** The two
outcomes the review left open are not equivalent:

  - ONLY refusing would have reproduced the Critical finding of that same review —
    a gate no installation can pass, since nothing, anywhere,
    provisions Node on the target. A package that always refuses
    never installs;
  - laying it down is possible, and without guessing: `hooks/resolve.py` has ALREADY validated
    the `node` of the machine running the hooks against the
    `engines.node` bound that `plateforme/package.json` declares. It is that
    very `node` that gets dropped. **Intended side effect**: the version that
    `resolve` validated becomes the version the service runs — which
    incidentally closes minor #6 of task 3 ("the Node check
    queries the machine running the hook, never the target"). The two
    machines were not the same; from now on the runtime travels from one
    to the other, so the measurement bears on what will run.

What is dropped, and nothing more — the EXACT shape read on the tree
laid down by hand on 29 August 2026 (`ls -la /opt/nivuus/node/bin`,
`ls /opt/nivuus/node/lib/node_modules`):

    <prefix>/bin/node              (the binary, ~124 MiB)
    <prefix>/bin/npm  -> ../lib/node_modules/npm/bin/npm-cli.js
    <prefix>/bin/npx  -> ../lib/node_modules/npm/bin/npx-cli.js
    <prefix>/lib/node_modules/npm  (~20 MiB)

⚠️ THE GLOBAL PACKAGES UNRELATED TO THIS REPOSITORY ARE NOT COPIED
(`bats`, `corepack`, `@github/copilot`, `@google/gemini-cli`: 237 MiB on their
own, measured in batch 10A). The need boils down to `bin/node` and `npm`.

⚠️ THE TWO LINKS ARE RECREATED AS LINKS, never followed: their target is
RELATIVE and points inside the dropped tree, so it resolves
correctly at the destination. Following them would drop two copies of the same
script under a name that would pretend to be `npm`.

Like `commun.py`, `depot_arbre.py` and `vm.py`, this module is not a hook
runnable on its own: `hooks/install.py` imports it.
"""
import os
import pathlib
import shutil
import subprocess

from borne_node import lire_borne_node, verifier_version, version_de
from commun import lire_node_bin
from depot_arbre import copier_arbre, make_world_readable

# The three executables expected under `<prefix>/bin`, and the only global
# package that travels. Stated here rather than guessed by enumeration: an
# nvm `bin/` also holds the global packages of its owner.
EXECUTABLES = ("node", "npm", "npx")
PAQUET_GLOBAL = pathlib.Path("lib") / "node_modules" / "npm"


def _manquants(prefixe: pathlib.Path) -> list:
    """What a usable runtime prefix lacks: nothing, or the names to report."""
    return [nom for nom in ("bin/node",) + (str(PAQUET_GLOBAL),)
            if not (prefixe / nom).exists()]


def _prefixe_sur_le_path():
    """The prefix of the `node` this machine runs, or `(None, raison)`.

    Never `command -v node`: on this machine, `node` is a zsh FUNCTION that
    sources nvm, and a `subprocess` never goes through an interactive
    shell function anyway — that is the diagnosis of batch 10A (see
    `commun.py::NODE_BIN_DEFAUT`). `process.execPath` returns
    `<prefix>/bin/node`, so `parents[1]` is the prefix.
    """
    try:
        r = subprocess.run(
            ["node", "-e", "process.stdout.write(process.execPath)"],
            capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as exc:
        return None, f"node is missing or silent on this machine ({exc})"
    if r.returncode != 0 or not r.stdout.strip():
        return None, ("`node -e process.execPath` returned nothing usable "
                      f"(code {r.returncode})")
    return pathlib.Path(r.stdout.strip()).resolve().parents[1], None


def _runtime_depose_conforme(deja: pathlib.Path):
    """The dropped runtime, only if its `node` satisfies the CURRENT
    package's `engines.node`: a later desk release may raise the bound, and
    reusing an older runtime would let the install succeed and leave a
    service unable to start. Same bound and verdict as resolve.py."""
    version = version_de(deja / "bin" / "node")
    if version is None:
        return None, (f"the Node runtime already dropped at {deja} does not "
                      "answer `node --version`: not a runtime to stand on")
    borne, raison = lire_borne_node()
    if raison:
        return None, raison
    version, raison = verifier_version(version, borne)
    if raison:
        return None, (f"the Node runtime already dropped at {deja} cannot "
                      f"stand in: {raison}. Run this hook with a compatible "
                      "`node` on the PATH, or name one in DESK_NODE_SOURCE")
    return deja, None


def racine_node_source(root=None):
    """The Node prefix to deploy. Returns `(chemin, None)` or `(None, raison)`.

    Overridable by `DESK_NODE_SOURCE` — for the tests (which lay out a
    dummy tree of a few bytes rather than copying 144 MiB for each
    scenario), and for an operator who would like to deploy a runtime other than
    the one running the hook.

    Otherwise, the prefix is DERIVED from the interpreter itself (see
    `_prefixe_sur_le_path`). And when this machine has no `node` on its
    PATH, the runtime ALREADY DROPPED under `root` (the parent of
    `lire_node_bin()`) stands in, provided it is complete. That is the
    REPLAY case: `nivuus update desk` runs this hook again on a target
    whose only Node is the one its first install dropped — nowhere on
    PATH, by construction (measured 2026-10-03: a plain root PATH has no
    `node` on the reference machine either). Refusing there would mark
    desk failed on every update. It stands in only if its version still
    satisfies this package's `engines.node` (see _runtime_depose_conforme).
    The PATH stays first so an operator who runs the hook with a newer
    `node` still upgrades the runtime.
    """
    brut = os.environ.get("DESK_NODE_SOURCE")
    if brut:
        prefixe = pathlib.Path(brut)
    else:
        prefixe, raison = _prefixe_sur_le_path()
        if prefixe is None:
            deja = None
            if root is not None:
                deja = pathlib.Path(root) / lire_node_bin().lstrip("/")
                deja = deja.parent
                if not _manquants(deja):
                    return _runtime_depose_conforme(deja)
            return None, (
                f"{raison}: cannot locate a Node runtime to drop on the "
                "target, even though desk's systemd unit launches npm"
                + (f" — and none is already dropped at {deja} to stand in"
                   if deja is not None else ""))
    manquants = _manquants(prefixe)
    if manquants:
        return None, (
            f"the source Node runtime {prefixe} is incomplete: "
            f"{', '.join(manquants)} missing. A desk service without "
            "`node` or `npm` dropped cannot start (see "
            "hooks/assets/desk-plateforme.service::ExecStart)."
        )
    return prefixe, None


def deposer_node(prefixe: pathlib.Path, destination: pathlib.Path) -> None:
    """Copies the runtime from `prefixe` to `destination` (the target PREFIX,
    not its `bin/`), and makes it readable by all.

    `make_world_readable` is essential, not cosmetic:
    `DynamicUser=yes` runs the service under an ephemeral UID that
    belongs to no group shared with the files dropped by root —
    that is the real bug of batch 10A, already paid for on `/opt/nivuus/desk` (see
    `depot_arbre.py::make_world_readable`). A `bin/node` as `rwxr-x---`
    would give an `ExecStart` that fails before the first line of
    JavaScript.
    """
    if destination.exists() and prefixe.resolve() == destination.resolve():
        # A replay standing on the runtime already dropped here (see
        # racine_node_source): nothing to copy, and copying would first
        # delete the very tree it then reads from.
        return
    bin_dest = destination / "bin"
    if bin_dest.exists() or bin_dest.is_symlink():
        shutil.rmtree(bin_dest)
    bin_dest.mkdir(parents=True)
    for nom in EXECUTABLES:
        source = prefixe / "bin" / nom
        if not source.exists() and not source.is_symlink():
            continue          # a runtime may legitimately lack `npx`
        cible = bin_dest / nom
        if source.is_symlink():
            os.symlink(os.readlink(source), cible)
        else:
            shutil.copy2(source, cible)
    copier_arbre(prefixe / PAQUET_GLOBAL, destination / PAQUET_GLOBAL)
    make_world_readable(destination)
