// The shared harness of the `/agent` channel tests: constants, test peer, and
// enrolling a VM in the database.
//
// 🔴 IT IS EXTRACTED BEFORE THE ADDITION IT SERVES, never after. `canal.test.ts`
// held these eighty lines and weighed 361; sub-block G1 adds a
// second family of cases to it (the catalogue and the launch), which would have made it
// cross the 450-line gate that `CLAUDE.md` sets. Copying them into the
// new file would have produced two `ouvrirUrl` that would diverge at the first
// fix applied to only one of the two.
//
// ⚠️ THIS MODULE LIVES IN `src/`, AND THAT IS THE REPOSITORY CONVENTION for a test
// harness: `base/harnais.ts` has been there since P1, for the same reason — a helper
// that several test files import has nowhere else to live.

import { WebSocket } from 'ws';
import type { Pilote } from '../base/pilote';
import { enroler, lireParVm } from '../depot/agent';
import { hacher } from '../identite/mot-de-passe';

/// The signing secret of the SERVICE — the token one. Not to be confused
/// with the ENROLMENT secret below: they have neither the same lifetime,
/// nor the same holder, and mixing them up in a test would make the second
/// verifiable by the first.
export const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
/// The enrolment secret of the VM, the one `npm run admin:agent` draws.
export const SECRET_VM = 'un-secret-d-enrolement-de-la-vraie-longueur';
/// A real epoch: small values measure nothing (lesson of P1).
export const T0 = 1_787_000_000_000;
/// A prefix of the REAL length that `agents/prefixe.ts` produces.
export const P = 'RhH1x2QmTz9kLpVbNc7dAw';

/// Enrols a VM, `vm` row included: `agent_enrole.vm_id` REFERENCES it
/// (`0003-agents.sql`), and SQLite enforces the foreign key. The digest is
/// a REAL `scrypt` digest, never a short string.
export async function enrolerUneVm(p: Pilote, vmId: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [
        vmId,
        `vm-${vmId}`,
        '192.168.3.2',
    ]);
    await enroler(p, vmId, await hacher(SECRET_VM), P);
}

export interface Pair {
    socket: WebSocket;
    /// The REAL order of events, `message` and `close` as they
    /// arrived: that is what lets us assert that a refusal ARRIVED before
    /// the close, and not the reverse.
    ordre: string[];
    ferme: Promise<void>;
    /// Sends a raw message and returns the answer. BOUNDED, never an endless
    /// wait: a mute channel must turn red, not hang.
    dire(brut: string): Promise<Record<string, unknown>>;
    /// Waits for the next message PUSHED by the channel, WITHOUT sending anything.
    ///
    /// 🔴 IT IS NOT A `dire('')`. The `/agent` channel now pushes
    /// orders the peer never asked for (`lancer`), and waiting for them through a
    /// dummy send would be a RACE: `dire` only sends if no message
    /// has already arrived, so the test would send — or would not send —
    /// depending on scheduling, and would provoke a `forme` refusal one time out
    /// of two. Bounded for the same reason as `dire`.
    recevoir(): Promise<Record<string, unknown>>;
}

/// Opens a peer on a full URL — the path matters, the service
/// routing two of them (`http/serveur.ts`).
export function ouvrirUrl(url: string): Promise<Pair> {
    return new Promise((resolve, reject) => {
        const w = new WebSocket(url);
        const enAttente: ((m: Record<string, unknown>) => void)[] = [];
        const recus: Record<string, unknown>[] = [];
        const pair: Pair = {
            socket: w,
            ordre: [],
            ferme: new Promise((r) => w.once('close', () => r())),
            recevoir() {
                return new Promise((r, rej) => {
                    const minuteur = setTimeout(
                        () => rej(new Error('aucun message poussé par le canal en 2000 ms')),
                        2000,
                    );
                    enAttente.push((m) => {
                        clearTimeout(minuteur);
                        r(m);
                    });
                    const dejaLa = recus.shift();
                    if (dejaLa) enAttente.shift()!(dejaLa);
                });
            },
            dire(brut) {
                return new Promise((r, rej) => {
                    const minuteur = setTimeout(
                        () => rej(new Error(`aucune réponse du canal en 2000 ms à ${brut}`)),
                        2000,
                    );
                    enAttente.push((m) => {
                        clearTimeout(minuteur);
                        r(m);
                    });
                    const dejaLa = recus.shift();
                    if (dejaLa) enAttente.shift()!(dejaLa);
                    else w.send(brut);
                });
            },
        };
        w.on('message', (brut) => {
            pair.ordre.push('message');
            const m = JSON.parse(brut.toString()) as Record<string, unknown>;
            const attendu = enAttente.shift();
            if (attendu) attendu(m);
            else recus.push(m);
        });
        w.on('close', () => pair.ordre.push('close'));
        w.once('open', () => resolve(pair));
        w.once('error', (cause) => reject(cause));
    });
}

export function ouvrir(port: number): Promise<Pair> {
    return ouvrirUrl(`ws://127.0.0.1:${port}`);
}

/// Waits for `vu_a` to satisfy `predicat`, or FAILS after `borneMs`.
///
/// ⚠️ BOUNDED, AND FAILING ON EXPIRY. The write of `vu_a` is deliberately
/// launched WITHOUT being awaited (P1 rule: a promise rejected in a
/// `ws` handler takes down the whole process), so a lost write only
/// shows as a value that never arrives. An unbounded loop
/// would hang instead of turning red.
export async function attendreVu(
    p: Pilote,
    vmId: string,
    predicat: (vu: number | null) => boolean,
    quoi: string,
    borneMs = 2000,
): Promise<number | null> {
    const fin = Date.now() + borneMs;
    for (;;) {
        const ligne = await lireParVm(p, vmId);
        const vu = ligne?.vu_a === undefined || ligne.vu_a === null ? null : Number(ligne.vu_a);
        if (predicat(vu)) return vu;
        if (Date.now() > fin) {
            throw new Error(`vu_a ${quoi} jamais atteint pour ${vmId} en ${borneMs} ms (vu=${vu})`);
        }
        await new Promise((r) => setTimeout(r, 25));
    }
}
