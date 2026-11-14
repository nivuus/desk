import { describe, expect, it } from 'vitest';
import { FilesError } from './adaptateur';
import { renommer, remove, type RacineMutable } from './mutation';
import { monde, noeud } from './mutation.fixtures';

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

    it('🔴 copies through the STREAM when a whole-file read cannot be allocated', async () => {
        // Red: `file.slice(0, file.size).arrayBuffer()`, which materialises the
        // whole file in the tab — the fake refuses it, as a multi-gigabyte file would.
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        const trace = await renommer(m.racine, 'a.txt', 'b.txt', false);
        expect(m.arbre.files.get('b.txt')).toEqual(OCTETS);
        expect(trace.octets).toBe(OCTETS.length);
    });

    it('🔴 a stream that fails mid-copy ABORTS the writable, source intact', async () => {
        // Red: dropping the writable without `abort()` leaves its swap file and its lock.
        const m = monde((r) => r.files.set('a.txt', OCTETS), { failingStream: true });
        await expect(renommer(m.racine, 'a.txt', 'b.txt', false)).rejects.toBeInstanceOf(
            FilesError,
        );
        expect(m.compteurs.aborts).toBe(1);
        expect(m.arbre.files.get('a.txt')).toEqual(OCTETS);
        // Nothing was committed: the destination is the empty entry `create` made.
        expect(m.arbre.files.get('b.txt')).toEqual(new Uint8Array());
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

    it('🔴 is legal inside a SUB-FOLDER too, where the two parents are distinct handles', async () => {
        // Red: comparing the parent handles with `===`. The source and the
        // destination parents come from two separate walks, and the API returns
        // a new handle object on each call: only the root compared equal, and
        // `d/a.txt` → `d/A.txt` was refused as `deja-present`.
        const m = monde(
            (r) => {
                const d = noeud();
                d.files.set('a.txt', OCTETS);
                r.dossiers.set('d', d);
            },
            { insensible: true },
        );
        await renommer(m.racine, 'd/a.txt', 'd/A.txt', false);
        const d = m.arbre.dossiers.get('d')!;
        expect([...d.files.keys()]).toEqual(['A.txt']);
        expect(d.files.get('A.txt')).toEqual(OCTETS);
    });

    it('still refuses a case namesake in ANOTHER folder', async () => {
        // The same entry, not merely the same name: `d/a.txt` → `e/A.txt` with
        // `e/a.txt` present is a collision, not a case rename.
        const m = monde(
            (r) => {
                const d = noeud();
                d.files.set('a.txt', OCTETS);
                const e = noeud();
                e.files.set('a.txt', new Uint8Array([7]));
                r.dossiers.set('d', d);
                r.dossiers.set('e', e);
            },
            { insensible: true },
        );
        await expect(renommer(m.racine, 'd/a.txt', 'e/A.txt', false)).rejects.toMatchObject({
            code: 'deja-present',
        });
        expect(m.arbre.dossiers.get('e')!.files.get('a.txt')).toEqual(new Uint8Array([7]));
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
