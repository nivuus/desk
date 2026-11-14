#!/usr/bin/env python3
"""Tests of how the hooks REPLACE the files that hold the product's secrets.

`desk.env` (written by `install`, extended by `activate`) and
`turnserver.conf` both carry secrets in plain text. Each writer must:

- leave, at every instant, either the complete previous content or the
  complete new content at the path (temporary file in the same directory,
  fsync, rename) — a failure leaves the previous bytes and no stray file;
- never expose the content to "other";
- end with the environment file owner-only (0600), whatever its previous mode;
- keep, on the TURN configuration, the group and group-read bit the
  distribution sets up for its daemon, and nothing for "other".

Failures are injected by patching `os.fsync` / `os.replace` IN THIS PROCESS:
nothing here touches the disk outside a `tempfile.TemporaryDirectory()`.

Run: python3 tests/test_desk_atomic_write.py
"""
import os
import pathlib
import stat
import sys
import tempfile
from unittest import mock

RACINE = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RACINE / "hooks"))
from env_file import add_env_variables  # noqa: E402
from installed_files import write_env, write_turnserver_conf  # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


def mode_of(path: pathlib.Path) -> int:
    return stat.S_IMODE(path.stat().st_mode)


def siblings(path: pathlib.Path) -> list:
    return sorted(p.name for p in path.parent.iterdir())


def write_turn(path: pathlib.Path) -> None:
    write_turnserver_conf(path, "203.0.113.10", "203.0.113.10", "s" * 64,
                          "192.168.3.2")


WRITERS = {
    "write_env": lambda p: write_env(p, {"KEY": "new"}),
    "add_env_variables": lambda p: add_env_variables(p, {"AGENT_VM": "vm-1"}),
    "write_turnserver_conf": write_turn,
}

# --- 1. An existing 0644 environment file comes out 0600 ---------------------
for name in ("write_env", "add_env_variables"):
    with tempfile.TemporaryDirectory() as tmp:
        target = pathlib.Path(tmp) / "desk.env"
        target.write_text("OLD=1\n", encoding="utf-8")
        os.chmod(target, 0o644)
        WRITERS[name](target)
        check(f"{name}: an existing 0644 file ends 0600", oct(mode_of(target)),
              oct(0o600))

# --- 2. A failure during the write leaves the previous bytes, no stray file --
for name, writer in WRITERS.items():
    for primitive in ("os.fsync", "os.replace"):
        with tempfile.TemporaryDirectory() as tmp:
            target = pathlib.Path(tmp) / "secret.file"
            previous = b"PREVIOUS=content\nSECOND=line\n"
            target.write_bytes(previous)
            os.chmod(target, 0o600)
            before = siblings(target)
            raised = None
            with mock.patch(primitive, side_effect=OSError(5, "injected I/O error")):
                try:
                    writer(target)
                except OSError as exc:
                    raised = exc.strerror
            check(f"{name}: a failing {primitive} is reported, never swallowed",
                  raised, "injected I/O error")
            check(f"{name}: a failing {primitive} leaves the previous bytes",
                  target.read_bytes(), previous)
            check(f"{name}: a failing {primitive} leaves no temporary file",
                  siblings(target), before)

# --- 3. Success really replaces the file (new inode), complete content -------
for name, writer in WRITERS.items():
    with tempfile.TemporaryDirectory() as tmp:
        target = pathlib.Path(tmp) / "secret.file"
        target.write_text("OLD=1\n", encoding="utf-8")
        os.chmod(target, 0o600)
        inode_before = target.stat().st_ino
        writer(target)
        check(f"{name}: success replaces the inode",
              target.stat().st_ino != inode_before, True)
        check(f"{name}: success leaves only the target in the directory",
              siblings(target), ["secret.file"])
        check(f"{name}: the new content ends with a newline",
              target.read_text(encoding="utf-8").endswith("\n"), True)

# --- 4. A new file is created owner-only -------------------------------------
for name in ("write_env", "write_turnserver_conf"):
    with tempfile.TemporaryDirectory() as tmp:
        target = pathlib.Path(tmp) / "sub" / "new.file"
        WRITERS[name](target)
        check(f"{name}: a new file is created 0600", oct(mode_of(target)),
              oct(0o600))

# --- 5. The TURN configuration keeps the daemon's group and group-read bit ---
# The distribution's postinst sets root:turnserver 0640 on the file; the
# process's own group stands in for `turnserver` here (no root required).
own_gid = os.getgid()
for previous_mode, expected in ((0o640, 0o640), (0o644, 0o640), (0o600, 0o600),
                                (0o604, 0o600)):
    with tempfile.TemporaryDirectory() as tmp:
        target = pathlib.Path(tmp) / "turnserver.conf"
        target.write_text("# packaged default\n", encoding="utf-8")
        os.chown(target, -1, own_gid)
        os.chmod(target, previous_mode)
        write_turn(target)
        check(f"turnserver.conf previously {oct(previous_mode)}: final mode",
              oct(mode_of(target)), oct(expected))
        check(f"turnserver.conf previously {oct(previous_mode)}: group kept",
              target.stat().st_gid, own_gid)

# --- 6. The environment file never keeps a group-read bit --------------------
with tempfile.TemporaryDirectory() as tmp:
    target = pathlib.Path(tmp) / "desk.env"
    target.write_text("OLD=1\n", encoding="utf-8")
    os.chmod(target, 0o640)
    write_env(target, {"KEY": "v"})
    check("write_env: a 0640 environment file ends 0600", oct(mode_of(target)),
          oct(0o600))

# --- 7. add_env_variables keeps the lines already there, in order ------------
with tempfile.TemporaryDirectory() as tmp:
    target = pathlib.Path(tmp) / "desk.env"
    target.write_text("A=1\nB=2", encoding="utf-8")
    add_env_variables(target, {"AGENT_VM": "vm-1", "AGENT_SECRET": "x"})
    check("add_env_variables: existing lines kept, new ones appended",
          target.read_text(encoding="utf-8"),
          "A=1\nB=2\nAGENT_VM=vm-1\nAGENT_SECRET=x\n")

if failures:
    print(f"FAILED ({len(failures)})")
    for f in failures:
        print(" -", f)
    sys.exit(1)
print("OK: atomic replacement of the secret-bearing files")
