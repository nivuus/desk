import { describe, expect, it } from 'vitest';
import { FilesError, type PoigneeBase } from './adaptateur';
import type { FluxInscriptible } from './ecriture';
import {
    renommer,
    remove,
    type MutableFileHandle,
    type RacineMutable,
} from './mutation';

/* ═══════════════════════════════════════════════════════════════════════════
   A MUTABLE FAKE FILE SYSTEM, IN TWO VARIANTS

   🔵 ONE EXPOSES `move()`, THE OTHER DOES NOT — and that is what makes BOTH
   branches of the rename green on the host. A type that required `move()`
   would make the fallback UNREACHABLE by a test.

   🔵 AND ONE IS CASE-INSENSITIVE, like the real local workstation (Windows,
   macOS by default) — the only way to exercise a pure case rename here.
   ═══════════════════════════════════════════════════════════════════════════ */

interface Noeud {
    files: Map<string, Uint8Array>;
    dossiers: Map<string, Noeud>;
}

function noeud(): Noeud {
    return { files: new Map(), dossiers: new Map() };
}

interface Compteurs {
    move: number;
    lectures: number;
    ecritures: number;
    octetsCopies: number;
}

interface Options {
    /** Is `move()` exposed? */
    withMove: boolean;
    /** Is the file system case-INSENSITIVE? */
    insensible: boolean;
    compteurs: Compteurs;
}

function absent(nom: string): never {
    throw new DOMException(`« ${nom} » cannot be found`, 'NotFoundError');
}

/** Finds the real key, taking case sensitivity into account. */
function cle(m: Map<string, unknown>, nom: string, o: Options): string | undefined {
    if (m.has(nom)) return nom;
    if (!o.insensible) return undefined;
    for (const k of m.keys()) if (k.toLowerCase() === nom.toLowerCase()) return k;
    return undefined;
}

function repertoire(n: Noeud, o: Options): RacineMutable {
    const self: RacineMutable = {
        kind: 'directory',
        name: 'd',
        async getDirectoryHandle(nom: string, opts?: { create?: boolean }): Promise<RacineMutable> {
            const k = cle(n.dossiers, nom, o);
            if (k !== undefined) return repertoire(n.dossiers.get(k)!, o);
            if (cle(n.files, nom, o) !== undefined) {
                throw new DOMException(`« ${nom} » is a file`, 'TypeMismatchError');
            }
            if (opts?.create !== true) absent(nom);
            const neuf = noeud();
            n.dossiers.set(nom, neuf);
            return repertoire(neuf, o);
        },
        async getFileHandle(
            nom: string,
            opts?: { create?: boolean },
        ): Promise<MutableFileHandle> {
            const k = cle(n.files, nom, o);
            if (k === undefined) {
                if (cle(n.dossiers, nom, o) !== undefined) {
                    throw new DOMException(`« ${nom} » is a folder`, 'TypeMismatchError');
                }
                if (opts?.create !== true) absent(nom);
                n.files.set(nom, new Uint8Array());
            }
            const reel = cle(n.files, nom, o)!;
            return file(n, reel, o);
        },
        values(): AsyncIterable<PoigneeBase> {
            return {
                async *[Symbol.asyncIterator]() {
                    for (const nom of n.dossiers.keys()) yield { kind: 'directory' as const, name: nom };
                    for (const nom of n.files.keys()) yield { kind: 'file' as const, name: nom };
                },
            };
        },
        async removeEntry(nom: string): Promise<void> {
            const kf = cle(n.files, nom, o);
            if (kf !== undefined) {
                n.files.delete(kf);
                return;
            }
            const kd = cle(n.dossiers, nom, o);
            if (kd === undefined) absent(nom);
            const enfant = n.dossiers.get(kd)!;
            // ⚠️ WITHOUT `recursive`: a non-empty directory is REFUSED, and the
            // real browser throws exactly this `DOMException`.
            if (enfant.files.size > 0 || enfant.dossiers.size > 0) {
                throw new DOMException(`« ${nom} » is not empty`, 'InvalidModificationError');
            }
            n.dossiers.delete(kd);
        },
    };
    if (o.withMove) {
        // The fake `move()` of a DIRECTORY. It OVERWRITES, like the real one.
        (self as { move?: unknown }).move = async (): Promise<void> => {
            throw new Error('directory move() is not exercised by this fake');
        };
    }
    return self;
}

function file(parent: Noeud, nom: string, o: Options): MutableFileHandle {
    const poignee: MutableFileHandle = {
        kind: 'file',
        name: nom,
        async getFile() {
            o.compteurs.lectures += 1;
            const octets = parent.files.get(nom)!;
            return {
                size: octets.length,
                lastModified: 0,
                slice(debut: number, fin: number) {
                    const t = octets.slice(debut, fin);
                    return {
                        arrayBuffer: async () =>
                            t.buffer.slice(t.byteOffset, t.byteOffset + t.byteLength) as ArrayBuffer,
                    };
                },
                arrayBuffer: async () =>
                    octets.buffer.slice(
                        octets.byteOffset,
                        octets.byteOffset + octets.byteLength,
                    ) as ArrayBuffer,
            };
        },
        async createWritable(): Promise<FluxInscriptible> {
            o.compteurs.ecritures += 1;
            let tampon = new Uint8Array();
            return {
                async write(d) {
                    const neuf = new Uint8Array(Math.max(tampon.length, d.position + d.data.length));
                    neuf.set(tampon);
                    neuf.set(d.data, d.position);
                    tampon = neuf;
                    o.compteurs.octetsCopies += d.data.length;
                },
                async close() {
                    parent.files.set(nom, tampon);
                },
            };
        },
    };
    if (o.withMove) {
        (poignee as { move?: unknown }).move = async (
            _dest: RacineMutable,
            newValue: string,
        ): Promise<void> => {
            o.compteurs.move += 1;
            const octets = parent.files.get(nom)!;
            parent.files.delete(nom);
            parent.files.set(newValue, octets);
        };
    }
    return poignee;
}

function monde(
    contenu: (racine: Noeud) => void,
    o: Partial<Options> = {},
): { racine: RacineMutable; arbre: Noeud; compteurs: Compteurs } {
    const arbre = noeud();
    contenu(arbre);
    const compteurs: Compteurs = { move: 0, lectures: 0, ecritures: 0, octetsCopies: 0 };
    const options: Options = {
        withMove: o.withMove ?? false,
        insensible: o.insensible ?? false,
        compteurs,
    };
    return { racine: repertoire(arbre, options), arbre, compteurs };
}

const OCTETS = new Uint8Array([1, 2, 3, 4, 5]);

describe('the rename WITH move()', () => {
    it('🔴 is ONE call and copies NOTHING', async () => {
        // Red: calling the fallback anyway. The fake counts its reads and
        // its writes, and the cost of spec §3.5.1 would become true again.
        const m = monde((r) => r.files.set('a.txt', OCTETS), { withMove: true });
        const trace = await renommer(m.racine, 'a.txt', 'b.txt', false);
        expect(trace.parMove).toBe(true);
        expect(m.compteurs.move).toBe(1);
        expect(m.compteurs.lectures).toBe(0);
        expect(m.compteurs.ecritures).toBe(0);
        expect(m.arbre.files.has('a.txt')).toBe(false);
        expect(m.arbre.files.get('b.txt')).toEqual(OCTETS);
    });
});

describe('the rename WITHOUT move() — the LOCAL fallback', () => {
    it('copies then removes, and the source disappears', async () => {
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        const trace = await renommer(m.racine, 'a.txt', 'b.txt', false);
        expect(trace.parMove).toBe(false);
        expect(trace.octets).toBe(5);
        expect(trace.entrees).toBe(1);
        expect(m.arbre.files.has('a.txt')).toBe(false);
        expect(m.arbre.files.get('b.txt')).toEqual(OCTETS);
    });

    it('🔴 renames a directory containing a SUB-DIRECTORY', async () => {
        // 🔴 IT IS THE DEFECT OF THE OLD BRIDGE, `web/index.js:631`: a temporal
        // dead zone (`const newDir = await newDir.getDirectoryHandle(…)`
        // in the block where `newDir` is the parameter) makes renaming a
        // directory containing a subdirectory ALWAYS fail there.
        const m = monde((r) => {
            const projet = noeud();
            const sub = noeud();
            sub.files.set('profond.txt', OCTETS);
            projet.dossiers.set('sous', sub);
            projet.files.set('note.txt', new Uint8Array([9]));
            r.dossiers.set('projet', projet);
        });
        const trace = await renommer(m.racine, 'projet', 'archives/projet 2026', true);
        expect(trace.parMove).toBe(false);
        expect(m.arbre.dossiers.has('projet')).toBe(false);
        const cible = m.arbre.dossiers.get('archives')!.dossiers.get('projet 2026')!;
        expect(cible.files.get('note.txt')).toEqual(new Uint8Array([9]));
        expect(cible.dossiers.get('sous')!.files.get('profond.txt')).toEqual(OCTETS);
    });

    it('🔴 NO byte goes through the channel', async () => {
        // Red: orchestrating the copy with `Lire` + `Write` from the bridge. The
        // cost of spec §3.5.1 — "2 GiB of channel for 1 GiB" — would become
        // true again. This module receives NO channel: the seam does not exist, and
        // that is what guarantees it structurally.
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        await renommer(m.racine, 'a.txt', 'b.txt', false);
        // The bytes did travel — LOCALLY, between two handles.
        expect(m.compteurs.octetsCopies).toBe(5);
        // And `renommer` never had a channel to talk to: its signature only
        // carries the root.
        expect(renommer.length).toBe(4);
    });

    it('🔴 an interrupted copy leaves the SOURCE intact', async () => {
        // Red: removing the source BEFORE the copy ends. A cut
        // would then lose the file.
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        const parent = m.racine as RacineMutable & {
            getFileHandle: RacineMutable['getFileHandle'];
        };
        const vrai = parent.getFileHandle.bind(parent);
        parent.getFileHandle = async (nom, opts) => {
            if (opts?.create === true) throw new DOMException('disk full', 'QuotaExceededError');
            return vrai(nom, opts);
        };
        await expect(renommer(m.racine, 'a.txt', 'b.txt', false)).rejects.toBeInstanceOf(
            FilesError,
        );
        expect(m.arbre.files.get('a.txt')).toEqual(OCTETS);
    });
});

describe('the PURE CASE rename', () => {
    it('🔴 goes through an intermediate name on an INSENSITIVE host', async () => {
        // Red: renaming directly. On the insensitive fake, the destination
        // "already exists" — and it is the source. A naive implementation refuses
        // (`deja-present`) or, worse, overwrites.
        const m = monde((r) => r.files.set('a.txt', OCTETS), { insensible: true });
        await renommer(m.racine, 'a.txt', 'A.txt', false);
        expect([...m.arbre.files.keys()]).toEqual(['A.txt']);
        expect(m.arbre.files.get('A.txt')).toEqual(OCTETS);
    });

    it('also passes on a SENSITIVE host, where there is no collision', async () => {
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        await renommer(m.racine, 'a.txt', 'A.txt', false);
        expect([...m.arbre.files.keys()]).toEqual(['A.txt']);
    });
});

describe('the rename refusals', () => {
    it('🔴 refuses to OVERWRITE an existing destination', async () => {
        // `move()` overwrites silently: without resolving first,
        // renaming `brouillon.txt` to `note.txt` would destroy `note.txt` without a
        // word.
        const m = monde((r) => {
            r.files.set('brouillon.txt', OCTETS);
            r.files.set('note.txt', new Uint8Array([7]));
        }, { withMove: true });
        await expect(
            renommer(m.racine, 'brouillon.txt', 'note.txt', false),
        ).rejects.toMatchObject({ code: 'deja-present' });
        expect(m.arbre.files.get('note.txt')).toEqual(new Uint8Array([7]));
        expect(m.compteurs.move).toBe(0);
    });

    it('refuses a missing source as `introuvable`', async () => {
        const m = monde(() => {});
        await expect(renommer(m.racine, 'x.txt', 'y.txt', false)).rejects.toMatchObject({
            code: 'introuvable',
        });
    });

    it('refuses the root as `non-supporte`', async () => {
        const m = monde(() => {});
        await expect(renommer(m.racine, '', 'y', false)).rejects.toMatchObject({
            code: 'non-supporte',
        });
    });

    it('resolves the SOURCE through the canonicaliser', async () => {
        const m = monde((r) => r.files.set('Casse.txt', OCTETS));
        await renommer(m.racine, 'casse.txt', 'neuf.txt', false);
        expect(m.arbre.files.get('neuf.txt')).toEqual(OCTETS);
    });
});

describe('la suppression', () => {
    it('removes a file', async () => {
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        await remove(m.racine, 'a.txt', false);
        expect(m.arbre.files.size).toBe(0);
    });

    it('removes an EMPTY directory', async () => {
        const m = monde((r) => r.dossiers.set('vide', noeud()));
        await remove(m.racine, 'vide', true);
        expect(m.arbre.dossiers.size).toBe(0);
    });

    it('🔴 a NON-EMPTY directory returns `repertoire-non-vide`, and nothing is destroyed', async () => {
        // Red: passing `recursive: true`. **The local workstation's subtree
        // would disappear**, and the test could no longer see it.
        //
        // 🔵 And the code becomes DIAGNOSTIC: receiving it means that the
        // mirror has drifted — the local workstation carries entries the VM does
        // not know.
        const m = monde((r) => {
            const d = noeud();
            d.files.set('inconnu-de-la-vm.txt', OCTETS);
            r.dossiers.set('d', d);
        });
        await expect(remove(m.racine, 'd', true)).rejects.toMatchObject({
            code: 'repertoire-non-vide',
        });
        expect(m.arbre.dossiers.get('d')!.files.size).toBe(1);
    });

    it('🔴 `InvalidModificationError` does NOT become `deja-present` here', async () => {
        // The SAME `DOMException` means two things depending on the verb: "an
        // entry of the same name exists" on a creation (what F2 wrote in
        // `classer`), "the directory is not empty" on a `removeEntry`.
        // The classification is done where the verb is known.
        const m = monde((r) => {
            const d = noeud();
            d.files.set('x', OCTETS);
            r.dossiers.set('d', d);
        });
        const e = await remove(m.racine, 'd', true).catch((x: unknown) => x as FilesError);
        expect((e as FilesError).code).not.toBe('deja-present');
    });

    it('resolves the path through the canonicaliser', async () => {
        const m = monde((r) => r.files.set('Casse.txt', OCTETS));
        await remove(m.racine, 'CASSE.TXT', false);
        expect(m.arbre.files.size).toBe(0);
    });

    it('refuses the root as `non-supporte`', async () => {
        const m = monde(() => {});
        await expect(remove(m.racine, '', true)).rejects.toMatchObject({
            code: 'non-supporte',
        });
    });
});

describe('removing the source tree, after a copy fallback', () => {
    it('🔴 does NOT carry away an entry that appeared in the meantime: it FAILS, source intact', async () => {
        // 🔵 That is what makes leaf-by-leaf removal SAFER than
        // `recursive: true`, and not only more verbose: we only remove
        // what the copy has just enumerated. An entry that appeared since makes
        // the removal fail, instead of being destroyed silently.
        const m = monde((r) => {
            const d = noeud();
            d.files.set('connu.txt', OCTETS);
            r.dossiers.set('d', d);
        });
        const source = m.arbre.dossiers.get('d')!;
        const racine = m.racine as RacineMutable & {
            removeEntry: RacineMutable['removeEntry'];
        };
        const vrai = racine.removeEntry.bind(racine);
        racine.removeEntry = async (nom) => {
            // Just BEFORE `retirerArbre` removes the directory — which it
            // has just emptied —, an entry appears on the local workstation, as
            // if the user had just dropped a file there.
            if (nom === 'd') source.files.set('surgi.txt', new Uint8Array([42]));
            return vrai(nom);
        };
        await expect(renommer(m.racine, 'd', 'e', true)).rejects.toBeInstanceOf(FilesError);
        // The source still EXISTS, and the entry that popped up was not destroyed.
        expect(m.arbre.dossiers.has('d')).toBe(true);
        expect(m.arbre.dossiers.get('d')!.files.has('surgi.txt')).toBe(true);
    });
});
