#!/usr/bin/env python3
"""Tests of the activate hook of the desk package.

The hook is exercised through its REAL interface — a subprocess called
`--phase activate --root <root>`, fed {"hw":…, "answers":…} on
stdin — exactly as `installer/packages/runner.py::run_activate`
invokes it (`hw` carries resolve's facts, MERGED; no `--root`
is passed in production, but `argparse` gives it the same default `/`
as `console/hooks/activate.py`, which makes the hook callable the same
way in a test).

Each test lays down its OWN root under `tempfile.TemporaryDirectory()`:
never `/etc/systemd/system`, `/opt` or the real PATH of the machine running
these tests. `npm` and `systemctl` are replaced by fake scripts
laid down on a `PATH` rebuilt for each call — never the
`npm`/`systemctl` of the real system.

🔴 TASK 6 — since `hooks/vm.py::poser_projfs`/`poser_vb_audio` were wired
into `main()`, THIS HOOK RESOLVES `winrm_exec.py` THROUGH
`NIVUUS_PACKAGES_DIR` ON EVERY CALL, EVEN BEFORE CHECKING
`plateforme/`. `appeler()` therefore builds, BY DEFAULT, a WORKING fake
`console/guest/winrm_exec.py` (never the real one, never a network
call) under a test `NIVUUS_PACKAGES_DIR` — otherwise ALL the
scenarios below would fail on "console absent" before reaching
the reason they really want to exercise. `packages_dir=False` simulates
`console` absent, for the dedicated scenarios that exercise THAT refusal.

Run: python3 tests/test_desk_activate.py
"""
import json
import os
import pathlib
import subprocess
import sys
import tempfile

from desk_activate_fixtures import (
    HOOK,
    HW_WITH_FACTS,
    RACINE,
    REPONSES,
    appeler,
    lire_commandes,
    lire_env,
    load_unit,
    poser_faux_npm,
    poser_faux_systemctl,
    poser_faux_winrm_exec,
    poser_racine_installee,
)

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# --- Does the unit REALLY laid down by task 4 point where this hook thinks?
# ⚠️ Synchronisation guard: if `hooks/assets/desk-plateforme.service`
# changed its target (WantedBy=), `WANTS_SUBDIR` of `activate.py` must follow
# — this test would go red before the link gets laid in the wrong place.
ini_unite = load_unit(RACINE / "hooks" / "assets" / "desk-plateforme.service")
check("the unit is WantedBy=multi-user.target (assumption of activate.py)",
      ini_unite.get("Install", "WantedBy", fallback=""), "multi-user.target")


# === Scenario 1: complete, successful activation ============================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)

    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    log_npm = root / "npm.log"
    poser_faux_npm(bin_dir, log_npm)
    log_systemctl = root / "systemctl.log"
    poser_faux_systemctl(bin_dir, log_systemctl)

    r = appeler(root, bin_dir)
    check("the hook succeeds (rc=0)", r.returncode, 0)
    if r.returncode != 0:
        failures.append(f"hook stderr: {r.stderr!r}")

    # --- Arming: a link, never an enable ---------------------------
    lien = root / "etc" / "systemd" / "system" / "multi-user.target.wants" / "desk-plateforme.service"
    check("the service is armed by a link", os.path.islink(lien), True)
    check("the link points to a unit that exists",
          os.path.exists(os.path.realpath(lien)), True)
    check("the link does point TO the unit laid down by install",
          os.path.realpath(lien),
          os.path.realpath(root / "etc" / "systemd" / "system" / "desk-plateforme.service"))

    # --- systemctl NEVER invoked under a test --root -------------------
    check("no daemon-reload/start under --root != /",
          log_systemctl.exists(), False)

    # --- The password ONLY travels through stdin --------------------------
    commandes_vues = [c["argv"] for c in lire_commandes(log_npm)]
    reponses = REPONSES
    check("no password on the command line",
          any("--password" in c or reponses["admin_password"] in " ".join(c)
              for c in commandes_vues), False)

    # --- It was indeed received, but on stdin -----------------------------
    entrees = lire_commandes(log_npm)
    user_call = next(c for c in entrees if "admin:utilisateur" in c["argv"])
    check("the password arrives on the stdin of admin:utilisateur",
          user_call["stdin"].strip(), reponses["admin_password"])
    check("--email is passed in argv (it is not a secret)",
          "ada@exemple.test" in user_call["argv"], True)

    # --- The order: account BEFORE enrolment ----------------------------------
    ordre = [c["argv"] for c in entrees]
    idx_compte = next(i for i, c in enumerate(ordre) if "admin:utilisateur" in c)
    idx_agent = next(i for i, c in enumerate(ordre) if "admin:agent" in c)
    check("the account is created BEFORE the agent enrolment", idx_compte < idx_agent, True)

    # --- admin:agent: ENROLMENT mode, --vm is a NAME, --adresse set ---
    appel_agent = next(c for c in entrees if "admin:agent" in c["argv"])
    check("--vm is present", "--vm" in appel_agent["argv"], True)
    check("--adresse is present", "--adresse" in appel_agent["argv"], True)
    check("--roter is NOT used (this hook enrols only once)",
          "--roter" in appel_agent["argv"], False)

    # --- TASK 13: the VM just enrolled is ASSIGNED to the created account --
    # Hole found in production on 29 August 2026: without this call,
    # `vm.utilisateur_id` stays NULL and `GET /vm` returns `{"vms":[]}` for
    # the user nevertheless created - see the comment of activate.py.
    appel_attribuer = next((c for c in entrees if "admin:attribuer" in c["argv"]), None)
    check("admin:attribuer is invoked after enrolment (VM assignment)",
          appel_attribuer is not None, True)
    if appel_attribuer is not None:
        check("--email is passed to the assignment (the account just created)",
              "ada@exemple.test" in appel_attribuer["argv"], True)
        check("--vm is passed to the assignment, and it is the vm_id returned by "
              "admin:agent (never the name, never invented)",
              "vm-test-uuid" in appel_attribuer["argv"], True)
        idx_attribuer = ordre.index(appel_attribuer["argv"])
        check("the assignment is launched AFTER enrolment, never before",
              idx_attribuer > idx_agent, True)
        check("no secret in the assignment call (email/vm are not secrets)",
              appel_attribuer["stdin"], "")

    # --- The environment passed to npm carries the config of desk.env --------
    # 🔴 FIX, ROUND 1: the previous version of this check looked at
    # "cwd" under a label that talked about PLATEFORME_BASE_URL — it
    # could not go red, cwd is set UNCONDITIONALLY by lancer_npm(),
    # whether the environment merge is correct or not. The fake npm
    # now logs the value REALLY received by the child process
    # (env_base_url, see FAUX_NPM); we compare it with the one written by
    # poser_racine_installee() into desk.env.
    expected_value = "/var/lib/nivuus-desk/plateforme.sqlite"
    check("PLATEFORME_BASE_URL from desk.env reaches the npm process (admin:utilisateur)",
          user_call.get("env_base_url"), expected_value)
    check("PLATEFORME_BASE_URL from desk.env reaches the npm process (admin:agent)",
          appel_agent.get("env_base_url"), expected_value)

    # --- AGENT_VM / AGENT_SECRET written into desk.env, AFTER the content already
    # there ------------------------------------------------------------------
    env_final = lire_env(root / "etc" / "nivuus" / "desk.env")
    check("AGENT_VM is the vm_id returned by admin:agent (never the name passed)",
          env_final.get("AGENT_VM"), "vm-test-uuid")
    check("AGENT_SECRET is the secret returned by admin:agent",
          env_final.get("AGENT_SECRET"), "secret-de-test-0123456789abcdef")
    check("the content already there (laid down by install) is still present",
          env_final.get("PLATEFORME_SECRET_JETON"), "x" * 64)
    check("desk.env stays in mode 600 (it carries secrets)",
          oct(os.stat(root / "etc" / "nivuus" / "desk.env").st_mode & 0o777), oct(0o600))

    # --- Idempotence: a second call replays NEITHER the account NOR
    # the enrolment (AGENT_VM/AGENT_SECRET already present) ---------------------
    # 🔴 vm.nom has NO uniqueness constraint: without the idempotence guard,
    # this second call would create one more ORPHAN VM instead of refusing.
    # It is the check that makes it visible: zero new npm command.
    log_npm.unlink()
    r2 = appeler(root, bin_dir)
    check("a second pass also succeeds (idempotent)", r2.returncode, 0)
    check("the link stays a link after a second pass", os.path.islink(lien), True)
    check("the second pass relaunches NO npm command (already activated)",
          log_npm.exists(), False)
    env_apres_second_passage = lire_env(root / "etc" / "nivuus" / "desk.env")
    check("AGENT_VM is unchanged after the second pass",
          env_apres_second_passage.get("AGENT_VM"), "vm-test-uuid")
    check("AGENT_SECRET is unchanged after the second pass",
          env_apres_second_passage.get("AGENT_SECRET"), "secret-de-test-0123456789abcdef")


# === Scenario 2: desk.env does not exist yet (install has not been
# completed) — clean refusal, never a Python traceback =========================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    unite_dir = root / "etc" / "systemd" / "system"
    unite_dir.mkdir(parents=True)
    (unite_dir / "desk-plateforme.service").write_text(
        (RACINE / "hooks" / "assets" / "desk-plateforme.service").read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    # No plateforme/, no desk.env: install never ran on
    # this root.
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir)
    check("root not installed: the hook refuses cleanly (rc != 0)",
          r.returncode != 0, True)
    check("the refusal names the missing plateforme directory",
          "plateforme" in (r.stderr or ""), True)
    check("no Python traceback reaches the operator",
          "Traceback" in (r.stderr or ""), False)
    # Arming, for its part, must still have happened: it precedes the account
    # creation in the hook's order.
    lien = unite_dir / "multi-user.target.wants" / "desk-plateforme.service"
    check("arming still happened before the refusal", os.path.islink(lien), True)


# === Scenario 2bis (FIX, ROUND 1): plateforme/ laid down, desk.env
# ABSENT — the precise case the review named: partial install, or an
# operator who deleted the file. The hook must REFUSE rather than
# recreate desk.env silently — it is this refusal that prevents
# add_env_variables() from writing a NEW file (write-then-chmod
# window), since it never reaches it in this case. ========================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    unite_dir = root / "etc" / "systemd" / "system"
    unite_dir.mkdir(parents=True)
    (unite_dir / "desk-plateforme.service").write_text(
        (RACINE / "hooks" / "assets" / "desk-plateforme.service").read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    # plateforme/ IS laid down (install partially played) ...
    (root / "opt" / "nivuus" / "desk" / "plateforme").mkdir(parents=True)
    # ... but etc/nivuus/desk.env does NOT exist.
    env_absent = root / "etc" / "nivuus" / "desk.env"
    check("scenario precondition: desk.env does not exist", env_absent.exists(), False)

    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir)
    check("desk.env absent (plateforme present): clean refusal (rc != 0)",
          r.returncode != 0, True)
    check("the refusal names desk.env, not only plateforme",
          "desk.env" in (r.stderr or ""), True)
    check("no Python traceback", "Traceback" in (r.stderr or ""), False)
    check("desk.env was NOT created in its place (no silent write)",
          env_absent.exists(), False)
    check("no npm command was launched (the refusal precedes every call)",
          (root / "npm.log").exists(), False)
    # Arming, for its part, precedes this refusal: it must still have happened.
    lien = unite_dir / "multi-user.target.wants" / "desk-plateforme.service"
    check("arming still happened before this refusal too",
          os.path.islink(lien), True)


# === Scenario 3: missing answers — clean refusal =======================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir, answers={"auth_mode": "motdepasse"})
    check("without admin_email/admin_password: clean refusal (rc != 0)",
          r.returncode != 0, True)
    check("no Python traceback", "Traceback" in (r.stderr or ""), False)


# === Scenario 4: malformed stdin input — clean refusal, never a traceback ==
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    env = dict(os.environ)
    env["PATH"] = str(bin_dir) + os.pathsep + env.get("PATH", "")
    r = subprocess.run([sys.executable, str(HOOK), "--phase", "activate",
                         "--root", str(root)],
                        input="this is not JSON", env=env,
                        capture_output=True, text=True)
    check("unreadable stdin: clean refusal (rc != 0)", r.returncode != 0, True)
    check("no Python traceback on a malformed input",
          "Traceback" in (r.stderr or ""), False)


# === The RED — Step 5: a link to a missing unit must RAISE =========
# Root WITHOUT the systemd unit (install did not go all the way, or a
# corrupted disk): armer_unite() must raise rather than lay a dead link.

# --- Arm 1: the function itself, called directly -------------------
import importlib.util  # noqa: E402

# 🔴 TASK 6: `activate.py` now does `from vm import ...`, and `vm.py`
# lives next to it in `hooks/` — exactly like `commun.py`. An import
# by PATH (`spec_from_file_location`) does NOT go through the mechanism that
# automatically adds the script's directory to `sys.path` (that is
# done by the interpreter for a LAUNCHED script, not for a module loaded
# this way): without this line, `exec_module` raises `ModuleNotFoundError: vm`.
sys.path.insert(0, str(HOOK.parent))

spec = importlib.util.spec_from_file_location("desk_activate", HOOK)
activate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(activate)

with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    (root / "etc" / "systemd" / "system").mkdir(parents=True)
    # The unit is NOT laid down: it is the case the task must turn red.
    a_leve = False
    try:
        activate.armer_unite(root, "desk-plateforme.service")
    except FileNotFoundError:
        a_leve = True
    check("armer_unite() raises on a missing unit (never a dead link)",
          a_leve, True)
    lien_mort = (root / "etc" / "systemd" / "system" / "multi-user.target.wants"
                 / "desk-plateforme.service")
    check("no dead link was created", os.path.lexists(lien_mort), False)

# --- Arm 2: the complete hook, as a subprocess, same scenario ------------
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    (root / "etc" / "systemd" / "system").mkdir(parents=True)
    (root / "opt" / "nivuus" / "desk" / "plateforme").mkdir(parents=True)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir)
    check("RED: the complete hook fails (rc != 0) on a missing unit",
          r.returncode != 0, True)
    check("RED: the refusal names the missing unit",
          "desk-plateforme.service" in (r.stderr or ""), True)
    lien_mort = (root / "etc" / "systemd" / "system" / "multi-user.target.wants"
                 / "desk-plateforme.service")
    check("RED: no dead link is left behind", os.path.lexists(lien_mort), False)
    # Proof that this failure path precedes EVERYTHING else: no npm
    # command was ever launched.
    check("RED: no npm command was launched",
          (root / "npm.log").exists(), False)


# === Task 6: the wiring of hooks/vm.py into main() — B exercises
# poser_projfs (through the absence of the inter-package contract), C exercises
# poser_vb_audio (through arming it without a payload). Their OWN behaviour
# (redemarrage_requis, the fake executor, the winrm_exec.py contract) is
# already exercised in detail by tests/test_desk_vm.py; here, only the WIRING
# into main() counts. ========================================================

def _root_for_task6(tmp):
    """Installed root + fake npm, shared by the two scenarios below."""
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")
    return root, bin_dir


def _check_clean_refusal(prefix, r, doit_contenir, root):
    """A clean refusal: non-zero exit code, the reason named in
    stderr, no Python traceback, and no npm command launched — the refusal
    must precede any attempt at account creation or enrolment."""
    check(f"{prefix}: clean refusal (rc != 0)", r.returncode != 0, True)
    check(f"{prefix}: the refusal names {doit_contenir!r}",
          doit_contenir in (r.stderr or ""), True)
    check(f"{prefix}: no Python traceback", "Traceback" in (r.stderr or ""), False)
    check(f"{prefix}: no npm launched", (root / "npm.log").exists(), False)


# --- B: `console` absent (no winrm_exec.py under NIVUUS_PACKAGES_DIR) —
# clean refusal of the COMPLETE hook, not only of the isolated hooks/vm.py module --
with tempfile.TemporaryDirectory() as tmp:
    root, bin_dir = _root_for_task6(tmp)
    r = appeler(root, bin_dir, packages_dir=False)
    chemin_attendu = str(root / "console-absent-here" / "console" / "guest" / "winrm_exec.py")
    _check_clean_refusal("console absent", r, chemin_attendu, root)
    # Arming the service precedes the WinRM resolution in main():
    # it must still have happened before this refusal.
    lien = (root / "etc" / "systemd" / "system" / "multi-user.target.wants"
            / "desk-plateforme.service")
    check("console absent: arming still happened", os.path.islink(lien), True)


# --- B-bis: THE WINDOWS VM GATE, MOVED FROM `resolve` -----------
# 🔴 FIX OF THE CRITICAL FINDING OF THE FINAL BRANCH REVIEW (30 August 2026).
# `hooks/resolve.py` carried `if not hw.get("vm_windows"): refuser(...)`:
# a key that NO producer of the engine sets, tested at a phase where the
# VM cannot exist yet. The gate now lives HERE, and it is a
# MEASUREMENT — a real WinRM exchange. This scenario is its RED: `console` IS
# installed (the inter-package contract resolves), but the guest does not
# answer. It differs from scenario B above, where it is `console` that
# is missing: two different causes, two different messages.
with tempfile.TemporaryDirectory() as tmp:
    root, bin_dir = _root_for_task6(tmp)
    packages_dir = root / "fake-packages-dir"
    guest = packages_dir / "console" / "guest"
    guest.mkdir(parents=True)
    (guest / "fetch_payload.py").write_text("", encoding="utf-8")
    (guest / "winrm_exec.py").write_text(
        "#!/usr/bin/env python3\n"
        "import sys\n"
        # The text SAYS it is fake: see the same precaution in
        # tests/test_desk_vm.py, where a message imitating a real network
        # failure had misled a reviewer.
        "sys.stderr.write('FAKE test winrm_exec.py: simulated failure, "
        "no VM contacted\\n')\n"
        "sys.exit(1)\n", encoding="utf-8")
    (guest / "winrm_exec.py").chmod(0o755)

    r = appeler(root, bin_dir, packages_dir=packages_dir)
    _check_clean_refusal("VM unreachable", r, "simulated failure", root)
    check("VM unreachable: the refusal names the Windows VM, not ProjFS",
          "the Windows VM does not answer" in (r.stderr or ""), True)
    check("VM unreachable: the refusal says console provisions it",
          "console provisions it" in (r.stderr or ""), True)
    check("VM unreachable: the refusal says it is NOT resolve's role",
          "never in resolve" in (r.stderr or ""), True)


# --- C: VB-Audio ARMED (vb_audio=true), no payload — clean refusal of the
# complete hook. ⚠️ Since correction round 1, `hooks/resolve.py`
# already refuses this case EARLIER, before the installation (see
# tests/test_desk_resolve.py): this scenario stays useful as DEFENCE IN
# DEPTH — it exercises that `activate.py`, called alone (as a direct
# replay by the engine would do, without going through resolve again), never claims
# to have installed VB-Audio when no payload exists. ---------------------
with tempfile.TemporaryDirectory() as tmp:
    root, bin_dir = _root_for_task6(tmp)
    packages_dir, log_winrm = root / "fake-packages-dir", root / "winrm.log"
    poser_faux_winrm_exec(packages_dir, log_winrm)  # ProjFS succeeds (empty output)
    r = appeler(root, bin_dir, answers=dict(REPONSES, vb_audio=True),
                packages_dir=packages_dir)
    _check_clean_refusal("vb_audio armed without payload", r, "VB-Audio", root)
    # ProjFS comes first: vb_audio raises before even reaching the fake
    # winrm_exec.py, so a single command (ProjFS's) was seen.
    check("ProjFS set up before the vb_audio refusal (raises before winrm_exec.py)",
          len(lire_commandes(log_winrm)), 1)


# === Scenario 5 (TASK 13): the assignment is REFUSED (VM already assigned
# to another account) - the hook must fail cleanly, name the cause, and
# NOT write AGENT_VM/AGENT_SECRET as if nothing happened. ============
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    os.environ["FAUX_NPM_ATTRIBUER_ECHEC"] = "1"
    try:
        r = appeler(root, bin_dir)
    finally:
        del os.environ["FAUX_NPM_ATTRIBUER_ECHEC"]

    check("assignment refused: the hook fails cleanly (rc != 0)",
          r.returncode != 0, True)
    check("no Python traceback", "Traceback" in (r.stderr or ""), False)
    check("the refusal names the assignment",
          "assignment" in (r.stderr or ""), True)
    entrees = lire_commandes(root / "npm.log")
    check("account AND enrolment did run before the assignment refusal",
          any("admin:agent" in c["argv"] for c in entrees), True)


if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - activate hook tests passed")
