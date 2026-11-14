import { describe, expect, it } from 'vitest';
import { FilesError } from './adaptateur';
import { createWriter, type FluxInscriptible, type RacineInscriptible } from './ecriture';

/* ── A FAKE FILE SYSTEM, CASE-INSENSITIVE BY CONSTRUCTION ─────────────────
   🔴 INSENSITIVITY IS THE POINT, NOT A DETAIL OF CONVENIENCE. It is what
   Windows and macOS do by default, and it is what makes the loss possible:
   `getFileHandle('CASSE.TXT', { create: true })` opens `Casse.txt` there. A
   case-SENSITIVE fake would make the guard's test VACUOUS — it would
   simply create a second file, and nothing would ever be overwritten. */

interface Noeud {
    kind: 'file' | 'directory';
    name: string;
    contenu: number[];
    enfants: Map<string, Noeud>;
}

function dossier(name = ''): Noeud {
    return { kind: 'directory', name, contenu: [], enfants: new Map() };
}

class Faux {
    ouverts = 0;
    fermes = 0;
    /** Streams aborted: their swap file is discarded, nothing is committed. */
    aborts = 0;
    /** When set, every `write()` of a stream fails. */
    failingWrites = false;
    racineNoeud = dossier();

    /**
     * `caseSensitive`: a byte-exact host (Linux), where `Docs` and `docs`, or a
     * composed and a decomposed name, are distinct entries. By default the fake
     * is case-INSENSITIVE, like an ordinary local workstation.
     */
    constructor(private readonly caseSensitive = false) {}

    private trouver(parent: Noeud, nom: string): Noeud | undefined {
        for (const enfant of parent.enfants.values()) {
            if (this.caseSensitive ? enfant.name === nom : enfant.name.toLowerCase() === nom.toLowerCase()) {
                return enfant;
            }
        }
        return undefined;
    }

    /** The names a directory holds, in insertion order. */
    names(path = ''): string[] {
        let node = this.racineNoeud;
        for (const part of path.split('/').filter((c) => c.length > 0)) {
            const next = this.trouver(node, part);
            if (next === undefined) return [];
            node = next;
        }
        return [...node.enfants.values()].map((n) => n.name);
    }

    poser(chemin: string, contenu: number[] = []): void {
        const parts = chemin.split('/');
        let ici = this.racineNoeud;
        for (const p of parts.slice(0, -1)) {
            let next = this.trouver(ici, p);
            if (next === undefined) {
                next = dossier(p);
                ici.enfants.set(p, next);
            }
            ici = next;
        }
        const nom = parts[parts.length - 1];
        ici.enfants.set(nom, { kind: 'file', name: nom, contenu, enfants: new Map() });
    }

    lire(chemin: string): { nom: string; contenu: number[] } | undefined {
        const parts = chemin.split('/');
        let ici = this.racineNoeud;
        for (const p of parts.slice(0, -1)) {
            const next = this.trouver(ici, p);
            if (next === undefined) return undefined;
            ici = next;
        }
        const n = this.trouver(ici, parts[parts.length - 1]);
        return n === undefined ? undefined : { nom: n.name, contenu: n.contenu };
    }

    racine(): RacineInscriptible {
        return this.envelopper(this.racineNoeud);
    }

    private envelopper(noeud: Noeud): RacineInscriptible {
        const faux = this;
        return {
            kind: 'directory',
            name: noeud.name,
            async getDirectoryHandle(nom, options) {
                let enfant = faux.trouver(noeud, nom);
                if (enfant === undefined) {
                    if (!options?.create) throw new DOMException('absent', 'NotFoundError');
                    enfant = dossier(nom);
                    noeud.enfants.set(nom, enfant);
                }
                return faux.envelopper(enfant);
            },
            async getFileHandle(nom, options) {
                let enfant = faux.trouver(noeud, nom);
                if (enfant === undefined) {
                    if (!options?.create) throw new DOMException('absent', 'NotFoundError');
                    enfant = { kind: 'file', name: nom, contenu: [], enfants: new Map() };
                    noeud.enfants.set(nom, enfant);
                }
                const cible = enfant;
                return {
                    kind: 'file',
                    name: cible.name,
                    async getFile() {
                        return {
                            size: cible.contenu.length,
                            lastModified: 0,
                            slice: () => ({ arrayBuffer: async () => new ArrayBuffer(0) }),
                            arrayBuffer: async () => new ArrayBuffer(0),
                        };
                    },
                    async createWritable(options): Promise<FluxInscriptible> {
                        faux.ouverts += 1;
                        // 🔴 WITHOUT `keepExistingData`, THE FILE STARTS FROM ZERO.
                        // It is what the real `createWritable()` does, and it is
                        // what keeps a file rewritten shorter from keeping
                        // its tail of bytes.
                        const tampon = options?.keepExistingData ? [...cible.contenu] : [];
                        return {
                            async write({ position, data }) {
                                if (faux.failingWrites) {
                                    throw new DOMException('disk full', 'QuotaExceededError');
                                }
                                for (let i = 0; i < data.length; i += 1) {
                                    tampon[position + i] = data[i];
                                }
                            },
                            async close() {
                                faux.fermes += 1;
                                cible.contenu = tampon.map((o) => o ?? 0);
                            },
                            async abort() {
                                faux.aborts += 1;
                            },
                        };
                    },
                };
            },
            async *values() {
                for (const enfant of noeud.enfants.values()) {
                    yield { kind: enfant.kind, name: enfant.name };
                }
            },
        };
    }
}

const octets = (...o: number[]) => new Uint8Array(o);

describe('the case guard', () => {
    it('🔴 REFUSES to write into a case homonym', async () => {
        const faux = new Faux();
        faux.poser('Casse.txt', [1, 2, 3]);
        const e = createWriter(faux.racine());
        await expect(e.write('CASSE.TXT', 0, octets(9), true, true)).rejects.toThrow(
            FilesError,
        );
    });

    it('🔴 …AND `Casse.txt` IS NOT OVERWRITTEN — this is THE F2 test', async () => {
        // 🔴 **A SEPARATE TEST, AND IT IS LESSON ①A-bis OF P2.** `expect`
        // stops a test at its FIRST failing assertion: putting the refusal
        // and the non-overwrite in the same test would mean the second would be
        // EXERCISED BY NOTHING as soon as the first falls — and it is the second that
        // carries the data loss. The red shows it: without the guard, the one
        // above fails on "promise resolved instead of rejecting", and
        // this one on the CONTENT.
        const faux = new Faux();
        faux.poser('Casse.txt', [1, 2, 3]);
        const e = createWriter(faux.racine());
        await e.write('CASSE.TXT', 0, octets(9), true, true).catch(() => {});
        expect(faux.lire('Casse.txt')).toEqual({ nom: 'Casse.txt', contenu: [1, 2, 3] });
    });

    it('🔴 …AND NO STREAM IS EVEN OPENED', async () => {
        // Third assertion, third test, same reason. "Nothing is
        // written" and "nothing is even opened" do not follow from one
        // another: a stream opened then abandoned leaves a swap file.
        const faux = new Faux();
        faux.poser('Casse.txt', [1, 2, 3]);
        const e = createWriter(faux.racine());
        await e.write('CASSE.TXT', 0, octets(9), true, true).catch(() => {});
        expect(faux.ouverts).toBe(0);
    });

    it('carries the `casse-ambigue` code and NAMES both files', async () => {
        const faux = new Faux();
        faux.poser('Casse.txt');
        const e = createWriter(faux.racine());
        const error = await e
            .write('CASSE.TXT', 0, octets(9), true, true)
            .then(() => undefined)
            .catch((x: unknown) => x as FilesError);
        expect(error).toBeInstanceOf(FilesError);
        if (error === undefined) throw new Error('inatteignable');
        expect(error.code).toBe('casse-ambigue');
        // The message stays in the console and in the shell page; it must say
        // what the user can do, that is, rename one of the two.
        expect(error.message).toContain('CASSE.TXT');
        expect(error.message).toContain('Casse.txt');
    });

    it('also refuses an ambiguous CREATION', async () => {
        const faux = new Faux();
        faux.poser('Dossier');
        const e = createWriter(faux.racine());
        await expect(e.create('DOSSIER', true)).rejects.toThrow(/case/);
    });

    it('writes into the EXACT name when it exists', async () => {
        // 🔴 A guard that is too strict would make ALL writes fail: this test is
        // what prevents it.
        const faux = new Faux();
        faux.poser('Casse.txt', [1, 2, 3]);
        const e = createWriter(faux.racine());
        await e.write('Casse.txt', 0, octets(7, 8), true, true);
        expect(faux.lire('Casse.txt')?.contenu).toEqual([7, 8]);
    });

    it('creates when nothing resembles the requested name', async () => {
        const faux = new Faux();
        faux.poser('autre.txt');
        const e = createWriter(faux.racine());
        await e.write('neuf.txt', 0, octets(4), true, true);
        expect(faux.lire('neuf.txt')?.contenu).toEqual([4]);
    });
});

describe('les flux', () => {
    it('🔴 a file rewritten SHORTER does not keep its tail of bytes', async () => {
        // 🔴 It is the EXACT defect of the old bridge (spec §12): it used
        // `keepExistingData: true` without `truncate`. Passing `true` here makes
        // the tail survive, and the local file then carries a content the
        // VM NEVER had.
        const faux = new Faux();
        faux.poser('note.txt', [1, 2, 3, 4, 5, 6, 7, 8]);
        const e = createWriter(faux.racine());
        await e.write('note.txt', 0, octets(9, 9), true, true);
        expect(faux.lire('note.txt')?.contenu).toEqual([9, 9]);
    });

    it('opens ONCE and closes ONCE, over several chunks', async () => {
        const faux = new Faux();
        const e = createWriter(faux.racine());
        await e.write('gros.bin', 0, octets(1, 2), true, false);
        await e.write('gros.bin', 2, octets(3, 4), false, false);
        await e.write('gros.bin', 4, octets(5), false, true);
        expect([faux.ouverts, faux.fermes]).toEqual([1, 1]);
        expect(faux.lire('gros.bin')?.contenu).toEqual([1, 2, 3, 4, 5]);
    });

    it('writes NOTHING until the last chunk has arrived', async () => {
        // 🔵 The ATOMICITY of `createWritable()`: the commit happens at
        // `close()`. An interrupted push leaves the local file UNCHANGED.
        const faux = new Faux();
        faux.poser('note.txt', [42]);
        const e = createWriter(faux.racine());
        await e.write('note.txt', 0, octets(1, 2), true, false);
        // Unchanged BEFORE the `close()`: that is atomicity.
        expect(faux.lire('note.txt')?.contenu).toEqual([42]);
        await e.write('note.txt', 2, octets(3), false, true);
        expect(faux.lire('note.txt')?.contenu).toEqual([1, 2, 3]);
    });

    it('🔴 abandoning ABORTS the streams left open, and commits nothing', async () => {
        // Red: `close()` here COMMITTED an interrupted push — the local file was
        // replaced by the bytes that happened to have arrived.
        const faux = new Faux();
        faux.poser('a.txt', [42]);
        const e = createWriter(faux.racine());
        await e.write('a.txt', 0, octets(1), true, false);
        e.abandonner();
        // `abandonner` is SYNCHRONOUS: the `abort()` is started without being awaited,
        // because it is called from the closing of the channel, which is too.
        await Promise.resolve();
        expect([faux.fermes, faux.aborts]).toEqual([0, 1]);
        expect(faux.lire('a.txt')?.contenu).toEqual([42]);
    });

    it('🔴 a replay ABORTS the previous stream instead of committing it', async () => {
        // Red: `close()` on the interrupted stream COMMITTED its partial bytes
        // to the local file before the replay started over.
        const faux = new Faux();
        faux.poser('a.txt', [42]);
        const e = createWriter(faux.racine());
        await e.write('a.txt', 0, octets(1), true, false);
        // The push is interrupted, then restarted from the beginning.
        await e.write('a.txt', 0, octets(7, 7), true, false);
        expect([faux.ouverts, faux.fermes, faux.aborts]).toEqual([2, 0, 1]);
        expect(faux.lire('a.txt')?.contenu).toEqual([42]);
        await e.write('a.txt', 2, octets(7), false, true);
        expect(faux.lire('a.txt')?.contenu).toEqual([7, 7, 7]);
    });

    it('🔴 a failed write ABORTS its stream before forgetting it', async () => {
        // Red: the stream was dropped, neither closed nor aborted — its swap
        // file and its lock stayed behind.
        const faux = new Faux();
        const e = createWriter(faux.racine());
        faux.failingWrites = true;
        await expect(e.write('a.txt', 0, octets(1), true, false)).rejects.toMatchObject({
            code: 'disque-plein',
        });
        expect(faux.aborts).toBe(1);
        // Forgotten: the next chunk of that push is refused, not written to a dead stream.
        faux.failingWrites = false;
        await expect(e.write('a.txt', 1, octets(2), false, true)).rejects.toThrow(/stream/);
    });

    it('🔴 refuses a NON-initial chunk without an open stream', async () => {
        // Opening here would write a file TRUNCATED to this chunk: the
        // truncation would be silent, which is worse than a refusal.
        const faux = new Faux();
        const e = createWriter(faux.racine());
        await expect(e.write('a.txt', 64, octets(1), false, true)).rejects.toThrow(/stream/);
        expect(faux.lire('a.txt')).toBeUndefined();
    });
});

describe('the creations', () => {
    it('creates a directory, and an EMPTY file without truncating it', async () => {
        const faux = new Faux();
        faux.poser('deja.txt', [1, 2, 3]);
        const e = createWriter(faux.racine());
        await e.create('dossier', true);
        await e.create('deja.txt', false);
        // 🔴 A CREATION OPENS NO STREAM: opening one would TRUNCATE the
        // existing local file, whereas a creation has no effect on what
        // is already there.
        expect(faux.lire('deja.txt')?.contenu).toEqual([1, 2, 3]);
        expect(faux.ouverts).toBe(0);
    });

    it('creates the intermediate directories of a deep path', async () => {
        const faux = new Faux();
        const e = createWriter(faux.racine());
        await e.write('a/b/c.txt', 0, octets(5), true, true);
        expect(faux.lire('a/b/c.txt')?.contenu).toEqual([5]);
    });
});

// Composed (NFC) and decomposed (NFD) spellings of the same names, written as
// escapes so that the two sequences are unmistakable: an accented Latin name and
// a Korean (Hangul) one, where NFD splits each syllable into conjoining jamo.
const RESUME_NFC = 'r\u00e9sum\u00e9';
const RESUME_NFD = 're\u0301sume\u0301';
const HANGUL_NFC = '\ud55c\uae00';
const HANGUL_NFD = '\u1112\u1161\u11ab\u1100\u1173\u11af';

describe('the shared name resolution, on the write path', () => {
    it('the four spellings really are two pairs of distinct sequences', () => {
        // A guard for the test itself: equal strings would make the cases below vacuous.
        expect(RESUME_NFC).not.toBe(RESUME_NFD);
        expect(RESUME_NFD.normalize('NFC')).toBe(RESUME_NFC);
        expect(HANGUL_NFC).not.toBe(HANGUL_NFD);
        expect(HANGUL_NFD.normalize('NFC')).toBe(HANGUL_NFC);
    });

    it('🔴 on a case-SENSITIVE host, a write to `Docs/x` lands in the existing `docs`', async () => {
        // Red: the writer created a sibling `Docs/` while the reader resolves
        // `Docs/` to `docs/` — the VM would never see the file it wrote.
        const faux = new Faux(true);
        faux.poser('docs/autre.txt');
        const e = createWriter(faux.racine());
        await e.write('Docs/x.txt', 0, octets(5), true, true);
        expect(faux.names()).toEqual(['docs']);
        expect(faux.lire('docs/x.txt')?.contenu).toEqual([5]);
    });

    for (const [script, nfc, nfd] of [
        ['accented Latin', RESUME_NFC, RESUME_NFD],
        ['Hangul', HANGUL_NFC, HANGUL_NFD],
    ] as const) {
        it(`🔴 a composed directory name matches the entry stored decomposed (${script})`, async () => {
            // Red: the private fold had no Unicode normalisation, so a second,
            // composed directory was created next to the decomposed one.
            const faux = new Faux(true);
            faux.poser(`${nfd}/autre.txt`);
            const e = createWriter(faux.racine());
            await e.write(`${nfc}/x.txt`, 0, octets(5), true, true);
            expect(faux.names()).toEqual([nfd]);
            expect(faux.lire(`${nfd}/x.txt`)?.contenu).toEqual([5]);
        });

        it(`🔴 a composed FILE name is refused next to its decomposed namesake (${script})`, async () => {
            // Red: a DUPLICATE was created. The final component keeps the F2
            // guard: a namesake that is not the exact name is refused, never
            // overwritten — normalisation now counts as well as case.
            const faux = new Faux(true);
            faux.poser(`${nfd}.txt`, [1, 2, 3]);
            const e = createWriter(faux.racine());
            await expect(e.write(`${nfc}.txt`, 0, octets(9), true, true)).rejects.toMatchObject({
                code: 'casse-ambigue',
            });
            expect(faux.names()).toEqual([`${nfd}.txt`]);
            expect(faux.lire(`${nfd}.txt`)?.contenu).toEqual([1, 2, 3]);
        });
    }

    it('🔴 an intermediate component that is AMBIGUOUS is refused, nothing written', async () => {
        const faux = new Faux(true);
        faux.poser('docs/a.txt');
        faux.poser('Docs/b.txt');
        const e = createWriter(faux.racine());
        await expect(e.write('DOCS/x.txt', 0, octets(5), true, true)).rejects.toMatchObject({
            code: 'casse-ambigue',
        });
        expect(faux.names()).toEqual(['docs', 'Docs']);
    });
});

describe('the classification of failures', () => {
    it('🔴 an exceeded quota returns `disque-plein`, and not `interne`', async () => {
        // Letting it fall into the `default` of `classer` would give `interne`, and
        // the log would no longer say WHY: the user would not know
        // they have to free up space.
        const faux = new Faux();
        const racine = faux.racine();
        const vraiGetFileHandle = racine.getFileHandle.bind(racine);
        racine.getFileHandle = async (nom, options) => {
            const poignee = await vraiGetFileHandle(nom, options);
            return {
                ...poignee,
                createWritable: async () => {
                    throw new DOMException('plein', 'QuotaExceededError');
                },
            };
        };
        const e = createWriter(racine);
        const error = await e
            .write('a.txt', 0, octets(1), true, true)
            .then(() => undefined)
            .catch((x: unknown) => x as FilesError);
        expect(error).toBeInstanceOf(FilesError);
        expect(error?.code).toBe('disque-plein');
    });
});
