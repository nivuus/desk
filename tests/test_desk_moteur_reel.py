#!/usr/bin/env python3
"""The resolve → install → activate chain, PLAYED THROUGH THE ENGINE'S API.

🔴 WHY THIS SUITE EXISTS. The final branch review (30 August 2026)
named as the ROOT CAUSE of its only Critical finding the fact that "no
installation was ever played by the real engine". The concrete defect
was `hooks/resolve.py` refusing on `hw["vm_windows"]`, a key that NO
producer of the engine sets: the hook therefore ALWAYS refused, and
`installer/installer/install-engine/steps/packages.py::plan_packages`
turns a refusal into a `StepError`, which stops the WHOLE installation. Nine
reviews and eight test suites did not see it, for ONE single reason:
**all of them built the input context themselves**
(`{"hw": {"vm_windows": True}}`). A test that invents its input cannot
discover that nobody produces it.

`tests/test_desk_contrat_hw.py` set up the STATIC guard (the keys of the
producer, read through `ast`). This suite is the notch above: it does not
read the contract, it RUNS it — `hw` comes from
`installer/installer/common/hardware.py::detect_all()`, the manifest from
`packages/manifest.py::load_manifest`, the answers from
`packages/wizard.py::validate_answers`, and the three hooks are launched by
`packages/runner.py`, as subprocesses, through the real jsonl protocol.

--- 🔴 WHAT THIS SUITE FORBIDS ITSELF, AND WHY ------------------------

The complete engine PARTITIONS AND WIPES DISKS: `install-engine/run.py`
calls `partition.partition_and_format()` (line 82) right after
`plan_packages()` (line 72). This suite NEVER CALLS `run.py`, nor
`plan_packages()`, nor `apply_packages()` — the latter also does
`chroot_run(target, ["apt-get", ...])` and `os.symlink` in
`<target>/etc/systemd/system/multi-user.target.wants/`. It calls the
THREE functions of `packages/runner.py` that run the hooks, and them
alone.

🔴 A SINGLE SUBSTITUTION, AND IT IS NAMED: `run_activate` PASSES NO
`--root` (see `runner.py::run_activate`, which calls `_run_hook(...)`
without the `root` parameter, whose default is `""`), so in production the
`activate` hook works on `/`. On THIS machine, `/etc/systemd/system/
desk-plateforme.service` EXISTS (the real installation of batch 10A): calling
`run_activate` as is would create the `multi-user.target.wants/` link there then
launch `systemctl daemon-reload` and `systemctl start` — on the owner's
systemd, as root. This suite therefore calls `runner._run_hook(...,
root=<temporary root>)`, the function that `run_activate` itself
uses, with the SAME context (`merge_into_hw(hw, facts)`, the real
merge of the engine) and a single extra argument. Step ⑦ below measures
that the host has not moved, and the contract of `run_activate` (no `root`
in its signature) is exercised in step ⑥ rather than assumed.

⚠️ `NIVUUS_PACKAGES_DIR` is deliberately pointed to an EMPTY directory
before the `activate` phase: `hooks/vm.py::chemin_winrm_exec` looks there for
`console/guest/winrm_exec.py`, and a `console` really found would send
a real WinRM exchange to the Windows VM. The expected refusal is therefore
DETERMINISTIC, and no packet leaves this machine.

--- What this suite DOES NOT COVER (named rather than implied) -----

  - PARTITIONING and formatting (`partition_and_format`),
    debootstrap, the bootloader: never called, by construction;
  - `apply_packages()` itself — the chrooted `apt-get`, the first-boot
    activation unit, the writing of `etc/nivuus/packages.json`;
  - the REAL FIRST BOOT: no `systemctl` runs here, so nothing
    establishes that the service starts — that is what batch 10A had established
    by hand, on this machine, and that no test replays;
  - the target HARDWARE: `detect_all()` describes the machine running
    these tests, never the appliance;
  - the end of `activate` (administrator account, enrolment, assignment of
    the VM): it is UNREACHABLE without a Windows VM, and step ⑥ measures
    precisely that we stop at that very gate.

--- WHAT WAS MEASURED, ON 30 AUGUST 2026 (carried HERE, not in a gitignored
    report: "a proof must never live in a gitignored
    report") ------------------------------------------------------------

The RED was played by reinserting the original gate —
`if not hw.get("vm_windows"): refuser(...)` — into a COPY OUTSIDE THE
TRACKED TREE (`/var/tmp/desk-rouge-vm-windows/paquet`, `hooks/` copied
outright, `plateforme/`, `client/`, `proto/` and the two YAML files through links), then
pointing this suite at it through `DESK_PAQUET_RACINE`. Output (recorded
verbatim at the time, when the messages were still in French):

    FAIL (3)
      - resolve accepte cette machine: got False, want True
      - resolve ne donne aucune raison de refus: got 'aucune VM Windows
        detectee sur cette machine', want ''
      - resolve a REFUSE : l'installation entiere s'arreterait ici
        (StepError). Raison rendue par le hook : 'aucune VM Windows
        detectee sur cette machine'

🔴 AND THE PROOF THAT REALLY COUNTS: on THE SAME mutated copy, the suite
`tests/test_desk_resolve.py` **as it was the day before the
fix** (`git show dd263bb~1:tests/test_desk_resolve.py`, the one that
calls `appeler(hw={"vm_windows": True}, ...)` in seven places) returned
`OK - tests du hook resolve passés`, code 0. Yesterday's defect, yesterday's
suite: GREEN. It is the whole pattern of the Critical finding, reproduced and measured.

GREEN on today's product: `make test` returns the eleven suites
green, including `OK - resolve/install/activate chain played through the
engine's API`.

Run: python3 tests/test_desk_moteur_reel.py
"""
import inspect
import json
import os
import pathlib
import shutil
import sys
import tempfile

RACINE_DEPOT = pathlib.Path(__file__).resolve().parents[1]
# Overridable — SAME CONVENTION as `DESK_INSTALLER_RACINE` below, and
# it is through it that the RED of this suite is played: we point to a COPY
# outside the tracked tree, into which the original `hw["vm_windows"]` gate
# was reinserted. See the § "What this suite forbids itself".
PAQUET = pathlib.Path(os.environ.get("DESK_PAQUET_RACINE") or RACINE_DEPOT)
INSTALLER = pathlib.Path(os.environ.get("DESK_INSTALLER_RACINE")
                         or (RACINE_DEPOT.parent / "installer"))
sys.path.insert(0, str(INSTALLER / "installer"))

# ⚠️ IF THE SIBLING REPOSITORY IS ABSENT, THIS SUITE FAILS, it does not skip
# itself: "a fallback `||` turns a missing file into a green check".
from common import hardware                                    # noqa: E402
from packages import runner                                    # noqa: E402
from packages.dependencies import missing_dependencies         # noqa: E402
from packages.discovery import discover                        # noqa: E402
from packages.facts import merge_into_hw, shadowed_facts       # noqa: E402
from packages.manifest import load_manifest                    # noqa: E402
from packages.wizard import load_questions, validate_answers   # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


def check_vrai(label, condition, detail=""):
    if not condition:
        failures.append(f"{label}: false{(' — ' + detail) if detail else ''}")


def terminer(base):
    """Erases the temporary root, prints the verdict, exits."""
    if base is not None and str(base).startswith(tempfile.gettempdir()):
        shutil.rmtree(base, ignore_errors=True)
    if failures:
        print(f"FAIL ({len(failures)})")
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("OK - resolve/install/activate chain played through the engine's API")
    sys.exit(0)


class Collecteur:
    """The progress sink the engine expects from a caller.

    It is the ONLY piece this suite supplies itself, and it carries
    no input data: the real `emit` is the portal's
    (`install-engine/progress.py`), an observer, never a source of
    context. It is kept so that the jsonl trace of the hooks is readable
    when a check goes red.
    """

    def __init__(self):
        self.lignes = []

    def info(self, etape, pct, msg):
        self.lignes.append(("info", etape, pct, msg))

    def warn(self, etape, pct, msg):
        self.lignes.append(("warn", etape, pct, msg))


# --- ⓪ The safety guard, before anything else ------------------------------
# It does not protect from a careless mistake: it protects from the case where
# `tempfile` would be diverted. A target root that is not under the
# temporary directory makes the suite fail BEFORE writing a single byte.

base = pathlib.Path(tempfile.mkdtemp(prefix="desk-moteur-reel-"))
cible = base / "cible"
cible.mkdir()
sans_console = base / "without-console"
sans_console.mkdir()
catalogue = base / "packages"
catalogue.mkdir()

if not str(cible.resolve()).startswith(tempfile.gettempdir()):
    print(f"REFUSED: the target root {cible} is not under "
          f"{tempfile.gettempdir()}; nothing was written.", file=sys.stderr)
    sys.exit(1)
check_vrai("the target root is never /", str(cible.resolve()) != "/")

# --- ① `hw` comes from the ENGINE, never from us ----------------------------
# `install-engine/run.py:68` does `hw = hardware.detect_all()` and passes it
# VERBATIM to `plan_packages` (`:72`) then to `run_resolve`. It is that very call
# we replay — the detection is entirely READ-ONLY (lsblk, ip, lspci,
# /proc), no command writes.

hw = hardware.detect_all()

check_vrai("detect_all() returns a mapping", isinstance(hw, dict))
check("the eight keys of the producer", sorted(hw), sorted([
    "disks", "ethernet", "wifi", "gpus", "cpu", "iommu", "memory_mib",
    "passthrough_candidates"]))
# 🔴 THE CHECK THAT CARRIES THE CRITICAL FINDING: the key on which `resolve.py`
# refused is produced by NOBODY. This is not an opinion about the code,
# it is the real producer, run.
check_vrai("no producer sets vm_windows", "vm_windows" not in hw,
           f"keys returned: {sorted(hw)}")

# --- ② The manifest and the catalogue, through the engine ------------------------
# `discover()` is the function `plan_packages` calls; we give it a
# temporary catalogue (a link to this repository, a link to `console`) rather than
# setting `NIVUUS_PACKAGES_DIR` in the environment — that variable is
# also the one `hooks/vm.py` reads, and a `console` found would send a
# real WinRM exchange (see the head docstring).

os.symlink(PAQUET, catalogue / "desk")
if (INSTALLER / "console" / "nivuus-package.yaml").is_file():
    os.symlink(INSTALLER / "console", catalogue / "console")

manifestes, errors = discover(root=str(catalogue))
check("no manifest refused by discover()", errors, [])
noms = sorted(m.name for m in manifestes)
check_vrai("the engine discovers desk", "desk" in noms, f"discovered: {noms}")
check_vrai("the engine discovers console (hard prerequisite)", "console" in noms,
           f"discovered: {noms}")

manifeste = load_manifest(str(PAQUET / "nivuus-package.yaml"))
check("the manifest played is indeed desk", manifeste.name, "desk")

# The dependency gate of the engine, exercised IN BOTH DIRECTIONS — a check
# never seen red is not a check.
seul = missing_dependencies([manifeste], manifestes)
check_vrai("desk alone lacks console",
           [m.requires for m in seul] == ["console"],
           f"missing: {[m.requires for m in seul]}")
with_ = missing_dependencies(manifestes, manifestes)
check("desk with console lacks nothing", with_, [])

# --- ③ The answers, validated by the REAL wizard --------------------------
# The RAW answers are those of an operator (that is their nature: the
# wizard asks them). What comes from the engine is their VALIDATION and the
# filling of defaults — `vb_audio` is not written here, it is
# `validate_answers` that must set it to `False` from `wizard.yaml`.

questions = load_questions(str(PAQUET / manifeste.questions_file))
answers = validate_answers(questions, {
    "admin_email": "operateur@example.test",
    "admin_password": "an acceptance password, never a real secret",
    "auth_mode": "motdepasse",
})
check("the wizard sets the vb_audio default", answers.get("vb_audio"), False)
check("four validated answers", sorted(answers), sorted(
    ["admin_email", "admin_password", "auth_mode", "vb_audio"]))

# --- ④ resolve, through `run_resolve` ----------------------------------------
# 🔴 IT IS THE CHECK THAT GOES RED ON THE ORIGINAL DEFECT. With the
# `hw["vm_windows"]` gate in place, `resolution.ok` is False and `reason` carries
# the refusal sentence — and on the engine side this refusal becomes a `StepError` that
# stops the whole installation (`steps/packages.py:206-207`).

emetteur = Collecteur()
resolution = runner.run_resolve(manifeste, hw, answers, emetteur)

check("resolve accepts this machine", resolution.ok, True)
check("resolve gives no refusal reason", resolution.reason, "")
if not resolution.ok:
    failures.append(
        "resolve REFUSED: the whole installation would stop here "
        f"(StepError). Reason returned by the hook: {resolution.reason!r}")
    terminer(base)

# `userspace` tier: the engine must return an empty platform block.
check("no kernel module resolved", resolution.platform.modules, ())
check("no kernel command line resolved",
      resolution.platform.kernel_cmdline, ())

# The facts are those the hook emitted, read back by `parse_facts_event`.
check("the six facts of the resolve -> activate channel", sorted(resolution.facts),
      sorted(["node_version", "turn_ecoute", "turn_relais", "hote",
              "proxy_confiance", "port"]))
check_vrai("the hook spoke the progress protocol",
           any(l[3].startswith("[desk]") for l in emetteur.lignes),
           f"lines: {emetteur.lignes}")

# --- ⑤ install, through `run_install`, on a temporary root --------------
# ⚠️ `run_install` DOES NOT RECEIVE THE `facts`: that is what
# `apply_packages` does (`run_install(manifest, hw, answers, target, emit)`), and
# it is reproduced as is. The hook therefore DERIVES what it needs itself.

emetteur_install = Collecteur()
runner.run_install(manifeste, hw, answers, str(cible), emetteur_install)

env_pose = cible / "etc" / "nivuus" / "desk.env"
check_vrai("desk.env is laid down under the target root", env_pose.is_file(),
           str(env_pose))
check("desk.env is only readable by its owner",
      oct(env_pose.stat().st_mode & 0o777), "0o600")

values = {}
for ligne in env_pose.read_text(encoding="utf-8").splitlines():
    if "=" in ligne and not ligne.startswith("#"):
        cle, _, value = ligne.partition("=")
        values[cle.strip()] = value.strip()

check("PLATEFORME_AUTH comes from the validated answer",
      values.get("PLATEFORME_AUTH"), answers["auth_mode"])
check_vrai("PLATEFORME_HOTE is never a universal listen address",
           values.get("PLATEFORME_HOTE") not in
           ("0.0.0.0", "::", "[::]", "*", None),
           f"value: {values.get('PLATEFORME_HOTE')!r}")
check_vrai("the token secret is at least 32 characters long",
           len(values.get("PLATEFORME_SECRET_JETON", "")) >= 32)

unite = cible / "etc" / "systemd" / "system" / "desk-plateforme.service"
check_vrai("the systemd unit is LAID DOWN", unite.is_file(), str(unite))
check_vrai("the __NODE_BIN__ token is substituted",
           "__NODE_BIN__" not in unite.read_text(encoding="utf-8"))
check_vrai("the unit is NOT armed by install",
           not (cible / "etc" / "systemd" / "system" /
                "multi-user.target.wants" / "desk-plateforme.service").exists())
check_vrai("proto/ts is deployed (ERR_MODULE_NOT_FOUND otherwise)",
           (cible / "opt/nivuus/desk/proto/ts/plateforme.ts").is_file())
check_vrai("tsx is deployed (npm start = tsx src/index.ts)",
           (cible / "opt/nivuus/desk/plateforme/node_modules/.bin/tsx").exists())
check_vrai("the node runtime is DROPPED, not assumed",
           (cible / "opt/nivuus/node/bin/node").is_file())
check_vrai("turnserver.conf is laid down",
           (cible / "etc" / "turnserver.conf").is_file())

# --- ⑥ activate: the contract of `run_activate`, then the chain ------------
# 🔴 THE CONTRACT IS EXERCISED, NOT ASSUMED: it is because `run_activate`
# takes no `root` that this suite cannot call it as is on
# this machine (see the head docstring).

signature = inspect.signature(runner.run_activate)
check_vrai("run_activate takes NO root parameter",
           "root" not in signature.parameters,
           f"parameters: {list(signature.parameters)}")
check_vrai("run_activate does receive the facts",
           "facts" in signature.parameters)
check_vrai("run_install receives NO facts",
           "facts" not in inspect.signature(runner.run_install).parameters)

# The merge is the engine's, not ours.
hw_active = merge_into_hw(hw, resolution.facts)
check("no fact is shadowed by the fresh detection",
      shadowed_facts(hw, resolution.facts), [])
check("the port measured by resolve survives the merge",
      hw_active.get("port"), resolution.facts["port"])

# No `console` here: the VM gate refuses without ever touching the network.
os.environ["NIVUUS_PACKAGES_DIR"] = str(sans_console)

before = {
    "host unit": os.lstat("/etc/systemd/system/desk-plateforme.service")
    if os.path.exists("/etc/systemd/system/desk-plateforme.service") else None,
    "host env": os.lstat("/etc/nivuus/desk.env")
    if os.path.exists("/etc/nivuus/desk.env") else None,
}

emetteur_activate = Collecteur()
activate_error = None
try:
    runner._run_hook(manifeste, "activate", hw_active, answers,
                     root=str(cible), emit=emetteur_activate)
except runner.HookError as exc:
    activate_error = str(exc)

# 🔴 WHAT THE CHAIN ESTABLISHES HERE: `activate` ARMS the unit (a link, under the
# temporary root), then STOPS AT THE VM GATE — the gate the
# final review moved from `resolve` to `activate`. A refusal is the
# RIGHT result on a machine without a Windows VM; what counts is WHICH
# assertion refuses, not the exit code alone.
lien = (cible / "etc" / "systemd" / "system" / "multi-user.target.wants"
        / "desk-plateforme.service")
check_vrai("activate ARMED the unit, under the temporary root",
           lien.is_symlink(), str(lien))
check("the armed link is RELATIVE", os.readlink(lien),
      "../desk-plateforme.service")
check_vrai("activate refuses at the Windows VM gate",
           activate_error is not None
           and "the Windows VM does not answer" in activate_error,
           f"error returned: {activate_error!r}")
check_vrai("the refusal names winrm_exec.py, never a network failure",
           activate_error is not None
           and "winrm_exec.py not found" in activate_error,
           f"error returned: {activate_error!r}")

# --- ⑦ THE HOST HAS NOT MOVED ----------------------------------------------
# The check that counts: had this suite called `run_activate` the way
# production does, the `multi-user.target.wants/` link of THIS machine and
# `systemctl start` would have run. We measure the opposite.

apres = {
    "host unit": os.lstat("/etc/systemd/system/desk-plateforme.service")
    if os.path.exists("/etc/systemd/system/desk-plateforme.service") else None,
    "host env": os.lstat("/etc/nivuus/desk.env")
    if os.path.exists("/etc/nivuus/desk.env") else None,
}
for nom in before:
    a, b = before[nom], apres[nom]
    check(f"{nom}: mtime unchanged",
          None if a is None else a.st_mtime_ns,
          None if b is None else b.st_mtime_ns)
terminer(base)
