#!/usr/bin/env python3
"""`engines.node` of plateforme/package.json, and what satisfies it.

Shared by two hooks that measure two different runtimes: `resolve.py`
validates the `node` of the machine running the wizard, before the target
exists; `depot_node.py`, at a REPLAY (`nivuus update desk`), validates the
runtime its first install dropped on the target, the only one the target
has. One bound, one parser, one verdict - a second copy of the comparison
would validate its copy (the "487" wreck of docs/claude/pitfalls-docs-size-vm.md).

Like `commun.py`, not a hook runnable on its own.
"""
import json
import pathlib
import re
import subprocess

RACINE = pathlib.Path(__file__).resolve().parents[1]


def lire_borne_node():
    """Reads back `engines.node` from plateforme/package.json.

    🔴 READ FROM THE FILE, NEVER COPIED FROM A PLAN: a copied range outlives
    the reality it described. Returns (bound, None) on success, or
    (None, raison) if the file is unreadable or declares no `engines.node`
    - a failure path like any other, which turns into a refusal, never an
    exception.
    """
    chemin = RACINE / "plateforme" / "package.json"
    try:
        contenu = json.loads(chemin.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return None, f"cannot read {chemin}: {exc}"
    borne = (contenu.get("engines") or {}).get("node")
    if not borne or not isinstance(borne, str):
        return None, f"{chemin} declares no usable engines.node bound"
    return borne, None


def parser_borne(borne: str):
    """Parses a two-clause bound ('>=A.B.C <X.Y.Z') into two tuples.

    Understands ONLY the two operators actually present in
    `plateforme/package.json` (`>=` and `<`): a format richer than the one
    we measured need not be guessed, it must make parsing fail - and
    therefore refuse, never crash.
    """
    mini = maxi = None
    for clause in borne.split():
        correspond = re.match(r"^(>=|<)(\d+)\.(\d+)\.(\d+)$", clause)
        if not correspond:
            return None
        operateur, a, b, c = correspond.groups()
        value = (int(a), int(b), int(c))
        if operateur == ">=":
            mini = value
        else:
            maxi = value
    if mini is None or maxi is None:
        return None
    return mini, maxi


def version_de(node_bin="node"):
    """`<node_bin> --version` without its leading `v`, or None when the
    binary is missing, silent or failing. `node` alone asks the PATH."""
    try:
        r = subprocess.run([str(node_bin), "--version"], capture_output=True,
                           text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0 or not r.stdout.strip():
        return None
    return r.stdout.strip().lstrip("v")


def check_version(version: str, borne_brute: str):
    """(version, None) when `version` satisfies the bound, (None, raison) otherwise."""
    bornes = parser_borne(borne_brute)
    if bornes is None:
        return None, f"unreadable engines.node bound: {borne_brute!r}"
    mini, maxi = bornes
    try:
        version_tuple = tuple(int(x) for x in version.split(".")[:3])
    except ValueError:
        return None, f"unreadable node version: {version!r}"
    if not (mini <= version_tuple < maxi):
        return None, (
            f"node {version} does not satisfy engines.node={borne_brute!r} "
            "declared by plateforme/package.json"
        )
    return version, None
