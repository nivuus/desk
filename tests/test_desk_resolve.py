#!/usr/bin/env python3
"""Tests of the resolve hook of the desk package.

The hook is exercised through its REAL interface — a subprocess fed
{"hw":…, "answers":…} on stdin, which answers in jsonl on stdout — and not through
an import: that is how the engine calls it, and a package must be able to
run on a Debian that has never seen this engine.

Run: python3 tests/test_desk_resolve.py
"""
import json
import os
import pathlib
import subprocess
import sys

RACINE = pathlib.Path(__file__).resolve().parents[1]
HOOK = RACINE / "hooks" / "resolve.py"

sys.path.insert(0, str(RACINE / "hooks"))
from commun import HOTE_DEFAUT, PROXY_DEFAUT  # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


def appeler(hw=None, answers=None, env=None):
    """Calls the hook like the engine: stdin JSON, stdout jsonl."""
    contexte = json.dumps({"hw": hw or {}, "answers": answers or {}})
    r = subprocess.run([sys.executable, str(HOOK)], input=contexte,
                       capture_output=True, text=True, env=env)
    evenements = []
    for ligne in r.stdout.splitlines():
        ligne = ligne.strip()
        if not ligne:
            continue
        try:
            evenements.append(json.loads(ligne))
        except json.JSONDecodeError:
            pass          # the engine relays these lines as progress
    return r.returncode, evenements


REPONSES = {"admin_email": "a@b.c", "admin_password": "x",
            "auth_mode": "motdepasse", "vb_audio": False}


def refus(evenements):
    return [e for e in evenements if e.get("event") == "refuse"]


# 🔴 THIS FILE NO LONGER SETS ANY `hw` KEY — AND THAT IS THE FIX OF THE
# CRITICAL FINDING OF THE FINAL BRANCH REVIEW (30 August 2026). Every call
# below set a truthy `vm_windows` key, and a leading scenario
# exercised the refusal on the same key set false. Both were GREEN, and
# both were false to the real world: **no producer of that key exists**
# (`installer/installer/common/hardware.py::detect_all()` returns eight, none
# of that name), so the hook ALWAYS refused in the engine — a refusal that
# `steps/packages.py` turns into a `StepError`, that is, stopping
# the WHOLE installation. The suite could not see it because it
# BUILT itself the fact whose consumption it checked.
#
# The gate moved to `hooks/activate.py` (the only phase where the VM can
# exist), and the `hw` contract is now frozen by a dedicated suite:
# `tests/test_desk_contrat_hw.py`, which reads the keys of the PRODUCER instead of
# inventing them. `hw={}` below is not a convenience: it is what
# `resolve` must be able to accept.

# --- The nominal case: facts, no refusal ------------------------------
rc, ev = appeler(hw={}, answers=REPONSES)
check("hw without any key: no refusal", refus(ev), [])
check("hw without any key: exit code 0", rc, 0)
faits = [e for e in ev if e.get("event") == "facts"]
check("hw without any key: one facts event", len(faits), 1)
mesures = faits[0]["facts"] if faits else {}
check("BOTH TURN addresses are derived",
      all(k in mesures for k in ("turn_ecoute", "turn_relais")), True)

# 🔴 docker-compose.coturn.yml requires TURN_LISTENING_IP **and** TURN_RELAY_IP,
# both mandatory: bounding the listen address alone would leave the relay allocations
# on every interface — measured on 21 August 2026, 23 distinct addresses
# including the public address.

# --- The platform listen address (hote), and the trusted proxy --
# 🔴 FIXES A REAL BUG FOUND IN BATCH 10A (29 August 2026): `hote` MUST be
# DIFFERENT from `turn_ecoute` — the two were conflated before this
# fix (see commun.py::HOTE_DEFAUT), which would have made the
# service listen on the PUBLIC address derived for TURN. This test should have
# gone red before the fix (a check never seen red is not
# a check): it could not, for lack of any assertion on `hote` at
# all — which is precisely what this addition closes.
check("hote is present and non-empty", bool(mesures.get("hote")), True)
check("hote is the fixed default (never the default route)",
      mesures.get("hote"), HOTE_DEFAUT)
check("hote is NEVER the TURN address (the two derivations are separate)",
      mesures.get("hote") == mesures.get("turn_ecoute"), False)
check("proxy_confiance is present and non-empty",
      bool(mesures.get("proxy_confiance")), True)
check("proxy_confiance is the default (reasoned, see commun.py)",
      mesures.get("proxy_confiance"), PROXY_DEFAUT)

# --- The pomerium mode is ACCEPTED since batch 10A (problem C) ----------
# Chosen explicitly by the repository owner for the real
# commissioning. The trusted proxy is DERIVED (commun.py::lire_proxy_confiance),
# never asked: no refusal must bite here any more.
rc, ev = appeler(hw={},
                 answers={**REPONSES, "auth_mode": "pomerium"})
check("pomerium: no refusal (derived proxy, no longer asked)", refus(ev), [])
faits_pomerium = [e for e in ev if e.get("event") == "facts"]
mesures_pomerium = faits_pomerium[0]["facts"] if faits_pomerium else {}
check("pomerium: facts carries proxy_confiance",
      bool(mesures_pomerium.get("proxy_confiance")), True)

# --- But the refusal STILL BITES if the derived address is unusable ----
# 🔴 A CHECK NEVER SEEN RED IS NOT A CHECK: this
# scenario forces `DESK_HOTE` to a universal listen address and checks that
# `resolve` refuses BEFORE the installation rather than letting
# `plateforme/src/config.ts::lireConfig` fail later on an already
# partitioned disk.
env_hote_universelle = dict(os.environ)
env_hote_universelle["DESK_HOTE"] = "0.0.0.0"
rc, ev = appeler(hw={},
                 answers={**REPONSES, "auth_mode": "pomerium"},
                 env=env_hote_universelle)
r = refus(ev)
check("DESK_HOTE=0.0.0.0: refusal", len(r), 1)
check("DESK_HOTE=0.0.0.0: the refusal names the universal listen address",
      bool(r and "universal listen" in r[0].get("reason", "").lower()), True)

# --- An unknown mode value RAISES, it does not fall back -------------
rc, ev = appeler(hw={},
                 answers={**REPONSES, "auth_mode": "motdepass"})
check("unknown mode: refusal", len(refus(ev)), 1)

# --- vb_audio=true: REFUSAL before the installation, never a block AFTER —
# correction round 1 on task 6. No VB-Audio payload is supplied
# by this package (personal licence): `poser_vb_audio(armee=True)` raises
# `NotImplementedError` in `hooks/vm.py`, but AFTER the account and
# the agent enrolment have already been attempted (see hooks/activate.py) —
# an activation that would then NEVER pass again. The refusal must come
# HERE, before a single byte touches the disk.
rc, ev = appeler(hw={}, answers={**REPONSES, "vb_audio": True})
r = refus(ev)
check("vb_audio=true: refusal", len(r), 1)
check("vb_audio=true: exit code 0", rc, 0)
check("vb_audio=true: the sentence names VB-Audio",
      bool(r and "VB-Audio" in r[0].get("reason", "")), True)
check("vb_audio=true: the sentence states the personal licence",
      bool(r and "personal" in r[0].get("reason", "").lower()), True)
check("vb_audio=true: the sentence suggests unticking the option",
      bool(r and "untick" in r[0].get("reason", "").lower()), True)

# --- vb_audio=false (the wizard default): no VB-Audio refusal ----
rc, ev = appeler(hw={}, answers={**REPONSES, "vb_audio": False})
check("vb_audio=false: no refusal", refus(ev), [])

# --- A malformed input is a refusal, never an exception ------------
# Correction round 1 (29 August 2026): the hook let these three inputs
# raise as is (JSONDecodeError or AttributeError, exit code 1,
# full traceback) — the central invariant of this hook broken by a
# trivially reachable path. Each check exercises BOTH things at
# once: exit code 0 AND the presence of a `refuse` carrying a
# sentence — a check that only verified the exit code would pass
# on a hook gone silent.
def appeler_brut(stdin_texte):
    """Like appeler(), but sends stdin_texte AS IS — not re-encoded
    JSON — to exercise inputs json.dumps cannot
    produce (unreadable JSON, a root that is not an object)."""
    r = subprocess.run([sys.executable, str(HOOK)], input=stdin_texte,
                       capture_output=True, text=True)
    evenements = []
    for ligne in r.stdout.splitlines():
        ligne = ligne.strip()
        if not ligne:
            continue
        try:
            evenements.append(json.loads(ligne))
        except json.JSONDecodeError:
            pass
    return r.returncode, evenements


for label, stdin_texte in [
    ("unreadable JSON", "this is not json"),
    ("root that is not an object", "[1,2,3]"),
    ("mistyped hw", json.dumps({"hw": "not a dict", "answers": {}})),
]:
    rc, ev = appeler_brut(stdin_texte)
    r = refus(ev)
    check(f"{label}: exit code 0", rc, 0)
    check(f"{label}: a refusal is emitted", len(r), 1)
    check(f"{label}: the refusal carries a sentence", bool(r and r[0].get("reason")), True)

# --- The GENERIC guard: what NO path anticipated -------------------
# 🔴 MINOR #4 OF TASK 3, RECOMMENDED BEFORE MERGE BY THE RESULTS
# DOCUMENT AND CONFIRMED BY THE FINAL BRANCH REVIEW. The three inputs
# above all fall into the cases that `charger_contexte()` knows how to NAME
# — they therefore do NOT exercise the `try/except Exception` of `main()`, which is
# the central invariant of this hook ("a refusal is data, never an
# exception"). The only witness of its red was a `RecursionError` played by
# hand by a reviewer, in a session that no longer exists.
#
# The TWO inputs below raise INSIDE `json.load` itself, each through
# an exception that is NOT `json.JSONDecodeError` — hence outside every
# named `except` of `charger_contexte()`:
#   - bytes that are not UTF-8: `UnicodeDecodeError`, raised
#     by the stream decoder BEFORE a single JSON character exists;
#   - an integer literal of more than 4,300 digits: `ValueError` raised by
#     CPython's integer conversion (limit `sys.set_int_max_str_digits`),
#     on a JSON that is nevertheless perfectly WELL FORMED.
# Neither is a case this hook anticipated, and that is the point:
# a generic guard only exercised by inputs dictated to it
# would prove nothing of what it exists to cover.
#
# ⚠️ WHAT I TRIED FIRST, AND WHICH NO LONGER BITES: a JSON of excessive
# depth (the reviewer's `RecursionError`). Measured on 30 August 2026 on
# this interpreter: `json.load` swallows without a blink a depth of
# 4 × `sys.getrecursionlimit()`, and the input then falls back into the NAMED case
# "the root is not an object". The test would have been green without ever
# reaching the guard — exactly the "a check never seen
# red" pattern. It is replaced, not patched.
def appeler_octets(data: bytes):
    """Like `appeler_brut`, but sends raw BYTES — needed to
    exercise an input that cannot be decoded as UTF-8, which the
    text mode of `subprocess` cannot express."""
    r = subprocess.run([sys.executable, str(HOOK)], input=data,
                       capture_output=True)
    evenements = []
    for ligne in r.stdout.decode("utf-8", "replace").splitlines():
        ligne = ligne.strip()
        if not ligne:
            continue
        try:
            evenements.append(json.loads(ligne))
        except json.JSONDecodeError:
            pass
    return r.returncode, evenements


for label, octets in [
    ("bytes not decodable as UTF-8", b"\xff\xfe\x00{"),
    ("integer literal of 5,000 digits", b'{"hw":' + b"1" * 5000 + b"}"),
]:
    rc, ev = appeler_octets(octets)
    r = refus(ev)
    check(f"generic guard ({label}): exit code 0", rc, 0)
    check(f"generic guard ({label}): a refusal is emitted", len(r), 1)
    check(f"generic guard ({label}): the refusal names the unexpected",
          bool(r and "unexpected" in r[0].get("reason", "").lower()), True)

# Negative control of THIS check: a malformed input the hook knows how to
# NAME must NOT end up in the generic guard — otherwise the assertion
# above would pass for any input and would prove nothing about the
# unexpected path.
rc_temoin, ev_temoin = appeler_brut("[1,2,3]")
r_temoin = refus(ev_temoin)
check("negative control: a NAMED input does not go through the generic guard",
      bool(r_temoin and "unexpected" not in r_temoin[0].get("reason", "").lower()),
      True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - resolve hook tests passed")
