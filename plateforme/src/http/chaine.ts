// The chain of HTTP routers, extracted from `serveur.ts` on 22 August 2026.
//
// 🔴 WHY THIS FILE EXISTS: `serveur.ts` was at 475 lines out of 500 at
// the time the "page behind Pomerium" batch had to add a TENTH
// router to it.
// ⚠️ THIS SENTENCE SAID "ELEVENTH" UNTIL THE FINAL REVIEW, here and in
// `serveur.ts`: the count was wrong on both sides, and nobody had
// rerun it. Here it is, with its command — the only thing that is authoritative:
//   grep -cE '^    (if \(await servir|return servir)' plateforme/src/http/chaine.ts (policy: allow-fr - file name)
//     -> 10
//
// `CLAUDE.md` prescribes the extraction as a DEDICATED task, BEFORE the one that adds —
// "extract, never compress" — because the margin regained by an
// extraction gets lost again if it is treated as a given (paid for six times).
//
// ⚠️ THIS EXTRACTION CHANGES NO BEHAVIOUR. The order of the routers, the
// comments that explain it and the final `false` are taken over VERBATIM. The
// only change is that `deps` arrives as a parameter instead of being captured
// by a closure.

import type { IncomingMessage, ServerResponse } from 'node:http';
import { servirAuth, type DependancesAuth } from './routes-auth';
import { servirIdentite, type DependancesIdentite } from './routes-identite';
import { servirVm, type DependancesVm } from './routes-vm';
import { servirSession, type DependancesSession } from './routes-session';
import { servirApplications, type DependancesApplications } from './routes-applications';
import { servirIcone, type DependancesIcone } from './routes-icone';
import { servirTeleversement, type DependancesTeleversement } from './routes-televersement';
import { servirInstallation, type DependancesInstallation } from './routes-installation';
import { servirSante, type DependancesSante } from './routes-sante';
import { servirPage, type DependancesPage } from './page/routes-page';

/// Everything the chain consumes, gathered.
///
/// ⚠️ AN INTERSECTION, NOT AN INTERFACE THAT EXTENDS: two `Dependances*` that
/// declared the same key with incompatible types would make an
/// `extends` FAIL at declaration, whereas the intersection lets the conflict
/// show up at the CALL, on the real object — that is, where it gets fixed.
export type DependancesRoutage = DependancesIdentite &
    DependancesAuth &
    DependancesVm &
    DependancesApplications &
    DependancesIcone &
    DependancesTeleversement &
    DependancesInstallation &
    DependancesSession &
    DependancesSante &
    DependancesPage;

/// Tries the routers in order, and returns `false` if none has served.
///
/// 🔴 THE ORDER IS SIGNIFICANT, AND SINCE 22 AUGUST 2026 IT IS BINDING.
/// The original sentence — "not binding here: the FOUR sets of paths
/// are DISJOINT" — was wrong twice, and contradicted another one
/// of the same file fifty lines further down ("THE SERVER IS CHAINED
/// LAST, AND IT IS THE GUARANTEE, NOT A CONVENIENCE"). The count, rerun:
///   grep -cE '^    (if \(await servir|return servir)' plateforme/src/http/chaine.ts (policy: allow-fr - file name)
///     -> 10
/// **TEN routers, NINE of which have DISJOINT sets of paths** — each one compares
/// exactly, or splits by SEGMENTS and compares their NUMBER, never by
/// `startsWith`. **THE TENTH, THE PAGE SERVER, IS NOT DISJOINT FROM THE
/// OTHERS: it resolves ANY path**, its SPA fallback folding every
/// path without an extension onto the page (`hub.html` since 30 August 2026,
/// `index.html` before — see `page/resolution.ts::PAGE`). Its position is
/// therefore not a convenience but a guarantee — see its line, at the end of the
/// function.
///
/// 🔴 WHAT THIS TENTH ROUTER CHANGES FOR ALL THE OTHERS, AND THAT NONE
/// OF THEM HAD WRITTEN: when `PLATEFORME_PAGE` is armed, a `false` returned
/// by a router NO LONGER NECESSARILY LANDS ON THE GENERIC 404. On a
/// `GET`/`HEAD`, the server can answer `200 text/html` in its place; outside
/// `GET`/`HEAD` it steps aside, and the generic 404 takes over as
/// before. Several route modules carry the sentence "the generic 404
/// then answers alone": it dates from before this server, and each one now
/// points here.
///
/// 🔴 THE APPLICATIONS ROUTER IS CHAINED BEFORE THE 404, AND IT IS THE
/// ONLY LINE THAT KEEPS IT ALIVE. Without it, its two routes would fall into
/// the fallback above — that is, the most discreet failure possible: the
/// service answers, listens, and serves the NINE others. `serveur.test.ts`
/// holds it through a dedicated test, as it already holds the `/agent` channel.
///
/// ⚠️ THE BODY OF THE 404 IS NOT TOUCHED: "nothing tested it before P2, and
/// changing it would be an undeclared side effect". It now lives in
/// `./introuvable.ts`, from where the two mode guards return it themselves.
export async function servirTout(
    requete: IncomingMessage,
    reponse: ServerResponse,
    deps: DependancesRoutage,
): Promise<boolean> {
    // 🔴 `servirIdentite` IS CHAINED FIRST, AND THAT IS NOT IRRELEVANT:
    // `/auth/moi` and the two paths of `servirAuth` are DISJOINT
    // today, but all three share the `/auth/` prefix. The day
    // one of them compared by prefix, it is this order that would decide —
    // silently.
    if (await servirIdentite(requete, reponse, deps)) return true;
    if (await servirAuth(requete, reponse, deps)) return true;
    if (await servirVm(requete, reponse, deps)) return true;
    if (await servirApplications(requete, reponse, deps)) return true;
    if (await servirIcone(requete, reponse, deps)) return true;
    // 🔴 THE TWO G3 ROUTERS, AND THESE ARE THE ONLY LINES THAT KEEP THEM
    // ALIVE. Without them, their SEVEN routes would fall into the fallback: the
    // most discreet failure there is, since the service answers, listens, and
    // correctly serves the EIGHT other routers.
    // ⚠️ "THE SIX OTHERS" WAS THE WORD, AND IT AGED SILENTLY — two
    // routers have been chained since. The two counts of this sentence,
    // remeasured on 22 August 2026: SEVEN routes (the four listed at the top of
    // `routes-televersement.ts`, the three at the top of `routes-installation.ts`),
    // and 10 - 2 = EIGHT other routers.
    // RED PLAYED: removing the first one makes test (6ter) of
    // `entetes-routeurs.test.ts` fail, and IT ALONE — `1 failed | 10 passed`.
    //
    // ⚠️ The two share the `/televersement/` prefix: the first
    // serves `…/tranche/:n`, `…/sceller` and the state, the second `…/contenu`
    // only. The sets stay DISJOINT — each one splits by SEGMENTS and
    // compares their NUMBER exactly, never by `startsWith` —, so neither
    // can steal the path of the other. The order is a belt, not the
    // guarantee.
    if (await servirTeleversement(requete, reponse, deps)) return true;
    if (await servirInstallation(requete, reponse, deps)) return true;
    if (await servirSession(requete, reponse, deps)) return true;
    // ⚠️ `/sante` IS CHAINED LATE, and the order is not irrelevant here: putting
    // it first would run its path comparison before those of the
    // guarded routes. Its set of paths stays DISJOINT from those of the eight
    // routers that precede it, so none can steal the path of
    // another; the order is a belt, not a guarantee.
    //
    // 🔴 THREE CLAIMS OF THIS SENTENCE DIED IN THIS VERY BATCH, AND
    // NO PER-TASK REVIEW COULD SEE IT — the extraction moved these
    // lines VERBATIM, correctly, and the NEXT task chained the server
    // AFTER. Each half was right.
    //   ① "CHAINED LAST": it no longer is, the page server
    //      follows it. It is `servirPage` that is last, one line further down.
    //   ② "THE ONLY UNAUTHENTICATED ROUTE": the server is a SECOND one
    //      — it consults no token. The property that remains true of
    //      `/sante` is being the only unauthenticated route that TOUCHES THE
    //      DATABASE, which is exactly what its cache exists to bound
    //      (see `routes-sante.ts`, where the same sentence is corrected).
    //   ③ "THE SIX SETS": there are TEN, nine of them disjoint — see the
    //      count and its command at the top of this function.
    if (await servirSante(requete, reponse, deps)) return true;
    // 🔴 THE PAGE SERVER IS CHAINED LAST, AND IT IS THE GUARANTEE, NOT
    // A CONVENIENCE. It is the ONLY one of the ten whose set of paths is not
    // disjoint from that of the others: it resolves any path. Chained
    // first, a file named `sante` or `vm` dropped in the root would steal
    // the path of an API router, and the service would answer 200 with a
    // plausible body. Chained here, an API router has already returned `true`: it cannot
    // be supplanted. `routes-page.test.ts` holds this line through a
    // dedicated test.
    //
    // ⚠️ THIS POSITION IS NOT ENOUGH FOR EVERYTHING, AND THE BATCH PAID FOR IT: a router
    // that returns `false` WHILE THE PATH IS ITS OWN still gets
    // supplanted — that is what killed the mode-carrying `404` of the two
    // authentication guards. The remedy is not here, it is on their side: they
    // now answer their `404` themselves (`./introuvable.ts`).
    return servirPage(requete, reponse, deps);
}
