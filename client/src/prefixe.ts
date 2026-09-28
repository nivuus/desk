// The VM prefix, browser side: where it comes from, and how it composes a
// session name.
//
// 🔴 THIS MODULE IS PURE AND DOM-FREE, on the explicit precedent of
// `client/src/jeton.ts`: `client/` has no `vitest.config.*`, so
// the test environment is the default Node — there is neither `window` nor
// `localStorage`. The vault and the query string are PARAMETERS;
// `globalThis` is only touched as an argument's default, at call time,
// never at module load.
//
// ✅ THE DEFINITIVE SOURCE OF THE PREFIX IS THE PLATFORM, AND IT HAS EXISTED SINCE
// SUB-BLOCK P4. `client/src/connexion.ts` calls `POST /session`
// (`plateforme/src/http/routes-session.ts`) once the token is set, and writes
// here through `poserPrefixe`. P3 had turned the literal into a PARAMETER and
// left the source to P4; it is done, and this module only had to gain its two
// writers. `lirePrefixe` has not changed by a line.
//
// ⚠️ THE COST SPEC §10 NAMES — "a misconfigured platform silently falls back
// into a shared namespace" — IS REDUCED, NOT SETTLED, and
// one must say by what. What now keeps it from going unnoticed comes down to
// two guards, at both ends: `poserPrefixe` THROWS on the empty string rather
// than writing it to the vault, and the route returns 409 `aucune-vm` instead of a (policy: allow-fr, wire refusal code)
// 200 with an empty prefix. What remains: the prefix is PER VM and not per session,
// and `signaling/propriete.ts` stays in memory — two human clients of the
// same VM find the same prefix again after a service restart.

/// What READING needs, and nothing more.
export interface Coffre {
    getItem(cle: string): string | null;
}

/// What WRITING needs.
///
/// ⚠️ TWO INTERFACES RATHER THAN ONE, unlike `jeton.ts` which only has
/// one: widening `Coffre` would forbid passing `lirePrefixe` a read-only
/// view, without bringing it anything. `window.localStorage` satisfies
/// both, and it is the only production caller.
export interface CoffreEcrivable extends Coffre {
    setItem(cle: string, value: string): void;
    removeItem(cle: string): void;
}

export const CLE_PREFIXE = 'guac.prefixe';

/// The separator, as spec §3.4 writes it. Mirror of
/// `SEPARATEUR_PREFIXE` in `agent/src/superviseur/protocole.rs`: both
/// ends compose the SAME identifier, and a divergence would only show in a
/// real session.
export const SEPARATEUR = ':';

function coffreParDefaut(): Coffre | undefined {
    const global = globalThis as { localStorage?: Coffre };
    return global.localStorage;
}

function requeteParDefaut(): string {
    const global = globalThis as { location?: { search?: string } };
    return global.location?.search ?? '';
}

/// The VM's prefix: the vault's, otherwise the query string's,
/// otherwise the empty string.
///
/// 🔴 THE VAULT COMES BEFORE THE QUERY. In the other order, a `?prefixe=`
/// left in a bookmarked URL would overwrite at each reload the
/// prefix the platform set, and the page would open the sessions of
/// another VM.
///
/// 🔴 ABSENCE RETURNS THE EMPTY STRING, never `undefined` and never an
/// exception: the page must fall back to `bureau`, which spec §10 already
/// sets as a rule.
export function lirePrefixe(
    coffre: Coffre | undefined = coffreParDefaut(),
    requete: string = requeteParDefaut(),
): string {
    const duCoffre = coffre?.getItem(CLE_PREFIXE);
    if (duCoffre !== null && duCoffre !== undefined && duCoffre !== '') return duCoffre;
    return new URLSearchParams(requete).get('prefixe') ?? '';
}

/// Writes the prefix the platform has just delivered.
///
/// 🔴 THROWS ON THE EMPTY STRING, AND IT IS THE GUARD, NOT A COURTESY
/// CHECK. An empty prefix written to the vault would not be neutral there:
/// `lirePrefixe` treats it as an absence (l. `duCoffre !== ''`),
/// would fall back to `?prefixe=` then to `''`, and the page would SILENTLY join
/// the shared namespace — the exact silent failure
/// spec §10 names. A caller that has no prefix has none to write:
/// it calls `clearPrefix`.
///
/// ⚠️ IT THROWS RATHER THAN RETURNING A `boolean`: the only caller is
/// wiring (`connexion.ts`), and an ignored boolean would be indistinguishable there from a
/// success. It is the same argument `orchestration/refus.ts` makes in
/// the opposite direction — there an EXPECTED refusal is returned as a value, here a
/// WRONG programming is voiced as an exception.
export function poserPrefixe(coffre: CoffreEcrivable, prefixe: string): void {
    if (prefixe === '') {
        throw new Error(
            'empty prefix refused: writing it to the store would join ' +
                "the shared namespace without anything saying so (spec §10). " +
                'An absence of VM is written through effacerPrefixe.',
        );
    }
    coffre.setItem(CLE_PREFIXE, prefixe);
}

/// Removes the prefix. Called when the user has NO VM.
///
/// 🔴 NOT CALLING IT WOULD LEAVE THE PREFIX OF A VM ONE NO LONGER HAS, and the
/// page would open its sessions in the name of another machine — the vault coming
/// before the query string, nothing would correct it. It is also what gives
/// the local trial mode back its `?prefixe=`: as long as the vault carries something,
/// the query is of no use.
export function clearPrefix(coffre: CoffreEcrivable): void {
    coffre.removeItem(CLE_PREFIXE);
}

/// What to do with the prefix a VM has just announced.
///
/// 🔴 **THIS RULE EXISTS BECAUSE THE HUB SET NO PREFIX** (final
/// review of August 31st, 2026, critical ②). `poserPrefixe` had only ONE
/// production caller — `connexion.ts::fetchTheSession` —, which only runs
/// on the sign-in page. Yet a visitor behind Pomerium obtains their
/// token **on the hub** (`jeton.ts::assurerAccesFrais` → `/auth/moi`) without
/// ever going through that screen: `lirePrefixe()` then returned `''`, the hub
/// listened on the session `bureau` while the agent announced on
/// `<prefixe>:bureau`, and **no `fenetre-ouverte` ever arrived**. The
/// election lock, not prefixed either, made vacuous the protection
/// `bureau/porteur-dom.ts::nomDuVerrou` loudly claims to offer.
/// ⚠️ **The defect PREDATES the `navigation-hub-unique` workstream** — it dates from the
/// Pomerium fix of August 30th, 2026, and `shell-page.ts` suffered from it too.
/// It becomes critical because the hub became the ONLY surface.
///
/// 🔴 **A RULE, NOT WIRING**, by this repository's reproducible criterion: changing
/// it changes what the product DECIDES (which session it listens on), it does not
/// route a decision taken elsewhere.
///
/// ⚠️ **THE EMPTY STRING MEANS "ERASE", IT DOES NOT THROW.** `poserPrefixe`
/// throws on `''`, and that is right for IT: a caller that has no prefix
/// has none to write. But a service announcing `prefixe: ''` is not
/// a wrong programming of the client — it is a VM without a prefix, and the correct
/// gesture is to erase, never to make catalogue population throw.
/// That is what distinguishes this rule from the guard of `poserPrefixe`, and the
/// two are written side by side so that they are not confused.
export type ChoixPrefixe = { action: 'poser'; prefixe: string } | { action: 'effacer' };

export function prefixeDeLaVm(annonce: unknown): ChoixPrefixe {
    if (typeof annonce === 'string' && annonce !== '') return { action: 'poser', prefixe: annonce };
    return { action: 'effacer' };
}

/// Applies the choice above to the vault. **Wiring**, kept here so that
/// the two callers (`connexion.ts` one day, `hub/page.ts` today) do not
/// copy the `if`.
export function retenirLePrefixe(coffre: CoffreEcrivable, annonce: unknown): void {
    const choix = prefixeDeLaVm(annonce);
    if (choix.action === 'poser') poserPrefixe(coffre, choix.prefixe);
    else clearPrefix(coffre);
}

/// Composes a session identifier: `<prefix>:<name>`, or `<name>` alone
/// when no prefix is known.
export function composer(prefixe: string, nom: string): string {
    if (prefixe === '') return nom;
    return `${prefixe}${SEPARATEUR}${nom}`;
}
