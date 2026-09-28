// The file protocol server, browser side. **PURE** — neither DOM, nor
// WebRTC, nor File System Access API: it receives bytes and returns some, and
// the adapter is injected into it.
//
// 🔴 THE BROWSER IS A SERVER, AND NOTHING ELSE. It never asks for anything:
// no correlation belongs to it. A frame carrying an ANSWER type
// (`TYPE_ENTREES`, `TYPE_META`, `TYPE_DATA`, `TYPE_ECHEC`) can therefore only be
// an echo, a loop, or a confused peer — it is IGNORED and logged,
// never interpreted as a request.
//
// It is the exact mirror of the defect task 17 fixes on the agent side, where
// `transport/evenements.rs` switched on the `data.binary` flag alone and
// therefore took any binary frame for a mouse input. A switch that does not
// name its cases is paid for with each new message — this repository paid for it four
// times on the catch-all arm of `capteur/pont_media.rs`.
//
// ⚠️ EVERY **REQUEST** RECEIVES AN ANSWER, including when it fails. Answering
// nothing would leave the command in flight on the agent side until it expires, and
// Explorer would freeze on a failure that is nonetheless immediate.
//
// 🔴 AN **ANNOUNCEMENT** RECEIVES NONE, AND THE LIST OF ANNOUNCEMENTS IS CLOSED.
// The invariant above was written in capitals and without exception — "A
// REQUEST ALWAYS RECEIVES AN ANSWER" — and F2 introduces a THIRD family
// of messages: `TYPE_DUES`, the announcement of owed writes. It expects nothing,
// and not answering it leaves NOTHING in flight: no table entry
// corresponds to it on the bridge side.
//
// The invariant is therefore REWRITTEN, never bypassed. An arm returning `null`
// WITHOUT the family being named would be exactly the silent catch-all
// arm this repository paid for FOUR times on `capteur/pont_media.rs` (D5
// `Sommeil`, D6 `Part`, D7 `Audio`, D8 `PleinEcran`).
//
// The three families, and what we do with them:
//   REQUESTS      (1..5, 7, 8) → an answer, always;
//   ANNOUNCEMENTS (6)          → an injected callback, and `null`;
//   ANSWERS       (64..)       → ignored: the browser asks for nothing.
//
// ⚠️ THE NUMBERING IS NOT CONTIGUOUS PER FAMILY: 6 is an ANNOUNCEMENT, 7 and 8
// are REQUESTS. F2 skipped 7 and 8 for F3, which avoided a late
// renumbering — and it is this NAMED switch that says the family, never the value.
//
// ⚠️ F3'S INVARIANT IS F1'S, AND NOT F2'S EXCEPTION:
// `TYPE_RENOMMER` and `TYPE_DELETE` are REQUESTS. They receive
// `Fait` or `Echec`, always. Answering nothing would leave the command in flight
// on the bridge side until it expires, and Explorer would freeze on a failure
// that is nonetheless immediate.
//
// The only case where we do not answer WITHOUT it being an announcement is when
// we have no request — unreadable frame, answer type, unknown type.

import {
    TYPE_ATTRIBUTS,
    TYPE_CREATE,
    TYPE_RENOMMER,
    TYPE_DELETE,
    TYPE_DATA,
    TYPE_BONJOUR,
    TYPE_DUES,
    TYPE_ECHEC,
    TYPE_WRITE,
    TYPE_ENTREES,
    TYPE_FAIT,
    TYPE_LIRE,
    TYPE_LISTER,
    TYPE_META,
    TYPE_RAFRAICHIR,
    decoder,
    encoderTexte,
} from '../../../proto/ts/fichiers';
import {
    encodeData,
    encodeEchec,
    encodeEntrees,
    encodeBonjour,
    encodeMeta,
    parseChemin,
    parseCreate,
    parseDues,
    parseWrite,
    parseLire,
    parseRenommer,
    parseDelete,
    type Due,
} from '../../../proto/ts/fichiers-entetes';
import { FilesError, type Adaptateur } from './adaptateur';
import type { Ecrivain } from './ecriture';
import type { Mutateur } from './mutation-service';

/** Where the frames we could not handle go. Injected, hence observable. */
export type Journal = (message: string) => void;

export interface Serveur {
    /**
     * Handles a received frame and returns the frame to send back, or `null` if there
     * is nothing to answer.
     */
    traiter(octets: ArrayBuffer): Promise<ArrayBuffer | null>;
}

/** What the server can do besides reading, since F2. */
export interface OptionsServeur {
    /** The writer. **Absent = read-only**, that is, F1's server. */
    ecrivain?: Ecrivain;
    /**
     * The callback of the `TYPE_DUES` announcement. **INJECTED**, hence observable: it is
     * the shell page that decides what to do with it, and this module stays PURE.
     */
    onDues?: (dues: Due[], retenues: boolean) => void;
    /**
     * The mutator — renaming and deletion. **Absent = read-only.**
     *
     * ⚠️ **DISTINCT from the writer, and staying so is the point**: `PONT_MUTATION`
     * and `PONT_ECRITURE` are two distinct bench variables on the agent side, and
     * confusing them would make a renaming acceptance run also cut the
     * temp+rename idiom it wants to exercise.
     */
    mutateur?: Mutateur;
    /**
     * A MUTATION failed here. Same reason as [`onEchecEcriture`]: the
     * browser is the only one knowing the cause, and it has no one else to
     * tell it to.
     *
     * ⚠️ **A rename carries TWO paths**, and the message must name
     * both: "cannot rename X" does not say to what, and that is
     * precisely what the user must check.
     */
    onEchecMutation?: (quoi: string, code: string) => void;
    /**
     * The rename fallback COPIED. **The instrumentation spec §3.5.1
     * requires.**
     *
     * ⚠️ **DECLARED DIVERGENCE FROM THE PLAN**, which has it "returned to the bridge
     * in the header of the `Fait` answer". `TYPE_FAIT` has **no shape
     * of its own** — its header is `{}`, F2 writes it out in full, and giving it
     * one would require one more shared vector for purely
     * diagnostic data. The trace therefore goes out through the injected log, **where the
     * browser KNOWS what it did**; the bridge, for its part, logs what IT
     * knows — it is the doctrine "a trace says what it KNOWS".
     */
    onRenommagePorCopie?: (de: string, vers: string, octets: number, entrees: number) => void;
    /**
     * A write failed HERE, on the browser side.
     *
     * 🔴 **THE BROWSER IS THE ONLY ONE KNOWING THE CAUSE, and it has no one to
     * tell it to.** The code does cross the wire to the agent, which
     * logs it — but **it reaches NO Windows application**: the handle
     * has been closed for a long time (see `pont::notifications`). This callback is
     * therefore the SHORTEST path to the only person it concerns:
     * the user, in front of their shell page.
     */
    onEchecEcriture?: (chemin: string, code: string) => void;
}

export function createServer(
    adaptateur: Adaptateur,
    journal: Journal = () => {},
    options: OptionsServeur = {},
): Serveur {
    return {
        async traiter(octets) {
            let trame;
            try {
                trame = decoder(octets);
            } catch (e) {
                journal(`unreadable frame, ignored: ${(e as Error).message}`);
                return null;
            }

            switch (trame.type) {
                // ── REQUESTS: they receive an answer, always.
                case TYPE_LISTER:
                case TYPE_ATTRIBUTS:
                case TYPE_LIRE:
                case TYPE_WRITE:
                case TYPE_CREATE:
                case TYPE_RENOMMER:
                case TYPE_DELETE:
                    break;
                // ── ANNOUNCEMENT: it receives NOTHING, and the family is NAMED.
                case TYPE_DUES: {
                    try {
                        const annonce = parseDues(trame.entete);
                        options.onDues?.(annonce.dues, annonce.retenues);
                    } catch (e) {
                        journal(`unreadable dues announcement: ${(e as Error).message}`);
                    }
                    return null;
                }
                // ── ANSWERS: the browser asks for nothing.
                case TYPE_FAIT:
                case TYPE_ENTREES:
                case TYPE_META:
                case TYPE_DATA:
                case TYPE_ECHEC:
                    journal(
                        `answer ignored: the browser requests nothing ` +
                            `(type=${trame.type}, correlation=${trame.correlation})`,
                    );
                    return null;
                default:
                    journal(
                        `unknown type ignored: type=${trame.type}, ` +
                            `correlation=${trame.correlation}`,
                    );
                    return null;
            }

            try {
                return await servir(
                    adaptateur,
                    options,
                    trame.type,
                    trame.correlation,
                    trame.entete,
                    trame.charge,
                );
            } catch (e) {
                // The code is the adapter's when it carries one; everything
                // else — malformed header included — is `interne`. Inventing
                // a more precise code would make the agent translate a wrong HRESULT
                // rather than a vague one.
                const code = e instanceof FilesError ? e.code : 'interne';
                journal(`failure ${code} on correlation ${trame.correlation}: ${(e as Error).message}`);
                if (trame.type === TYPE_RENOMMER || trame.type === TYPE_DELETE) {
                    // ⚠️ The path is reread from the header rather than kept:
                    // the failure may have come from its PARSING, in which case there is
                    // nothing to name.
                    const quoi = mutationDe(trame.type, trame.entete);
                    if (quoi !== undefined) options.onEchecMutation?.(quoi, code);
                } else if (trame.type === TYPE_WRITE || trame.type === TYPE_CREATE) {
                    // ⚠️ The path is reread from the header rather than kept:
                    // the failure may have come from its PARSING, in which case there is
                    // nothing to name, and guessing would be worse than keeping quiet.
                    const chemin = cheminDe(trame.entete);
                    if (chemin !== undefined) options.onEchecEcriture?.(chemin, code);
                }
                return encoderTexte(TYPE_ECHEC, trame.correlation, encodeEchec(code));
            }
        },
    };
}

/**
 * What a failed mutation must NAME, if the header is readable.
 *
 * ⚠️ **A rename carries TWO paths**, and both matter: "cannot
 * rename X" does not say to what, and that is precisely what the
 * user must check — the destination may already exist.
 */
function mutationDe(type: number, entete: unknown): string | undefined {
    if (typeof entete !== 'object' || entete === null) return undefined;
    const o = entete as { chemin?: unknown; de?: unknown; vers?: unknown };
    if (type === TYPE_DELETE) {
        return typeof o.chemin === 'string' ? o.chemin : undefined;
    }
    if (typeof o.de === 'string' && typeof o.vers === 'string') {
        return `${o.de} → ${o.vers}`;
    }
    return undefined;
}

/** The path of a write header, if it is readable. */
function cheminDe(entete: unknown): string | undefined {
    if (typeof entete !== 'object' || entete === null) return undefined;
    const chemin = (entete as { chemin?: unknown }).chemin;
    return typeof chemin === 'string' ? chemin : undefined;
}

async function servir(
    adaptateur: Adaptateur,
    options: OptionsServeur,
    type: number,
    correlation: number,
    entete: unknown,
    charge: Uint8Array,
): Promise<ArrayBuffer> {
    const ecrivain: Ecrivain | undefined = options.ecrivain;
    if (type === TYPE_RENOMMER || type === TYPE_DELETE) {
        const mutateur = options.mutateur;
        if (mutateur === undefined) {
            // ⚠️ **NOT `interne`: `protege-en-ecriture`.** A drive mounted
            // without a mutator and a failed drive do not call for the same
            // gesture — the counterexample is the old bridge, which returned `EPERM`
            // at nine distinct sites.
            throw new FilesError(
                'protege-en-ecriture',
                'this drive can neither rename nor remove',
            );
        }
        if (type === TYPE_RENOMMER) {
            const r = parseRenommer(entete);
            const trace = await mutateur.renommer(r.de, r.vers, r.repertoire);
            if (!trace.parMove) {
                options.onRenommagePorCopie?.(r.de, r.vers, trace.octets, trace.entrees);
            }
        } else {
            const s = parseDelete(entete);
            await mutateur.remove(s.chemin, s.repertoire);
        }
        return encoderTexte(TYPE_FAIT, correlation, '{}');
    }
    if (type === TYPE_WRITE || type === TYPE_CREATE) {
        if (ecrivain === undefined) {
            // ⚠️ **NOT `interne`: `protege-en-ecriture`.** A server mounted
            // read-only and a failed server do not call for the same
            // gesture, and that is the whole point of `CodeEchec` — the counterexample
            // is the old bridge, which returned `EPERM` at nine distinct sites.
            throw new FilesError(
                'protege-en-ecriture',
                'this drive is mounted read-only',
            );
        }
        if (type === TYPE_WRITE) {
            const e = parseWrite(entete);
            // 🔴 **THE HEADER AND THE PAYLOAD MUST CORROBORATE EACH OTHER.** Writing a
            // quantity of bytes the sender did not believe it was sending is the
            // kind of divergence no downstream check catches: only
            // a digest would say so. It is the exact mirror of the check
            // the agent already applies to `Data` answers.
            if (e.longueur !== charge.length) { // policy: allow-fr - wire key of the file protocol
                throw new FilesError(
                    'interne',
                    `inconsistent Ecrire header: ${e.longueur} announced, ${charge.length} received`,
                );
            }
            await ecrivain.write(e.chemin, e.position, charge, e.premier, e.dernier); // policy: allow-fr - wire key of the file protocol
        } else {
            const c = parseCreate(entete);
            await ecrivain.create(c.chemin, c.repertoire);
        }
        // ⚠️ **EMPTY HEADER `{}`.** `TYPE_FAIT` has no shape of its own: what
        // identifies the acknowledged write is the CORRELATION, not the header.
        return encoderTexte(TYPE_FAIT, correlation, '{}');
    }
    if (type === TYPE_LISTER) {
        const { chemin } = parseChemin(entete);
        const entrees = await adaptateur.lister(chemin);
        // Empty binary payload: the entries fit in the header.
        return encoderTexte(TYPE_ENTREES, correlation, encodeEntrees(entrees));
    }
    if (type === TYPE_ATTRIBUTS) {
        const { chemin } = parseChemin(entete);
        const m = await adaptateur.attributs(chemin);
        return encoderTexte(
            TYPE_META,
            correlation,
            encodeMeta(m.nom, m.repertoire, m.taille, m.modifie), // policy: allow-fr - wire keys of the file protocol
        );
    }
    const { chemin, position, longueur: length } = parseLire(entete); // policy: allow-fr - wire key of the file protocol
    const octets = await adaptateur.lire(chemin, position, length);
    // 🔴 THE ANNOUNCED LENGTH IS THE ONE ACTUALLY READ, never the one requested.
    // A file read to its end returns less; copying the request would make
    // the frame lie, and the agent would refuse it for header/payload inconsistency
    // — it is the only check preventing writing into ProjFS's buffer
    // a quantity the sender did not believe it was sending.
    return encoderTexte(TYPE_DATA, correlation, encodeData(position, octets.length), octets);
}

/**
 * The frame of the `Bonjour` announcement — **browser → bridge**.
 *
 * 🔴 **It expects NO answer, and its correlation is IGNORED**: the bridge
 * routes it **before** looking up a correlation in the table, precisely because
 * it has none. The value `0` is therefore padding, not an identifier.
 *
 * ⚠️ **`racine` is the `name` of the directory handle, and it is a HINT,
 * not a proof**: `isSameEntry()` compares two live handles, never a
 * handle with a memory. Two namesake directories on two different disks
 * would pass for a single one, and nothing here would say so.
 */
export function trameBonjour(racine: string, forcer: boolean): ArrayBuffer {
    return encoderTexte(TYPE_BONJOUR, 0, encodeBonjour(racine, forcer));
}

/**
 * The frame of the `Rafraichir` announcement — **browser → bridge**.
 *
 * Its header is `{}`: what identifies it is its TYPE. Giving it a shape
 * would make a structure to pin that pins nothing — the precedent of
 * `TYPE_FAIT`, written in `proto/src/files/entetes.rs`.
 */
export function trameRafraichir(): ArrayBuffer {
    return encoderTexte(TYPE_RAFRAICHIR, 0, '{}');
}
