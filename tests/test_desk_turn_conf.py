#!/usr/bin/env python3
"""Tests of the peer restrictions of the TURN relay configuration.

The generated `turnserver.conf` (install hook) and `docker-compose.coturn.yml`
must deny every non-public destination and allow back only the controlled
machine's address. The verdicts below are computed by a MODEL of how coturn
4.6.1 applies those directives (`src/server/ns_turn_server.c::good_peer_addr`,
`src/client/ns_turn_ioaddr.c`), including its byte-wise IPv6 comparison and
its "all-zero lower bound means any" rule — so a range coturn would
evaluate differently from its prefix is caught here, not on a running relay.

No coturn is installed or started: everything is computed on the text.
The install hook runs under a `tempfile.TemporaryDirectory()` root.

Run: python3 tests/test_desk_turn_conf.py
"""
import ipaddress
import os
import pathlib
import re
import sys
import tempfile

RACINE = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RACINE / "hooks"))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from commun import ADRESSE_VM_DEFAUT, read_vm_address  # noqa: E402
from installed_files import write_turnserver_conf  # noqa: E402
from turn_peers import DENIED_PEER_NETWORKS, coturn_range  # noqa: E402
from desk_install_fixtures import FACTS, appeler  # noqa: E402

failures = []


def check(label, got, want):
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# --- A model of coturn 4.6.1's peer check -------------------------------------
def _parse(text: str):
    address = ipaddress.ip_address(text)  # raises like make_ioa_addr fails
    return address.version, address.packed


def _any(addr) -> bool:
    return not any(addr[1])


def _less_eq(a, b) -> bool:
    if a[0] != b[0]:
        return (2 if a[0] == 4 else 10) < (2 if b[0] == 4 else 10)
    if a[0] == 4:
        return int.from_bytes(a[1], "big") <= int.from_bytes(b[1], "big")
    return all(x <= y for x, y in zip(a[1], b[1]))  # byte-wise, as coturn does


def _in_range(rng, addr) -> bool:
    low, high = rng
    if _any(low) or _less_eq(low, addr):
        return True if _any(high) else _less_eq(addr, high)
    return False


def _parse_range(value: str):
    if "/" in value:
        raise ValueError(f"coturn 4.6.1 cannot parse {value!r}")
    first, _, last = value.partition("-")
    low = _parse(first)
    return low, (_parse(last) if last else low)


def coturn_accepts(lines, peer: str) -> bool:
    directives = [line.partition("=") for line in lines]
    flags = {key for key, sep, _ in directives if not sep}
    allowed = [_parse_range(v) for key, _, v in directives if key == "allowed-peer-ip"]
    denied = [_parse_range(v) for key, _, v in directives if key == "denied-peer-ip"]
    address = ipaddress.ip_address(peer)
    if "no-multicast-peers" in flags and address.is_multicast:
        return False
    if "allow-loopback-peers" not in flags and (
            (address.version == 4 and peer.startswith("127.")) or peer == "::1"):
        return False
    if address.is_unspecified:
        return False
    addr = _parse(peer)
    if any(_in_range(r, addr) for r in allowed):
        return True
    return not any(_in_range(r, addr) for r in denied)


def directives_of(text: str) -> list:
    return [line.strip() for line in text.splitlines()
            if line.strip() and not line.lstrip().startswith("#")]


DENIED_SAMPLES = (
    "0.1.2.3", "10.0.0.1", "10.255.255.254", "100.64.0.1", "100.127.255.254",
    "127.0.0.1", "169.254.169.254", "172.16.0.1", "172.31.255.254",
    "192.0.0.8", "192.0.2.1", "192.168.0.1", "192.168.3.1", "192.168.255.254",
    "198.18.0.1", "198.51.100.7", "203.0.113.7", "224.0.0.1", "239.255.255.250",
    "240.0.0.1", "255.255.255.255",
    "::1", "::ffff:10.0.0.1", "::ffff:192.168.3.2", "64:ff9b::a00:1",
    "2001:db8::1", "2002:c0a8:301::1", "fc00::1", "fd12:3456::1", "fe80::1",
    "febf:ffff::1", "fec0::1", "ff02::1",
)
PUBLIC_SAMPLES = ("8.8.8.8", "1.1.1.1", "93.184.216.34", "172.32.0.1",
                  "100.128.0.1", "2606:4700:4700::1111", "2a00:1450:4007::1")

# --- 1. The generated turnserver.conf ----------------------------------------
with tempfile.TemporaryDirectory() as tmp:
    path = pathlib.Path(tmp) / "turnserver.conf"
    write_turnserver_conf(path, "203.0.113.10", "203.0.113.11", "s" * 64,
                          "192.168.3.2")
    lines = directives_of(path.read_text(encoding="utf-8"))

peer_lines = [l for l in lines if l.startswith(("denied-peer-ip", "allowed-peer-ip"))]
check("no prefix notation in any peer directive (coturn 4.6.1 rejects it)",
      [l for l in peer_lines if "/" in l], [])
check("exactly one allowed peer, the controlled machine",
      [l for l in lines if l.startswith("allowed-peer-ip")],
      ["allowed-peer-ip=192.168.3.2"])
for flag in ("no-multicast-peers", "no-tcp-relay"):
    check(f"{flag} is set", flag in lines, True)
check("no allocation quota is invented",
      [l for l in lines if l.startswith(("user-quota", "total-quota"))], [])
for network in DENIED_PEER_NETWORKS:
    check(f"{network} is denied as a range",
          f"denied-peer-ip={coturn_range(network)}" in lines, True)
for peer in DENIED_SAMPLES:
    check(f"coturn refuses peer {peer}", coturn_accepts(lines, peer), False)
for peer in PUBLIC_SAMPLES:
    check(f"coturn accepts public peer {peer}", coturn_accepts(lines, peer), True)
check("coturn accepts the controlled machine",
      coturn_accepts(lines, "192.168.3.2"), True)
for own in ("203.0.113.10", "203.0.113.11"):
    check(f"coturn refuses the relay's own address {own}",
          coturn_accepts(lines, own), False)

# The model must be able to say "accepted" for a private address: without
# the allow line, the controlled machine is refused (negative control).
without_allow = [l for l in lines if not l.startswith("allowed-peer-ip")]
check("negative control: without the allow line the machine is refused",
      coturn_accepts(without_allow, "192.168.3.2"), False)

# --- 2. Invalid addresses stop the write, they never vanish in coturn --------
for raw in ("not-an-ip", "", "0.0.0.0", "127.0.0.1", "224.0.0.1", "203.0.113.10"):
    with tempfile.TemporaryDirectory() as tmp:
        path = pathlib.Path(tmp) / "turnserver.conf"
        try:
            write_turnserver_conf(path, "203.0.113.10", "203.0.113.10", "s" * 64, raw)
            raised = False
        except ValueError:
            raised = True
        check(f"controlled machine address {raw!r} is refused", raised, True)
        check(f"controlled machine address {raw!r}: nothing written",
              path.exists(), False)

# --- 3. The allowed address follows the shared definition and its override ---
previous = os.environ.pop("GUEST_IP", None)
check("read_vm_address: the default", read_vm_address(), ADRESSE_VM_DEFAUT)
os.environ["GUEST_IP"] = "10.20.30.40"
check("read_vm_address: GUEST_IP overrides", read_vm_address(), "10.20.30.40")
os.environ.pop("GUEST_IP")
if previous is not None:
    os.environ["GUEST_IP"] = previous

for guest_ip, expected in ((None, ADRESSE_VM_DEFAUT), ("10.20.30.40", "10.20.30.40")):
    with tempfile.TemporaryDirectory() as tmp:
        root = pathlib.Path(tmp)
        env = dict(os.environ)
        env.pop("GUEST_IP", None)
        if guest_ip:
            env["GUEST_IP"] = guest_ip
        r = appeler(root, facts=FACTS, env=env)
        check(f"install with GUEST_IP={guest_ip}: exit code", r.returncode, 0)
        conf = root / "etc" / "turnserver.conf"
        lines = directives_of(conf.read_text(encoding="utf-8")) if conf.is_file() else []
        check(f"install with GUEST_IP={guest_ip}: allowed peer",
              [l for l in lines if l.startswith("allowed-peer-ip")],
              [f"allowed-peer-ip={expected}"])

# --- 4. docker-compose.coturn.yml carries the same restrictions --------------
compose = (RACINE / "docker-compose.coturn.yml").read_text(encoding="utf-8")
options = re.findall(r"^\s*- --([a-z-]+)(?:=(.*\S))?\s*$", compose, re.MULTILINE)
denied_compose = [v for key, v in options if key == "denied-peer-ip"]
check("compose: the same denied ranges, in the same order",
      denied_compose,
      [coturn_range(r) for r in DENIED_PEER_NETWORKS]
      + ["${TURN_LISTENING_IP}", "${TURN_RELAY_IP}"])
allowed_compose = [v for key, v in options if key == "allowed-peer-ip"]
check("compose: one allowed peer, mandatory, from GUEST_IP",
      len(allowed_compose) == 1 and allowed_compose[0].startswith("${GUEST_IP:?"),
      True)
for flag in ("no-multicast-peers", "no-tcp-relay"):
    check(f"compose: {flag} is set", (flag, "") in options, True)

if failures:
    print(f"FAILED ({len(failures)})")
    for f in failures:
        print(" -", f)
    sys.exit(1)
print("OK - TURN relay peer restrictions")
