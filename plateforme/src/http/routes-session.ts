// `POST /session`: the user asks to open a session on THEIR VM.
//
// 🔴 WHAT THIS ROUTE RETURNS, AND WHAT IT DOES NOT. The spec §4 « P4 »
// writes that it returns « the session id, the prefix AND the
// ICE configuration ». It returns `{ vm, nom, prefixe, etat }`, and nothing
// else (divergence E4, decision D7). Two reasons, the first of which is
// decisive:
//
// ① THE ICE CONFIGURATION IS PER SESSION, and an HTTP route would only know
//    one of N. `signaling/ice.ts` builds its TURN identifier from the
//    SESSION NAME and signs the whole; yet a VM opens `<prefix>:bureau`
//    PLUS one session per window (`<prefix>:w-1`, `w-2`, …). The route
//    could only serve the control session, and the relay would keep
//    serving all the others. A second delivery path that covers one
//    session out of N is not a simplification: it is a second place to
//    keep in sync, which one has no right to rely on.
//
// ② THE COMPOSED SESSION NAME WOULD BE A THIRD COPY OF `bureau`. The
//    constant already lives in Rust (`agent/src/superviseur/protocole.rs`,
//    `NOM_SESSION_DE_CONTROLE`) and, on the TypeScript side, as an inline literal in
//    `client/src/bureau/porteur-dom.ts` (`composer(deps.prefixe, 'bureau')` —
//    a named `NOM_SESSION_DE_CONTROLE` lived in `client/src/shell-page.ts`
//    before task 9 inlined it, 31 August 2026), and the spec §2.6 already names
//    this duplication as a known defect.
//
// ⚠️ NOR DOES IT RETURN `adresse`: that is internal topology which
// the browser has no use for — it talks to the signaling, never to the VM.
//
// ⚠️ NO CLIENT CODE IS DEPRIVED OF ANYTHING by this decision, and
// that is what makes it free: `client/src/prefixe.ts::composer` builds
// `<prefix>:bureau`, and `client/src/webrtc.ts` receives its `ice-config` from the
// relay as today.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { inventaireStatique } from '../orchestration/inventaire-statique';
import { BACKEND_STATIQUE, CODE_HTTP } from '../orchestration/refus';
import { laVmDe } from '../orchestration/selection';
import { adresseSource } from './adresse-source';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { ligne } from '../obs/journal';
import { lirePorteur } from './porteur';
import { BUDGET_REQUETES, cleRequetes, type Budget, type Frein } from '../securite/frein';

export interface DependancesSession {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    /// 🔴 THE CLOCK COMES FROM HERE, never `Date.now()` read in this module: that is
    /// what makes the transition of criterion ④ observable within a
    /// test run, where there would otherwise be only one instant.
    maintenant: () => number;
    /// 🔴 THE « ANY REQUEST » BRAKE, SHARED with `routes-vm.ts` AND
    /// `signaling/relais.ts` — see `securite/frein.ts::BUDGET_REQUETES`.
    /// This route has no notion of failure: its abuse is a VOLUME,
    /// never a series of failed attempts.
    frein: Frein;
    /// The proxies whose `X-Forwarded-For` header is trusted — same set
    /// as `routes-auth.ts` and `routes-vm.ts`, never a second one.
    proxyDeConfiance: ReadonlySet<string>;
}

const CHEMIN = '/session';

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ UNCONDITIONAL, and set on EVERY response — including the
        // ERROR responses (401, 405, 413, 429, 500, 503), which
        // often carry more information than a normal response. They are spread
        // BEFORE `cors` so that the origin policy, which is optional,
        // can never overwrite them by mistake.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(JSON.stringify(corps));
}

/// Records the request on the « any request » budget, and logs IF AND
/// ONLY IF the brake has just bitten — same rule and same reason as
/// `routes-auth.ts::compterLEchec` and `routes-vm.ts::compterLaRequete`: the
/// next request will be refused at the very top of `servirSession`, before
/// ever calling this function again.
function compterLaRequete(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
    instant: number,
): void {
    frein.echec(cles, instant);
    const apres = frein.consulter(cles, instant);
    if (!apres.freine) return;
    console.warn(
        ligne('frein-requetes', {
            route: CHEMIN,
            adresse,
            retry_apres_s: apres.retryApresS,
            entrees: frein.size(),
            evictions: frein.evictions(),
        }),
    );
}

export async function servirSession(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesSession,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    // EXACT comparison, never a `startsWith`.
    if (chemin !== CHEMIN) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    // The preflight request: see the twin comment of `routes-vm.ts`.
    // Without it, the route is unreachable from a browser, the
    // `Authorization` header making the request not simple.
    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }
    if (req.method !== 'POST') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    // 🔴 THE « ANY REQUEST » BRAKE IS CONSULTED HERE — BEFORE `lirePorteur` and
    // before any database access. Same position and same reason as
    // `routes-vm.ts`: counting after the work being bounded does not bound it.
    const adresseRequete = adresseSource(
        req.socket.remoteAddress,
        Array.isArray(req.headers['x-forwarded-for'])
            ? req.headers['x-forwarded-for'].join(',')
            : req.headers['x-forwarded-for'],
        deps.proxyDeConfiance,
    );
    // 🔴 A MISCONFIGURED `PLATEFORME_PROXY_DE_CONFIANCE` MAKES THIS
    // BRAKE DEGENERATE INTO A GLOBAL BRAKE, AND ITS SEVERITY CHANGED WITH THIS BATCH — see the
    // full paragraph at `routes-vm.ts` (same position, same key
    // `BUDGET_REQUETES`, same witness: the `frein-requetes` line that names
    // the retained address), never copied so as not to drift.
    const clesRequetes: readonly (readonly [string, Budget])[] = [
        [cleRequetes(adresseRequete), BUDGET_REQUETES],
    ];
    const verdictRequetes = deps.frein.consulter(clesRequetes, deps.maintenant());
    if (verdictRequetes.freine) {
        rep.setHeader('Retry-After', String(verdictRequetes.retryApresS));
        repondre(rep, 429, { refus: 'trop-de-requetes' }, cors);
        return true;
    }
    compterLaRequete(deps.frein, clesRequetes, adresseRequete, deps.maintenant());

    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) {
        repondre(rep, porteur.code, { refus: porteur.motif }, cors);
        return true;
    }

    // 🔴 THE REQUEST BODY IS NOT READ, AND THERE IS NOTHING TO PUT IN IT:
    // the partial index `vm_un_utilisateur` guarantees zero or one VM per (policy: allow-fr - frozen wire key or SQLite column)
    // user, so there is no VM to designate. That is what spares
    // this route the 4 KiB bound of `routes-auth.ts` — ⚠️ AND THE DAY
    // A BODY BECOMES NECESSARY, THE BOUND WILL TOO. Without a bound,
    // an authenticated peer would grow the memory of the service at will; the
    // only reason it is missing here is that there is nothing to read.
    const orchestrateur = inventaireStatique(deps.base, deps.maintenant);
    // `laVmDe` and not `find`: it THROWS if the inventory carried two VMs for
    // the same user, which the partial index makes impossible in the database — and
    // if the database carried it anyway, that is a defect, not a preference to
    // express through a silent choice.
    const sienne = laVmDe(await orchestrateur.lister(), porteur.userId);

    if (sienne === undefined) {
        // ③ An empty listing is NOT a refusal, and that is why this path
        // returns 409 and not 200: a 200 would write an empty string into the
        // browser vault, `lirePrefixe` would fall back to `''`, and the page
        // would SILENTLY join the shared namespace — the silent failure
        // that the spec §10 names.
        repondre(rep, CODE_HTTP['aucune-vm'], { motif: 'aucune-vm' }, cors);
        return true;
    }

    const etat = await orchestrateur.etat(sienne.id);
    // The body common to both outcomes, written ONCE so that they
    // cannot diverge on the prefix.
    const commun = { vm: sienne.id, nom: sienne.nom, prefixe: sienne.prefixe, etat };

    if (etat !== 'prete') {
        // ④ 503: the VM does belong to this user, it does not answer. It is a
        // state of the world, not an error in the request.
        //
        // 🔴 `redemarrage` IS THE ADMISSION, NOT THE FEATURE. The framing promises
        // « VM unreachable -> the hub shows it, OFFERS A RESTART »; with the
        // v1 backend the hub SHOWS it and says it cannot restart. The
        // field carries the SAME reason and the SAME backend as the typed refusal of
        // `orchestrateur.start`, of which it is the HTTP projection — and it is
        // returned here rather than left to the browser to guess, because a
        // missing field reads as an oversight.
        //
        // ⚠️ THE PREFIX IS RETURNED ANYWAY: it is known and right, and the
        // browser needs it so as not to join the shared space while
        // waiting for the VM to come back.
        repondre(
            rep,
            CODE_HTTP['agent-injoignable'],
            {
                ...commun,
                motif: 'agent-injoignable',
                redemarrage: {
                    possible: false,
                    motif: 'non-supporte',
                    backend: BACKEND_STATIQUE,
                },
            },
            cors,
        );
        return true;
    }

    repondre(rep, 200, commun, cors);
    return true;
}
