#!/usr/bin/env python3
"""Building `agent.exe` and publishing it where its consumers read it.

`desk` is the single owner of the Windows agent: it holds the source,
cross-compiles it, and publishes the result in ITS OWN deployed tree:

    /opt/nivuus/desk/agent/agent.exe
    /opt/nivuus/desk/agent/agent.manifest.json

`/opt/nivuus/<package>/` is where the suite already lets one package reach
another's deployed files (`home-stock` and `home-desk` write into
`/opt/nivuus/home-manager/config`). It is NOT `/opt/nivuus-packages/console/`
any more: this hook used to drop the binary inside the `console` package's
tree, which `nivuus update console` replaces wholesale - the agent then went
back to whatever `console` shipped, and nothing replayed this activation. It
is not `/var/lib/nivuus-desk` either: the network-facing service can write
there, and what is published here ends up executed in the Windows guest.
`install` only ever replaces `plateforme/` and `client/dist` under
`/opt/nivuus/desk`, so the publication survives a reinstall.

The `console` package consumes the publication when it stages a guest
payload (`console/desk_agent.py` in nivuus/installer), and refuses a binary
that is not the one the manifest describes. The directory, the two file
names and the manifest format are therefore a CONTRACT between two
repositories: `tests/fixtures/agent-publication-contract.json` is the same
file in both, and `tests/test_desk_agent_contract.py` fails when this module
stops agreeing with it.

What a reader can observe, at any instant, is one of:

  * no manifest          - nothing is published (never, or mid-publication);
  * a manifest           - and beside it the complete binary it describes.

Never a manifest for a half-written or a different binary: the build lands
in a scratch directory inside the publication directory (same filesystem),
the previous manifest is removed, the binary is renamed into place, and the
new manifest is written last, through a temporary file and a rename.

Like `hooks/vm.py`, `hooks/administration.py` and `hooks/env_file.py`, this
module is not a hook runnable on its own: `hooks/activate.py` imports it.
"""
import contextlib
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass

from atomic_file import fsync_directory, replace_file

PACKAGE_ROOT = pathlib.Path(__file__).resolve().parents[1]

# The contract with nivuus/installer's console package. Change nothing here
# without changing the shared fixture and `console/desk_agent.py` together.
AGENT_PUBLICATION_DIR = "/opt/nivuus/desk/agent"
AGENT_BINARY_NAME = "agent.exe"
AGENT_MANIFEST_NAME = "agent.manifest.json"
AGENT_MANIFEST_SCHEMA = 1

# Root-owned, never readable by "other": the consumer runs as root.
_MANIFEST_MODE = 0o640
_SCRATCH_PREFIX = ".build-"


@dataclass(frozen=True)
class PublishedAgent:
    binary: pathlib.Path
    manifest: pathlib.Path
    sha256: str
    size: int
    desk_version: str


def publication_dir(root: pathlib.Path) -> pathlib.Path:
    """`AGENT_PUBLICATION_DIR` under `root` (`/` in production)."""
    return pathlib.Path(root) / AGENT_PUBLICATION_DIR.lstrip("/")


def package_version() -> str:
    """The `version:` of this package's manifest - the build's identity.

    Read with a pattern, not a YAML parser: the hooks use the standard
    library only, and the release pipeline writes this line as a plain
    top-level scalar.
    """
    manifest = PACKAGE_ROOT / "nivuus-package.yaml"
    match = re.search(r"^version:[ \t]*['\"]?([^'\"\s#]+)",
                      manifest.read_text(encoding="utf-8"), re.MULTILINE)
    if not match:
        raise RuntimeError(f"{manifest} carries no top-level 'version:' line")
    return match.group(1)


def manifest_text(sha256: str, size: int, desk_version: str) -> str:
    """The manifest, exactly as published: sorted keys, two-space indent,
    one trailing newline. Deterministic, so an unchanged binary republished
    by the same version yields the same bytes."""
    return json.dumps({"schema": AGENT_MANIFEST_SCHEMA,
                       "file": AGENT_BINARY_NAME,
                       "sha256": sha256,
                       "size": size,
                       "desk_version": desk_version},
                      indent=2, sort_keys=True) + "\n"


def _sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _fsync_file(path: pathlib.Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def build_agent(directory: pathlib.Path) -> None:
    """Runs `scripts/build-agent-croise.sh <directory>`, which drops
    `agent.exe` there; raises `RuntimeError` (the script's output) on a
    non-zero code. The script is overridable through
    `DESK_BUILD_AGENT_SCRIPT` (never in production) so that tests never
    compile the real agent (~40 s) - see `tests/test_desk_payload.py`.
    """
    default = PACKAGE_ROOT / "scripts" / "build-agent-croise.sh"
    script = pathlib.Path(os.environ.get("DESK_BUILD_AGENT_SCRIPT") or default)
    proc = subprocess.run([str(script), str(directory)],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(f"{script} failed (code {proc.returncode}): "
                           f"{detail or 'no output'}")


def publish_agent(root: pathlib.Path, build=None,
                  desk_version: str | None = None) -> PublishedAgent:
    """Builds `agent.exe` and publishes it with its manifest under `root`.

    `build`: a fake in tests, `build_agent` by default; it receives the
    directory to drop `agent.exe` into. Raises `RuntimeError` when the build
    returns without having dropped it - a silent no-op must not read as a
    publication.

    When what was just built is byte-identical to what is already published
    under the same version, nothing is rewritten: the publication a consumer
    may be reading stays untouched.
    """
    build = build or build_agent
    version = desk_version if desk_version is not None else package_version()
    directory = publication_dir(root)
    directory.mkdir(parents=True, exist_ok=True)
    # A scratch directory left by a run that was killed: only this hook
    # writes here, and two activations of one package never run together.
    for leftover in directory.glob(f"{_SCRATCH_PREFIX}*"):
        shutil.rmtree(leftover)

    binary = directory / AGENT_BINARY_NAME
    manifest = directory / AGENT_MANIFEST_NAME
    scratch = pathlib.Path(tempfile.mkdtemp(dir=directory, prefix=_SCRATCH_PREFIX))
    try:
        build(scratch)
        built = scratch / AGENT_BINARY_NAME
        if not built.is_file():
            raise RuntimeError(f"the build returned but dropped no "
                               f"{AGENT_BINARY_NAME} in {scratch}: nothing "
                               "is published")
        size = built.stat().st_size
        if size == 0:
            raise RuntimeError(f"the build dropped an empty {AGENT_BINARY_NAME}: "
                               "nothing is published")
        sha256 = _sha256(built)
        text = manifest_text(sha256, size, version)
        published = PublishedAgent(binary, manifest, sha256, size, version)
        if _already_published(binary, manifest, text, sha256):
            return published
        # From here until the last line, no manifest exists: a reader sees
        # "nothing published", never a manifest beside another binary.
        with contextlib.suppress(FileNotFoundError):
            manifest.unlink()
        fsync_directory(directory)
        _fsync_file(built)
        os.replace(built, binary)
        fsync_directory(directory)
        replace_file(manifest, text, _MANIFEST_MODE)
        return published
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


def _already_published(binary: pathlib.Path, manifest: pathlib.Path,
                       text: str, sha256: str) -> bool:
    try:
        if manifest.read_text(encoding="utf-8") != text:
            return False
        return _sha256(binary) == sha256
    except (OSError, UnicodeDecodeError):
        return False
