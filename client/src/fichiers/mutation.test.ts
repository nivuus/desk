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
   UN FAUX SYSTÈME DE FICHIERS MUTABLE, EN DEUX VARIANTES

   🔵 L'UNE EXPOSE `move()`, L'AUTRE NON — et c'est ce qui rend les DEUX
   branches du renommage vertes sur l'hôte. Un type qui imposerait `move()`
   rendrait le repli INATTEIGNABLE par un test.

   🔵 ET L'UNE EST INSENSIBLE À LA CASSE, comme le poste local réel (Windows,
   macOS par défaut) — la seule façon d'éprouver ici le renommage de casse pure.
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
    /** `move()` est-elle exposée ? */
    withMove: boolean;
    /** Le système de fichiers est-il INSENSIBLE à la casse ? */
    insensible: boolean;
    compteurs: Compteurs;
}

function absent(nom: string): never {
    throw new DOMException(`« ${nom} » est introuvable`, 'NotFoundError');
}

/** Trouve la clé réelle, en tenant compte de la sensibilité à la casse. */
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
                throw new DOMException(`« ${nom} » est un fichier`, 'TypeMismatchError');
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
                    throw new DOMException(`« ${nom} » est un dossier`, 'TypeMismatchError');
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
            // ⚠️ SANS `recursive` : un répertoire non vide est REFUSÉ, et le
            // navigateur réel lève exactement cette `DOMException`.
            if (enfant.files.size > 0 || enfant.dossiers.size > 0) {
                throw new DOMException(`« ${nom} » n'est pas vide`, 'InvalidModificationError');
            }
            n.dossiers.delete(kd);
        },
    };
    if (o.withMove) {
        // Le faux `move()` d'un RÉPERTOIRE. Il ÉCRASE, comme le vrai.
        (self as { move?: unknown }).move = async (): Promise<void> => {
            throw new Error('move() de répertoire n’est pas exercée par ce faux');
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

describe('le renommage AVEC move()', () => {
    it('🔴 est UN appel et ne copie RIEN', async () => {
        // Rouge : appeler le repli quand même. Le faux compte ses lectures et
        // ses écritures, et le coût de la spec §3.5.1 redeviendrait vrai.
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

describe('le renommage SANS move() — le repli LOCAL', () => {
    it('copie puis supprime, et la source disparaît', async () => {
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        const trace = await renommer(m.racine, 'a.txt', 'b.txt', false);
        expect(trace.parMove).toBe(false);
        expect(trace.octets).toBe(5);
        expect(trace.entrees).toBe(1);
        expect(m.arbre.files.has('a.txt')).toBe(false);
        expect(m.arbre.files.get('b.txt')).toEqual(OCTETS);
    });

    it('🔴 renomme un répertoire contenant un SOUS-RÉPERTOIRE', async () => {
        // 🔴 C'EST LE DÉFAUT DE L'ANCIEN PONT, `web/index.js:631` : une zone
        // morte temporelle (`const newDir = await newDir.getDirectoryHandle(…)`
        // dans le bloc où `newDir` est le paramètre) fait que le renommage d'un
        // répertoire contenant un sous-répertoire y échoue TOUJOURS.
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

    it('🔴 AUCUN octet ne passe par le canal', async () => {
        // Rouge : orchestrer la copie par `Lire` + `Write` depuis le pont. Le
        // coût de la spec §3.5.1 — « 2 Gio de canal pour 1 Gio » — redeviendrait
        // vrai. Ce module ne reçoit AUCUN canal : la couture n'existe pas, et
        // c'est ce qui le garantit structurellement.
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        await renommer(m.racine, 'a.txt', 'b.txt', false);
        // Les octets ont bien transité — LOCALEMENT, entre deux poignées.
        expect(m.compteurs.octetsCopies).toBe(5);
        // Et `renommer` n'a jamais eu de canal à qui parler : sa signature ne
        // porte que la racine.
        expect(renommer.length).toBe(4);
    });

    it('🔴 une copie interrompue laisse la SOURCE intacte', async () => {
        // Rouge : supprimer la source AVANT la fin de la copie. Une coupure
        // perdrait alors le fichier.
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        const parent = m.racine as RacineMutable & {
            getFileHandle: RacineMutable['getFileHandle'];
        };
        const vrai = parent.getFileHandle.bind(parent);
        parent.getFileHandle = async (nom, opts) => {
            if (opts?.create === true) throw new DOMException('disque plein', 'QuotaExceededError');
            return vrai(nom, opts);
        };
        await expect(renommer(m.racine, 'a.txt', 'b.txt', false)).rejects.toBeInstanceOf(
            FilesError,
        );
        expect(m.arbre.files.get('a.txt')).toEqual(OCTETS);
    });
});

describe('le renommage de CASSE PURE', () => {
    it('🔴 passe par un nom intermédiaire sur un poste INSENSIBLE', async () => {
        // Rouge : renommer directement. Sur le faux insensible, la destination
        // « existe déjà » — et c'est la source. Une implémentation naïve refuse
        // (`deja-present`) ou, pire, écrase.
        const m = monde((r) => r.files.set('a.txt', OCTETS), { insensible: true });
        await renommer(m.racine, 'a.txt', 'A.txt', false);
        expect([...m.arbre.files.keys()]).toEqual(['A.txt']);
        expect(m.arbre.files.get('A.txt')).toEqual(OCTETS);
    });

    it('passe aussi sur un poste SENSIBLE, où il n’y a pas de collision', async () => {
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        await renommer(m.racine, 'a.txt', 'A.txt', false);
        expect([...m.arbre.files.keys()]).toEqual(['A.txt']);
    });
});

describe('les refus du renommage', () => {
    it('🔴 refuse d’ÉCRASER une destination existante', async () => {
        // `move()` écrase silencieusement : sans la résolution préalable,
        // renommer `brouillon.txt` en `note.txt` détruirait `note.txt` sans un
        // mot.
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

    it('refuse une source absente en `introuvable`', async () => {
        const m = monde(() => {});
        await expect(renommer(m.racine, 'x.txt', 'y.txt', false)).rejects.toMatchObject({
            code: 'introuvable',
        });
    });

    it('refuse la racine en `non-supporte`', async () => {
        const m = monde(() => {});
        await expect(renommer(m.racine, '', 'y', false)).rejects.toMatchObject({
            code: 'non-supporte',
        });
    });

    it('résout la SOURCE par le canonicaliseur', async () => {
        const m = monde((r) => r.files.set('Casse.txt', OCTETS));
        await renommer(m.racine, 'casse.txt', 'neuf.txt', false);
        expect(m.arbre.files.get('neuf.txt')).toEqual(OCTETS);
    });
});

describe('la suppression', () => {
    it('supprime un fichier', async () => {
        const m = monde((r) => r.files.set('a.txt', OCTETS));
        await remove(m.racine, 'a.txt', false);
        expect(m.arbre.files.size).toBe(0);
    });

    it('supprime un répertoire VIDE', async () => {
        const m = monde((r) => r.dossiers.set('vide', noeud()));
        await remove(m.racine, 'vide', true);
        expect(m.arbre.dossiers.size).toBe(0);
    });

    it('🔴 un répertoire NON VIDE rend `repertoire-non-vide`, et rien n’est détruit', async () => {
        // Rouge : passer `recursive: true`. **Le sous-arbre du poste local
        // disparaîtrait**, et le test ne pourrait plus le voir.
        //
        // 🔵 Et le code devient DIAGNOSTIQUE : le recevoir signifie que le
        // miroir a dérivé — le poste local porte des entrées que la VM ne
        // connaît pas.
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

    it('🔴 `InvalidModificationError` ne devient PAS `deja-present` ici', async () => {
        // La MÊME `DOMException` veut dire deux choses selon le verbe : « une
        // entrée du même nom existe » sur une création (ce que F2 a écrit dans
        // `classer`), « le répertoire n'est pas vide » sur un `removeEntry`.
        // La classification est faite là où le verbe est connu.
        const m = monde((r) => {
            const d = noeud();
            d.files.set('x', OCTETS);
            r.dossiers.set('d', d);
        });
        const e = await remove(m.racine, 'd', true).catch((x: unknown) => x as FilesError);
        expect((e as FilesError).code).not.toBe('deja-present');
    });

    it('résout le chemin par le canonicaliseur', async () => {
        const m = monde((r) => r.files.set('Casse.txt', OCTETS));
        await remove(m.racine, 'CASSE.TXT', false);
        expect(m.arbre.files.size).toBe(0);
    });

    it('refuse la racine en `non-supporte`', async () => {
        const m = monde(() => {});
        await expect(remove(m.racine, '', true)).rejects.toMatchObject({
            code: 'non-supporte',
        });
    });
});

describe('le retrait de l’arbre source, après un repli de copie', () => {
    it('🔴 n’emporte PAS une entrée apparue entre-temps : il ÉCHOUE, source intacte', async () => {
        // 🔵 C'est ce qui rend le retrait feuille à feuille PLUS SÛR que
        // `recursive: true`, et pas seulement plus verbeux : on ne retire que
        // ce que la copie vient d'énumérer. Une entrée apparue depuis fait
        // échouer le retrait, au lieu d'être détruite en silence.
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
            // Juste AVANT que `retirerArbre` ne retire le répertoire — qu'il
            // vient de vider —, une entrée apparaît sur le poste local, comme
            // si l'utilisateur venait d'y déposer un fichier.
            if (nom === 'd') source.files.set('surgi.txt', new Uint8Array([42]));
            return vrai(nom);
        };
        await expect(renommer(m.racine, 'd', 'e', true)).rejects.toBeInstanceOf(FilesError);
        // La source EXISTE toujours, et l'entrée surgie n'a pas été détruite.
        expect(m.arbre.dossiers.has('d')).toBe(true);
        expect(m.arbre.dossiers.get('d')!.files.has('surgi.txt')).toBe(true);
    });
});
