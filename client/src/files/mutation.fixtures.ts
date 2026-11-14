import type { PoigneeBase } from './adaptateur';
import type { FluxInscriptible } from './ecriture';
import type { MutableFileHandle, RacineMutable } from './mutation';

/* ═══════════════════════════════════════════════════════════════════════════
   A MUTABLE FAKE FILE SYSTEM, IN TWO VARIANTS

   🔵 ONE EXPOSES `move()`, THE OTHER DOES NOT — and that is what makes BOTH
   branches of the rename green on the host. A type that required `move()`
   would make the fallback UNREACHABLE by a test.

   🔵 AND ONE IS CASE-INSENSITIVE, like the real local workstation (Windows,
   macOS by default) — the only way to exercise a pure case rename here.
   ═══════════════════════════════════════════════════════════════════════════ */

export interface Noeud {
    files: Map<string, Uint8Array>;
    dossiers: Map<string, Noeud>;
}

export function noeud(): Noeud {
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

export function monde(
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
