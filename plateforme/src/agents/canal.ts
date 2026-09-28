// The loop of the `/agent` channel: enrolment, heartbeat, fresh token — and, since
// sub-block G1, everything sub-project ④ sends through it.
//
// ⚠️ THIS INVENTORY WAS WRITTEN THREE TIMES AND WENT STALE TWICE, and that is
// why it is no longer written here: G1 put `catalogue`, `lancee` and
// `lancer` in it; G2 added `icones-manquantes` without updating this line; G3
// adds `installer`, `progression` and `termine`. **The list that is
// authoritative is the protocol one** (`proto/ts/plateforme.ts`), and the branches
// of ④ live in `canal-apps.ts`. An enumeration copied here would be wrong
// at the next sub-block, as it was at the two previous ones.
//
// ⚠️ THE CHANNEL IS THEREFORE NO LONGER ONLY AN IDENTITY CHANNEL, and this
// first line said the opposite until 20 August 2026. Three variants
// were added to it (`catalogue` and `lancee` upstream, `lancer` downstream),
// and `PLATEFORME_VERSION` went to 2 for that.
//
// ⚠️ **THIS PARAGRAPH IS A DATED RECORD, AND IT STAYS TRUE AS HISTORY**:
// the version did go to 2 FOR THAT. It is 4 today
// — 3 for the icons of G2, 4 for the installation of G3 —, and striking it out
// would make wrong what is not.
//
// It is the ONLY consumer of the `plateforme` protocol
// (`proto/ts/plateforme.ts`), and it copies no message shape: it
// calls the encoders and the parser of the mirror. A copy would diverge
// silently, and `plateforme-vectors.json` would no longer compare anything the
// platform really puts on the wire.
//
// 🔴 THIS CHANNEL SHARES NOTHING WITH THE RELAY — no guard, no membership
// registry, no session observer.
//
// ⚠️ DO NOT MIX UP TWO REGISTRIES SINCE G1. The sentence above stays
// true of the membership registry of the RELAY (`identite/garde.ts`), which it still does not
// share. But this channel now keeps ANOTHER one, its
// own: `agents/registre.ts`, the table of live agent sockets, without
// which `POST /application/:id/lancer` would have no way to reach the
// VM. The two only look alike by name. It is the direct consequence
// of E4: enrolment is ASYNCHRONOUS (it reads `agent_enrole` and derives a
// `scrypt` digest), whereas the relay guard is PURE and SYNCHRONOUS. Making them
// live together would force one of the two to give way.
//
// 🔴 WHAT THE CHANNEL ISSUES IS EXACTLY WHAT THE GUARD DEMANDS, and the two
// ends must be read together (`identite/garde.ts`):
//   - the token is of TYPE `agent` (claim `sty`), otherwise the guard refuses it
//     for the `agent` role;
//   - its SUBJECT is the PREFIX of the VM, because the guard requires the subject
//     to prefix the requested session name.
// A channel that signed the VM identifier instead of the prefix would issue
// perfectly valid tokens that NOTHING would accept — a silent end-to-end
// failure, tested for that reason by a test that crosses both modules.
//
// ⚠️ NO PROMISE IS AWAITED IN A `message` HANDLER, AND NONE
// IS LEFT WITHOUT `catch`. A promise rejected in a `ws` event
// handler takes down the whole Node process — a failure mode that
// `signaling/relais.ts` and `signaling/trace.ts` both document. Everything
// asynchronous here goes out through `void … .catch(…)`.

import type { IncomingMessage } from 'node:http';
import type { WebSocket, WebSocketServer } from 'ws';
import {
    encodeBattementRecu,
    encodeEnrole,
    encodeIconesManquantes,
    encodeRefus,
    parseVersLaPlateforme,
    type CatalogueMessage,
    type MotifCanal,
} from '../../../proto/ts/plateforme';
import type { Magasin } from '../apps/icones';
import type { Pilote } from '../base/pilote';
import { fusionner } from '../apps/catalogue';
import { marquerVu } from '../depot/agent';
import { appliquer, lireConnues } from '../depot/application';
import { DUREE_JETON_ACCES_MS, signer } from '../identite/jeton';
import { adresseSource } from '../http/adresse-source';
import { ligne as ligneDeJournal } from '../obs/journal';
import {
    BUDGET_ADRESSE,
    BUDGET_COMPTE,
    cleAdresse,
    cleVm,
    type Budget,
    type Frein,
} from '../securite/frein';
import { estMontantDeQuatre, reemettreLesInstallations, traiter } from './canal-apps';
import { verifierEnrolement } from './enrolement';
import type { RegistreAgents } from './registre';


export interface OptionsCanal {
    base: Pilote;
    /// The token SIGNING secret — never the enrolment one of a
    /// VM, which lives hashed in the database and never leaves `agent_enrole`.
    secretJeton: string;
    /// The clock is a PARAMETER, never `Date.now()` read here: that is what
    /// makes the expiry of a token assertable on an EXACT value, and what
    /// lets the test see a fresh token succeed a dead one.
    maintenant: () => number;
    /// The registry of live agent sockets.
    ///
    /// 🔴 IT IS REQUIRED, NEVER OPTIONAL, and for the exact reason that makes
    /// `base` required in `http/serveur.ts`: a channel that registered
    /// nobody would be indistinguishable from correct operation as seen by the peer — it
    /// would enrol, beat, push its catalogue, and EVERY launch
    /// would return `agent-injoignable`. A silent failure, and one of those one only
    /// diagnoses by reading this file.
    registre: RegistreAgents;
    /// The icon store, queried after each `catalogue` to know what
    /// is MISSING. `undefined` = no inventory is pushed.
    ///
    /// ⚠️ OPTIONAL, unlike `registre`, and the asymmetry lies in the
    /// consequences: a missing registry makes EVERY launch unreachable —
    /// a silent failure —, whereas a missing store only costs icons that
    /// do not arrive. The channel tests that do not talk about icons
    /// therefore do not have to mount one.
    magasin?: Magasin;
    /// The brake, SHARED with the authentication routes — a single table,
    /// never two. Two distinct brakes would diverge the day one got
    /// hardened, and their ADDRESS budgets would add up: an attacker
    /// would get twice what the constants announce by alternating
    /// the two doors.
    frein: Frein;
    /// The proxies whose `X-Forwarded-For` header we trust. EMPTY by default.
    proxyDeConfiance: ReadonlySet<string>;
    dureeJetonMs?: number;
}

/// The reasons that CLOSE the socket, and those that leave it open.
///
/// 🔴 `enrolement` closes: a refused peer that kept its connection could
/// retry without limit on the same socket. Closing does not stop it from
/// reconnecting — it makes it pay the cost, and makes the number of
/// attempts countable at the level above.
///
/// ✅ **THAT LEVEL HAS EXISTED SINCE P5, AND IT IS IN THIS FILE**: the brake is
/// consulted a hundred and sixty lines further down, before `verifierEnrolement`. This
/// sentence said "the denial of service that P5 must brake … the day we
/// want to curb it": that day has come, and the same file spells it out
/// at that very site. The close stays what it was — the
/// free half —, and the brake is the other one.
///
/// ⚠️ `version` closes TOO, and that is a decision of this module the plan did not
/// prescribe: a peer that does not speak our version will NEVER succeed
/// on this connection. Leaving it open would make it loop at full speed, whereas
/// closing hands control back to its exponential-backoff retry.
///
/// `forme` and `sequence` leave the socket OPEN: these are errors the
/// peer can recover from — a malformed message can be resent, an inverted
/// sequence is fixed by enrolling. Same split as the relay, which lets
/// a malformed message be retried and closes on a refused handshake.
const MOTIFS_FERMANTS: readonly MotifCanal[] = ['enrolement', 'version'];

/// The WebSocket close code 1008 — "policy violation". It is
/// the one the relay uses on a refused handshake: a peer that reads both
/// channels does not have to know two conventions.
const FERMETURE_POLITIQUE = 1008;

export function servirLeCanalAgent(wss: WebSocketServer, options: OptionsCanal): void {
    const { base, secretJeton, maintenant, registre, frein, proxyDeConfiance, magasin } = options;
    const dureeJetonMs = options.dureeJetonMs ?? DUREE_JETON_ACCES_MS;

    // ⚠️ THE UPGRADE REQUEST IS NOW RECEIVED, and `ws` is what
    // supplies it: `http/serveur.ts` already does `wss.emit('connection', client,
    // requete)`, and a standalone `WebSocketServer` passes it natively. It is
    // the only place where the peer address is readable — a WebSocket, once
    // upgraded, no longer carries it.
    wss.on('connection', (socket: WebSocket, requete?: IncomingMessage) => {
        // Read ONCE per connection: it does not change along the way,
        // and reading it again on every message would cost without teaching anything.
        const adresse = adresseSource(
            requete?.socket.remoteAddress,
            Array.isArray(requete?.headers['x-forwarded-for'])
                ? requete.headers['x-forwarded-for'].join(',')
                : requete?.headers['x-forwarded-for'],
            proxyDeConfiance,
        );
        const parAdresse: readonly [string, Budget] = [cleAdresse(adresse), BUDGET_ADRESSE];

        // The state of THIS connection, and nothing else. It is born empty: as long
        // as no enrolment has succeeded, this peer is nobody.
        let vmId: string | undefined;
        let prefixe: string | undefined;

        function refuser(motif: MotifCanal): void {
            // ⚠️ SEND THEN CLOSE, never the reverse: a close that
            // came before the message would truncate the reason, and the peer would see
            // its connection drop without knowing whether it must fix itself, update
            // or give up.
            envoyer(socket, encodeRefus(motif));
            if (MOTIFS_FERMANTS.includes(motif)) socket.close(FERMETURE_POLITIQUE, motif);
        }

        /// Signs an agent token and returns the pair (token, expiry).
        ///
        /// 🔴 THE INSTANT IS READ ONLY ONCE and serves BOTH: signing with one
        /// instant and announcing an expiry computed on another would make
        /// `expire_a` lie by the gap between the two reads — a small,
        /// permanent lie that nothing would catch up since the agent trusts
        /// the announcement.
        function jetonNeuf(sujet: string): { jeton: string; expireA: number } {
            const instant = maintenant();
            return {
                jeton: signer(sujet, secretJeton, instant, dureeJetonMs, 'agent'),
                expireA: instant + dureeJetonMs,
            };
        }

        socket.on('message', (brut) => {
            const lecture = parseVersLaPlateforme(brut.toString());
            if (!lecture.ok) {
                refuser(lecture.motif);
                return;
            }

            if (lecture.message.type === 'battement') {
                if (vmId === undefined || prefixe === undefined) {
                    // 🔴 A HEARTBEAT AUTHENTICATES NOBODY. Answering it with
                    // `battement-recu` would issue an AGENT TOKEN to a peer
                    // that presented no secret — that is, the leak that
                    // this whole sub-block exists to close, through another
                    // door.
                    refuser('sequence');
                    return;
                }

                const instant = maintenant();
                // ⚠️ LAUNCHED WITHOUT BEING AWAITED, with its `catch`: the heartbeat
                // is an OBSERVATION, never a dependency of the channel. A database
                // momentarily unavailable must not take down the connection
                // of an agent that is itself perfectly fine. The cost is named: a
                // lost write only shows in the log.
                void marquerVu(base, vmId, instant).catch((cause) => {
                    console.error(`vu_a non avancé pour la VM ${vmId} : ${String(cause)}`);
                });

                const { jeton, expireA } = jetonNeuf(prefixe);
                envoyer(socket, encodeBattementRecu(jeton, expireA));
                return;
            }

            // 🔴 A TYPE GUARD, NO LONGER A FALL-THROUGH. `enroler` was the REMAINDER
            // of an `if/else`, and the widening of the union by sub-block G3
            // made `progression` and `termine` two members of that remainder —
            // hence two messages that the destructuring below would have read
            // as an enrolment. `tsc` said so, and it got lucky: the
            // same fragility on a value rather than a type would have gone through
            // silently. The predicate NAMES the four types of ④.
            if (estMontantDeQuatre(lecture.message)) {
                if (vmId === undefined) {
                    // 🔴 NO CATALOGUE, NO OUTCOME, NO PROGRESS WITHOUT
                    // ENROLMENT. Accepting a catalogue here would let an anonymous
                    // peer WRITE INTO THE `application` TABLE of a VM it
                    // has not authenticated; accepting an outcome would let it
                    // resolve someone else's request, and fake a
                    // launch that never happened; accepting a progress report
                    // or a `termine` would let it write into the `installation`
                    // table of a VM that is not its own — and therefore
                    // declare someone else's installation succeeded, or refused.
                    // It is the hole the `sequence` refusal of the heartbeat already
                    // closes, through four other doors.
                    refuser('sequence');
                    return;
                }
                traiter(
                    { base, registre, magasin, socket, vmId, maintenant, envoyer: (brut) => envoyer(socket, brut) },
                    lecture.message,
                );
                return;
            }

            const { vm, secret } = lecture.message;

            // 🔴 THE BRAKE IS CONSULTED HERE, AND THE POSITION IS WHAT COUNTS:
            // BEFORE `verifierEnrolement`, hence before it reads
            // `agent_enrole` AND before it derives a `scrypt` digest.
            // `scrypt` is memory-hard and deliberately expensive (68 ms
            // measured on 20 August 2026): an attacker who triggers it at
            // will exhausts the service without ever guessing a secret. A
            // BRAKE PLACED AFTER THE CHECK PROTECTS NOTHING.
            //
            // This file announced this day since P3, in the comment of
            // `MOTIFS_FERMANTS`: "closing does not stop it from reconnecting
            // — it makes the number of attempts countable at the level
            // above the day we want to curb it". P5 is that day.
            const cles: readonly (readonly [string, Budget])[] = [
                [cleVm(vm), BUDGET_COMPTE],
                parAdresse,
            ];
            if (frein.consulter(cles, maintenant()).freine) {
                // 🔴 THE REASON IS `enrolement`, AND NOTHING ELSE. A distinct
                // `frein` reason would give the attacker the information
                // "this VM exists and I made it trigger": that is
                // the ENUMERATION ORACLE that `agents/enrolement.ts` closes over
                // three paragraphs, reopened through the brake door. The
                // LOG, on the other hand, tells the two apart — same split as
                // `identite/garde.ts` (`message` on the wire, `journal` on our
                // side).
                journaliserLeFrein(frein, cles, adresse, maintenant());
                refuser('enrolement');
                return;
            }
            // ⚠️ THE `catch` IS MANDATORY AND IT IS NOT DECORATIVE:
            // `verifierEnrolement` RAISES on a digest written by a
            // future version of the service (`identite/mot-de-passe.ts` refuses an
            // unknown algorithm rather than returning a `false` indistinguishable
            // from a wrong secret). This exception is translated HERE into an
            // `enrolement` refusal — the same as all the others, so as to enumerate
            // nothing — and logged WITH its cause, which never carries the
            // secret.
            void verifierEnrolement(base, vm, secret, (ligne) => console.warn(ligne))
                .then((verdict) => {
                    if (!verdict.ok) {
                        compterLEchec(frein, cles, adresse, maintenant());
                        refuser(verdict.motif);
                        return;
                    }
                    // 🔴 SUCCESS CLEARS THE KEY OF THE VM, NEVER THAT OF THE
                    // ADDRESS — same rule as `/auth/connexion`. Clearing it
                    // too would whitewash an attacker who owns a valid
                    // VM: they would only have to enrol between two
                    // bursts to reset their address budget to zero.
                    frein.succes(cleVm(verdict.vmId));
                    vmId = verdict.vmId;
                    prefixe = verdict.prefixe;
                    // 🔴 IT IS HERE, AND NOWHERE ELSE, THAT THE VM BECOMES
                    // REACHABLE. Registration follows authentication and never
                    // precedes it: a peer that has not proven its secret
                    // must not be able to receive the launch orders of a
                    // VM. The LAST one registered wins, and the old socket is
                    // closed (`agents/registre.ts`).
                    registre.inscrire(verdict.vmId, socket);

                    const instant = maintenant();
                    // ⚠️ ENROLMENT ADVANCES `vu_a` AS WELL, and it is not
                    // a convenience: it IS a sign of life, the first one. Without
                    // it, a VM that just connected would stay `vu_a =
                    // null` — hence `injoignable` (`agents/fraicheur.ts`) —
                    // until its first heartbeat, and a console would
                    // call it off while it is talking. The column keeps all
                    // its meaning: `null` still says "enrolled by
                    // the administrator, never connected since".
                    void marquerVu(base, verdict.vmId, instant).catch((cause) => {
                        console.error(
                            `vu_a non posé à l'enrôlement de la VM ${verdict.vmId} : ${String(cause)}`,
                        );
                    });

                    const { jeton, expireA } = jetonNeuf(verdict.prefixe);
                    envoyer(socket, encodeEnrole(verdict.prefixe, jeton, expireA));

                    // 🔴 THE RE-EMISSION OF PENDING INSTALLATIONS. A WebSocket
                    // `push` has NO delivery guarantee: without it,
                    // an order emitted during an outage would be lost WITH NO
                    // END, and the user would wait for an installation that
                    // nobody would ever relaunch. It is the same safety net as
                    // the `complet = true` of the catalogue, and the G1 acceptance run
                    // saw that net work on the real path.
                    //
                    // ⚠️ IT ONLY TARGETS THE `en_attente` ONES, AND THAT IS THE
                    // FIRST OF THE TWO BELTS AGAINST A DOUBLE
                    // RUN: as soon as an agent has reported a progress,
                    // the row turns `en_cours` and stops being re-emitted. The
                    // second belt is the marker on the VM disk,
                    // and it protects from the case where the first one lost its database.
                    //
                    // ⚠️ `void … .catch(…)`, NEVER `await`: a perfectly
                    // valid enrolment must not fail because the
                    // database is momentarily unavailable, and a promise
                    // rejected without `catch` would take down the whole Node process.
                    void reemettreLesInstallations(
                        { base, vmId: verdict.vmId, envoyer: (brut) => envoyer(socket, brut) },
                    ).catch((cause) => {
                        console.error(
                            `réémission des installations impossible pour la VM `
                                + `${verdict.vmId} : ${String(cause)}`,
                        );
                    });
                })
                .catch((cause) => {
                    // The VM name is logged, the secret never: it has just
                    // been refused, writing it again elsewhere would make no sense
                    // and would expose it in a trace file.
                    console.error(`enrôlement en échec pour la VM ${vm} : ${String(cause)}`);
                    // ⚠️ THIS PATH COUNTS TOO. A digest written by a
                    // future version of the service makes `verifier` RAISE: without
                    // this counting, an attacker who found a way to make it
                    // raise would have a full-cost path with no
                    // brake.
                    compterLEchec(frein, cles, adresse, maintenant());
                    refuser('enrolement');
                });
        });

        socket.on('close', () => {
            // ⚠️ THE SOCKET IS PASSED, AND IT MATTERS. An agent that restarts
            // registers BEFORE the close of the previous one is notified:
            // a bare removal would then erase the registration of the NEW one, and the VM
            // would become unreachable at the very moment it reconnects.
            //
            // Without this removal, a dead VM would stay "reachable" until the
            // next enrolment, and each launch would cost
            // `DELAI_LANCEMENT_MS` before failing on a silence.
            if (vmId !== undefined) registre.retirer(vmId, socket);
        });
    });
}

/// Records the failure, and logs IF AND ONLY IF the brake has just
/// bitten.
///
/// 🔴 WHY NOT ONE LINE PER REFUSAL — same reason as `http/routes-auth.ts`,
/// and it bites harder here: a refused enrolment attempt CLOSES the
/// socket, so an attacker in a loop opens one connection per
/// attempt. One trace per refusal would therefore write one line per connection,
/// on the very path the brake has just made free. `CLAUDE.md` carries
/// the rule since the TURN project: "count or sample, never
/// trace per packet".
///
/// The transition is detected by consulting again AFTER the failure: the next
/// attempt is refused BEFORE reaching `verifierEnrolement`, so it never calls
/// this function. There is EXACTLY one line per key and per window.
function compterLEchec(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
    instant: number,
): void {
    frein.echec(cles, instant);
    if (!frein.consulter(cles, instant).freine) return;
    journaliserLeFrein(frein, cles, adresse, instant);
}

/// The line the operator reads, and that the requester will never see.
///
/// ⚠️ IT NAMES THE RETAINED ADDRESS, and that is the ONLY remedy for the failure
/// mode of `http/adresse-source.ts`: an operator who put a proxy in place
/// without declaring trust in it will see the address of their proxy here on every
/// line, and will understand that their per-address brake has become GLOBAL.
function journaliserLeFrein(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
    instant: number,
): void {
    const verdict = frein.consulter(cles, instant);
    console.warn(
        ligneDeJournal('frein', {
            route: '/agent',
            adresse,
            cles: cles.map(([cle]) => cle).join(' '),
            retry_apres_s: verdict.retryApresS,
            // Without these two, SATURATION of the brake would be invisible: under
            // saturation an eviction gives its budget back to a targeted VM.
            entrees: frein.taille(),
            evictions: frein.evictions(),
        }),
    );
}

/// Only writes to an OPEN socket. A `send` on a socket being
/// closed raises, and that exception would cross the `message` handler.
function envoyer(socket: WebSocket, brut: string): void {
    if (socket.readyState === 1 /* OPEN */) socket.send(brut);
}
