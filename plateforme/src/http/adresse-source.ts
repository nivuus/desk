// Who does this request come from? PURE function, no regular expression on
// the format of an address, no environment read.
//
// 🔴 WHY THIS MODULE EXISTS. Behind a reverse proxy,
// `req.socket.remoteAddress` is the address OF THE PROXY — the same for
// everybody. A per-address throttle based on it degenerates into a GLOBAL throttle: the
// service refuses itself at the 51st failure, whatever its origin.
// And `X-Forwarded-For` is FORGEABLE by the requester: trusting it blindly
// would make the throttle bypassable with one header line.
//
// 🔴 THE DEFAULT IS TO TRUST NOTHING. The trust set is EMPTY when
// `PLATEFORME_PROXY_DE_CONFIANCE` is absent — same doctrine as
// `PLATEFORME_ORIGINE_CLIENT` (`config.ts`), whose absence produces a refusal and
// not a permission.
//
// 🔴 AND WE TAKE THE LAST ELEMENT, NEVER THE FIRST. It is the classic
// mistake, and here is why it is one: `nginx` with
// `$proxy_add_x_forwarded_for` APPENDS the address of its peer to what the client
// sent. A client that sends `X-Forwarded-For: 203.0.113.7` therefore produces
// `203.0.113.7, <its real address>`. The FIRST element is the one the
// client forged; the LAST is the only one the proxy wrote itself.
//
// ⚠️ THIS RULE ASSUMES EXACTLY ONE TRUSTED PROXY AT THE HEAD OF THE CHAIN.
// With two chained proxies, the last element is the address of the FIRST
// PROXY, not that of the client. P5 does not deliver the N-hop chain: the
// versioned configuration sets only one, and `deploiement/nginx.conf`
// documents it. It is a DECLARED limit, not an omission.
//
// ⚠️ THE FAILURE MODE OF FORGETTING IS NAMED, and the remedy is NOT to
// trust the header by default — that would be trading a loud failure for
// a silent bypass. If the operator sets up a proxy without declaring trust
// in it, ALL requests carry the proxy address and the per-address throttle
// becomes global. The remedy is that THE THROTTLE TRACE NAMES THE ADDRESS
// IT RETAINED: an operator who reads `adresse=172.18.0.5` on every line
// recognises the address of their proxy. The runbook says so too.

/// The value returned when the peer has no address — an already closed socket
/// returns `undefined` for `remoteAddress`.
///
/// 🔴 IT IS NAMED, AND THAT IS NOT AFFECTATION. Without it, the throttle
/// key would become the string `"undefined"` by an interpolation accident:
/// it is exactly the trap that `signaling/turn-harnais.ts` documents for
/// `process.env` (`"undefined"` is a TRUTHY string), and it plays out again here.
/// A named value can be read in a trace and searched for in this file.
///
/// ⚠️ All the peers without an address therefore SHARE one throttle budget. That is
/// intended: it is the only behaviour that does not make the throttle bypassable
/// by closing one's socket before the service reads its address.
export const ADRESSE_INCONNUE = 'adresse-inconnue';

/// The prefix of IPv4 addresses encapsulated in IPv6. Node commonly returns
/// `::ffff:172.18.0.5` for an IPv4 peer on a dual stack.
const PREFIXE_MAPPE = '::ffff:';

/// ⚠️ NORMALISING IS MANDATORY ON BOTH SIDES — the value compared with
/// the trust set AND the value returned. Otherwise the same client
/// counts twice depending on the stack used, and its budget doubles; and a proxy
/// declared in its bare form would never be recognised in its encapsulated
/// form, which would make trust fail SILENTLY.
function normaliser(adresse: string): string {
    return adresse.startsWith(PREFIXE_MAPPE) ? adresse.slice(PREFIXE_MAPPE.length) : adresse;
}

/// Is the peer one of the declared proxies?
///
/// 🔴 IT LOOKS ONLY AT `remoteAddress`, NEVER AT `X-Forwarded-For` — unlike
/// `adresseSource` just above, and the difference is the point.
/// Honouring a header supplied by the attacker to decide whether to trust
/// the attacker is circular. `adresseSource` is right to do it, for its part:
/// it attributes a request to a client once the peer has already been judged.
export function pairDeConfiance(
    remote: string | undefined,
    confiance: ReadonlySet<string>,
): boolean {
    if (remote === undefined || remote === '') return false;
    const pair = normaliser(remote);
    for (const declare of confiance) {
        if (normaliser(declare) === pair) return true;
    }
    return false;
}

export function adresseSource(
    remote: string | undefined,
    enteteXff: string | undefined,
    confiance: ReadonlySet<string>,
): string {
    if (remote === undefined || remote === '') return ADRESSE_INCONNUE;
    const pair = normaliser(remote);

    // The set is normalised at comparison time rather than when the
    // configuration is read: `config.ts` returns what the operator wrote, and it is
    // here — the only reader — that the form is decided.
    let deConfiance = false;
    for (const declare of confiance) {
        if (normaliser(declare) === pair) {
            deConfiance = true;
            break;
        }
    }
    if (!deConfiance) return pair;

    if (enteteXff === undefined) return pair;
    // The LAST NON-EMPTY element: `203.0.113.7, ` has an empty last element,
    // and taking it would return an empty throttle key that all malformed
    // requests would share.
    const elements = enteteXff.split(',');
    for (let i = elements.length - 1; i >= 0; i--) {
        const element = elements[i].trim();
        if (element !== '') return normaliser(element);
    }
    return pair;
}
