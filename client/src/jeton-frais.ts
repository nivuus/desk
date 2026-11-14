// The fresh-token provider every page shares: the hub, its desktop and file
// bridge, and the session window.
//
// 🔴 MOVED HERE FROM `hub/page.ts::jetonFrais`, NOT COPIED. The session window
// (`main.ts`) presented the RAW vault content through `webrtc.ts`, while an
// access token lives ten minutes and a window opens whenever the remote
// machine announces one: it needed the hub's wiring, and two copies of it
// would drift. The RULE stays in `jeton.ts::assurerAccesFrais`; what lives
// here is the network wiring around it (the two calls it injects), with
// `fetch`, the vault and the clock injected so that it is testable in Node.

import { assurerAccesFrais, paireDeReponse, type Coffre, type ReponseAuthMoi } from './jeton';

/// What this module needs from `fetch`, and nothing more.
export type FetchJeton = (
    url: string,
    init?: { method?: string; headers?: Record<string, string>; body?: string },
) => Promise<ReponseAuthMoi>;

export interface DepsJetonFrais {
    coffre: Coffre;
    /// The platform's origin, WITHOUT a trailing slash.
    base: string;
    fetch: FetchJeton;
    maintenant: () => number;
}

/// Returns a function to call BEFORE EACH USE of the access token: a local
/// expiry test, and a network call only when the token is stale.
export function fournisseurDeJetonFrais(deps: DepsJetonFrais): () => Promise<string | undefined> {
    return () =>
        assurerAccesFrais(
            deps.coffre,
            deps.base,
            (url) => deps.fetch(url),
            deps.maintenant(),
            async (corps) => {
                // 🔴 WRAPPED, as `accesParPomerium` always was: without it a
                // network exception (offline, DNS, CORS) would propagate through
                // `assurerAccesFrais` to every caller instead of falling back to
                // Pomerium (step ③ of `assurerAccesFrais`).
                try {
                    const reponse = await deps.fetch(`${deps.base}/auth/rafraichir`, {
                        method: 'POST',
                        headers: { 'content-type': 'application/json' },
                        body: JSON.stringify(corps),
                    });
                    if (!reponse.ok) return undefined;
                    // The shape is VALIDATED, never asserted: an incomplete body
                    // would otherwise be written as is to the vault.
                    return paireDeReponse(await reponse.json().catch(() => undefined));
                } catch {
                    return undefined;
                }
            },
        );
}
