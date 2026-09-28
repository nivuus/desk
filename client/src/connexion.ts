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
//   ② the service says `aucune-vm`: we erase. Leaving in place the prefix of a
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
import { installerSelecteurDeThemeAuDOM } from './design/selecteur-theme';
import { effacerPrefixe, poserPrefixe } from './prefixe';
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
installerSelecteurDeThemeAuDOM(document.querySelector<HTMLElement>('#themes')!);

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

function afficher(texte: string, ton: Ton): void {
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
async function chercherLaSession(acces: string): Promise<void> {
    afficher('recherche de votre machine…', 'neutre');
    const session = await fetch(`${plateformeUrl}/session`, {
        method: 'POST',
        // 🔴 L'EN-TÊTE `Authorization` REND LA REQUÊTE NON SIMPLE, donc
        // soumise à une requête préalable `OPTIONS`. C'est le défaut que la
        // recette de la tâche 8 a trouvé et fermé côté service ; il est
        // rappelé ici parce qu'aucun test de ce répertoire ne peut le voir.
        headers: { authorization: `Bearer ${acces}` },
    });
    const sien = await session.json().catch(() => undefined);

    // ① Un préfixe est un préfixe, qu'il vienne d'un 200 ou d'un 503.
    if (typeof sien?.prefixe === 'string') {
        // ⚠️ CE `poserPrefixe` PEUT LEVER, et c'est voulu : il ne le fait que
        // sur une chaîne vide, c'est-à-dire sur un service qui aurait
        // délivré un préfixe qui n'en est pas un. L'exception traverse alors
        // vers l'appelant (`submit`), dont le `catch` réseau attrape le
        // message qui CITE la cause en entier — le mot « injoignable » est
        // alors imprécis, la phrase qu'il encadre ne l'est pas. Déclaré
        // plutôt que découvert.
        poserPrefixe(window.localStorage, sien.prefixe);
    } else if (sien?.motif === 'aucune-vm') {
        // ② Aucune VM : le coffre est nettoyé, sans quoi le préfixe d'hier
        // survivrait à l'attribution qu'on vient de perdre.
        //
        // 🔴 CE LITTÉRAL EST UNE COPIE, ET RIEN NE LA CONFRONTE À SA
        // SOURCE (relevé à la revue transverse de P4, non corrigé). Sa
        // source canonique est `MOTIFS` dans
        // `plateforme/src/orchestration/refus.ts`, un tableau `as const`
        // dont le type DÉRIVE, précisément pour qu'ajouter un motif sans
        // lui donner son code HTTP soit une erreur de compilation. Cette
        // propriété s'arrête à la frontière du paquet : `client/` ne peut
        // pas importer de `plateforme/`, et le seul paquet partagé est
        // `proto/`, que P4 s'interdit de toucher (sa version appartient au
        // sous-bloc G1). CONSÉQUENCE À CONNAÎTRE : renommer `aucune-vm`
        // côté service laisserait ce test toujours faux, donc le préfixe
        // périmé au coffre — une panne MUETTE, que ni `npm run typecheck`
        // ni aucun test de ce dépôt ne verrait. Le remède est de faire
        // descendre `MOTIFS` dans `proto/ts` ; il est LÉGUÉ, pas fait.
        effacerPrefixe(window.localStorage);
    }

    if (!session.ok) {
        // Le motif du service, tel quel — et pour `agent-injoignable`, ce
        // que le service AVOUE ne pas savoir faire. Le cadrage promet « VM
        // injoignable -> le hub l'indique, propose redémarrage » ; avec le
        // backend statique le hub indique, et dit qu'il ne sait pas
        // redémarrer. Taire cet aveu ferait attendre un bouton qui n'existe
        // pas.
        const etat = sien?.etat ? ` (état : ${sien.etat})` : '';
        const aveu =
            sien?.redemarrage?.possible === false
                ? ` — la plateforme ne sait pas la redémarrer (${sien.redemarrage.motif}, backend ${sien.redemarrage.backend})`
                : '';
        afficher(`${sien?.motif ?? sien?.refus ?? session.status}${etat}${aveu}`, 'danger');
        return;
    }

    window.location.href = suite;
}

/// Demande l'identité au service AVANT de montrer le formulaire.
///
/// 🔴 C'EST LE 404 QUI PORTE LE MODE JUSQU'ICI, et c'est pourquoi cette page
/// n'a aucune variable de mode à connaître. Elle est bâtie statiquement par
/// Vite et ne peut lire aucune configuration du serveur : elle DEMANDE. Un
/// 404 signifie « ce montage authentifie par mot de passe » ; un 200, « le
/// proxy m'a déjà identifié ».
///
/// 🔴 CETTE PROMESSE A ÉTÉ MORTE SANS BRUIT, ET ELLE EST RÉPARÉE CÔTÉ SERVICE,
/// PAS ICI (22 août 2026). Le servant de fichiers statiques de la plateforme,
/// chaîné en dernier, replie tout chemin sans extension sur `index.html` :
/// avec `PLATEFORME_PAGE` armée, `/auth/moi` rendait `200 text/html` en mode
/// `motdepasse` — donc « le proxy m'a déjà identifié », ce qui est FAUX. Cette
/// page ne cassait que par ACCIDENT : le `.catch(() => undefined)` de
/// `reponse.json()` faisait retomber `accesDeReponse` sur `undefined`, donc le
/// formulaire, au bon endroit pour une mauvaise raison. La garde de mode de
/// `plateforme/src/http/routes-identite.ts` rend désormais le `404`
/// ELLE-MÊME ; rien ne change ici.
///
/// ⚠️ TOUT ÉCHEC RETOMBE SUR LE FORMULAIRE, y compris un échec réseau. C'est
/// le repli le moins surprenant : l'utilisateur voit un écran sur lequel il
/// peut agir, plutôt qu'une page vide dont rien ne dit ce qu'elle attend.
///
/// 🔴 LE CORPS DE CETTE FONCTION A DESCENDU DANS `jeton.ts::accesParPomerium`
/// LE 30 AOÛT 2026 — la fonction, PAS la décision qui l'entoure. Le hub
/// (`hub/page.ts`) avait le MÊME besoin (obtenir un jeton par Pomerium) sans
/// pouvoir courir au chargement inconditionnellement comme cette page-ci
/// (lui ne doit appeler le réseau QUE si le coffre est vide) : recopier ce
/// bloc aurait laissé deux copies dériver, exactement le patron que
/// `CLAUDE.md` interdit. Ce qui reste ICI — appeler, poser, enchaîner sur
/// `chercherLaSession` — est du câblage propre à CETTE page ; la validation
/// du corps (`accesDeReponse`, dans `jeton.ts` depuis le 21 août 2026) et
/// désormais l'appel réseau lui-même sont partagés, testés là-bas.
async function tenterPomerium(): Promise<boolean> {
    const acces = await accesParPomerium(plateformeUrl, window.fetch.bind(window));
    if (acces === undefined) return false;
    poserAcces(window.localStorage, acces);
    await chercherLaSession(acces);
    return true;
}

formulaire.addEventListener('submit', async (evenement) => {
    evenement.preventDefault();
    bouton.disabled = true;
    afficher('connexion…', 'neutre');

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
            // Le motif du service, tel quel — voir l'en-tête.
            afficher(`refusé : ${corps?.refus ?? reponse.status}`, 'danger');
            return;
        }

        poser(window.localStorage, {
            acces: corps.acces,
            rafraichissement: corps.rafraichissement,
        });
        // Le mot de passe ne survit pas à la connexion : le champ est vidé
        // avant de quitter la page, pour qu'un retour arrière du navigateur ne
        // le retrouve pas rempli.
        champMotDePasse.value = '';

        await chercherLaSession(corps.acces);
    } catch (cause) {
        // Un échec RÉSEAU se dit comme tel : sur une autre origine, c'est le
        // symptôme d'une `PLATEFORME_ORIGINE_CLIENT` absente côté service
        // (`plateforme/src/config.ts`), et le confondre avec un refus
        // d'identifiants enverrait chercher le défaut au mauvais endroit.
        afficher(`plateforme injoignable (${String(cause)})`, 'danger');
    } finally {
        bouton.disabled = false;
    }
});

// ⚠️ Le formulaire est CACHÉ le temps de la tentative, puis remontré si elle
// échoue : l'afficher d'abord ferait clignoter un écran de connexion sur un
// montage qui n'en demande aucun.
formulaire.hidden = true;
afficher('identification…', 'neutre');
void tenterPomerium().then((abouti) => {
    if (abouti) return;
    formulaire.hidden = false;
    afficher('', 'neutre');
});
