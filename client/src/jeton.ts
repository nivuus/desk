// The only place in the client that knows WHERE the token lives and whether it is still
// fresh. The rest of the browser goes through here.
//
// 🔴 THIS MODULE IS PURE AND DOM-FREE. Noted on August 19th, 2026: `client/` has
// no `vitest.config.*`, so the test environment is the default
// Node — there is neither `window` nor `localStorage`. The `Coffre` is a
// PARAMETER; `globalThis.localStorage` is only touched as an argument's
// default, at call time, never at module load. A `const coffre =
// localStorage` at the top of the file would suffice to make this module impossible to
// load under Node, hence impossible to test.
//
// 🔴 THE STORAGE IS `localStorage`, AND THE COST IS STATED HERE RATHER THAN DISCOVERED:
// a token in `localStorage` is readable by ANY script of the page, hence by
// a script injection. `sessionStorage` does not fit — the shell page
// opens its windows through `window.open` (`bureau/porteur-dom.ts`, `shell-page.ts`
// before the hub became the only surface, August 31st, 2026), and session
// storage is not guaranteed to be shared with them, which would force each
// window to reconnect. It is a TRADE-OFF, not an oversight.
//
// ⚠️ **P5 HAS PASSED, AND THE TRADE-OFF WAS NOT REOPENED** (cross-cutting review,
// August 20th, 2026). This sentence announced that "it w[ould] reopen [at] sub-block
// P5, with the security headers": the headers were delivered — two by
// the service (`plateforme/src/http/entetes.ts`), the rest by the proxy
// (`deploiement/nginx.conf`, including a CSP with `script-src 'self'`) — and **the
// storage has not changed**. It is not an oversight either: a CSP reduces
// the injection surface without removing it, and the real remedy — an
// `HttpOnly` cookie — stays **outside ⑤'s scope**, which delivered no cookie.
// **The trade-off holds, and it is now OWED rather than ANNOUNCED.**
//
// ⚠️ `exp` IS IN MILLISECONDS, and it is not a misreading:
// `plateforme/src/identite/jeton.ts` declares this deliberate divergence from
// RFC 7519, so that there is only one time unit in the whole
// service. This file is its browser-side mirror; changing one without
// the other would silently break freshness.

export interface Coffre {
    getItem(cle: string): string | null;
    setItem(cle: string, valeur: string): void;
    removeItem(cle: string): void;
}

export interface Paire {
    acces: string;
    rafraichissement: string;
}

export const CLE_ACCES = 'guac.jeton.acces';
export const CLE_RAFRAICHISSEMENT = 'guac.jeton.rafraichissement';

/// The default vault, read AT CALL TIME and never at load. Returns
/// `undefined` outside a browser, which lets the caller decide — rather than
/// throwing at the first `import` under Node.
function coffreParDefaut(): Coffre | undefined {
    const global = globalThis as { localStorage?: Coffre };
    return global.localStorage;
}

export function poser(coffre: Coffre, paire: Paire): void {
    coffre.setItem(CLE_ACCES, paire.acces);
    coffre.setItem(CLE_RAFRAICHISSEMENT, paire.rafraichissement);
}

/// Erases BOTH keys. Erasing only one would leave a usable refresh
/// behind a sign-out.
export function vider(coffre: Coffre): void {
    coffre.removeItem(CLE_ACCES);
    coffre.removeItem(CLE_RAFRAICHISSEMENT);
}

/// Sets the access token alone, and ERASES the refresh token.
///
/// 🔴 THE ERASURE IS THE POINT, NOT A COMFORT CLEAN-UP. The `pomerium`
/// mode delivers no refresh token. A token left by an
/// earlier `motdepasse` setup would be presented to a route that now returns
/// 404, and the symptom would be an unexplained sign-out ten minutes
/// after each page opening.
///
/// ⚠️ WHAT THE PRODUCT DOES ON EXPIRY, AND NOT WHAT ONE WOULD LIKE IT TO
/// DO — AND THIS PARAGRAPH WAS WRONG FOR TEN DAYS, THEN AGAIN
/// FOR ONE DAY. It first wrote "on expiry, the client
/// calls `GET /auth/moi` again" whereas **no code did so**, and the
/// cross-cutting review of the `auth-pomerium` workstream flagged it. It then
/// asserted the opposite — that `rafraichirSiNecessaire` (below) "has no
/// production caller", that the `grep` below "returns FIVE lines, one
/// definition and four test uses, not one call", and that the only path
/// to `/auth/moi` was a manual reload. 🔴 **ALL THREE
/// BECAME FALSE ON AUGUST 31ST, 2026**, when `assurerAccesFrais` (below)
/// took `rafraichirSiNecessaire` for its step ② and `accesParPomerium` for
/// its step ③, and `hub/page.ts` called `assurerAccesFrais` before each
/// use. **Rerun the command, never copy its figure:**
///
///     grep -rn 'rafraichirSiNecessaire(' client/src --include='*.ts' | grep -v '///'
///
/// ⚠️ THE SECOND `grep` IS NOT DECORATIVE: without it, the command counts THE
/// LINES OF THIS COMMENT, and the announced figure stops being the one it
/// returns. The correction wave of August 21st, 2026 paid for this pattern FOUR times
/// in the same round — a quoted `grep` anchors on syntax, never on a
/// name the surrounding prose repeats.
///
/// **WHAT THE PRODUCT DOES TODAY**, in `pomerium` mode: the vault's token
/// is tested by `assurerAccesFrais` **before each use**; if it
/// is stale within `MARGE_FRAICHEUR_MS`, `/auth/moi` is called again without any
/// gesture being needed. The Pomerium cookie living 8640 h, this round trip
/// is silent. ⚠️ What has NOT changed: `tenterPomerium`
/// (`connexion.ts`) still runs **only when the sign-in page
/// loads**, and `poserAcces` — the function below — stays the only
/// place that erases the refresh token.
export function poserAcces(coffre: Coffre, acces: string): void {
    coffre.setItem(CLE_ACCES, acces);
    coffre.removeItem(CLE_RAFRAICHISSEMENT);
}

/// The access token carried by the body of `GET /auth/moi`, or `undefined` if
/// that body carries no usable one.
///
/// 🔴 IT IS A RULE, NOT WIRING, AND THAT IS WHY IT LIVES HERE AND NOT
/// IN `connexion.ts`. This repository's criterion is reproducible — "a
/// condition is a rule if CHANGING it changes what the PRODUCT decides; it
/// is wiring if it only routes a decision already taken elsewhere,
/// and tested there". This one routes NOTHING: it VALIDATES a value the
/// service is contractually bound to supply, and no one else
/// validates it. **What a removal produces, measured rather than assumed**: the body
/// `{}` makes the string `"undefined"` written to the vault, then
/// `Bearer undefined` sent to `POST /session`, then a session error displayed
/// instead of the sign-in form — **and the vault stays poisoned** for
/// all subsequent loads. The product decides something else; it is therefore
/// indeed a rule, and it is held by this file's tests.
///
/// 🔴 THE EMPTY STRING IS REFUSED SEPARATELY FROM THE NON-STRING, and the empty
/// string test is not redundant: `typeof '' === 'string'`. It is the same
/// trap `plateforme/src/config.ts` paid for — `env.X ?? 'defaut'` does not
/// catch `''`. A `''` set in the vault would be a token no
/// `Authorization` can carry, and `jetonAcces` would return it as if it
/// were worth something.
///
/// ⚠️ PURE, AND WITHOUT A VAULT: it sets nothing itself. Setting is the gesture of
/// `poserAcces` just above, and keeping them distinct is what lets
/// the caller touch NOTHING when the answer is bad.
export function accesDeReponse(corps: unknown): string | undefined {
    if (typeof corps !== 'object' || corps === null) return undefined;
    const acces = (corps as { acces?: unknown }).acces;
    if (typeof acces !== 'string' || acces === '') return undefined;
    return acces;
}

/// The pair carried by the body of `POST /auth/rafraichir`, or `undefined` if
/// that body carries no usable one.
///
/// 🔴 **REUSES `accesDeReponse` FOR THE `acces` HALF, DOES NOT COPY IT**
/// — both routes share the same shape for that field, and a second
/// validation to keep in agreement would be debt. The same criterion (string,
/// NOT EMPTY) is applied to `rafraichissement`: `typeof '' === 'string'`, the
/// trap already paid for by `plateforme/src/config.ts` and by `accesDeReponse`
/// itself — an empty string would otherwise slip through.
///
/// 🔴 **ADDED AS A REVIEW FIX (round 1), NOT IN THE FIRST DRAFT**:
/// `assurerAccesFrais` (below) is the FIRST production caller of
/// `rafraichirSiNecessaire`, which writes everything its `appel` returns
/// directly to the vault (`poser`, in `rafraichirSiNecessaire`). Without this
/// guard, a body `{ acces: 'X' }` without `rafraichissement` — or the reverse —
/// would have poisoned the vault exactly like the defect `accesDeReponse`
/// exists to prevent on `/auth/moi`, without any test seeing it: this
/// path had stayed WITHOUT a production caller until this task.
export function paireDeReponse(corps: unknown): Paire | undefined {
    const acces = accesDeReponse(corps);
    if (acces === undefined) return undefined;
    const rafraichissement = (corps as { rafraichissement?: unknown }).rafraichissement;
    if (typeof rafraichissement !== 'string' || rafraichissement === '') return undefined;
    return { acces, rafraichissement };
}

export function jetonAcces(coffre: Coffre | undefined = coffreParDefaut()): string | undefined {
    return coffre?.getItem(CLE_ACCES) ?? undefined;
}

/* ── AUTOMATIC ACCESS — ADDED ON AUGUST 30TH, 2026, TO CLOSE AN
   INCOMPLETENESS FOUND IN PRODUCTION THAT MORNING ─────────────────────────

   The hub (`hub/page.ts`), served at the root since the day before, merely
   READ the vault and complained if it was empty ("No token:
   sign in first.", with nothing to do). The only code that knew how to
   obtain a token through Pomerium was `connexion.ts::tenterPomerium`, and it
   ran ONLY WHEN THE SIGN-IN PAGE LOADED. As long as the root
   served the session page, no one had seen a visitor land
   DIRECTLY on the hub without having gone through that screen: the batch that put the
   hub at the root had checked that `/` SERVES the hub, never that a visitor
   WITHOUT A TOKEN could use it — a check never seen turning red.

   The two functions below MOVE DOWN here, where they are tested, so
   that `connexion.ts` (which still calls Pomerium on load) AND
   `hub/page.ts` (which must call it ONLY if the vault is empty)
   SHARE them instead of copying it — the clause this fix imposes on itself. */

export interface ReponseAuthMoi {
    ok: boolean;
    json(): Promise<unknown>;
}
export type AppelAuthMoi = (url: string) => Promise<ReponseAuthMoi>;

/// Asks Pomerium for a fresh access token — the PATH
/// `tenterPomerium` (`connexion.ts`) had, EXTRACTED here as is (the same three
/// gestures: call, check `ok`, validate the body through `accesDeReponse`).
///
/// Returns `undefined` on any outcome that is NOT a usable token: an
/// unreachable network, an unreadable body, and — the case of the
/// `motdepasse` mode — a `404`, which `routes-identite.ts::servirIdentite` returns
/// ITSELF to carry the mode to the client (see the header of
/// `connexion.ts` around `tenterPomerium`). This function does NOT distinguish
/// these outcomes from one another: it is up to the CALLER to decide what to do with them
/// (redirect to the sign-in screen, for instance), never up to it to
/// choose in its place.
export async function accesParPomerium(
    base: string,
    appel: AppelAuthMoi,
): Promise<string | undefined> {
    try {
        const reponse = await appel(`${base}/auth/moi`);
        if (!reponse.ok) return undefined;
        return accesDeReponse(await reponse.json().catch(() => undefined));
    } catch {
        return undefined;
    }
}

/// The freshness margin: a token expiring in less than this is
/// treated as stale.
///
/// ⚠️ **UNCALIBRATED CONSTANT, AND DECLARED AS SUCH** — like the
/// forty others of this repository (`CLAUDE.md`, "no constant is
/// calibrated"). It is enough for a call sent with a valid token
/// not to arrive expired, without forcing a round trip at each gesture.
export const MARGE_FRAICHEUR_MS = 30_000;

/// Ensures a **USABLE** access token is available, obtaining it
/// if needed.
///
/// 🔴 **WHAT DISTINGUISHES IT FROM `assurerAcces`, WHICH IT REPLACES: it checks
/// whether the vault's token is STALE.** `assurerAcces` returned the vault's
/// content as soon as it was not empty — an expired token was therefore returned as
/// is, and each call then failed without anything linking the failure to
/// the expiry. It is the second half of the request of August 31st, 2026
/// ("if I go to hub.html, it validates and refreshes my connection").
///
/// Four steps, in this order, each attempted only if the previous one
/// fails:
///   ① the vault carries a token still fresh within `margeMs` → return it,
///      **without any network**: a round trip at each gesture would be a cost
///      for a case that does not need it;
///   ② `rafraichirSiNecessaire` — the path of the `motdepasse` mode;
///   ③ `accesParPomerium` → `GET /auth/moi` — the `pomerium` mode, the one of
///      production;
///   ④ `undefined`, **vault emptied**: up to the caller to send back to the sign-in
///      screen.
///
/// 🔴 **IT VERIFIES NO SIGNATURE**, and `expireAvant` already says so: the
/// browser does not have the secret. What is avoided here is a useless round trip
/// and an unexplained failure, never an authorisation decision —
/// that stays with the service, on each handshake.
export async function assurerAccesFrais(
    coffre: Coffre,
    base: string,
    appelAuthMoi: AppelAuthMoi,
    maintenant: number,
    appelRafraichissement: (
        corps: unknown,
    ) => Promise<{ acces: string; rafraichissement: string } | undefined>,
    margeMs: number = MARGE_FRAICHEUR_MS,
): Promise<string | undefined> {
    // ① et ② : `rafraichirSiNecessaire` porte DÉJÀ les deux, et il est testé.
    // Le réécrire ici en produirait une seconde version à tenir d'accord.
    if (await rafraichirSiNecessaire(coffre, maintenant, margeMs, appelRafraichissement)) {
        return jetonAcces(coffre);
    }
    // ⚠️ `rafraichirSiNecessaire` a VIDÉ le coffre en rendant `false` : il n'y
    // a plus rien à présenter, et c'est bien l'état voulu si ③ échoue aussi.
    const frais = await accesParPomerium(base, appelAuthMoi);
    if (frais === undefined) return undefined;
    poserAcces(coffre, frais);
    return frais;
}

export function jetonRafraichissement(
    coffre: Coffre | undefined = coffreParDefaut(),
): string | undefined {
    return coffre?.getItem(CLE_RAFRAICHISSEMENT) ?? undefined;
}

/// Dit si le jeton sera périmé à `instant`.
///
/// 🔴 LIT `exp` SANS VÉRIFIER LA SIGNATURE, ET C'EST DÉLIBÉRÉ : le navigateur
/// n'a pas le secret de signature et ne peut donc RIEN vérifier. Croire qu'il
/// vérifie serait pire que savoir qu'il ne le fait pas — la seule vérification
/// qui compte est celle du service (`identite/jeton.ts`), sur chaque poignée
/// de main. Ce que cette fonction sert, c'est à éviter un aller-retour inutile,
/// pas à décider d'une autorisation.
///
/// Un jeton ILLISIBLE est réputé périmé : le tenir pour valable ferait échouer
/// la session plus tard, ailleurs, sur un refus que rien ne relierait à ici.
export function expireAvant(jeton: string, instant: number): boolean {
    const morceaux = jeton.split('.');
    if (morceaux.length !== 3) return true;
    try {
        const charge: unknown = JSON.parse(
            decoderBase64url(morceaux[1]),
        );
        if (typeof charge !== 'object' || charge === null) return true;
        const exp = (charge as { exp?: unknown }).exp;
        if (typeof exp !== 'number' || !Number.isFinite(exp)) return true;
        return exp <= instant;
    } catch {
        return true;
    }
}

/// Décode un segment base64url en texte. Écrit à la main plutôt qu'avec
/// `Buffer` : ce module tourne dans un navigateur, où `Buffer` n'existe pas.
function decoderBase64url(segment: string): string {
    const base64 = segment.replace(/-/g, '+').replace(/_/g, '/');
    const complet = base64 + '='.repeat((4 - (base64.length % 4)) % 4);
    const binaire = atob(complet);
    // `atob` rend des unités de code latin-1 : les recomposer en UTF-8 avant
    // de parler de JSON, sans quoi un courriel accentué casserait la lecture.
    const octets = Uint8Array.from(binaire, (c) => c.charCodeAt(0));
    return new TextDecoder().decode(octets);
}

/// Rafraîchit la paire si le jeton d'accès expire dans moins de `margeMs`.
///
/// Rend `true` si, au retour, le coffre porte un jeton d'accès utilisable —
/// qu'il ait fallu appeler ou non. Rend `false` quand il n'y a plus rien à
/// présenter : le coffre est alors VIDÉ, pour que l'appelant renvoie vers
/// l'écran de connexion plutôt que de boucler sur un refus.
///
/// `appel` est INJECTÉ, jamais `fetch` global : c'est ce qui rend cette règle
/// éprouvable sans réseau.
///
/// 🔴 **`await appel(...)` EST ENVELOPPÉ — AJOUTÉ EN CORRECTION DE REVUE
/// (round 1), PAS AU PREMIER JET.** `accesParPomerium` (plus haut dans ce
/// fichier) a sa propre garde de ce genre DEPUIS TOUJOURS, avec son test
/// dédié (« rend `undefined` sur un réseau injoignable, sans lever ») — mais
/// cette fonction-ci n'appelait AUCUN `appel` de production avant
/// `assurerAccesFrais` (30-31 août 2026) : sans appelant réel, une exception
/// non rattrapée ici n'avait jamais eu l'occasion de se voir. Une panne
/// réseau (hors ligne, DNS, CORS) est donc traitée exactement comme un refus
/// (`!neuve`) : le coffre est vidé, l'appelant retombe sur Pomerium plutôt
/// que de voir l'exception remonter non gérée jusqu'à un `void demarrer()`
/// ou un `.then()` sans `.catch`.
export async function rafraichirSiNecessaire(
    coffre: Coffre,
    maintenant: number,
    margeMs: number,
    appel: (corps: unknown) => Promise<{ acces: string; rafraichissement: string } | undefined>,
): Promise<boolean> {
    const acces = jetonAcces(coffre);
    if (acces !== undefined && !expireAvant(acces, maintenant + margeMs)) return true;

    const rafraichissement = jetonRafraichissement(coffre);
    if (rafraichissement === undefined) {
        // Rien à présenter : on efface ce qui reste plutôt que de laisser un
        // accès périmé que la poignée de main refuserait.
        vider(coffre);
        return false;
    }

    let neuve: { acces: string; rafraichissement: string } | undefined;
    try {
        neuve = await appel({ rafraichissement });
    } catch {
        neuve = undefined;
    }
    if (!neuve) {
        vider(coffre);
        return false;
    }
    poser(coffre, neuve);
    return true;
}
