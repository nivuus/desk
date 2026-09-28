import { describe, expect, it, vi } from 'vitest';
import {
    TYPE_ATTRIBUTS,
    TYPE_CREATE,
    TYPE_DATA,
    TYPE_ECHEC,
    TYPE_WRITE,
    TYPE_ENTREES,
    TYPE_FAIT,
    TYPE_RENOMMER,
    TYPE_DELETE,
    TYPE_LIRE,
    TYPE_LISTER,
    TYPE_META,
    encoder,
} from '../../../proto/ts/fichiers';
import { parseData, parseEchec, parseEntrees, parseMeta } from '../../../proto/ts/fichiers-entetes';
import { decoder } from '../../../proto/ts/fichiers';
import { FilesError, type Adaptateur } from './adaptateur';
import { createServer } from './protocole';
import type { Ecrivain } from './ecriture';
import type { Mutateur } from './mutation-service';

/** A fake writer: the protocol knows NO file system. */
function fauxEcrivain(surcharge: Partial<Ecrivain> = {}): Ecrivain & { vus: string[] } {
    const vus: string[] = [];
    return {
        vus,
        write: async (chemin, position, octets, premier, last) => {
            vus.push(`ecrire ${chemin} @${position} +${octets.length} ${premier}/${last}`);
        },
        create: async (chemin, repertoire) => {
            vus.push(`creer ${chemin} ${repertoire}`);
        },
        abandonner: () => vus.push('abandonner'),
        ...surcharge,
    };
}

/** A fake adapter: the protocol knows NO file system. */
function fauxAdaptateur(surcharge: Partial<Adaptateur> = {}): Adaptateur {
    return {
        lister: async () => [{ nom: 'a.txt', repertoire: false, taille: 7, modifie: 42 }],
        attributs: async () => ({
            nom: 'Nom Stocké.txt',
            repertoire: false,
            taille: 1234,
            modifie: 1_690_000_000_000,
        }),
        lire: async () => new Uint8Array([9, 8, 7]),
        ...surcharge,
    };
}

describe('file protocol server', () => {
    it('answers LISTER with ENTREES', async () => {
        const serveur = createServer(fauxAdaptateur());
        const reponse = await serveur.traiter(encoder(TYPE_LISTER, 11, { chemin: 'dossier' }));
        const trame = decoder(reponse!);
        expect(trame.type).toBe(TYPE_ENTREES);
        expect(trame.correlation).toBe(11);
        expect(parseEntrees(trame.entete).entrees[0].nom).toBe('a.txt');
        // Empty binary payload: the entries are in the header.
        expect(trame.charge.length).toBe(0);
    });

    it('answers ATTRIBUTS with META', async () => {
        const serveur = createServer(fauxAdaptateur());
        const trame = decoder((await serveur.traiter(encoder(TYPE_ATTRIBUTS, 3, { chemin: '' })))!);
        expect(trame.type).toBe(TYPE_META);
        expect(parseMeta(trame.entete).taille).toBe(1234);
        // 🔴 **THE CANONICAL NAME CROSSES THE WIRE.** Without it, the substitute
        // would be created under the name the application TYPED, and not under the one
        // that exists on the local workstation.
        expect(parseMeta(trame.entete).nom).toBe('Nom Stocké.txt');
    });

    it('🔴 answers LIRE with DONNEES whose length is the one ACTUALLY read', async () => {
        // 🔴 A file read up to its end returns FEWER bytes than requested.
        // Copying the REQUESTED length into the header would make the frame lie,
        // and the agent would refuse it for a header/payload mismatch — the only
        // check that exists to prevent writing into the ProjFS buffer
        // an amount the sender did not believe it was sending.
        const serveur = createServer(fauxAdaptateur({ lire: async () => new Uint8Array([1, 2]) }));
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_LIRE, 5, { chemin: 'gros.bin', position: 64, longueur: 4096 }),
            ))!,
        );
        expect(trame.type).toBe(TYPE_DATA);
        const entete = parseData(trame.entete);
        expect(entete.position).toBe(64);
        expect(entete.longueur).toBe(2);
        expect([...trame.charge]).toEqual([1, 2]);
    });

    it('🔴 an answer to an unknown correlation is ignored', async () => {
        // The browser is a SERVER: it never asks for anything, so no
        // correlation belongs to it. A frame of RESPONSE type can only be
        // an echo, a loop, or a confused peer — decoding it as a
        // request is the exact defect task 17 fixes on the agent side, where a
        // dispatch on the binary flag alone took every frame for a
        // mouse input.
        const journal = vi.fn();
        const serveur = createServer(fauxAdaptateur(), journal);
        for (const type of [TYPE_ENTREES, TYPE_META, TYPE_DATA, TYPE_ECHEC]) {
            expect(await serveur.traiter(encoder(type, 77, {}))).toBeNull();
        }
        expect(journal).toHaveBeenCalledTimes(4);
        // 🔴 THE LOG MUST SAY WHICH OF THE TWO CAUSES, and it is a
        // SURVIVING MUTATION that required it: removing the arm of the response
        // types let these frames fall into the "unknown type" catch-all,
        // which ignores them TOO — the test went green on a dispatch that
        // no longer named its cases. Yet the two causes do not call for the same
        // gesture: a received response says the agent is sending us back our own
        // traffic, an unknown type says the two ends do not have the same
        // version. Confusing the two is the catch-all arm of
        // `capteur/pont_media.rs`, which this repository paid for four times.
        for (const appel of journal.mock.calls) {
            expect(appel[0]).toMatch(/answer ignored/);
            expect(appel[0]).toMatch(/77/);
        }
    });

    it('an unknown type is ignored, and the log tells it apart from an answer', async () => {
        const journal = vi.fn();
        const serveur = createServer(fauxAdaptateur(), journal);
        expect(await serveur.traiter(encoder(200, 9, {}))).toBeNull();
        expect(journal.mock.calls[0][0]).toMatch(/unknown type/);
        expect(journal.mock.calls[0][0]).toMatch(/200/);
    });

    it('an unreadable frame is ignored rather than bringing the channel down', async () => {
        const journal = vi.fn();
        const serveur = createServer(fauxAdaptateur(), journal);
        const mauvaise = new Uint8Array(encoder(TYPE_LISTER, 1, { chemin: '' }));
        mauvaise[0] = 2; // version 2
        expect(await serveur.traiter(mauvaise.buffer as ArrayBuffer)).toBeNull();
        expect(journal.mock.calls[0][0]).toMatch(/version/i);
    });

    it('🔴 an adapter failure becomes a CODE, never a string', async () => {
        const serveur = createServer(
            fauxAdaptateur({
                attributs: async () => {
                    throw new FilesError('acces-refuse', 'permission revoked');
                },
            }),
        );
        const trame = decoder((await serveur.traiter(encoder(TYPE_ATTRIBUTS, 4, { chemin: 'x' })))!);
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(trame.correlation).toBe(4);
        expect(parseEchec(trame.entete).code).toBe('acces-refuse');
    });

    it('an unexpected failure becomes internal, and the correlation is RETURNED', async () => {
        // 🔴 Answering nothing would leave the command in flight on the agent side until
        // it times out: Explorer would freeze on a failure that is, for its part,
        // immediate.
        const serveur = createServer(
            fauxAdaptateur({
                lister: async () => {
                    throw new Error('something blew up');
                },
            }),
        );
        const trame = decoder((await serveur.traiter(encoder(TYPE_LISTER, 6, { chemin: '' })))!);
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(trame.correlation).toBe(6);
        expect(parseEchec(trame.entete).code).toBe('interne');
    });

    it('a malformed header is REFUSED, and the correlation is returned', async () => {
        const serveur = createServer(fauxAdaptateur());
        // `chemin` absent: the parser of the shared protocol throws.
        const trame = decoder((await serveur.traiter(encoder(TYPE_LISTER, 8, { rien: 1 })))!);
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(trame.correlation).toBe(8);
        expect(parseEchec(trame.entete).code).toBe('interne');
    });
});

describe('the F2 write verbs', () => {
    it('🔴 a write ALWAYS receives a FAIT or an ECHEC', async () => {
        // Returning nothing would leave the command in flight on the agent side until
        // `WRITE_TIMEOUT` — thirty seconds during which the write thread
        // would push nothing more, and the count of pending writes would not move.
        const ecrivain = fauxEcrivain();
        const serveur = createServer(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(
                encoder(
                    TYPE_WRITE,
                    7,
                    { chemin: 'note.txt', position: 0, longueur: 3, premier: true, dernier: true },
                    new Uint8Array([1, 2, 3]),
                ),
            ))!,
        );
        expect(trame.type).toBe(TYPE_FAIT);
        expect(trame.correlation).toBe(7);
        expect(ecrivain.vus).toEqual(['ecrire note.txt @0 +3 true/true']);
    });

    it('a creation receives a FAIT', async () => {
        const ecrivain = fauxEcrivain();
        const serveur = createServer(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(encoder(TYPE_CREATE, 8, { chemin: 'dossier', repertoire: true })))!,
        );
        expect(trame.type).toBe(TYPE_FAIT);
        expect(ecrivain.vus).toEqual(['creer dossier true']);
    });

    it('🔴 THE WRITER FAILURE CODE GOES THROUGH, it is not overwritten', async () => {
        // Returning `interne` for everything would destroy the cause at emission —
        // exactly the defect of `web/index.js:669`, which emitted
        // `JSON.stringify(e)` and returned `"{}"` for every `Error`.
        const ecrivain = fauxEcrivain({
            write: async () => {
                throw new FilesError('casse-ambigue', 'homonyme');
            },
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_WRITE, 9, {
                    chemin: 'a.txt',
                    position: 0,
                    longueur: 0,
                    premier: true,
                    dernier: true,
                }),
            ))!,
        );
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(parseEchec(trame.entete).code).toBe('casse-ambigue');
    });

    it('🔴 without a writer, the write is refused as `protege-en-ecriture`', async () => {
        // NOT `interne`: "this drive is read-only" and "the drive
        // is broken" do not call for the same gesture, and that is the whole point
        // of `CodeEchec`.
        const serveur = createServer(fauxAdaptateur());
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_WRITE, 1, {
                    chemin: 'a.txt',
                    position: 0,
                    longueur: 0,
                    premier: true,
                    dernier: true,
                }),
            ))!,
        );
        expect(parseEchec(trame.entete).code).toBe('protege-en-ecriture');
    });

    it('🔴 refuses a frame whose header and payload contradict each other', async () => {
        // Writing an amount of bytes the sender did not believe it was sending
        // is the kind of divergence no downstream check catches:
        // only a digest would tell.
        const ecrivain = fauxEcrivain();
        const serveur = createServer(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(
                encoder(
                    TYPE_WRITE,
                    2,
                    { chemin: 'a.txt', position: 0, longueur: 99, premier: true, dernier: true },
                    new Uint8Array([1, 2, 3]),
                ),
            ))!,
        );
        expect(trame.type).toBe(TYPE_ECHEC);
        // NOTHING must have been written: the refusal comes BEFORE the writer.
        expect(ecrivain.vus).toEqual([]);
    });
});

describe('reporting a write failure', () => {
    it('🔴 NAMES the file and the cause to the shell page', async () => {
        // 🔴 The browser is the ONLY one to know the cause, and it has nobody
        // to tell it to: the code does cross the wire, but it reaches
        // NO Windows application — the handle was closed long ago.
        // This callback is the shortest path to the only person it
        // concerns.
        const vus: Array<[string, string]> = [];
        const ecrivain = fauxEcrivain({
            write: async () => {
                throw new FilesError('disque-plein', 'no space left');
            },
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            ecrivain,
            onEchecEcriture: (chemin, code) => vus.push([chemin, code]),
        });
        await serveur.traiter(
            encoder(TYPE_WRITE, 4, {
                chemin: 'dossier/rapport.docx',
                position: 0,
                longueur: 0,
                premier: true,
                dernier: true,
            }),
        );
        expect(vus).toEqual([['dossier/rapport.docx', 'disque-plein']]);
    });

    it('names NOTHING when the header itself is unreadable', async () => {
        // Guessing a path that was not read would be worse than staying silent: the
        // shell page would name a file at random.
        const vus: unknown[] = [];
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            ecrivain: fauxEcrivain(),
            onEchecEcriture: (...a) => vus.push(a),
        });
        await serveur.traiter(encoder(TYPE_WRITE, 5, { rien: 'at all' }));
        expect(vus).toEqual([]);
    });
});


/** A fake mutator: the protocol knows NO file system. */
function fauxMutateur(surcharge: Partial<Mutateur> = {}): Mutateur & { vus: string[] } {
    const vus: string[] = [];
    return {
        vus,
        async renommer(de, vers, repertoire) {
            vus.push(`renommer ${de} -> ${vers} ${repertoire}`);
            return { parMove: true, octets: 0, entrees: 0 };
        },
        async remove(chemin, repertoire) {
            vus.push(`supprimer ${chemin} ${repertoire}`);
        },
        ...surcharge,
    };
}

describe('the two verbs of F3', () => {
    it('🔴 RENOMMER ALWAYS answers — with FAIT', async () => {
        // Red: returning `null`. The command would stay in flight on the bridge side
        // **until it times out**, and Explorer would freeze on a
        // failure that is nevertheless immediate. It is the invariant `protocole.ts` states
        // in capitals since F1, and that F3 does NOT relax: `TYPE_RENOMMER`
        // is a REQUEST, not an announcement.
        const mutateur = fauxMutateur();
        const serveur = createServer(fauxAdaptateur(), () => {}, { mutateur });
        const reponse = await serveur.traiter(
            encoder(TYPE_RENOMMER, 11, { de: 'a.txt', vers: 'b.txt', repertoire: false }),
        );
        expect(reponse).not.toBeNull();
        expect(decoder(reponse!).type).toBe(TYPE_FAIT);
        expect(mutateur.vus).toEqual(['renommer a.txt -> b.txt false']);
    });

    it('🔴 SUPPRIMER ALWAYS answers — with FAIT', async () => {
        const mutateur = fauxMutateur();
        const serveur = createServer(fauxAdaptateur(), () => {}, { mutateur });
        const reponse = await serveur.traiter(
            encoder(TYPE_DELETE, 12, { chemin: 'd', repertoire: true }),
        );
        expect(reponse).not.toBeNull();
        expect(decoder(reponse!).type).toBe(TYPE_FAIT);
        expect(mutateur.vus).toEqual(['supprimer d true']);
    });

    it('🔴 WITHOUT a mutator, it is `protege-en-ecriture` and NOT `interne`', async () => {
        // A drive mounted without a mutator and a broken drive do not call for
        // the same gesture — the counter-example is the old bridge, which returned
        // `EPERM` at nine distinct sites.
        const serveur = createServer(fauxAdaptateur(), () => {});
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_RENOMMER, 13, { de: 'a', vers: 'b', repertoire: false }),
            ))!,
        );
        expect(parseEchec(trame.entete).code).toBe('protege-en-ecriture');
    });

    it('🔴 a RENAME failure names BOTH paths', async () => {
        // "cannot rename X" does not say to what, and that is
        // precisely what the user must check.
        const vus: Array<[string, string]> = [];
        const mutateur = fauxMutateur({
            renommer: async () => {
                throw new FilesError('deja-present', 'already there');
            },
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            mutateur,
            onEchecMutation: (quoi, code) => vus.push([quoi, code]),
        });
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_RENOMMER, 14, { de: 'x.txt', vers: 'y.txt', repertoire: false }),
            ))!,
        );
        expect(parseEchec(trame.entete).code).toBe('deja-present');
        expect(vus).toEqual([['x.txt → y.txt', 'deja-present']]);
    });

    it('a REMOVAL failure names the path', async () => {
        const vus: Array<[string, string]> = [];
        const mutateur = fauxMutateur({
            remove: async () => {
                throw new FilesError('repertoire-non-vide', 'not empty');
            },
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            mutateur,
            onEchecMutation: (quoi, code) => vus.push([quoi, code]),
        });
        await serveur.traiter(encoder(TYPE_DELETE, 15, { chemin: 'd', repertoire: true }));
        expect(vus).toEqual([['d', 'repertoire-non-vide']]);
    });

    it('🔵 the COPY fallback is INSTRUMENTED, and `move()` is not', async () => {
        // The instrumentation spec §3.5.1 requires. It goes out through the log
        // because the browser is the ONLY one to know what it did.
        const vus: Array<[string, string, number, number]> = [];
        const parCopie = fauxMutateur({
            renommer: async () => ({ parMove: false, octets: 4096, entrees: 3 }),
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            mutateur: parCopie,
            onRenommagePorCopie: (de, vers, octets, entrees) =>
                vus.push([de, vers, octets, entrees]),
        });
        await serveur.traiter(
            encoder(TYPE_RENOMMER, 16, { de: 'p', vers: 'q', repertoire: true }),
        );
        expect(vus).toEqual([['p', 'q', 4096, 3]]);

        // And on the `move` branch, NOTHING is instrumented: there is nothing to
        // measure.
        vus.length = 0;
        const parMove = createServer(fauxAdaptateur(), () => {}, {
            mutateur: fauxMutateur(),
            onRenommagePorCopie: (de, vers, octets, entrees) =>
                vus.push([de, vers, octets, entrees]),
        });
        await parMove.traiter(
            encoder(TYPE_RENOMMER, 17, { de: 'a', vers: 'b', repertoire: false }),
        );
        expect(vus).toEqual([]);
    });
});
