#!/usr/bin/env python3
"""The `hw` contract: what the ENGINE sends, never what a test invents.

🔴 THIS SUITE EXISTS BECAUSE OF A CRITICAL FINDING OF THE FINAL BRANCH REVIEW
(30 August 2026). `hooks/resolve.py` refused on `hw.get("vm_windows")` —
a key that NO producer of the engine sets. The package therefore could
not install: the refusal becomes a `StepError` on the engine side
(`installer/installer/install-engine/steps/packages.py`), which stops
the WHOLE installation.

**The eight existing suites could not see it, and that is the lesson**:
they BUILD the context they send (`{"hw": {"vm_windows":
True}}`), so they themselves produced the fact whose consumption they
checked. A test that invents its input can never discover that
nobody produces it.

What this suite freezes, and nothing else freezes:

  ① the set of keys that `installer/installer/common/hardware.py::
     detect_all()` really RETURNS — read in the sibling repository, by
     syntax analysis (`ast`), never run (the real detection
     invokes `lsblk`, `lspci`…: we want the CONTRACT, not the machine);
  ② that `hooks/resolve.py` reads NO `hw` key absent from this
     set — it is exactly the Critical finding, frozen;
  ③ that `hooks/activate.py` reads no `hw` key absent from this
     set WIDENED to the `facts` that `resolve` emits itself: the engine
     merges them into `hw` before calling `activate`
     (`installer/installer/packages/runner.py::run_activate`,
     `merge_into_hw`);
  ④ that a `resolve` fed the context the ENGINE would send — an `hw`
     carrying exactly the keys of `detect_all()`, and nothing more —
     emits NO refusal;
  ⑤ that NO fact emitted by `resolve` is a LITERAL hardcoded in the
     emission dict — it is the HOLE that ③ left open, under a name
     close to the Critical finding: ③ allows `activate` to read any key
     PRESENT in `CLES_FACTS`, but says NOTHING about how that
     key was obtained. A hardcoded fact (`"vm_repond": True`) is a
     key of `CLES_FACTS` like any other, and ③ would therefore let it
     through as is — it is exactly the shape of the real defect found on 30 August
     2026 (`hooks/resolve.py:393`, before the fix), and the reason
     the Critical finding above could reappear under a different name
     despite ③ already being in place.

⚠️ IF THE SIBLING REPOSITORY IS ABSENT, THIS SUITE FAILS, it does not skip itself:
a contract that cannot be checked is not a checked contract, and "a
fallback `||` turns a missing file into a green check". The path is
overridable through `DESK_INSTALLER_RACINE` — same convention as
`tests/test_desk_manifeste.py`, which already imports the sibling engine.

Run: python3 tests/test_desk_contrat_hw.py
"""
import ast
import json
import os
import pathlib
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
INSTALLER = pathlib.Path(os.environ.get("DESK_INSTALLER_RACINE")
                          or (RACINE.parent / "installer"))
HARDWARE = INSTALLER / "installer" / "common" / "hardware.py"

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# --- ① The keys the engine REALLY produces --------------------------

def cles_de_detect_all(chemin: pathlib.Path) -> set:
    """The keys of the literal dict that `detect_all()` returns.

    Read through `ast`, never run: `detect_all()` shells out to `lsblk`,
    `lspci`, `/proc/meminfo`… — we want the SHAPE of the contract, not the state of
    the machine running these tests.
    """
    arbre = ast.parse(chemin.read_text(encoding="utf-8"))
    for noeud in ast.walk(arbre):
        if isinstance(noeud, ast.FunctionDef) and noeud.name == "detect_all":
            for interne in ast.walk(noeud):
                if isinstance(interne, ast.Return) and isinstance(interne.value, ast.Dict):
                    return {cle.value for cle in interne.value.keys
                            if isinstance(cle, ast.Constant)}
    return set()


if not HARDWARE.is_file():
    failures.append(
        f"the producer of `hw` is not found: {HARDWARE} — this suite "
        "can NOT check the contract without it, and a check that "
        "skips itself when its part is missing is a green check by accident. "
        "Override DESK_INSTALLER_RACINE if the sibling repository lives elsewhere."
    )
    CLES_MOTEUR = set()
else:
    CLES_MOTEUR = cles_de_detect_all(HARDWARE)

check("detect_all() returns a non-empty set of keys", bool(CLES_MOTEUR), True)


# --- What a hook READS from `hw`, and what `resolve` EMITS as facts -------

def keys_read_in(chemin: pathlib.Path, nom_variable: str) -> set:
    """The literal keys read on `<nom_variable>` — `x.get("k")` and
    `x["k"]` — in the given file."""
    arbre = ast.parse(chemin.read_text(encoding="utf-8"))
    cles = set()
    for noeud in ast.walk(arbre):
        if (isinstance(noeud, ast.Call)
                and isinstance(noeud.func, ast.Attribute)
                and noeud.func.attr == "get"
                and isinstance(noeud.func.value, ast.Name)
                and noeud.func.value.id == nom_variable
                and noeud.args and isinstance(noeud.args[0], ast.Constant)
                and isinstance(noeud.args[0].value, str)):
            cles.add(noeud.args[0].value)
        if (isinstance(noeud, ast.Subscript)
                and isinstance(noeud.value, ast.Name)
                and noeud.value.id == nom_variable
                and isinstance(noeud.slice, ast.Constant)
                and isinstance(noeud.slice.value, str)):
            cles.add(noeud.slice.value)
    return cles


def cles_des_facts(chemin: pathlib.Path) -> set:
    """The keys of the `facts` dict that `resolve.py` emits."""
    arbre = ast.parse(chemin.read_text(encoding="utf-8"))
    for noeud in ast.walk(arbre):
        if not isinstance(noeud, ast.Dict):
            continue
        for cle, value in zip(noeud.keys, noeud.values):
            if (isinstance(cle, ast.Constant) and cle.value == "facts"
                    and isinstance(value, ast.Dict)):
                return {k.value for k in value.keys
                        if isinstance(k, ast.Constant)}
    return set()


CLES_FACTS = cles_des_facts(RACINE / "hooks" / "resolve.py")
check("resolve.py does emit a non-empty `facts` dict", bool(CLES_FACTS), True)


# --- ② `resolve` only reads what `detect_all()` produces -----------------
# 🔴 IT IS THE CRITICAL FINDING, FROZEN. Before 30 August 2026, this assertion
# would have returned {'vm_windows'} — a key found nowhere in the WHOLE sibling
# repository (`grep -rn vm_windows ../installer/`: no output).
lues_resolve = keys_read_in(RACINE / "hooks" / "resolve.py", "hw")
check("resolve reads no hw key that detect_all() does not produce",
      sorted(lues_resolve - CLES_MOTEUR), [])


# --- ③ `activate`: detect_all() WIDENED to resolve's facts --------------
# The engine merges the facts INTO hw before calling activate
# (runner.py::run_activate -> merge_into_hw), so a facts key is
# legitimate there — and a key that is NEITHER in detect_all() NOR in facts
# can come from nowhere.
lues_activate = keys_read_in(RACINE / "hooks" / "activate.py", "hw")
check("activate only reads detect_all() or resolve's facts from hw",
      sorted(lues_activate - (CLES_MOTEUR | CLES_FACTS)), [])


# --- ④ The context the ENGINE sends makes nobody refuse ----------
# 🔴 THIS IS NOT A BUILT CONTEXT: its keys are exactly those that
# `detect_all()` returns, read above. The VALUES are empty (no
# disk, no GPU…) — it is the worst case, and the one that a fresh
# machine where nothing is partitioned yet resembles most.
contexte_moteur = {cle: [] for cle in sorted(CLES_MOTEUR)}
REPONSES = {"admin_email": "a@b.c", "admin_password": "x",
            "auth_mode": "motdepasse", "vb_audio": False}
proc = subprocess.run(
    [sys.executable, str(RACINE / "hooks" / "resolve.py")],
    input=json.dumps({"hw": contexte_moteur, "answers": REPONSES}),
    capture_output=True, text=True)
evenements = []
for ligne in proc.stdout.splitlines():
    ligne = ligne.strip()
    if not ligne:
        continue
    try:
        evenements.append(json.loads(ligne))
    except json.JSONDecodeError:
        pass
refus = [e for e in evenements if e.get("event") == "refuse"]
check("ENGINE context: exit code 0", proc.returncode, 0)
check("ENGINE context: NO refusal", [e.get("reason") for e in refus], [])
check("ENGINE context: a facts event is emitted",
      len([e for e in evenements if e.get("event") == "facts"]), 1)


# --- ⑤ No emitted fact is a hardcoded literal -----------------------
# 🔴 WHAT ESCAPED ③ ABOVE: ③ checks that the key is KNOWN
# (present in `CLES_FACTS`), never that its VALUE was MEASURED. A
# measured value always comes from an EXPRESSION (a variable already validated
# earlier in the hook, a call); a constant written by hand
# directly in the emission dict (`True`, `None`, a number, a
# string literal) can NOT be a measurement — by construction, it
# depends on no input. It is exactly the shape of `"vm_repond": True`.
def facts_with_literals(chemin: pathlib.Path) -> list:
    """The keys of the `facts` dict whose value is a hardcoded literal."""
    arbre = ast.parse(chemin.read_text(encoding="utf-8"))
    for noeud in ast.walk(arbre):
        if not isinstance(noeud, ast.Dict):
            continue
        for cle, value in zip(noeud.keys, noeud.values):
            if (isinstance(cle, ast.Constant) and cle.value == "facts"
                    and isinstance(value, ast.Dict)):
                return sorted(
                    k.value for k, v in zip(value.keys, value.values)
                    if isinstance(k, ast.Constant) and isinstance(v, ast.Constant)
                )
    return []


check("no fact emitted by resolve is a hardcoded literal",
      facts_with_literals(RACINE / "hooks" / "resolve.py"), [])

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print(f"OK - hw contract: {len(CLES_MOTEUR)} keys produced by "
      f"detect_all(), {len(CLES_FACTS)} facts emitted by resolve")
