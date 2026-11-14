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

import {
    assurerAccesFrais,
    issueDeReponseRafraichissement,
    MARGE_FRAICHEUR_MS,
    sansVerrou,
    type Coffre,
    type IssueRafraichissement,
    type ReponseAuthMoi,
    type Verrou,
} from './jeton';

/// What this module needs from `fetch`, and nothing more.
export type FetchJeton = (
    url: string,
    init?: { method?: string; headers?: Record<string, string>; body?: string },
) => Promise<ReponseAuthMoi & { status: number }>;

export interface DepsJetonFrais {
    coffre: Coffre;
    /// The platform's origin, WITHOUT a trailing slash.
    base: string;
    fetch: FetchJeton;
    maintenant: () => number;
    /// The cross-tab lock (`verrouDuNavigateur()` in production).
    verrou: Verrou;
}

/// The Web Locks API of this browser, or no lock where it is missing (it is
/// only exposed in a secure context). Without it, two tabs can still refresh
/// at the same moment; nothing else in the page can serialise them.
export function verrouDuNavigateur(): Verrou {
    const locks = (globalThis as { navigator?: { locks?: LockManager } }).navigator?.locks;
    if (locks === undefined) return sansVerrou;
    // The DOM typing makes `request` return `Promise<R>` where `R` is the
    // callback's return type, here a promise; the specification resolves the
    // returned promise with the callback's SETTLED result, hence `T`.
    return <T>(nom: string, travail: () => Promise<T>) =>
        locks.request(nom, { mode: 'exclusive' }, travail) as unknown as Promise<T>;
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
            async (corps): Promise<IssueRafraichissement> => {
                let reponse: ReponseAuthMoi & { status: number };
                try {
                    reponse = await deps.fetch(`${deps.base}/auth/rafraichir`, {
                        method: 'POST',
                        headers: { 'content-type': 'application/json' },
                        body: JSON.stringify(corps),
                    });
                } catch {
                    // 🔴 THE REQUEST NEVER GOT AN ANSWER (offline, DNS, CORS):
                    // that is unavailability, and it says nothing about the
                    // refresh token — the store is left untouched.
                    return { issue: 'indisponible' };
                }
                // The shape is VALIDATED, never asserted: an incomplete body is
                // never written to the vault (`issueDeReponseRafraichissement`).
                const contenu = await reponse.json().catch(() => undefined);
                return issueDeReponseRafraichissement(reponse.status, reponse.ok, contenu);
            },
            MARGE_FRAICHEUR_MS,
            deps.verrou,
        );
}
