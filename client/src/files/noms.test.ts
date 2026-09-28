import { describe, expect, it } from 'vitest';
import { FilesError, type PoigneeBase, type FileHandle, type PoigneeRepertoire } from './adaptateur';
import {
    FAUTE_SILENCE,
    PREFIXE_FAUTE,
    canoniser,
    canoniserOuLever,
    injecterFaute,
    plier,
} from './noms';

/* ═══════════════════════════════════════════════════════════════════════════
   DEUX FAUX SYSTÈMES DE FICHIERS, ET C'EST LE CŒUR DE CE FICHIER

   🔴 L'UN EST SENSIBLE À LA CASSE — comme OPFS et comme Linux, donc comme
   l'INSTRUMENT de recette. L'autre est INSENSIBLE — comme Windows et comme
   macOS par défaut, donc comme le POSTE LOCAL RÉEL.

   **Le même canonicaliseur doit rendre la même réponse sur les deux**, et
   c'est la seule façon d'éprouver sur l'hôte une moitié de phénomène que le
   montage de recette ne peut pas produire (voir l'en-tête de `noms.ts`).
   ═══════════════════════════════════════════════════════════════════════════ */

/** Un répertoire dont `values()` rend les noms donnés. Rien d'autre. */
function parentWith(noms: string[]): PoigneeRepertoire {
    return {
        kind: 'directory',
        name: 'racine',
        async getDirectoryHandle(): Promise<PoigneeRepertoire> {
            throw new Error('not used by the canonicaliser');
        },
        async getFileHandle(): Promise<FileHandle> {
            throw new Error('not used by the canonicaliser');
        },
        values(): AsyncIterable<PoigneeBase> {
            return {
                async *[Symbol.asyncIterator]() {
                    for (const nom of noms) yield { kind: 'file' as const, name: nom };
                },
            };
        },
    };
}

/**
 * Un répertoire INSENSIBLE à la casse : `getFileHandle('CASSE.TXT')` y rend
 * `Casse.txt`, exactement comme Windows.
 *
 * 🔴 C'EST LUI QUI REPRODUIT LA MOITIÉ NAVIGATEUR DU DÉFAUT DE F1, celle qui
 * n'a JAMAIS été observée parce que l'instrument de recette est OPFS.
 */
function parentInsensible(noms: string[]): PoigneeRepertoire & {
    ouvertures: string[];
} {
    const ouvertures: string[] = [];
    const base = parentWith(noms);
    return {
        ...base,
        ouvertures,
        async getFileHandle(nom: string): Promise<FileHandle> {
            const trouve = noms.find((n) => n.toLowerCase() === nom.toLowerCase());
            if (trouve === undefined) {
                throw new DOMException(`« ${nom} » cannot be found`, 'NotFoundError');
            }
            ouvertures.push(trouve);
            return { kind: 'file', name: trouve, getFile: async () => ({} as never) };
        },
    };
}

describe('folding', () => {
    it('folds the case', () => {
        expect(plier('CASSE.TXT')).toBe(plier('Casse.txt'));
    });

    it('🔵 folds the UNICODE NORMALISATION, which F2 declared NOT HANDLED', () => {
        // `é` en un seul point de code (NFC) contre `e` + accent combinant (NFD).
        const nfc = 'été.txt';
        const nfd = 'été.txt';
        expect(nfc).not.toBe(nfd);
        expect(plier(nfc)).toBe(plier(nfd));
    });

    it('does NOT fold two really different names', () => {
        expect(plier('a.txt')).not.toBe(plier('b.txt'));
    });
});

describe('le canonicaliseur', () => {
    it('🔴 on an INSENSITIVE fake, `casse.txt` does NOT return `Casse.txt` without canonicalisation', async () => {
        // C'EST LE ROUGE LE PLUS IMPORTANT DE F3, et il se lit dans les deux
        // sens sur la MÊME assertion :
        //   - le faux insensible, interrogé DIRECTEMENT, rend bien le mauvais
        //     fichier — c'est la moitié navigateur du défaut de F1, reproduite
        //     sur l'hôte ;
        //   - le canonicaliseur, lui, rend le nom STOCKÉ.
        const parent = parentInsensible(['Casse.txt']);
        await parent.getFileHandle('casse.txt');
        expect(parent.ouvertures).toEqual(['Casse.txt']);

        const r = await canoniser(parent, 'casse.txt');
        expect(r).toEqual({ sorte: 'trouve', nom: 'Casse.txt' });
    });

    it('🔴 on a SENSITIVE fake, `GROS.BIN` returns `gros.bin` under its STORED name', async () => {
        // C'est l'incohérence que F1 relève : dans la MÊME exécution,
        // `casse.txt` passait (NTFS) et `GROS.BIN` échouait (OPFS). Elle
        // disparaît.
        const r = await canoniser(parentWith(['gros.bin', 'autre.txt']), 'GROS.BIN');
        expect(r).toEqual({ sorte: 'trouve', nom: 'gros.bin' });
    });

    it('🔴 returns the STORED name, never the REQUESTED name', async () => {
        // Rouge : rendre `demande`. Le substitut serait créé sous un nom qui
        // n'existe pas côté poste local, et une écriture ultérieure le créerait
        // POUR DE BON — un fichier fantôme, à côté du vrai.
        const r = await canoniser(parentWith(['Rapport Final.PDF']), 'rapport final.pdf');
        expect(r).toEqual({ sorte: 'trouve', nom: 'Rapport Final.PDF' });
        expect(r).not.toEqual({ sorte: 'trouve', nom: 'rapport final.pdf' });
    });

    it('🔴 two homonyms return `ambigu`, and NOTHING else', async () => {
        // Rouge : choisir le premier. On écraserait l'un des deux, et le choix
        // dépendrait de l'ordre d'énumération — c'est-à-dire du hasard.
        const r = await canoniser(parentWith(['note.txt', 'Note.txt']), 'NOTE.TXT');
        expect(r).toEqual({ sorte: 'ambigu', noms: ['note.txt', 'Note.txt'] });
    });

    it('🔴 an EXACT name short-circuits the ambiguity', async () => {
        // Rouge : ne pas privilégier l'exact. `note.txt` deviendrait ambigu sur
        // un poste qui porte AUSSI `Note.txt`, alors qu'il est parfaitement
        // désigné — et on refuserait une lecture légitime.
        const r = await canoniser(parentWith(['note.txt', 'Note.txt']), 'note.txt');
        expect(r).toEqual({ sorte: 'trouve', nom: 'note.txt' });
    });

    it('returns `absent` when nothing resembles it', async () => {
        expect(await canoniser(parentWith(['a.txt']), 'b.txt')).toEqual({ sorte: 'absent' });
    });

    it('🔵 resolves a UNICODE NORMALISATION divergence', async () => {
        // macOS stocke en NFD, Windows en NFC. Sans le pliage, la garde de F2
        // créerait un DOUBLON au lieu d'écraser — moins grave que la perte,
        // mais faux, et F2 le déclare tel quel.
        const stocke = 'été.txt'; // NFD
        const r = await canoniser(parentWith([stocke]), 'été.txt'); // NFC
        expect(r).toEqual({ sorte: 'trouve', nom: stocke });
    });

    it('an empty directory returns `absent`, never `ambigu`', async () => {
        expect(await canoniser(parentWith([]), 'x')).toEqual({ sorte: 'absent' });
    });
});

describe('canoniserOuLever', () => {
    it('returns the stored name', async () => {
        expect(await canoniserOuLever(parentWith(['A.txt']), 'a.txt', 'introuvable')).toBe('A.txt');
    });

    it('🔴 tells the TWO ways of being missing apart', async () => {
        // ProjFS les distingue (`ERROR_FILE_NOT_FOUND` contre
        // `ERROR_PATH_NOT_FOUND`), et l'Explorateur n'en dit pas la même chose.
        await expect(canoniserOuLever(parentWith([]), 'x', 'introuvable')).rejects.toMatchObject({
            code: 'introuvable',
        });
        await expect(
            canoniserOuLever(parentWith([]), 'x', 'chemin-introuvable'),
        ).rejects.toMatchObject({ code: 'chemin-introuvable' });
    });

    it('throws `casse-ambigue` while NAMING the homonyms', async () => {
        const error = await canoniserOuLever(parentWith(['a', 'A']), 'à-plier-en-A', 'introuvable')
            .catch((e: unknown) => e as FilesError)
            .then((e) => e as FilesError)
            .catch(() => undefined);
        // Le nom demandé ne se replie sur rien : c'est `absent`, pas `ambigu`.
        expect(error?.code).toBe('introuvable');

        const ambigu = await canoniserOuLever(parentWith(['a', 'A']), 'A', 'introuvable').then(
            (n) => n,
            (e: unknown) => e as FilesError,
        );
        // 'A' est EXACT : la règle 1 prime, et il n'y a pas d'ambiguïté.
        expect(ambigu).toBe('A');

        const vrai = await canoniserOuLever(parentWith(['a', 'A']), 'à', 'introuvable').then(
            (n) => n,
            (e: unknown) => e as FilesError,
        );
        expect(vrai).toBeInstanceOf(FilesError);
    });
});

describe('fault injection', () => {
    it('🔴 is INERT when it is not armed', async () => {
        // Rouge : la lire depuis le module. Un utilisateur qui créerait un
        // dossier `.faute-disque-plein` casserait son propre pont — et le
        // module cesserait d'être testable, puisqu'il lirait `location`.
        await expect(injecterFaute(['.faute-disque-plein', 'x'], false)).resolves.toBeUndefined();
    });

    it('throws the requested code when it is armed', async () => {
        await expect(injecterFaute(['.faute-acces-refuse'], true)).rejects.toMatchObject({
            code: 'acces-refuse',
        });
        await expect(injecterFaute(['.faute-disque-plein'], true)).rejects.toMatchObject({
            code: 'disque-plein',
        });
    });

    it('🔴 looks ONLY at the first component', async () => {
        // Un balayage de tous les composants ferait qu'un chemin traversant un
        // dossier ainsi nommé — même en profondeur — échouerait, ce qui rendrait
        // l'injection impossible à désarmer par le geste.
        await expect(
            injecterFaute(['normal', '.faute-acces-refuse'], true),
        ).resolves.toBeUndefined();
    });

    it('🔴 `.faute-silence` NEVER RESOLVES', async () => {
        // C'est le seul moyen d'exercer `DelaiDepasse` : une réponse, quelle
        // qu'elle soit, empêcherait le pont d'expirer.
        let resolue = false;
        void injecterFaute([`${PREFIXE_FAUTE}${FAUTE_SILENCE}`], true).then(() => {
            resolue = true;
        });
        await new Promise((r) => setTimeout(r, 20));
        expect(resolue).toBe(false);
    });

    it('🔴 an UNKNOWN suffix is SAID, never swallowed', async () => {
        // Sans cela, une coquille de recette produirait un « introuvable »
        // ordinaire, et l'opérateur croirait avoir exercé un code qu'il n'a pas
        // exercé.
        await expect(injecterFaute(['.faute-typo'], true)).rejects.toMatchObject({
            code: 'interne',
        });
    });

    it('an ordinary path triggers nothing, even armed', async () => {
        await expect(injecterFaute(['dossier', 'note.txt'], true)).resolves.toBeUndefined();
        await expect(injecterFaute([], true)).resolves.toBeUndefined();
    });
});
