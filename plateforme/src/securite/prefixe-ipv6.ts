// The unit the brake counts an IPv6 client in: its /64.
//
// An IPv6 client is delegated at least a /64 (RFC 6177 recommends /56 or
// /48 to sites), and SLAAC privacy addresses (RFC 8981) let it draw a new
// /128 at will. A budget keyed per /128 therefore never fills. The /64 is
// the smallest block a client cannot leave, so it is the brake's key.
//
// `node:net` validates (`isIPv6`) but has no API that yields an address's
// prefix or its expanded form (`BlockList` only answers membership), so the
// expansion is done here; no dependency for sixteen lines.

import { isIPv6 } from 'node:net';

/// The /64 of an IPv6 address in one canonical spelling (`2001:db8:1:2::/64`),
/// or the input unchanged when it is not an IPv6 address (IPv4 stays keyed
/// per address).
export function cleReseau(adresse: string): string {
    const sansZone = adresse.split('%')[0];
    if (!isIPv6(sansZone)) return adresse;
    const hextets = developper(sansZone.toLowerCase());
    return `${hextets.slice(0, 4).map((h) => h.toString(16)).join(':')}::/64`;
}

/// The eight 16-bit groups of a valid IPv6 address.
function developper(adresse: string): number[] {
    let texte = adresse;
    // A dotted IPv4 tail (`::ffff:192.0.2.1`) stands for the last two groups.
    const ipv4 = /(\d+)\.(\d+)\.(\d+)\.(\d+)$/.exec(texte);
    if (ipv4) {
        const [a, b, c, d] = ipv4.slice(1).map(Number);
        texte = `${texte.slice(0, ipv4.index)}${((a << 8) | b).toString(16)}:${((c << 8) | d).toString(16)}`;
    }
    const [tete, queue] = texte.split('::');
    const groupes = (s: string | undefined) => (s ? s.split(':').map((h) => parseInt(h, 16)) : []);
    const debut = groupes(tete);
    const fin = groupes(queue);
    const zeros = queue === undefined ? [] : new Array<number>(8 - debut.length - fin.length).fill(0);
    return [...debut, ...zeros, ...fin];
}
