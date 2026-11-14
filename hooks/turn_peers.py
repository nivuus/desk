#!/usr/bin/env python3
"""Which peers the TURN relay may reach on behalf of a credential holder.

A relay with no peer restriction relays towards ANY destination it can
route to: the holder of a valid relay credential — any browser the platform
handed ICE credentials to, for 24 h — could then reach private, shared,
link-local and other non-public addresses from the relay host. The relay
must only carry the product's own media.

Who the peers legitimately are (`plateforme/src/signaling/ice.ts` gives
relay credentials to BOTH peers; measured in
`docs/superpowers/plans/2026-07-29-traversee-nat-resultats.md`):  # policy: allow-fr - real file name

- the browser's relayed allocation sends to the agent's candidates, whose
  host candidate is the controlled machine's address (`commun.
  read_vm_address`) — the pair ICE nominates, since the relay reaches it;
- the agent's relayed allocation sends to the browser's candidates, which
  are public (server-reflexive) addresses.

So every non-public destination is denied and exactly ONE address is
allowed back: the controlled machine's. The relay's own addresses
(`listening-ip`, `relay-ip`) are denied too, even when public: a peer
there is the relay host itself, whose other services must not be reachable
through it; the relay-to-relay pair this costs is one that same
measurement found ICE never nominates in this topology.

`no-tcp-relay`: neither peer opens TCP relay allocations (RFC 6062) — the
agent's TURN client and WebRTC browsers relay over UDP — so the relay
refuses them rather than offer TCP connections to the allowed address.

Directive semantics, read in the coturn 4.6.1 sources (the version Debian
trixie ships, 4.6.1-2) — `examples/etc/turnserver.conf`,
`src/apps/relay/userdb.c::add_ip_list_range`,
`src/server/ns_turn_server.c::good_peer_addr`,
`src/client/ns_turn_ioaddr.c`:

- `denied-peer-ip` / `allowed-peer-ip` take ONE address or a range
  `first-last`. There is NO prefix notation: `10.0.0.0/8` fails to parse,
  is logged "Wrong address format" and the server starts WITHOUT that
  entry. Every entry is therefore written as a range.
- an address both allowed and denied is ALLOWED: the allow list is checked
  first. Loopback (unless `allow-loopback-peers`), multicast (with
  `no-multicast-peers`) and the zero address are refused before either
  list, so allowing them back is impossible.
- IPv6 addresses are compared byte by byte (`addr_less_eq`), which is only
  correct for a range that is exactly a prefix block (first address to
  last address); every range here is one. A range whose first address is
  the IPv6 unspecified address `::` would match EVERY IPv4 peer
  (`ioa_addr_in_range` treats an all-zero lower bound as "any", and an
  IPv4 address compares below an IPv6 one): no such range is emitted. The
  unspecified and loopback IPv6 addresses are refused by coturn itself.

Allocation quotas (`user-quota`, `total-quota`) are deliberately NOT set:
one session opens one peer connection per window, each gathering relayed
candidates on both sides, and no measurement in this repository bounds that
number — a quota would be an invented constant.
"""
import ipaddress

# IANA IPv4 and IPv6 Special-Purpose Address Registries (RFC 6890 and its
# updates): every block that is not globally reachable, plus multicast and
# the reserved space. IPv6 blocks that start at `::` are left out on
# purpose; see the module docstring.
DENIED_PEER_NETWORKS = tuple(ipaddress.ip_network(block) for block in (
    "0.0.0.0/8",          # "this network" (RFC 791)
    "10.0.0.0/8",         # private use (RFC 1918)
    "100.64.0.0/10",      # shared address space (RFC 6598)
    "127.0.0.0/8",        # loopback (RFC 1122)
    "169.254.0.0/16",     # link-local (RFC 3927)
    "172.16.0.0/12",      # private use (RFC 1918)
    "192.0.0.0/24",       # IETF protocol assignments (RFC 6890)
    "192.0.2.0/24",       # documentation TEST-NET-1 (RFC 5737)
    "192.88.99.0/24",     # deprecated 6to4 relay anycast (RFC 7526)
    "192.168.0.0/16",     # private use (RFC 1918)
    "198.18.0.0/15",      # benchmarking (RFC 2544)
    "198.51.100.0/24",    # documentation TEST-NET-2 (RFC 5737)
    "203.0.113.0/24",     # documentation TEST-NET-3 (RFC 5737)
    "224.0.0.0/4",        # multicast (RFC 5771)
    "240.0.0.0/4",        # reserved, and limited broadcast (RFC 1112, 919)
    "::ffff:0:0/96",      # IPv4-mapped (RFC 4291): would bypass the IPv4 list
    "64:ff9b::/96",       # IPv4/IPv6 translation (RFC 6052)
    "64:ff9b:1::/48",     # local-use IPv4/IPv6 translation (RFC 8215)
    "100::/64",           # discard-only (RFC 6666)
    "2001::/23",          # IETF protocol assignments, Teredo (RFC 2928, 4380)
    "2001:db8::/32",      # documentation (RFC 3849)
    "2002::/16",          # 6to4, embeds an IPv4 address (RFC 3056)
    "3fff::/20",          # documentation (RFC 9637)
    "fc00::/7",           # unique local (RFC 4193)
    "fe80::/10",          # link-local (RFC 4291)
    "fec0::/10",          # deprecated site-local (RFC 3879)
    "ff00::/8",           # multicast (RFC 4291)
))


def coturn_range(network) -> str:
    """`first-last`, the only multi-address form coturn 4.6.1 parses."""
    return f"{network.network_address}-{network.broadcast_address}"


def allowed_peer_address(raw: str, origin: str) -> str:
    """Validates one address to allow back, or raises ValueError.

    coturn skips an entry it cannot parse and starts anyway, so an invalid
    value must stop the install here rather than vanish there. A loopback,
    multicast or unspecified address can never be allowed back by coturn
    (see the module docstring): asking for it is a configuration error.
    """
    try:
        address = ipaddress.ip_address(str(raw).strip())
    except ValueError:
        raise ValueError(
            f"{origin}={raw!r} is not an IP address: the TURN relay can only "
            "allow a peer by address") from None
    if address.is_unspecified or address.is_loopback or address.is_multicast:
        raise ValueError(
            f"{origin}={raw!r} cannot be a relay peer (unspecified, "
            "loopback or multicast address)")
    return str(address)


def peer_restriction_lines(vm_address: str, relay_addresses) -> list:
    """The `turnserver.conf` lines that confine the relay's peers.

    `relay_addresses`: the relay's own addresses (listen and relay), denied
    one by one. The controlled machine's address is the only one allowed.
    """
    allowed = allowed_peer_address(vm_address, "the controlled machine's address")
    own = []
    for raw in relay_addresses:
        address = str(ipaddress.ip_address(str(raw).strip()))
        if address == allowed:
            raise ValueError(
                f"the controlled machine's address {allowed} is also an "
                "address of the TURN relay itself: the relay cannot both "
                "deny its own host and allow that peer")
        if address not in own:
            own.append(address)
    lines = ["no-multicast-peers", "no-tcp-relay"]
    lines += [f"denied-peer-ip={coturn_range(r)}" for r in DENIED_PEER_NETWORKS]
    lines += [f"denied-peer-ip={a}" for a in own]
    lines.append(f"allowed-peer-ip={allowed}")
    return lines
