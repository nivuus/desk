// The service's HTTP server, and the routing of the WebSocket upgrade.
//
// `noServer` rather than `{ server }`: the path routing is explicit.
// ✅ P3 ADDED `/agent` WITHOUT TOUCHING THE RELAY — the branch first, then
// the LOOP of the channel it serves (`agents/canal.ts`) —, which this sentence
// announced: the branch is a second comparison, and `wssRacine` did not
// move by a single line. With `{ server }`, `ws` would accept any upgrade on any
// path — that is the behaviour before P1 (the `new WebSocketServer({ port })`
// of the former `server.ts`, which set neither `host` nor `path`), and it is not
// extensible.
//
// ⚠️ TWO PATHS, TWO COMPARISONS, NO ROUTING TABLE. A table for
// two entries would be unpaid abstraction, and it would make less
// visible what matters here: refusal by DEFAULT. Any path that is neither
// `/signal` nor `/agent` gets a `404` and its socket is closed — it is an
// allow list, never a deny list, and that is why a path
// added some day by mistake opens nothing.
//
// 🔴 THE RELAY LEFT THE ROOT ON 21 AUGUST 2026, AND THE REASON IS THE PROXY,
// NOT TASTE. On the root, the page and the WebSocket upgrade competed for
// the same path, told apart only by the `Upgrade` header — a criterion
// Pomerium cannot route on. The relay serving TWO peers of different
// kinds (the browser, which the proxy authenticates; the Windows agent, which
// has neither browser nor cookie), a distinct path was needed so the
// proxy could guard the root without cutting off the agent. The session page
// (`client/src/main.ts`) and the shell page (`client/src/hub/page.ts` since
// the hub became the only surface, 31 August 2026; `client/src/
// shell-page.ts` at the time of that commit) now target `/signal` (via
// `client/src/adresse-plateforme.ts`), like the agent (`agent/src/
// signaling.rs`, `url_du_relais`). No known peer is affected —
// each was moved in the same commit.

import { createServer, type Server } from 'node:http';
import { WebSocketServer } from 'ws';
import type { Config } from '../config';
import type { Pilote } from '../base/pilote';
import { garde } from '../identite/garde';
import { ouvrirMagasin } from '../apps/icones';
import { ouvrirMagasinTranches } from '../apps/magasin-tranches';
import { startCleanup } from '../apps/nettoyage';
import { CacheSante } from './routes-sante';
import { ENTETES_SECURITE } from './entetes';
import { createSignalingServer } from '../signaling/relais';
import { ProprieteDeSession } from '../signaling/propriete';
import { observateurDeSession } from '../signaling/trace';
import { servirLeCanalAgent } from '../agents/canal';
import { RegistreAgents } from '../agents/registre';
import { Frein } from '../securite/frein';
import { servirTout } from './chaine';
import { repondreIntrouvable } from './introuvable';
import { absorbSocketErrors } from './erreurs-socket';
import {
    annonceProxyDeConfiance,
    annonceRacinePage,
    write,
    etatRacinePage,
} from './annonces';

/// The path of the platform <-> agent channel (P3). ⚠️ It is compared
/// EXACTLY: see the routing below.
const CHEMIN_AGENT = '/agent';

/// The path of the signaling relay. ⚠️ It is compared EXACTLY.
///
/// 🔴 IT LEFT THE ROOT ON 21 AUGUST 2026, AND THE REASON IS THE PROXY, NOT
/// TASTE. On the root, the page and the WebSocket upgrade competed for the same
/// path, told apart only by the `Upgrade` header — a criterion
/// Pomerium cannot route on. The relay serving TWO peers of different
/// kinds (the browser, which the proxy authenticates; the Windows agent, which
/// has neither browser nor cookie), a distinct path was needed so the
/// proxy could guard the root without cutting off the agent.
///
/// ✅ THIS MOVE SETTLES A DECLARED LEGACY of `deploiement/nginx.conf`, which
/// sub-block P5 had set aside for lack of the right to touch `agent/`.
const CHEMIN_SIGNAL = '/signal';

/// 🔴 THE MAXIMUM SIZE OF A WEBSOCKET FRAME, ON BOTH SERVERS.
///
/// MEASURED on 20 August 2026:
/// `plateforme/node_modules/ws/lib/websocket-server.js:74` carries
/// `maxPayload: 100 * 1024 * 1024` — A HUNDRED MEBIBYTES by default. The two
/// servers in this file were built without this option.
///
/// WHAT THAT OPENED, and it is not theoretical: AN ANONYMOUS PEER COULD
/// PUSH A 100 MiB FRAME BEFORE ANY AUTHENTICATION. The SHAPE
/// check runs before the guard — `signaling/relais.ts:84-86` writes it itself,
/// "`isJsonObject` is called some thirty lines before
/// `garde.verify`" —, so that `JSON.parse` on 100 MiB is an
/// allocation then a CPU spike, per socket and per frame, offered to whoever
/// reaches the port. And the `/agent` channel is the SECOND anonymous door:
/// bounding only `/signal` would leave half the problem whole.
///
/// With this option, `ws` closes the socket with 1009 ("message too big")
/// WITHOUT EVER handing the frame to the `message` handler.
///
/// ⚠️ THE VALUE IS NOT CALIBRATED, AND ITS FLOOR IS REASONED, NOT MEASURED.
/// The biggest legitimate message is an SDP offer or answer, which for the
/// sessions of this repository fits in a few kilobytes; 256 KiB leaves two
/// orders of magnitude of margin. NO REAL SDP WAS MEASURED to set
/// this figure, and saying so beats implying a calibration.
///
/// ⚠️ **WHAT IT DOES NOT CLOSE — THIS SENTENCE HAD BECOME HALF WRONG
/// IN CORRECTION ROUND 1 (25 August 2026), which bounded the NUMBER of
/// connections on `/signal` ONLY.** It said "a peer can still
/// open MANY CONNECTIONS", true of both paths when written, now only
/// true of one: `/signal` (`signaling/relais.ts`) bounds the
/// number of connections per address from `connection` on, BEFORE the first
/// message (`securite/frein.ts::BUDGET_REQUETES`); `/agent`
/// (`agents/canal.ts`) STILL DOES NOT — its brake is only consulted
/// on MESSAGE (a `{vm, secret}` attempt), never on connection, and
/// a peer that stays silent after opening is counted by nothing, neither by it
/// nor by `deploiement/nginx.conf` (no `limit_conn`/`limit_req`).
export const TRAME_MAX_OCTETS = 256 * 1024;

export interface ServicePlateforme {
    port: number;
    close(): Promise<void>;
}

/// `base` is REQUIRED, never optional: a service that paired peers without
/// recording anything would be indistinguishable from correct operation (spec §6), and
/// that is the exact class of silent failure this whole repository is
/// written against. `demarrage.ts` also guarantees the port only opens after
/// the database and its migrations.
export async function startServer(config: Config, base: Pilote): Promise<ServicePlateforme> {
    // The routers are tried IN ORDER; if none recognises the
    // path, the P1 404 is kept WORD FOR WORD. ⚠️ Do not change its
    // body: nothing tested it before P2, and changing it would be an undeclared
    // side effect. `routes-auth.test.ts` now freezes it.
    //
    // 🔴 THE CHAINING HAS NOT BEEN DONE HERE SINCE 22 AUGUST 2026: IT WAS
    // EXTRACTED INTO `./chaine.ts` (`servirTout`, exported), BECAUSE THIS (policy: allow-fr - file name)
    // FILE WAS REACHING 475/500 LINES AND THE NEXT BATCH HAD TO
    // ADD A TENTH ROUTER TO IT — the extraction freed the margin BEFORE
    // the addition, as `CLAUDE.md` prescribes. What remains HERE is the CALL
    // SITE, in the shape `void … .then(servie => …).catch(…)`,
    // KEPT AS IS. That is what this file has held itself to since P1: the
    // `.catch` is the only thing preventing a promise rejected in a
    // Node event handler from taking down the whole process, and a
    // rewrite of this body would lose it without anything saying so. The diff
    // on the `createServer` body thus stays a single line — the call
    // to `servirTout`, now imported, rather than defined locally.
    //
    // ⚠️ THE TEN ROUTERS SHARE THEIR DEPENDENCIES, and `Date.now` is
    // passed here as to the guard, the trace and the channel: no module of the
    // service reads a clock itself. That is what makes the freshness
    // bound assertable on an exact value in the route tests.
    // ⚠️ "THE THREE ROUTERS" WAS THE WORDING, AND IT DATED FROM P4: seven have
    // been chained since, without this sentence moving. The count is
    // rerun, not copied:
    //   grep -cE '^    (if \(await servir|return servir)' plateforme/src/http/chaine.ts (policy: allow-fr - file name)
    //     -> 10
    // The registry of live agent sockets, built ONCE and shared
    // between the channel (which registers into it) and the routes (which launch through it). It is the
    // only place in the service that builds one.
    //
    // ⚠️ IT HAS THE SAME COST AS `ProprieteDeSession`, AND IT IS NAMED IN THE SAME
    // PLACE: it does not survive a restart. After a restart, no
    // agent is in it until it has re-enrolled, and every launch
    // returns `agent-injoignable` — noisily. The recovery is the agent's
    // reconnection, which rebuilds it without anyone persisting it.
    const registreAgents = new RegistreAgents();

    // 🔴 ONE SINGLE BRAKE FOR THE WHOLE SERVICE, built HERE and shared between
    // the authentication routes and the `/agent` channel. Two distinct
    // brakes WOULD DIVERGE the day one was hardened (D4), and their
    // address budgets would add up: an attacker would get
    // double what the constants announce by alternating between the two doors.
    //
    // ⚠️ IT HAS THE SAME COST AS `ProprieteDeSession` AND `RegistreAgents`, AND IT
    // IS NAMED IN THE SAME PLACE: it does not survive a restart, and THE
    // BRAKE OF ONE INSTANCE ONLY PROTECTS THAT INSTANCE. Two instances
    // would multiply each budget by two, without anything saying so — that is
    // one of the reasons the deployment declares only one.
    const frein = new Frein();

    // The `/sante` cache, built ONCE and alive for the lifetime of the
    // service — like `ProprieteDeSession`, `RegistreAgents` and the brake.
    // A per-request cache would cache nothing.
    const cacheSante = new CacheSante();

    // The icon store, opened ONCE for the lifetime of the service. It creates
    // its directory if missing and LOGS the retained path: the variable
    // being optional, that is the only thing that makes visible to the operator
    // the store they are actually working on.
    const magasin = ouvrirMagasin(config.repertoireIcones, (chemin) => {
        console.info(`icon store: ${chemin}`);
    });

    // The SLICE store, also opened ONCE, and logging its
    // path for exactly the same reason as the icon one.
    //
    // 🔴 IT IS READ BY **TWO** ROUTERS — `servirTeleversement` writes the
    // slices into it, `servirInstallation` serves their concatenation to the agent — and
    // it is the SAME one, never two: two stores opened on the same directory
    // would be two views of one disk, and the second would not
    // necessarily see what the first has just written.
    const magasinTranches = ouvrirMagasinTranches(config.repertoireTeleversements, (chemin) => {
        console.info(`chunk store: ${chemin}`);
    });

    // 🔴 WITHOUT THIS CALL, `evincer` OF THE TWO STORES ABOVE IS INVOKED
    // BY NOBODY — correction round 1, see `apps/nettoyage.ts` for the
    // cadence and its reason. AWAITED: the first round has finished before this
    // service answers a request, including in tests.
    const nettoyage = await startCleanup({
        base,
        magasin,
        tranches: magasinTranches,
        maintenant: Date.now,
    });

    // 🔴 THE THIRD OPTIONAL DISK ROOT ANNOUNCES ITSELF LIKE THE OTHER
    // TWO, AND IT DID NOT USED TO. The two stores above
    // have logged their retained path since G2 and G3, with the reason written
    // above them; `PLATEFORME_PAGE`, added on 22 August 2026, logged
    // NOTHING — neither at startup nor per request —, so that a
    // nonexistent root returned `404 introuvable` on every page, strictly
    // indistinguishable from the variable being absent. See `./annonces.ts`.
    //
    // ⚠️ IT IS HERE, BEFORE `http.listen`, AND NOWHERE ELSE: an announcement posted
    // after the port opens would arrive after the first request served.
    write(annonceRacinePage(await etatRacinePage(config.racinePage)));
    // 🔴 SAME CLASS OF SILENT FAILURE, SAME REMEDY — and the final review rightly
    // grouped them together: a host name in
    // `PLATEFORME_PROXY_DE_CONFIANCE` matches no `remoteAddress`,
    // so `pairDeConfiance` refuses everyone, `/auth/moi` returns `401` to
    // Pomerium itself, and the service answers anyway. The runbook
    // already documents it — but a runbook does not turn red.
    write(annonceProxyDeConfiance(config.proxyDeConfiance));

    const deps = {
        base,
        secretJeton: config.secretJeton,
        origineClient: config.origineClient,
        maintenant: Date.now,
        // ⚠️ **TWO** ROUTERS READ IT — `servirIdentite` AND `servirAuth`. This
        // comment said "ONLY `servirIdentite`" until 21 August 2026:
        // it is THE SAME MISTAKE as the `registre` scar, twenty lines further
        // down, made again in the following commit. The two guards have
        // OPPOSITE POLARITIES and partition the modes — `/auth/moi` in
        // `pomerium`, `/auth/connexion` and `/auth/rafraichir` in `motdepasse`;
        // the invariant linking them is written in both places.
        auth: config.auth,
        // ⚠️ `servirApplications` AND `servirInstallation` READ IT — the
        // first to launch an application, the second to push an install
        // order to an already connected VM. The others ignore it. It is set here rather than passed separately so that the
        // chaining stays a single line per router, and because one dependency object
        // per router would make TEN lists to keep up to date — the
        // count said "four", and it too dated from P4.
        registre: registreAgents,
        // 🔴 THIS SENTENCE HAS ALREADY LIED TWICE IN A ROW — "ONLY servirAuth
        // READS IT", THEN "servirAuth AND routes-identite.ts, now TWO"
        // — EACH TIME BECAUSE A LATER BATCH ADDED A READER WITHOUT
        // COMING BACK TO FIX THIS LINE. The legacy of the missing brakes (D24,
        // 25 August 2026) adds TWO more: `routes-vm.ts` and
        // `routes-session.ts` now consult `frein` AND
        // `proxyDeConfiance` as well, for the "any request" budget
        // (`securite/frein.ts::BUDGET_REQUETES`) — a VOLUME budget,
        // distinct from the FAILURES one that `servirAuth` alone consults.
        //
        // ✅ THIS WHOLE BLOCK RECHECKED on 25 August 2026 BY THE COMMAND, not by
        // reading: `grep -ln 'deps\.<key>' plateforme/src/http/routes-*.ts`
        // (excluding `*.test.ts`, which SET the dependency without consuming it).
        // Readers — `frein`: `routes-auth.ts`, `routes-vm.ts`,
        // `routes-session.ts` — THREE; `proxyDeConfiance`: the three
        // above **AND** `routes-identite.ts` — FOUR; `cache`:
        // `routes-sante.ts`; `magasin`: `routes-icone.ts`.
        //
        // ⚠️ `signaling/relais.ts` READS BOTH TOO, FOR THE SAME BUDGET,
        // BUT NOT THROUGH THIS PATH: it does not receive this `deps` object — it is
        // built separately, further down in this function, and `frein` as well as
        // `config.proxyDeConfiance` are passed to it as POSITIONAL PARAMETERS
        // of `createSignalingServer`. The `grep -ln 'deps\.frein'` command
        // above therefore does NOT see it — look for `createSignalingServer`
        // for that reader (see further down in this same function).
        frein,
        proxyDeConfiance: config.proxyDeConfiance,
        cache: cacheSante,
        // ⚠️ ONLY `servirIcone` READS IT — same reason as `registre` and `frein`
        // above, and rechecked by the same command.
        magasin,
        // ⚠️ IT IS CALLED `tranches` AND NOT `magasin`, because `magasin` is
        // ALREADY TAKEN by the icon one, just above. `tsc` caught the
        // collision — the two G3 routers had first named it
        // `magasin` each on its own side — because the two types differ.
        // **The day two stores have the same shape, the service would serve
        // icons in place of slices without any check
        // flinching.**
        tranches: magasinTranches,
        // ⚠️ `registre` IS ALREADY HIGHER UP, and it now serves TWO
        // routers: `servirApplications` (the launch) and `servirInstallation`
        // (pushing the order). The comment that said it was read by only one
        // was fixed in place.
        // ⚠️ ONLY `servirPage` READS IT. Absent ⇒ the page server steps aside and the generic
        // 404 takes over again — the behaviour from before the batch.
        racinePage: config.racinePage,
    };

    // `servirTout`: see its extraction into `./chaine.ts`, explained higher (policy: allow-fr - file name)
    // up in this function.
    const http: Server = createServer((requete, reponse) => {
        void servirTout(requete, reponse, deps)
            .then((servie) => {
                if (servie) return;
                // ⚠️ THE GENERIC 404 COMES FROM NO ROUTER, and that is
                // why it must be handled here: without this line, an
                // unknown path would be the ONLY response of the service not to
                // carry `nosniff`. The BODY is not touched — "nothing
                // tested it before P2, and changing it would be an undeclared
                // side effect".
                //
                // 🔴 IT MOVED TO `./introuvable.ts` ON 22 AUGUST 2026, AND
                // IT IS NOT A CONVENIENCE FACTORING: the two mode
                // guards now answer THIS 404 themselves, because
                // the SPA fallback of the page server swallowed the one here. A
                // second text handwritten in each guard would drift from
                // this one without anything saying so. See the header of that
                // module.
                repondreIntrouvable(reponse);
            })
            .catch((cause) => {
                // A promise rejected without `catch` in a Node event
                // handler takes down the whole process — that is the failure
                // mode `signaling/relais.ts` already documents. The
                // cause is logged WITHOUT the request body, which
                // would carry the password (criterion ④).
                // ⚠️ THE LABEL NO LONGER NAMES "the authentication": since
                // P4 this `catch` covers ALL routers — there are TEN, not
                // three as this sentence said until 22 August 2026, and
                // the count is rerun from `chaine.ts`. A message that (policy: allow-fr - file name)
                // named the wrong one would send people looking in the wrong place.
                // It is the only line of this block that P4 changes, and it is
                // changed because it would otherwise have become WRONG.
                console.error(`HTTP route failed: ${String(cause)}`);
                if (!reponse.headersSent) {
                    reponse.writeHead(500, {
                        'content-type': 'application/json; charset=utf-8',
                        ...ENTETES_SECURITE,
                    });
                    reponse.end(JSON.stringify({ refus: 'interne' }));
                }
            });
    });

    // `maxPayload` on BOTH servers, never just one: see
    // `TRAME_MAX_OCTETS`. The bound applies inside `ws`, hence BEFORE the
    // `message` handler — that is what makes it useful, the relay's shape
    // check running before the guard.
    const wssRacine = new WebSocketServer({ noServer: true, maxPayload: TRAME_MAX_OCTETS });
    absorbSocketErrors(wssRacine);
    // The `/agent` channel (P3): its own `WebSocketServer`, which shares
    // with the relay neither guard, nor ownership registry, nor session
    // observer. That is the direct consequence of E4: enrolment is
    // ASYNCHRONOUS (it reads `agent_enrole`), and the relay guard is PURE and
    // SYNCHRONOUS. Making them live in the same server would force one of the
    // two to give way.
    const wssAgent = new WebSocketServer({ noServer: true, maxPayload: TRAME_MAX_OCTETS });
    absorbSocketErrors(wssAgent);
    // The guard is built HERE, from the configuration secret, and
    // it is the ONLY place in the service that builds one. It is REQUIRED
    // by the relay: there is no path that produces an open guard
    // outside a test, `PLATEFORME_SECRET_JETON` having no default.
    //
    // The ownership registry lives here too, hence for the lifetime of the service.
    // Its cost — it does not survive a restart — is written in
    // `signaling/propriete.ts`.
    const gardeDuService = garde(config.secretJeton, Date.now, new ProprieteDeSession());
    // 🔴 THE CHANNEL IS WIRED HERE, AND THIS IS THE ONLY LINE THAT BRINGS IT TO LIFE.
    // Without it, `wssAgent` would still accept the upgrade on `/agent` and
    // would listen to NOTHING: the peer would see a successful connection, then
    // silence — the exact silent failure this file already invokes to make
    // `base` REQUIRED. `canal.test.ts` holds it with a dedicated test.
    //
    // `Date.now` is passed here, as to the guard and the trace: all three
    // receive it from this function, and no module of the service reads a clock
    // itself. That is what makes the expiry of an agent token assertable
    // on an EXACT value in `canal.test.ts`.
    servirLeCanalAgent(wssAgent, {
        base,
        secretJeton: config.secretJeton,
        maintenant: Date.now,
        registre: registreAgents,
        // The SAME brake as the authentication routes: see its
        // construction above.
        frein,
        proxyDeConfiance: config.proxyDeConfiance,
        // The SAME store as the icon route, never a second one: two
        // stores would diverge, and the inventory of missing ones would point at
        // a disk the route does not serve.
        magasin,
    });

    // `Date.now` is passed HERE, and once only for the trace: it is the
    // only place on the trace path that reads a real clock, all the
    // rest receives it.
    //
    // ⚠️ `frein` AND `config.proxyDeConfiance`: THE SAME brake as the HTTP
    // routes and the `/agent` channel, never a second one — see its construction
    // above and `securite/frein.ts::BUDGET_REQUETES`. Before this batch, no
    // bound existed on the number of connections one address
    // could open on `/signal` — the legacy that `TRAME_MAX_OCTETS`,
    // above, already named without closing it.
    const relais = createSignalingServer(
        wssRacine,
        gardeDuService,
        frein,
        config.proxyDeConfiance,
        observateurDeSession(base, Date.now),
    );

    http.on('upgrade', (requete, socket, tete) => {
        // `requete.url` may carry a query string; only the path
        // decides the routing.
        const chemin = new URL(requete.url ?? '/', 'http://placeholder').pathname;
        // EXACT comparison, never a `startsWith`: `/agentaire` is not
        // `/agent`, and a prefix would open a whole family of paths that
        // nobody decided on.
        const wss =
            chemin === CHEMIN_SIGNAL ? wssRacine : chemin === CHEMIN_AGENT ? wssAgent : undefined;
        if (wss === undefined) {
            // Explicit refusal BEFORE any upgrade: the peer gets an HTTP 404
            // and its socket is closed, rather than staying open on a
            // service that will never listen to it.
            socket.write('HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n');
            socket.destroy();
            return;
        }
        wss.handleUpgrade(requete, socket, tete, (client) => {
            wss.emit('connection', client, requete);
        });
    });

    await new Promise<void>((resolve, reject) => {
        http.once('error', reject);
        // BOTH arguments, always: without `config.hote`, Node listens on
        // all interfaces, which criterion ④ exists to prevent.
        http.listen(config.port, config.hote, () => {
            http.removeListener('error', reject);
            resolve();
        });
    });

    const adresse = http.address();
    const port = typeof adresse === 'object' && adresse ? adresse.port : config.port;

    return {
        port,
        async close(): Promise<void> {
            // Called BEFORE everything else — but CORRECTED (correction round
            // 2): `arreter()` only prevents the NEXT scheduling
            // of the background cleanup, it does NOT interrupt a round already in flight.
            // A round started just before `close()` can therefore still hit
            // a database about to close; if it fails, it is
            // logged (`apps/nettoyage.ts`), never fatal. See the
            // comment of `arreter()` for what it really guarantees.
            nettoyage.arreter();
            await relais.close();
            // ⚠️ The second server is closed TOO, and explicitly. A
            // `WebSocketServer` in `noServer` mode does not stop with the HTTP
            // server: its already upgraded sockets would survive, and `close()`
            // would return on a service that is still listening.
            await new Promise<void>((resolve) => wssAgent.close(() => resolve()));
            await new Promise<void>((resolve) => http.close(() => resolve()));
        },
    };
}
