#!/usr/bin/env python3
"""The contract with nivuus/installer on where and how agent.exe is published.

`desk` publishes the agent and a manifest under /opt/nivuus/desk/agent; the
`console` package (nivuus/installer, `console/desk_agent.py`) reads them
there and refuses a manifest it does not recognise. Two repositories cannot
import each other, so the contract is a FILE:
`fixtures/agent-publication-contract.json` is the same in both
(nivuus/installer: console/tests/fixtures/), pinned here by its sha256. A
change of path or of manifest format has to change that file, hence the pin,
hence the other repository's copy and pin.

What this suite checks is that the PRODUCER agrees with it: its constants,
and the exact bytes it writes for the fixture's example.

Run: python3 tests/test_desk_agent_contract.py
"""
import hashlib
import json
import pathlib
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "hooks"))

import agent_payload  # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


FIXTURE = HERE / "fixtures" / "agent-publication-contract.json"
# Identical in nivuus/installer's console/tests/test_console_desk_agent.py.
# Editing the fixture means editing this pin in BOTH repositories.
FIXTURE_SHA256 = "ec159dca35360713b885bac96ecbc4d075bfbc5dcfbc368432fa55fac90a438f"
raw = FIXTURE.read_bytes()
check("the contract fixture is the one both repositories pinned",
      hashlib.sha256(raw).hexdigest(), FIXTURE_SHA256)
contract = json.loads(raw)
example = contract["example"]
binary = bytes.fromhex(example["binary_hex"])

check("contract version", contract["contract"], agent_payload.AGENT_MANIFEST_SCHEMA)
check("the publication directory",
      agent_payload.AGENT_PUBLICATION_DIR, contract["directory"])
check("the binary's name", agent_payload.AGENT_BINARY_NAME, contract["binary"])
check("the manifest's name", agent_payload.AGENT_MANIFEST_NAME, contract["manifest"])

check("manifest_text() yields the example manifest, byte for byte",
      agent_payload.manifest_text(hashlib.sha256(binary).hexdigest(), len(binary),
                                  example["desk_version"]),
      example["manifest_text"])

# And the real publication, end to end, writes exactly the contract's files
# at the contract's path (under a test root).
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)

    def build(directory):
        (directory / contract["binary"]).write_bytes(binary)

    agent_payload.publish_agent(root, build=build,
                                desk_version=example["desk_version"])
    directory = root / contract["directory"].lstrip("/")
    check("the published directory holds the contract's two files",
          sorted(p.name for p in directory.iterdir()),
          sorted([contract["binary"], contract["manifest"]]))
    check("the published binary is the example's",
          (directory / contract["binary"]).read_bytes(), binary)
    check("the published manifest is the example's, byte for byte",
          (directory / contract["manifest"]).read_bytes(),
          example["manifest_text"].encode("utf-8"))

if failures:
    print(f"FAIL ({len(failures)})")
    for failure in failures:
        print("  -", failure)
    sys.exit(1)
print("OK - agent publication contract checks passed")
