#!/usr/bin/env python3
"""Publishing `agent.exe`: where it lands, in which order, and what a
reader can observe at each instant.

`desk` is the single owner of the Windows agent. `hooks/agent_payload.py`
builds it and publishes it under `<root>/opt/nivuus/desk/agent/`, next to a
manifest written LAST; the `console` package reads both from there. Nothing
is written under another package's tree any more.

NO REAL BUILD HERE. The unit tests inject a fake `build` into
`publish_agent()`; the end-to-end tests (subprocess) use
`DESK_BUILD_AGENT_SCRIPT` (set by `desk_activate_fixtures.appeler()`) to
replace `scripts/build-agent-croise.sh` with a script that builds nothing -
never the real script, which would take ~40 s and a cross toolchain.

The manifest FORMAT and the path are pinned against the fixture shared with
nivuus/installer in `tests/test_desk_agent_contract.py`.

Run: python3 tests/test_desk_payload.py
"""
import importlib.util
import os
import pathlib
import sys
import tempfile

import hashlib
import json

from desk_activate_fixtures import (
    HOOK,
    appeler,
    poser_faux_npm,
    poser_racine_installee,
)

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# `hooks/activate.py` does `from vm import ...` / `from env_file import
# ...` / `from administration import ...`: an import by PATH
# (spec_from_file_location) does not go through the mechanism that
# automatically adds the script's directory to sys.path (precedent of
# test_desk_activate.py, task 6).
sys.path.insert(0, str(HOOK.parent))
spec = importlib.util.spec_from_file_location("desk_activate_payload", HOOK)
activate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(activate)


class _EnvTemporaire:
    """Sets an environment variable for the duration of the `with` block, and
    restores it exactly to its previous value (or absence). Avoids
    polluting the following tests with the `NIVUUS_PACKAGES_DIR` of a previous
    scenario."""

    def __init__(self, **values):
        self.values = values
        self.anciennes = {}

    def __enter__(self):
        for cle, value in self.values.items():
            self.anciennes[cle] = os.environ.get(cle)
            os.environ[cle] = value
        return self

    def __exit__(self, *_exc):
        for cle, ancienne in self.anciennes.items():
            if ancienne is None:
                os.environ.pop(cle, None)
            else:
                os.environ[cle] = ancienne


import agent_payload  # noqa: E402  (hooks/ is on sys.path, see above)


def drops(content: bytes):
    """A fake build: drops `content` as agent.exe in the directory it is given."""
    calls = []

    def build(directory):
        calls.append(directory)
        (directory / "agent.exe").write_bytes(content)
    build.calls = calls
    return build


def listing(directory):
    return sorted(p.name for p in directory.iterdir())


# === publish_agent(): the binary and its manifest, under desk's own tree ====
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    build = drops(b"fake-test-binary")
    published = agent_payload.publish_agent(root, build=build, desk_version="9.9.9")
    directory = root / "opt" / "nivuus" / "desk" / "agent"

    check("a single call to build", len(build.calls), 1)
    check("the build is handed a scratch directory INSIDE the publication "
          "directory (same filesystem: the rename is atomic)",
          build.calls[0].parent, directory)
    check("the binary is published under desk's own tree",
          published.binary, directory / "agent.exe")
    check("with the built content", published.binary.read_bytes(), b"fake-test-binary")
    check("nothing but the binary and its manifest is left there",
          listing(directory), ["agent.exe", "agent.manifest.json"])
    manifest = json.loads(published.manifest.read_text(encoding="utf-8"))
    check("the manifest describes that binary",
          manifest, {"schema": 1, "file": "agent.exe",
                     "sha256": hashlib.sha256(b"fake-test-binary").hexdigest(),
                     "size": len(b"fake-test-binary"), "desk_version": "9.9.9"})
    check("the manifest is not readable by other users",
          published.manifest.stat().st_mode & 0o007, 0)
    check("nothing is written under another package's tree",
          (root / "opt" / "nivuus-packages").exists(), False)

    # Same binary, same version: the publication is left untouched (a reader
    # may be copying it right now).
    before = (published.binary.stat().st_ino, published.manifest.stat().st_ino)
    agent_payload.publish_agent(root, build=drops(b"fake-test-binary"),
                                desk_version="9.9.9")
    check("republishing an identical build rewrites nothing",
          (published.binary.stat().st_ino, published.manifest.stat().st_ino), before)

    # A new build replaces both, and the manifest follows the binary.
    newer = agent_payload.publish_agent(root, build=drops(b"a-newer-binary"),
                                        desk_version="9.9.10")
    check("a new build replaces the binary", newer.binary.read_bytes(), b"a-newer-binary")
    check("and the manifest describes the NEW binary",
          json.loads(newer.manifest.read_text(encoding="utf-8"))["sha256"],
          hashlib.sha256(b"a-newer-binary").hexdigest())
    check("still nothing else in the directory",
          listing(directory), ["agent.exe", "agent.manifest.json"])


# === THE ORDER: the old manifest is gone BEFORE the binary changes, and the
# new one appears only AFTER. Observed from inside the two primitives that
# move the files, i.e. at the instants a concurrent reader could look. ======
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    agent_payload.publish_agent(root, build=drops(b"version-one"), desk_version="1")
    directory = root / "opt" / "nivuus" / "desk" / "agent"
    manifest_path = directory / "agent.manifest.json"
    observed = []
    real_replace, real_replace_file = os.replace, agent_payload.replace_file

    def spying_replace(src, dst):
        if pathlib.Path(dst).name == "agent.exe":
            observed.append(("binary renamed; manifest present before",
                             manifest_path.exists()))
        real_replace(src, dst)

    def spying_replace_file(path, content, mode, gid=None):
        observed.append(("manifest written; binary already the new one",
                         (directory / "agent.exe").read_bytes() == b"version-two"))
        real_replace_file(path, content, mode, gid)

    agent_payload.os.replace = spying_replace
    agent_payload.replace_file = spying_replace_file
    try:
        agent_payload.publish_agent(root, build=drops(b"version-two"), desk_version="2")
    finally:
        agent_payload.os.replace = real_replace
        agent_payload.replace_file = real_replace_file
    check("binary first (with NO manifest beside it), manifest last",
          observed, [("binary renamed; manifest present before", False),
                     ("manifest written; binary already the new one", True)])

    # Interrupted between the two: what is left is "nothing published",
    # never the previous manifest beside the new binary.
    def failing_replace_file(path, content, mode, gid=None):
        raise OSError(28, "No space left on device")

    agent_payload.replace_file = failing_replace_file
    raised = False
    try:
        agent_payload.publish_agent(root, build=drops(b"version-three"), desk_version="3")
    except OSError:
        raised = True
    finally:
        agent_payload.replace_file = real_replace_file
    check("a failure while writing the manifest propagates", raised, True)
    check("and leaves NO manifest (not the stale one)", manifest_path.exists(), False)
    check("the scratch directory is removed even then",
          listing(directory), ["agent.exe"])

    # The next activation repairs it, and clears a scratch directory left by
    # a run that was killed outright.
    (directory / ".build-killed").mkdir()
    (directory / ".build-killed" / "agent.exe").write_bytes(b"partial")
    agent_payload.publish_agent(root, build=drops(b"version-three"), desk_version="3")
    check("the next publication is complete, leftovers cleared",
          listing(directory), ["agent.exe", "agent.manifest.json"])


# === RED: a build that drops nothing, or an empty file -> RuntimeError, and
# what was published before is still there, intact ==========================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    agent_payload.publish_agent(root, build=drops(b"good"), desk_version="1")
    directory = root / "opt" / "nivuus" / "desk" / "agent"
    before = {p.name: p.read_bytes() for p in directory.iterdir()}
    for label, build in (("drops nothing", lambda directory: None),
                         ("drops an empty file", drops(b""))):
        raised = False
        try:
            agent_payload.publish_agent(root, build=build, desk_version="2")
        except RuntimeError:
            raised = True
        check(f"publish_agent() raises when the build {label}", raised, True)
        check(f"the previous publication survives a build that {label}",
              {p.name: p.read_bytes() for p in directory.iterdir()}, before)


# === package_version(): read from this package's own manifest ==============
check("the package version is the manifest's `version:` line",
      agent_payload.package_version() in
      (agent_payload.PACKAGE_ROOT / "nivuus-package.yaml").read_text(encoding="utf-8"),
      True)
check("and it is a bare token (no quotes, no comment)",
      any(c in agent_payload.package_version() for c in "'\" #"), False)


# === build_agent(): the REAL subprocess, but a FAKE script
# (DESK_BUILD_AGENT_SCRIPT) - never the real build-agent-croise.sh ==========
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    script = root / "fake-build.py"
    script.write_text(
        "#!/usr/bin/env python3\n"
        "import pathlib, sys\n"
        "d = pathlib.Path(sys.argv[1])\n"
        "d.mkdir(parents=True, exist_ok=True)\n"
        "(d / 'agent.exe').write_bytes(b'fake-test-pe32')\n",
        encoding="utf-8")
    script.chmod(0o755)

    with _EnvTemporaire(DESK_BUILD_AGENT_SCRIPT=str(script)):
        target = root / "out"
        agent_payload.build_agent(target)

    check("build_agent() invokes the fake script, which drops the file",
          (target / "agent.exe").read_bytes(), b"fake-test-pe32")

with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    script = root / "fake-build-fails.sh"
    script.write_text("#!/bin/sh\necho 'simulated build error' >&2\nexit 1\n",
                       encoding="utf-8")
    script.chmod(0o755)

    with _EnvTemporaire(DESK_BUILD_AGENT_SCRIPT=str(script)):
        detail = ""
        try:
            agent_payload.build_agent(root / "out")
        except RuntimeError as exc:
            detail = str(exc)

    check("build_agent() raises with the script's own error",
          "simulated build error" in detail, True)


# === The COMPLETE hook, as a subprocess: the agent is published even when
# the Windows VM is absent. console builds that VM from a payload that needs
# this very agent, and the engine orders nothing between the activations of
# two packages - so the publication must not wait behind the VM. ============
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir, packages_dir=False)      # console absent: no VM
    check("complete hook without a VM: refused (rc != 0)", r.returncode != 0, True)
    check("the refusal is about the VM, not about the agent",
          "Windows VM does not answer" in (r.stderr or ""), True)
    check("no Python traceback reaches the operator",
          "Traceback" in (r.stderr or ""), False)
    directory = root / "opt" / "nivuus" / "desk" / "agent"
    check("THE AGENT WAS PUBLISHED BEFORE THE VM GATE REFUSED",
          listing(directory) if directory.is_dir() else None,
          ["agent.exe", "agent.manifest.json"])


# === The COMPLETE hook: a failing build is a named refusal, and nothing
# VM-side is attempted after it ============================================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")
    failing = root / "fake-build-agent.py"       # the path appeler() reuses
    failing.write_text("#!/bin/sh\necho 'no cross toolchain here' >&2\nexit 1\n",
                       encoding="utf-8")
    failing.chmod(0o755)

    r = appeler(root, bin_dir)
    check("complete hook, build failing: refused (rc != 0)", r.returncode != 0, True)
    check("the refusal names the publication and carries the build's error",
          "publishing agent.exe refused" in (r.stderr or "")
          and "no cross toolchain here" in (r.stderr or ""), True)
    check("nothing was sent to the VM after the refusal",
          (root / "winrm.log").exists(), False)
    check("and no manifest claims an agent was published",
          (root / "opt" / "nivuus" / "desk" / "agent" / "agent.manifest.json").exists(),
          False)


# === The COMPLETE hook, as a subprocess: end-to-end success, with the
# FAKE build script set by default by appeler() ============================
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    poser_racine_installee(root)
    bin_dir = root / "fake-bin"
    bin_dir.mkdir()
    poser_faux_npm(bin_dir, root / "npm.log")

    r = appeler(root, bin_dir)
    check("complete hook: end-to-end success (rc == 0)", r.returncode, 0)
    if r.returncode != 0:
        failures.append(f"hook stderr: {r.stderr!r}")
    directory = root / "opt" / "nivuus" / "desk" / "agent"
    check("agent.exe and its manifest are published under desk's tree",
          listing(directory) if directory.is_dir() else None,
          ["agent.exe", "agent.manifest.json"])
    check("the hook writes NOTHING into the console package's tree",
          (root / "fake-packages-dir" / "console" / "guest" / "payload").exists(),
          False)
    check("the progress stream says where it was published",
          str(directory) in (r.stdout or ""), True)


if failures:
    print(f"FAIL ({len(failures)})")
    for f in failures:
        print("  -", f)
    sys.exit(1)
print("OK - agent.exe publication tests passed")
