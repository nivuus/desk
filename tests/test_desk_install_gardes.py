#!/usr/bin/env python3
"""The GUARDS of the install hook: what it REFUSES, and what it does not write.

Split from `tests/test_desk_install.py` on 30 August 2026, in a DEDICATED commit
and BEFORE the final branch review added its scenarios (the pre-flight
that refuses before any secret, the Node runtime not found, the malformed
`facts`): the scenarios file was at 392 lines out of 500 and the
additions took it to 523. EXTRACT, NEVER COMPRESS.

🔴 THE SELECTION RULE, STATED RATHER THAN SUFFERED: this file carries the
scenarios whose expected outcome is a REFUSAL (non-zero exit code, a
sentence on stderr, no Python traceback); `test_desk_install.py` carries those
whose expected outcome is an installation that SUCCEEDS. A new refusal
scenario goes here, a success scenario goes there.

The fixtures are shared: `tests/desk_install_fixtures.py`.

Run: python3 tests/test_desk_install_gardes.py
"""
import os
import pathlib
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RACINE / "hooks"))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from desk_install_fixtures import (  # noqa: E402
    FACTS,
    appeler,
    poser_source_minimale,
)

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# --- Installation 4: node_modules absent on the SOURCE side -> refusal -------------
# 🔴 PROBLEM B OF BATCH 10A: `npm start` requires `node_modules/.bin/tsx`, and
# nothing guaranteed it before this batch — a freshly cloned repository (or
# one packaged by a pipeline that never ran `npm install`) would have seen
# `install` "succeed" (code 0) while laying down a service structurally
# unable to start. This scenario builds a fake SOURCE root
# (DESK_SOURCE_RACINE) carrying a `plateforme/` WITHOUT node_modules, and
# checks that `install` REFUSES rather than letting it through.
with tempfile.TemporaryDirectory() as tmp4src, tempfile.TemporaryDirectory() as tmp4dst:
    source4 = pathlib.Path(tmp4src)
    poser_source_minimale(source4)
    # Deliberately NO node_modules under source4/plateforme.

    root4 = pathlib.Path(tmp4dst)
    env_source = dict(os.environ)
    env_source["DESK_SOURCE_RACINE"] = str(source4)
    r4 = appeler(root4, facts=FACTS, env=env_source)
    check("installation 4 (node_modules absent): NON-ZERO exit code",
          r4.returncode != 0, True)
    check("installation 4: the refusal names node_modules/.bin/tsx",
          "node_modules" in (r4.stderr or "") and "tsx" in (r4.stderr or ""),
          True)

    # --- Negative control: the SAME source root, WITH node_modules/.bin/tsx,
    # must succeed --------------------------------------------------------
    bin_dir4 = source4 / "plateforme" / "node_modules" / ".bin"
    bin_dir4.mkdir(parents=True)
    (bin_dir4 / "tsx").write_text("#!/usr/bin/env node\n", encoding="utf-8")
    (bin_dir4 / "tsx").chmod(0o755)
    root4b = pathlib.Path(tempfile.mkdtemp())
    try:
        r4b = appeler(root4b, facts=FACTS, env=env_source)
        check("negative control: node_modules/.bin/tsx present -> code 0",
              r4b.returncode, 0)
    finally:
        import shutil as _shutil
        _shutil.rmtree(root4b, ignore_errors=True)

# --- Installation 6: a universal facts["hote"] MUST BE REFUSED -------------
# 🔴 CORRECTION ROUND 1 (29 August 2026): the review showed that
# `facts.get("hote")`, when truthy, short-circuited `lire_hote()` —
# the ONLY function that checked `ECOUTES_UNIVERSELLES` — and therefore went
# through WITHOUT ANY CHECK. `facts = {..., "hote": "0.0.0.0", ...}` returned
# code 0 and wrote PLATEFORME_HOTE=0.0.0.0 into desk.env: exactly the
# defect this batch exists to close, back through a second path.
# Unreachable by the real engine TODAY (it passes no `facts` to
# `install`), but the head docstring of install.py itself says this
# channel exists for "a future engine, or this test file" — and the
# `facts` contract gained keys DURING this very batch. This scenario freezes
# the contract: a universal listen address in facts["hote"] MUST refuse,
# exactly as DESK_HOTE=0.0.0.0 already does for the derived value.
with tempfile.TemporaryDirectory() as tmp6:
    root6 = pathlib.Path(tmp6)
    facts_hote_universel = dict(FACTS)
    facts_hote_universel["hote"] = "0.0.0.0"
    r6 = appeler(root6, facts=facts_hote_universel)
    check("facts['hote']='0.0.0.0': NON-ZERO exit code (refusal)",
          r6.returncode != 0, True)
    check("facts['hote']='0.0.0.0': the refusal names the universal listen address",
          "universal listen" in (r6.stderr or "").lower(), True)
    check("facts['hote']='0.0.0.0': the refusal names facts[\"hote\"] (not DESK_HOTE)",
          'facts["hote"]' in (r6.stderr or ""), True)
    check("facts['hote']='0.0.0.0': desk.env is NOT written with this value",
          (root6 / "etc" / "nivuus" / "desk.env").is_file(), False)


# --- Installation 8: THE PRE-FLIGHT REFUSES BEFORE THE FIRST SECRET -----------
# 🔴 IMPORTANT FINDING OF THE FINAL BRANCH REVIEW, DEMONSTRATED BY RUNNING IT.
# `client/dist/` is gitignored: a freshly cloned repository carries none.
# `install.py` copied `client/dist` BEFORE reaching its `tsx` guard, so
# that it returned a PYTHON TRACEBACK — and returned it AFTER having already
# written `desk.env` WITH ITS TWO SECRETS. This scenario freezes the three
# properties of the remedy: a sentence, not a traceback; the WHOLE list of what
# is missing, not the first missing item; and `desk.env` NEVER created.
with tempfile.TemporaryDirectory() as tmp8src, tempfile.TemporaryDirectory() as tmp8:
    source8 = pathlib.Path(tmp8src)
    poser_source_minimale(source8)
    # `poser_source_minimale` lays down client/dist and proto/ts; we remove
    # client/dist to reproduce the fresh repository, and node_modules/.bin/tsx
    # was never laid down.
    import shutil as _sh
    _sh.rmtree(source8 / "client" / "dist")

    root8 = pathlib.Path(tmp8)
    env8 = dict(os.environ)
    env8["DESK_SOURCE_RACINE"] = str(source8)
    r8 = appeler(root8, facts=FACTS, env=env8)
    check("installation 8 (fresh repository): NON-ZERO exit code",
          r8.returncode != 0, True)
    check("installation 8: no Python traceback",
          "Traceback" in (r8.stderr or ""), False)
    check("installation 8: the refusal names client/dist",
          "client/dist" in (r8.stderr or ""), True)
    check("installation 8: the refusal ALSO names node_modules/.bin/tsx "
          "(the whole list, never the first missing item)",
          "node_modules/.bin/tsx" in (r8.stderr or ""), True)
    check("installation 8: the refusal suggests running npm run build",
          "npm run build" in (r8.stderr or ""), True)
    # 🔴 THE CHECK THAT COUNTS: no secret has been drawn.
    check("installation 8: desk.env is NOT created (no orphan secret)",
          (root8 / "etc" / "nivuus" / "desk.env").exists(), False)
    check("installation 8: nor is turnserver.conf",
          (root8 / "etc" / "turnserver.conf").exists(), False)

# --- Installation 9: THE NODE RUNTIME NOT FOUND IS A NAMED REFUSAL ------
# The counterpart of scenario 7: when the Node prefix does not carry what
# it should, the hook SAYS so (as it already does for `tsx` and for `proto/ts`),
# it does not lay down a silent service. The guard covered two of the three
# start conditions; this is the third.
with tempfile.TemporaryDirectory() as tmp9:
    root9 = pathlib.Path(tmp9)
    prefixe_vide = root9 / "node-incomplete"
    (prefixe_vide / "bin").mkdir(parents=True)
    r9 = appeler(root9, facts=FACTS, node_source=str(prefixe_vide))
    check("installation 9 (incomplete Node runtime): NON-ZERO exit code",
          r9.returncode != 0, True)
    check("installation 9: no Python traceback",
          "Traceback" in (r9.stderr or ""), False)
    check("installation 9: the refusal names the incomplete runtime",
          "incomplete" in (r9.stderr or ""), True)
    check("installation 9: the refusal names bin/node",
          "bin/node" in (r9.stderr or ""), True)
    check("installation 9: desk.env is NOT created",
          (root9 / "etc" / "nivuus" / "desk.env").exists(), False)

# --- Installation 10: the facts ARE NO LONGER TAKEN AT THEIR WORD -------------
# 🔴 MINOR #7 OF TASK 4, OVERTURNED BY THE FINAL BRANCH REVIEW: its
# reason ("`facts` comes from us, we do not defend against it") was REFUTED
# in this very branch, on `facts["hote"]` — see installation 6. The
# neighbouring keys nevertheless kept the same written reason.
for etiquette, cle, value, attendu in [
    ("universal turn_ecoute", "turn_ecoute", "0.0.0.0", "universal listen"),
    ("universal turn_relais", "turn_relais", "::", "universal listen"),
    ("universal proxy_confiance", "proxy_confiance", "*", "universal listen"),
    ("non-integer port", "port", "three-thousand", "is not an integer"),
    ("port out of range", "port", 70000, "outside the range"),
    ("boolean port", "port", True, "is a boolean"),
]:
    with tempfile.TemporaryDirectory() as tmp10:
        root10 = pathlib.Path(tmp10)
        facts10 = dict(FACTS)
        facts10[cle] = value
        r10 = appeler(root10, facts=facts10)
        check(f"facts {etiquette}: NON-ZERO exit code", r10.returncode != 0, True)
        check(f"facts {etiquette}: no Python traceback",
              "Traceback" in (r10.stderr or ""), False)
        check(f"facts {etiquette}: the refusal names the cause",
              attendu in (r10.stderr or "").lower(), True)
        check(f"facts {etiquette}: desk.env is NOT written",
              (root10 / "etc" / "nivuus" / "desk.env").exists(), False)

# Negative control of installation 10: the SAME keys, with sound values,
# pass — otherwise the six refusals above could come from a
# hook that refuses everything.
with tempfile.TemporaryDirectory() as tmp10b:
    root10b = pathlib.Path(tmp10b)
    r10b = appeler(root10b, facts=FACTS)
    check("negative control: sound facts still pass", r10b.returncode, 0)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - install hook guards passed")
