// The ONE reader of small JSON request bodies (`routes-auth.ts`,
// `routes-installation.ts`).
//
// 🔴 BYTES ARE COLLECTED, AND DECODED ONCE. A body arrives in network
// chunks, and a multi-byte UTF-8 character can straddle two of them:
// decoding each chunk on its own turns that character into U+FFFD, so a
// password or an e-mail written in any non-ASCII script stops matching.
//
// 🔴 THE CAP COUNTS BYTES, NOT UTF-16 UNITS, and it is checked DURING the
// read: accumulating first would let a peer fill memory before the refusal.
// A UTF-16 count would let a CJK body weigh three times the declared cap.
//
// ⚠️ ON OVERFLOW THE LOOP IS LEFT, `req.destroy()` IS NOT CALLED: leaving
// the async iterator stops the read, whereas destroying kills the socket and
// the client never reads the 413 (measured by `routes-televersement.ts`,
// whose reader follows the same rule). No dependency needed: `node:http`
// streams are async iterables.

import type { IncomingMessage } from 'node:http';

/// The decoded body, or `undefined` when it exceeds `maxOctets` bytes.
export async function lireCorpsJson(
    req: IncomingMessage,
    maxOctets: number,
): Promise<string | undefined> {
    const morceaux: Buffer[] = [];
    let total = 0;
    for await (const morceau of req) {
        const b = morceau as Buffer;
        total += b.length;
        if (total > maxOctets) return undefined;
        morceaux.push(b);
    }
    return Buffer.concat(morceaux).toString('utf8');
}
