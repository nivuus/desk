// Wiring of the login screen: a DOM form on one side,
// `POST /auth/connexion` on the other. No rule here — they are in
// `jeton.ts`, which is tested.
//
// ⚠️ THIS FILE IS NOT UNIT TESTED, and that is DECLARED rather than
// endured: it is the same convention as `hub/page.ts`, `hub/cartes.ts` and
// `main.ts`, which are not either. ⚠️ THIS LIST NAMED
// `shell-page.ts` until the final review of August 31st, 2026: since
// task 9, that file is only a sixteen-line redirect, and
// invoking it as the precedent for untested WIRING no longer says anything. What makes it tenable is the clause that comes with it:
// **any rule this file would carry must move down into `jeton.ts`**. If
// a condition appears here, it is in the wrong place.
//
// 🔴 THIS FILE ALSO REQUESTS ITS SESSION, AND THE TWO BRANCHES THAT FOLLOW
// ARE WIRING, NOT RULES — that is what allows them here despite the
// clause above. ⚠️ P4's PLAN CONTRADICTED ITSELF ON THIS POINT — its task
// 14 forbids any condition in this file, then prescribes its branches
// —, the implementer flagged it without deciding, and P4's CROSS-CUTTING REVIEW
// ARBITRATED IT HERE (August 20th, 2026): what the clause forbids is that a RULE
// lives in an untested file, not that an `if` appears there. The criterion that
// decides is REPRODUCIBLE: a condition is a rule if changing it
// changes what the PRODUCT decides; it is wiring if it only
// routes a decision already taken elsewhere, and tested there. The two branches
// below fall in the second case — they read a decision
// `routes-session.ts` took and that its tests cover. **The clause is therefore
// tightened, not loosened**, and the next `if` that appears here must
// pass this criterion or move down. The rules live at both ends, and both are
// tested: what a prefix is allowed to be is in `prefixe.ts`
// (`poserPrefixe` THROWS on the empty string), and what 200, 409 and 503 mean
// is in `plateforme/src/http/routes-session.ts`. What remains here only decides
// to WRITE, to ERASE, or to TOUCH NOTHING — and the third outcome
// is the reason there are only two branches:
//
//   ① the body carries a `prefixe` — 200 as well as 503 —: we write it. It is known
//     and correct in both cases, and the page needs it so as not to join
//     the shared namespace while waiting for the VM to come back;
//   ② the service says `aucune-vm`: we erase. Leaving in place the prefix of a (policy: allow-fr, wire refusal code)
//     VM we no longer have would open sessions in another machine's name;
//   ③ everything else — token refused, method, failure — says NOTHING about
//     the assignment: the vault is not touched. Erasing on a 401 would lose
//     a prefix that is still correct; it is an absence of branch, and it is
//     deliberate.
//
// ⚠️ WE ONLY REDIRECT ON 200, AND THE COST IS WRITTEN HERE RATHER THAN DISCOVERED:
// a developer without an enrolled VM STAYS on this screen. Redirecting to a shell
// that would join the shared namespace would be precisely what this
// sub-block exists to avoid. The local trial mode goes through `?prefixe=` on
// the shell's URL (`prefixe.ts`), and it still works — the vault is
// empty, so the query string takes over.
//
// ⚠️ THE FAILURE MESSAGE IS THE SERVICE'S, AS IS. It does not distinguish
// "unknown email" from "wrong password" (`plateforme/src/http/
// routes-auth.ts`: a message distinguishing them would be an account
// enumeration oracle). Enriching the text here would undo that property
// from the only place where nobody would think of looking for it.

import { accesParPomerium, poser, poserAcces } from './jeton';
import type { Ton } from './shell';
import { installThemeSelectorInDOM } from './design/selecteur-theme';
import { clearPrefix, poserPrefixe } from './prefixe';
import { adressePlateforme } from './adresse-plateforme';

const params = new URLSearchParams(window.location.search);
// The SAME convention as the signaling of `hub/page.ts` (`shell-page.ts` before
// the hub became the only surface, August 31st, 2026): a query
// parameter, otherwise THE PAGE'S ORIGIN. Inventing a second convention
// would require knowing which one applies where.
//
// 🔴 IT IS NO LONGER `http://<host>:8080`, AND THE CHANGE IS NOT COSMETIC.
// Behind the TLS proxy of `deploiement/nginx.conf`, the page is served over
// `https://` on 443: an `http://…:8080` there would be mixed content, refused
// by the browser, and nothing listens on 8080 from outside anyway.
// The rule lives in `adresse-plateforme.ts`, which is PURE and tested.
const plateformeUrl = adressePlateforme(window.location, params.get('plateforme'));
// Where to go once logged in. The parameter exists so that the screen
// can send back to the page that required login, and not only to
// the shell.
//
// 🔴 THE ROOT, NO LONGER `shell.html` (August 31st, 2026): the shell page became
// a redirect, and sending back there would make whoever has just logged in
// do a useless round trip.
const suite = params.get('suite') ?? '/';

const formulaire = document.querySelector<HTMLFormElement>('#connexion')!;
const champEmail = document.querySelector<HTMLInputElement>('#email')!;
const champMotDePasse = document.querySelector<HTMLInputElement>('#motdepasse')!;
const bouton = document.querySelector<HTMLButtonElement>('#valider')!;
const message = document.querySelector<HTMLDivElement>('#message')!;

// The theme selector — a reasoned extension of spec §5.2, justified in
// the header of `design/selecteur-theme.ts`.
installThemeSelectorInDOM(document.querySelector<HTMLElement>('#themes')!);

/* ── THE BANNER'S TONE: A TABLE, NOT A RULE ───────────────────────────────
   🔴 NO CONDITION IS ADDED TO THIS FILE, and that is the clause of its
   header. The branches below all existed BEFORE sub-block S3;
   it only gives each the tone class that matches it. The
   criterion of P4's cross-cutting review applies as is: a condition is
   a RULE if changing it changes what the product decides. Changing a tone
   changes no decision — neither the token set, nor the prefix written, nor the
   redirect. It is presentation.

   ⚠️ THESE THREE CLASSES ARE INVISIBLE TO CHECK §7.9, a known and
   declared limit: it only reads the literals of `classList.add('…')` and
   `className = '…'`, never a class that goes through a variable. They
   are indeed declared by `design/primitives/message.css` and used by
   `primitives.html` — it is the gallery and the eye that say so here, not the
   command. Same arbitration as `bureau/porteur-dom.ts`, which carries
   this wiring today. ⚠️ THIS SENTENCE NAMED `shell-page.ts` until
   the final review of August 31st, 2026: that file has carried NO
   `CLASSE_DE_TON` since task 9, and the arbitration attributed to it has
   moved with the rest. */
const CLASSE_DE_TON: Record<Ton, string> = {
    neutre: '',
    succes: 'message--succes',
    alerte: 'message--alerte',
    danger: 'message--danger',
};

function show(texte: string, ton: Ton): void {
    message.textContent = texte;
    message.classList.remove('message--succes', 'message--alerte', 'message--danger');
    const classe = CLASSE_DE_TON[ton];
    if (classe !== '') message.classList.add(classe);
}

/// What follows obtaining a token, whatever the path that obtained it.
///
/// ⚠️ IT IS NOT A RULE, IT IS WIRING — in the sense of the criterion set at
/// the head of this file: these branches only route a decision taken by
/// `routes-session.ts` and covered by ITS tests.
///
/// 🔴 THIS COMMENT WROTE "THE CLAUSE THEREFORE STAYS TIGHTENED, NOT
/// LOOSENED", AND IT WAS A FALSE COMPLETENESS CLAIM — corrected
/// rather than erased (cross-cutting review, August 21st, 2026). The sentence was true
/// of the branches MOVED into this function, and false of the NEW
/// function written just below: `tenterPomerium` had added there, in the
/// same commit, a guard on `corps.acces` which, for its part, was a RULE in the sense
/// of the criterion. The branch therefore loosened the clause in the very gesture where
/// it claimed to tighten it. **The rule has since moved down into
/// `jeton.ts` (`accesDeReponse`), where tests hold it**; what remains
/// here, and below, is wiring. This paragraph no longer says anything about what
/// this file will contain tomorrow: the criterion, for its part, stays the only thing to
/// apply to the next `if` that appears here.
///
/// ⚠️ BODY MOVED VERBATIM. Three substitutions, and ONLY THREE:
///   ① `corps.acces` becomes the parameter `acces`;
///   ② the early-exit `return`s stay `return`s — the function
///      returns `void`, so their meaning does not change;
///   ③ the `catch` and `finally` of the `submit` STAY with the caller:
///      moving them here would re-enable `bouton.disabled = false` on the
///      Pomerium path, where no button was ever disabled.
async function fetchTheSession(acces: string): Promise<void> {
    show('looking for your machine…', 'neutre');
    const session = await fetch(`${plateformeUrl}/session`, {
        method: 'POST',
        // 🔴 THE `Authorization` HEADER MAKES THE REQUEST NON-SIMPLE, hence
        // subject to an `OPTIONS` preflight request. It is the defect task 8's
        // acceptance run found and closed on the service side; it is
        // recalled here because no test in this directory can see it.
        headers: { authorization: `Bearer ${acces}` },
    });
    const sien = await session.json().catch(() => undefined);

    // ① A prefix is a prefix, whether it comes from a 200 or a 503.
    if (typeof sien?.prefixe === 'string') {
        // ⚠️ THIS `poserPrefixe` CAN THROW, and that is intended: it only does so
        // on an empty string, that is, on a service that would have
        // delivered a prefix that is not one. The exception then crosses
        // to the caller (`submit`), whose network `catch` catches the
        // message that QUOTES the cause in full — the word "unreachable" is
        // then imprecise, the sentence it frames is not. Declared
        // rather than discovered.
        poserPrefixe(window.localStorage, sien.prefixe);
    } else if (sien?.motif === 'aucune-vm') {
        // ② No VM: the vault is cleaned, otherwise yesterday's prefix
        // would outlive the assignment just lost.
        //
        // 🔴 THIS LITERAL IS A COPY, AND NOTHING CONFRONTS IT WITH ITS
        // SOURCE (noted at P4's cross-cutting review, not fixed). Its
        // canonical source is `MOTIFS` in
        // `plateforme/src/orchestration/refus.ts`, an `as const` array
        // whose type DERIVES, precisely so that adding a reason without
        // giving it its HTTP code is a compile error. That
        // property stops at the package boundary: `client/` cannot
        // import from `plateforme/`, and the only shared package is
        // `proto/`, which P4 forbids itself to touch (its version belongs to
        // sub-block G1). CONSEQUENCE TO KNOW: renaming `aucune-vm` (policy: allow-fr, wire refusal code)
        // on the service side would leave this test always false, hence the stale
        // prefix in the vault — a SILENT failure, which neither `npm run typecheck`
        // nor any test of this repository would see. The remedy is to move
        // `MOTIFS` down into `proto/ts`; it is LEFT AS LEGACY, not done.
        clearPrefix(window.localStorage);
    }

    if (!session.ok) {
        // The service's reason, as is — and for `agent-injoignable`, what
        // the service ADMITS it cannot do. The framing promises "VM
        // unreachable -> the hub says so, offers a restart"; with the
        // static backend the hub says so, and says it cannot
        // restart. Keeping that admission quiet would make one wait for a button that does not
        // exist.
        const etat = sien?.etat ? ` (state: ${sien.etat})` : '';
        const aveu =
            sien?.redemarrage?.possible === false
                ? ` — the platform cannot restart it (${sien.redemarrage.motif}, backend ${sien.redemarrage.backend})`
                : '';
        show(`${sien?.motif ?? sien?.refus ?? session.status}${etat}${aveu}`, 'danger');
        return;
    }

    window.location.href = suite;
}

/// Asks the service for the identity BEFORE showing the form.
///
/// 🔴 IT IS THE 404 THAT CARRIES THE MODE UP TO HERE, and that is why this page
/// has no mode variable to know. It is built statically by
/// Vite and cannot read any server configuration: it ASKS. A
/// 404 means "this deployment authenticates by password"; a 200, "the
/// proxy has already identified me".
///
/// 🔴 THIS PROMISE DIED SILENTLY, AND IT IS REPAIRED ON THE SERVICE SIDE,
/// NOT HERE (August 22nd, 2026). The platform's static file server,
/// chained last, folds any path without an extension onto `index.html`:
/// with `PLATEFORME_PAGE` armed, `/auth/moi` returned `200 text/html` in
/// `motdepasse` mode — hence "the proxy has already identified me", which is WRONG. This
/// page only broke by ACCIDENT: the `.catch(() => undefined)` of
/// `reponse.json()` made `accesDeReponse` fall back to `undefined`, hence the
/// form, in the right place for a wrong reason. The mode guard of
/// `plateforme/src/http/routes-identite.ts` now returns the `404`
/// ITSELF; nothing changes here.
///
/// ⚠️ ANY FAILURE FALLS BACK TO THE FORM, including a network failure. It is
/// the least surprising fallback: the user sees a screen they
/// can act on, rather than an empty page with nothing saying what it expects.
///
/// 🔴 THIS FUNCTION'S BODY MOVED DOWN INTO `jeton.ts::accesParPomerium`
/// ON AUGUST 30th, 2026 — the function, NOT the decision surrounding it. The hub
/// (`hub/page.ts`) had the SAME need (obtaining a token through Pomerium) without
/// being able to run unconditionally at load like this page
/// (it must call the network ONLY if the vault is empty): copying this
/// block would have let two copies drift, exactly the pattern
/// `CLAUDE.md` forbids. What stays HERE — calling, setting, chaining on to
/// `fetchTheSession` — is wiring specific to THIS page; validating
/// the body (`accesDeReponse`, in `jeton.ts` since August 21st, 2026) and
/// now the network call itself are shared, tested there.
async function tenterPomerium(): Promise<boolean> {
    const acces = await accesParPomerium(plateformeUrl, window.fetch.bind(window));
    if (acces === undefined) return false;
    poserAcces(window.localStorage, acces);
    await fetchTheSession(acces);
    return true;
}

formulaire.addEventListener('submit', async (evenement) => {
    evenement.preventDefault();
    bouton.disabled = true;
    show('Connecting…', 'neutre');

    try {
        const reponse = await fetch(`${plateformeUrl}/auth/connexion`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({
                email: champEmail.value,
                motdepasse: champMotDePasse.value,
            }),
        });
        const corps = await reponse.json().catch(() => undefined);

        if (!reponse.ok) {
            // The service's reason, as is — see the header.
            show(`refused: ${corps?.refus ?? reponse.status}`, 'danger');
            return;
        }

        poser(window.localStorage, {
            acces: corps.acces,
            rafraichissement: corps.rafraichissement,
        });
        // The password does not survive login: the field is emptied
        // before leaving the page, so that the browser's back button does not
        // find it filled in.
        champMotDePasse.value = '';

        await fetchTheSession(corps.acces);
    } catch (cause) {
        // A NETWORK failure is stated as such: on another origin, it is the
        // symptom of a `PLATEFORME_ORIGINE_CLIENT` missing on the service side
        // (`plateforme/src/config.ts`), and confusing it with a credentials
        // refusal would send one looking for the defect in the wrong place.
        show(`platform unreachable (${String(cause)})`, 'danger');
    } finally {
        bouton.disabled = false;
    }
});

// ⚠️ The form is HIDDEN for the duration of the attempt, then shown again if it
// fails: showing it first would flash a login screen on a
// deployment that requires none.
formulaire.hidden = true;
show('identification…', 'neutre');
void tenterPomerium().then((abouti) => {
    if (abouti) return;
    formulaire.hidden = false;
    show('', 'neutre');
});
