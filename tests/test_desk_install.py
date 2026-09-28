#!/usr/bin/env python3
"""Tests of the install hook of the desk package.

The hook is exercised through its REAL interface — a subprocess called
`--phase install --root <root>`, fed {"hw":…, "answers":…,
"facts":…} on stdin — exactly as `installer/packages/runner.py`
invokes it (`cmd = [sys.executable, hook, "--phase", phase]` then
`cmd += ["--root", root]` if a root is supplied), and exactly as
`console/hooks/install.py` is already tested in the sibling repository
(`installer/console/tests/test_console_install.py`): that precedent is the
source of the `--root` convention, preferred to an environment variable or
a context key because it is what the REAL engine sends.

Each test lays down its OWN root under `tempfile.TemporaryDirectory()`:
never `/opt`, `/etc` or `/etc/systemd/system` of the machine running these
tests.

Run: python3 tests/test_desk_install.py
"""
# ⚠️ `configparser`, `json`, `subprocess` and `HOOK` followed the fixtures
# into `desk_install_fixtures.py`: an extraction is never strictly
# verbatim, it leaves its imports behind — and an unused import is
# a failure family, not a detail.
import os
import pathlib
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]

sys.path.insert(0, str(RACINE / "hooks"))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from commun import HOTE_DEFAUT, NODE_BIN_DEFAUT, PROXY_DEFAUT  # noqa: E402
from installed_files import lire_secret_persiste  # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


from desk_install_fixtures import (  # noqa: E402
    FACTS,
    appeler,
    lire_env,
    load_unit,
    poser_source_minimale,
)

# --- Installation 1: WITH facts (the case where the engine would supply them) -----
with tempfile.TemporaryDirectory() as tmp1:
    root1 = pathlib.Path(tmp1)
    r1 = appeler(root1, facts=FACTS)
    check("installation 1: exit code 0", r1.returncode, 0)

    env1, chemin_env1 = lire_env(root1)

    # 🔴 The token secret is DRAWN AT RANDOM, never asked nor constant.
    check("the secret is at least 32 characters long",
          len(env1.get("PLATEFORME_SECRET_JETON", "")) >= 32, True)

    check("the environment file is in 600",
          oct(os.stat(chemin_env1).st_mode & 0o777), oct(0o600))

    # PLATEFORME_PAGE is what makes nginx optional: the platform serves
    # the built page itself since 22 August 2026.
    check("the page is served from client/dist",
          env1.get("PLATEFORME_PAGE", "").endswith("client/dist"), True)

    # 🔴 PLATEFORME_HOTE: in pomerium mode, the four universal listen addresses
    # make the start REFUSE. The installation must never set one.
    check("the listen address is never universal",
          env1.get("PLATEFORME_HOTE") in ("0.0.0.0", "::", "[::]", "*"), False)
    check("the listen address is non-empty", bool(env1.get("PLATEFORME_HOTE")), True)

    # With facts supplied: the port and the TURN address come from THEM, not from the
    # default nor from an independent derivation.
    check("the port comes from the facts when they are supplied",
          env1.get("PLATEFORME_PORT"), str(FACTS["port"]))

    # 🔴 REGRESSION CLOSED IN BATCH 10A (29 August 2026): PLATEFORME_HOTE MUST
    # come from facts["hote"], NEVER from facts["turn_ecoute"] — the two
    # were conflated before this fix (see hooks/commun.py::
    # HOTE_DEFAUT for the real bug the separation fixes: on the
    # development machine, the TURN derivation returns the PUBLIC address
    # of the default route, and PLATEFORME_HOTE inherited it). FACTS sets
    # two DIFFERENT values for "hote" and "turn_ecoute" precisely so
    # that this test cannot stay green by accident if the confusion
    # came back.
    check("PLATEFORME_HOTE comes from facts['hote'], never from turn_ecoute",
          env1.get("PLATEFORME_HOTE"), FACTS["hote"])
    check("PLATEFORME_HOTE is NOT the TURN address (the two are decoupled)",
          env1.get("PLATEFORME_HOTE") == FACTS["turn_ecoute"], False)
    check("PLATEFORME_PROXY_DE_CONFIANCE comes from facts['proxy_confiance']",
          env1.get("PLATEFORME_PROXY_DE_CONFIANCE"), FACTS["proxy_confiance"])

    # PLATEFORME_AUTH comes from the auth_mode answer, as is.
    check("PLATEFORME_AUTH comes from the auth_mode answer",
          env1.get("PLATEFORME_AUTH"), "motdepasse")

    # 🔴 Correction round 1, finding ①: DynamicUser=yes makes
    # /opt/nivuus/desk/plateforme READ ONLY (implied ProtectSystem=strict)
    # — the relative default of PLATEFORME_ICONES/
    # PLATEFORME_TELEVERSEMENTS ("donnees/icones"/"donnees/televersements",
    # under WorkingDirectory) would fail there with EROFS on first use. Both
    # MUST point under /var/lib/nivuus-desk, the only directory that
    # StateDirectory= makes writable.
    check("PLATEFORME_ICONES points under the writable state directory",
          env1.get("PLATEFORME_ICONES"), "/var/lib/nivuus-desk/icones")
    check("PLATEFORME_TELEVERSEMENTS points under the writable state directory",
          env1.get("PLATEFORME_TELEVERSEMENTS"),
          "/var/lib/nivuus-desk/televersements")
    check("the two directories are NOT the relative default under WorkingDirectory",
          any(v.startswith("donnees/")
              for v in (env1.get("PLATEFORME_ICONES", ""),
                        env1.get("PLATEFORME_TELEVERSEMENTS", ""))),
          False)

    # The coturn configuration (TURN_URL/TURN_SECRET) that `plateforme/src/
    # signaling/ice.ts::configurationIce` requires BOTH OF to announce
    # a relay.
    check("TURN_URL carries the listen address of the facts",
          env1.get("TURN_URL"), f"turn:{FACTS['turn_ecoute']}:3478")
    check("TURN_SECRET is set and non-empty",
          bool(env1.get("TURN_SECRET")), True)

    # --- What lives under /opt/nivuus/desk/ --------------------------------
    plateforme_dep = root1 / "opt" / "nivuus" / "desk" / "plateforme"
    check("plateforme/package.json is copied",
          (plateforme_dep / "package.json").is_file(), True)
    check("plateforme/src is copied",
          (plateforme_dep / "src").is_dir(), True)
    check("the DEV data is NOT copied (icons/uploads)",
          (plateforme_dep / "donnees").exists(), False)

    client_dep = root1 / "opt" / "nivuus" / "desk" / "client" / "dist"
    check("client/dist/index.html is copied",
          (client_dep / "index.html").is_file(), True)

    # 🔴 REAL FINDING OF BATCH 10A (29 August 2026): `proto/ts/` was
    # NOT copied at all, and `npm start` failed on the first module that
    # imports it (`ERR_MODULE_NOT_FOUND` on `../../../proto/ts/plateforme`,
    # measured on the real service). `proto/ts/` must be a SIBLING of
    # `plateforme/` under the deployed root, exactly as in this
    # development repository — never under `plateforme/`.
    proto_dep = root1 / "opt" / "nivuus" / "desk" / "proto" / "ts"
    check("proto/ts/plateforme.ts is copied (the service depends on it at run time)",
          (proto_dep / "plateforme.ts").is_file(), True)
    check("proto/ts/*.test.ts is NOT copied (never run by the service)",
          list(proto_dep.glob("*.test.ts")), [])

    # 🔴 REAL BUG FOUND IN BATCH 10A (29 August 2026), BY STARTING THE REAL
    # SERVICE: under the 027 umask of the machine's root, /opt/nivuus/desk and
    # /opt/nivuus/desk/plateforme were born as drwxr-x---, inaccessible to
    # the EPHEMERAL UID that `DynamicUser=yes` creates at each start — CHDIR
    # failed before a single line of JavaScript (see
    # hooks/install.py::make_world_readable). This test checks that EVERY
    # directory under opt/nivuus/desk (the PARENT included) is traversable
    # by "other" — the precise bit DynamicUser requires, distinct from the
    # READ-ONLY mode that ProtectSystem=strict imposes elsewhere.
    desk_dep = root1 / "opt" / "nivuus" / "desk"
    repertoires_non_traversables = [
        d for d, _dn, _fn in os.walk(desk_dep)
        if (os.stat(d).st_mode & 0o005) != 0o005
    ]
    check("every directory under opt/nivuus/desk is o+rx (DynamicUser)",
          repertoires_non_traversables, [])

    # --- The systemd unit ----------------------------------------------------
    unite = root1 / "etc" / "systemd" / "system" / "desk-plateforme.service"
    check("the unit is dropped", unite.is_file(), True)
    ini = load_unit(unite)
    # 🔴 PROBLEM A OF BATCH 10A: the shipped unit carried "/usr/bin/npm start",
    # a path that exists on NO Debian without the nodejs package (checked on
    # 29 August 2026 on this machine). The expected default is now
    # NODE_BIN_DEFAUT/npm — see commun.py::lire_node_bin.
    check("the unit launches npm from NODE_BIN_DEFAUT (never /usr/bin/npm)",
          ini.get("Service", "ExecStart", fallback=""),
          f"{NODE_BIN_DEFAUT}/npm start")
    check("the unit sets a PATH that contains NODE_BIN_DEFAUT (npm is a "
          "#!/usr/bin/env node script, it must find node)",
          NODE_BIN_DEFAUT in ini.get("Service", "Environment", fallback=""),
          True)
    check("the unit points to /opt/nivuus/desk/plateforme",
          ini.get("Service", "WorkingDirectory", fallback=""),
          "/opt/nivuus/desk/plateforme")
    check("the unit reads /etc/nivuus/desk.env",
          ini.get("Service", "EnvironmentFile", fallback=""),
          "/etc/nivuus/desk.env")
    check("the unit restarts on failure",
          ini.get("Service", "Restart", fallback=""), "on-failure")
    check("the unit arms a dedicated user (DynamicUser)",
          ini.get("Service", "DynamicUser", fallback=""), "yes")
    # ⚠️ This field is NOT armed (`systemctl enable`) here: that is the job
    # of task 5 (`activate`), through a LINK — never a `systemctl enable`
    # that fails silently. install only LAYS DOWN the unit.

    # --- The coturn configuration --------------------------------------------
    turnconf = root1 / "etc" / "turnserver.conf"
    check("turnserver.conf is dropped", turnconf.is_file(), True)
    contenu_turn = turnconf.read_text(encoding="utf-8")
    check("turnserver.conf carries the listen address",
          f"listening-ip={FACTS['turn_ecoute']}" in contenu_turn, True)
    check("turnserver.conf carries the relay address",
          f"relay-ip={FACTS['turn_relais']}" in contenu_turn, True)
    check("turnserver.conf carries the SAME secret as TURN_SECRET",
          f"static-auth-secret={env1['TURN_SECRET']}" in contenu_turn, True)
    check("turnserver.conf is in 600 (it carries a secret)",
          oct(os.stat(turnconf).st_mode & 0o777), oct(0o600))

# --- Installation 2, WITHOUT facts: the independent derivation ---------------
with tempfile.TemporaryDirectory() as tmp2:
    root2 = pathlib.Path(tmp2)
    r2 = appeler(root2, facts=None)
    check("installation 2 (without facts): exit code 0", r2.returncode, 0)
    env2, chemin_env2 = lire_env(root2)

    # 🔴 The default port is 3445: /etc/pomerium/config.yaml routes
    # https://app.allanic.me to 3445, and nothing else makes the route
    # already in place work (see hooks/resolve.py::PORT_DEFAUT, the same
    # reason, duplicated on purpose — see the comment of install.py).
    check("without facts: the port falls back to the default 3445",
          env2.get("PLATEFORME_PORT"), "3445")
    check("without facts: PLATEFORME_HOTE is still derived, never empty",
          bool(env2.get("PLATEFORME_HOTE")), True)
    check("without facts: the listen address is still not universal",
          env2.get("PLATEFORME_HOTE") in ("0.0.0.0", "::", "[::]", "*"), False)

    # 🔴 THE MOST IMPORTANT REGRESSION OF THIS FILE — WITHOUT FACTS, IT IS
    # THE PATH REALLY TAKEN BY THE ENGINE ("install DOES NOT receive
    # resolve's facts", see the head docstring of install.py). Before
    # batch 10A (29 August 2026), this path derived PLATEFORME_HOTE through
    # `deriver_adresse_hote()` = the interface of the DEFAULT IPv4 route —
    # which, on THIS machine, measured with /usr/bin/ip outside any shell
    # alias, is `ppp0` (PPPoE), a PUBLIC address (90.87.35.18).
    # `install.py` would therefore have set PLATEFORME_HOTE=90.87.35.18 during a
    # REAL installation without facts on THIS host — exposing the remote
    # desktop on the public internet with no Pomerium in front of it. This check
    # did NOT exist before this batch (the old one only checked "non-empty,
    # never universal" — 90.87.35.18 is neither empty nor universal, so it
    # passed without detecting anything). It MUST now be the FIXED default.
    check("without facts: PLATEFORME_HOTE is the FIXED default, never the "
          "default route (regression of 29 August 2026, see commun.py)",
          env2.get("PLATEFORME_HOTE"), HOTE_DEFAUT)
    check("without facts: PLATEFORME_PROXY_DE_CONFIANCE is the default",
          env2.get("PLATEFORME_PROXY_DE_CONFIANCE"), PROXY_DEFAUT)

    # 🔴 Two distinct installations DO NOT SHARE the secret.
    check("two installations do not share PLATEFORME_SECRET_JETON",
          env1["PLATEFORME_SECRET_JETON"] == env2["PLATEFORME_SECRET_JETON"],
          False)
    check("two installations do not share TURN_SECRET",
          env1["TURN_SECRET"] == env2["TURN_SECRET"], False)

# --- Installation 3: DESK_NODE_BIN configures the interpreter path -----
# 🔴 PROBLEM A OF BATCH 10A: proves that the path is indeed CONFIGURABLE
# (never a second /usr/bin/npm hardcoded elsewhere), with its own
# negative control — the default of installation 1/2 above, DIFFERENT from
# this value.
with tempfile.TemporaryDirectory() as tmp3:
    root3 = pathlib.Path(tmp3)
    env_override = dict(os.environ)
    env_override["DESK_NODE_BIN"] = "/opt/nivuus-test/node/bin"
    r3 = appeler(root3, facts=FACTS, env=env_override)
    check("installation 3 (DESK_NODE_BIN): exit code 0", r3.returncode, 0)
    unite3 = root3 / "etc" / "systemd" / "system" / "desk-plateforme.service"
    ini3 = load_unit(unite3)
    check("DESK_NODE_BIN does change ExecStart",
          ini3.get("Service", "ExecStart", fallback=""),
          "/opt/nivuus-test/node/bin/npm start")
    check("DESK_NODE_BIN does change the PATH set",
          "/opt/nivuus-test/node/bin" in ini3.get("Service", "Environment", fallback=""),
          True)
    check("DESK_NODE_BIN: ExecStart is NO LONGER the default (negative control)",
          ini3.get("Service", "ExecStart", fallback="") == f"{NODE_BIN_DEFAUT}/npm start",
          False)

# --- Installation 5: REPLAYING install TWICE ON THE SAME ROOT ---------
# 🔴 REAL BUG FOUND IN BATCH 10A (29 August 2026): a real reinstallation
# on `--root /`, with a `plateforme/node_modules/.bin/` carrying
# symlinks (tsx, vite, tsc, …), raised `shutil.Error` — `dirs_exist_ok=True`
# only covers DIRECTORIES, never the links they contain. No
# previous scenario of this file replayed install TWICE on the
# SAME root: it is exactly the pattern "a check never
# seen red is not a check". This scenario builds a minimal SOURCE root
# carrying a REAL symlink under node_modules/.bin (reproducing the
# real case), installs twice in a row on the SAME target root, and
# checks that the SECOND installation succeeds too.
with tempfile.TemporaryDirectory() as tmp5src, tempfile.TemporaryDirectory() as tmp5dst:
    source5 = pathlib.Path(tmp5src)
    poser_source_minimale(source5)
    (source5 / "plateforme" / "node_modules" / "a-package").mkdir(parents=True)
    (source5 / "plateforme" / "node_modules" / "a-package" / "cli.js").write_text(
        "#!/usr/bin/env node\n", encoding="utf-8")
    bin_dir5 = source5 / "plateforme" / "node_modules" / ".bin"
    bin_dir5.mkdir(parents=True)
    (bin_dir5 / "tsx").symlink_to("../a-package/cli.js")

    root5 = pathlib.Path(tmp5dst)
    env_source5 = dict(os.environ)
    env_source5["DESK_SOURCE_RACINE"] = str(source5)

    r5a = appeler(root5, facts=FACTS, env=env_source5)
    check("first installation (with symlink): exit code 0",
          r5a.returncode, 0)
    if r5a.returncode != 0:
        failures.append(f"first installation stderr: {r5a.stderr!r}")

    r5b = appeler(root5, facts=FACTS, env=env_source5)
    check("second installation ON THE SAME ROOT: exit code 0 "
          "(regression of 29 August 2026: shutil.Error on the symlinks "
          "of node_modules/.bin)", r5b.returncode, 0)
    if r5b.returncode != 0:
        failures.append(f"second installation stderr: {r5b.stderr!r}")

    lien5 = root5 / "opt" / "nivuus" / "desk" / "plateforme" / "node_modules" / ".bin" / "tsx"
    check("the symlink survives the reinstallation, still a link",
          lien5.is_symlink(), True)

# --- Installation 7: THE NODE RUNTIME IS DROPPED, NOT ASSUMED -------------
# 🔴 IMPORTANT FINDING OF THE FINAL BRANCH REVIEW (30 August 2026): `commun.py::
# NODE_BIN_DEFAUT` designates `/opt/nivuus/node/bin`, and NO hook dropped
# anything there — the tree present on the development machine had
# been copied there BY HAND during batch 10A, by a command that only lived
# in a gitignored report. A fresh installation therefore laid down a service
# structurally unable to start, without a word.
# This scenario uses the REAL runtime of this machine (`node_source=False`),
# that is the production path: a prefix derived from
# `process.execPath`, its real relative links, its ~144 MiB. The other
# scenarios use a fake prefix — they need not pay for the copy again.
with tempfile.TemporaryDirectory() as tmp7:
    root7 = pathlib.Path(tmp7)
    r7 = appeler(root7, facts=FACTS, node_source=False)
    check("installation 7 (real runtime): exit code 0", r7.returncode, 0)
    if r7.returncode != 0:
        failures.append(f"installation 7 stderr: {r7.stderr!r}")
    node_dep = root7 / NODE_BIN_DEFAUT.lstrip("/")
    check("bin/node is dropped", (node_dep / "node").is_file(), True)
    check("bin/npm is dropped", (node_dep / "npm").exists(), True)
    # 🔴 `npm` MUST STAY A LINK: its target is RELATIVE and points into
    # the dropped tree. Following it would drop a COPY of npm-cli.js under a
    # name that pretends to be npm, and `npm` would stop finding its modules.
    check("bin/npm is a LINK, never a followed copy",
          (node_dep / "npm").is_symlink(), True)
    check("the global npm package is dropped",
          (node_dep.parent / "lib" / "node_modules" / "npm").is_dir(), True)
    # 🔴 `DynamicUser=yes` runs the service under an ephemeral UID: a
    # `bin/node` in rwxr-x--- would make ExecStart fail before the first
    # line of JavaScript (real bug of batch 10A on /opt/nivuus/desk).
    check("bin/node is executable by others (DynamicUser)",
          bool(os.stat(node_dep / "node").st_mode & 0o001), True)
    # Negative control: the dropped unit does point to THAT very path.
    ini7 = load_unit(root7 / "etc" / "systemd" / "system" / "desk-plateforme.service")
    check("ExecStart points to the npm really dropped",
          ini7.get("Service", "ExecStart", fallback=""),
          f"{NODE_BIN_DEFAUT}/npm start")

# --- lire_secret_persiste: the five cases at unit level -------------------
# 🔴 REAL BUG FOUND AND FIXED ON 2026-09-08: `install.py` drew
# `PLATEFORME_SECRET_JETON` and `TURN_SECRET` UNCONDITIONALLY on every call
# (`write_secret()` twice, never read back), contradicting
# the docstring of `write_secret` that promised "drawn only once,
# never recomputed" — the idempotence gate of the release plan detected the
# non-idempotence (etc/nivuus/desk.env and etc/turnserver.conf change between
# two passes). `lire_secret_persiste` (hooks/installed_files.py) is the
# function that now really carries this invariant; these five
# scenarios exercise EACH case, separately from a complete installation,
# because a single subprocess hook cannot easily tell "key
# absent" from "empty value" from "file absent" in a single readable
# assertion.
with tempfile.TemporaryDirectory() as tmp_ls:
    dossier_ls = pathlib.Path(tmp_ls)

    # Case 1: the file does not exist at all (first install).
    absent = dossier_ls / "does-not-exist.env"
    check("lire_secret_persiste: file absent -> None",
          lire_secret_persiste(absent, "PLATEFORME_SECRET_JETON"), None)

    # Case 2: the file exists but does not carry the requested key (an
    # earlier installation of a version that did not write
    # this key yet).
    sans_cle = dossier_ls / "without-key.env"
    sans_cle.write_text("OTHER_KEY=a-value\n", encoding="utf-8")
    check("lire_secret_persiste: key absent from the file -> None",
          lire_secret_persiste(sans_cle, "PLATEFORME_SECRET_JETON"), None)

    # Case 3: the key is present but its value is empty, or made
    # only of spaces (truncated file or edited by hand) — BOTH
    # forms count as "nothing to reuse".
    empty_value = dossier_ls / "empty-value.env"
    empty_value.write_text(
        "PLATEFORME_SECRET_JETON=\nTURN_SECRET=   \n", encoding="utf-8")
    check("lire_secret_persiste: empty value -> None",
          lire_secret_persiste(empty_value, "PLATEFORME_SECRET_JETON"), None)
    check("lire_secret_persiste: value made of spaces -> None",
          lire_secret_persiste(empty_value, "TURN_SECRET"), None)

    # Case 4: the key is present with a usable value -> reused
    # as is.
    real_value = dossier_ls / "real-value.env"
    real_value.write_text(
        "PLATEFORME_SECRET_JETON=abc123\nAUTRE=x\n", encoding="utf-8")
    check("lire_secret_persiste: value present -> reused as is",
          lire_secret_persiste(real_value, "PLATEFORME_SECRET_JETON"),
          "abc123")

    # Case 5: the file EXISTS but reading it fails (permissions skewed
    # by a partial migration, disk error, ...) — NOT "absent", hence
    # NOT "nothing to reuse". 🔴 REVIEW OF 2026-09-08: a too broad `except OSError:
    # return None` swallowed THIS case exactly like case 1, and
    # would have drawn a NEW secret SILENTLY — the same silent
    # rotation as the original bug, through a narrower door. This
    # function must RAISE, never return None, so that install.py can
    # refuse instead of hallucinating a secret.
    #
    # A minimal DOUBLE rather than a real chmod'ed file: these suites
    # run as root on this machine, which overrides the POSIX
    # permissions — a real `chmod 000` would therefore NOT produce a PermissionError
    # here, and the scenario would stay green by accident. `CheminIllisible`
    # ONLY imitates the two methods that `lire_secret_persiste` uses,
    # in the same spirit as the other fake doubles of this directory
    # (`poser_faux_node_source`, etc.): is_file() says "present", read_text()
    # raises — exactly the shape of a real but unreadable file.
    class CheminIllisible:
        def is_file(self):
            return True

        def read_text(self, encoding="utf-8"):
            raise PermissionError(
                "permission denied (fake, case 5 of this scenario)")

    illisible = CheminIllisible()
    try:
        obtained_value = lire_secret_persiste(illisible, "PLATEFORME_SECRET_JETON")
        failures.append(
            "lire_secret_persiste: an unreadable file should have RAISED an "
            f"OSError, returned {obtained_value!r} without raising (a new secret "
            "drawn silently if this happened in install.py)")
    except FileNotFoundError:
        failures.append(
            "lire_secret_persiste: an unreadable file must NOT be "
            "confused with FileNotFoundError (the file IS present)")
    except OSError:
        pass  # expected: the read failure propagates.

# --- Installation 8: REPLAYING install PROVES the reuse of the secrets --
# The property the bug of 2026-09-08 violated, exercised end to end
# by the REAL hook (complete subprocess), not only by
# lire_secret_persiste in isolation: a first install on an EMPTY
# root does produce both secrets (nothing to reuse yet), and a
# SECOND install on the SAME root leaves them UNCHANGED.
with tempfile.TemporaryDirectory() as tmp8:
    root8 = pathlib.Path(tmp8)

    r8a = appeler(root8, facts=FACTS)
    check("installation 8, first pass: exit code 0",
          r8a.returncode, 0)
    if r8a.returncode != 0:
        failures.append(f"installation 8 stderr (1st pass): {r8a.stderr!r}")
    env8a, _chemin_env8a = lire_env(root8)
    turnconf8 = root8 / "etc" / "turnserver.conf"

    # First pass on an empty root: nothing to reuse -> both
    # secrets ARE produced (the case the bug did not break).
    check("first pass: PLATEFORME_SECRET_JETON is produced (>= 32 chars)",
          len(env8a.get("PLATEFORME_SECRET_JETON", "")) >= 32, True)
    check("first pass: TURN_SECRET is produced (>= 32 chars)",
          len(env8a.get("TURN_SECRET", "")) >= 32, True)

    r8b = appeler(root8, facts=FACTS)
    check("installation 8, second pass ON THE SAME ROOT: exit code 0",
          r8b.returncode, 0)
    if r8b.returncode != 0:
        failures.append(f"installation 8 stderr (2nd pass): {r8b.stderr!r}")
    env8b, _ = lire_env(root8)

    # 🔴 THE REGRESSION THIS SCENARIO GUARDS: before the fix, these two
    # equalities failed on every replay (secrets.token_hex(32) draws two
    # different values with overwhelming probability).
    check("PLATEFORME_SECRET_JETON is REUSED, never redrawn on replay",
          env8b.get("PLATEFORME_SECRET_JETON"),
          env8a.get("PLATEFORME_SECRET_JETON"))
    check("TURN_SECRET is REUSED, never redrawn on replay",
          env8b.get("TURN_SECRET"), env8a.get("TURN_SECRET"))

    # turnserver.conf stays consistent with TURN_SECRET after the replay
    # (both writers must always agree, replay or not).
    contenu_turn8 = turnconf8.read_text(encoding="utf-8")
    check("turnserver.conf STILL carries the same secret as TURN_SECRET "
          "after a replay",
          f"static-auth-secret={env8b['TURN_SECRET']}" in contenu_turn8, True)

if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - install hook tests passed")
