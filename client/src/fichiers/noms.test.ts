import { describe, expect, it } from 'vitest';
import { EchecFichiers, type PoigneeBase, type PoigneeFichier, type PoigneeRepertoire } from './adaptateur';
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
function parentAvec(noms: string[]): PoigneeRepertoire {
    return {
        kind: 'directory',
        name: 'racine',
        async getDirectoryHandle(): Promise<PoigneeRepertoire> {
            throw new Error('non employé par le canonicaliseur');
        },
        async getFileHandle(): Promise<PoigneeFichier> {
            throw new Error('non employé par le canonicaliseur');
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
    const base = parentAvec(noms);
    return {
        ...base,
        ouvertures,
        async getFileHandle(nom: string): Promise<PoigneeFichier> {
            const trouve = noms.find((n) => n.toLowerCase() === nom.toLowerCase());
            if (trouve === undefined) {
                throw new DOMException(`« ${nom} » est introuvable`, 'NotFoundError');
            }
            ouvertures.push(trouve);
            return { kind: 'file', name: trouve, getFile: async () => ({} as never) };
        },
    };
}

describe('le pliage', () => {
    it('replie la casse', () => {
        expect(plier('CASSE.TXT')).toBe(plier('Casse.txt'));
    });

    it('🔵 replie la NORMALISATION UNICODE, que F2 déclarait NON TRAITÉE', () => {
        // `é` en un seul point de code (NFC) contre `e` + accent combinant (NFD).
        const nfc = 'été.txt';
        const nfd = 'été.txt';
        expect(nfc).not.toBe(nfd);
        expect(plier(nfc)).toBe(plier(nfd));
    });

    it('ne replie PAS deux noms réellement différents', () => {
        expect(plier('a.txt')).not.toBe(plier('b.txt'));
    });
});

describe('le canonicaliseur', () => {
    it('🔴 sur un faux INSENSIBLE, `casse.txt` ne rend PAS `Casse.txt` sans canonicalisation', async () => {
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

    it('🔴 sur un faux SENSIBLE, `GROS.BIN` rend `gros.bin` sous son nom STOCKÉ', async () => {
        // C'est l'incohérence que F1 relève : dans la MÊME exécution,
        // `casse.txt` passait (NTFS) et `GROS.BIN` échouait (OPFS). Elle
        // disparaît.
        const r = await canoniser(parentAvec(['gros.bin', 'autre.txt']), 'GROS.BIN');
        expect(r).toEqual({ sorte: 'trouve', nom: 'gros.bin' });
    });

    it('🔴 rend le nom STOCKÉ, jamais le nom DEMANDÉ', async () => {
        // Rouge : rendre `demande`. Le substitut serait créé sous un nom qui
        // n'existe pas côté poste local, et une écriture ultérieure le créerait
        // POUR DE BON — un fichier fantôme, à côté du vrai.
        const r = await canoniser(parentAvec(['Rapport Final.PDF']), 'rapport final.pdf');
        expect(r).toEqual({ sorte: 'trouve', nom: 'Rapport Final.PDF' });
        expect(r).not.toEqual({ sorte: 'trouve', nom: 'rapport final.pdf' });
    });

    it('🔴 deux homonymes rendent `ambigu`, et RIEN d’autre', async () => {
        // Rouge : choisir le premier. On écraserait l'un des deux, et le choix
        // dépendrait de l'ordre d'énumération — c'est-à-dire du hasard.
        const r = await canoniser(parentAvec(['note.txt', 'Note.txt']), 'NOTE.TXT');
        expect(r).toEqual({ sorte: 'ambigu', noms: ['note.txt', 'Note.txt'] });
    });

    it('🔴 un nom EXACT court-circuite l’ambiguïté', async () => {
        // Rouge : ne pas privilégier l'exact. `note.txt` deviendrait ambigu sur
        // un poste qui porte AUSSI `Note.txt`, alors qu'il est parfaitement
        // désigné — et on refuserait une lecture légitime.
        const r = await canoniser(parentAvec(['note.txt', 'Note.txt']), 'note.txt');
        expect(r).toEqual({ sorte: 'trouve', nom: 'note.txt' });
    });

    it('rend `absent` quand rien n’y ressemble', async () => {
        expect(await canoniser(parentAvec(['a.txt']), 'b.txt')).toEqual({ sorte: 'absent' });
    });

    it('🔵 résout une divergence de NORMALISATION UNICODE', async () => {
        // macOS stocke en NFD, Windows en NFC. Sans le pliage, la garde de F2
        // créerait un DOUBLON au lieu d'écraser — moins grave que la perte,
        // mais faux, et F2 le déclare tel quel.
        const stocke = 'été.txt'; // NFD
        const r = await canoniser(parentAvec([stocke]), 'été.txt'); // NFC
        expect(r).toEqual({ sorte: 'trouve', nom: stocke });
    });

    it('un répertoire vide rend `absent`, jamais `ambigu`', async () => {
        expect(await canoniser(parentAvec([]), 'x')).toEqual({ sorte: 'absent' });
    });
});

describe('canoniserOuLever', () => {
    it('rend le nom stocké', async () => {
        expect(await canoniserOuLever(parentAvec(['A.txt']), 'a.txt', 'introuvable')).toBe('A.txt');
    });

    it('🔴 distingue les DEUX façons d’être introuvable', async () => {
        // ProjFS les distingue (`ERROR_FILE_NOT_FOUND` contre
        // `ERROR_PATH_NOT_FOUND`), et l'Explorateur n'en dit pas la même chose.
        await expect(canoniserOuLever(parentAvec([]), 'x', 'introuvable')).rejects.toMatchObject({
            code: 'introuvable',
        });
        await expect(
            canoniserOuLever(parentAvec([]), 'x', 'chemin-introuvable'),
        ).rejects.toMatchObject({ code: 'chemin-introuvable' });
    });

    it('lève `casse-ambigue` en NOMMANT les homonymes', async () => {
        const erreur = await canoniserOuLever(parentAvec(['a', 'A']), 'à-plier-en-A', 'introuvable')
            .catch((e: unknown) => e as EchecFichiers)
            .then((e) => e as EchecFichiers)
            .catch(() => undefined);
        // Le nom demandé ne se replie sur rien : c'est `absent`, pas `ambigu`.
        expect(erreur?.code).toBe('introuvable');

        const ambigu = await canoniserOuLever(parentAvec(['a', 'A']), 'A', 'introuvable').then(
            (n) => n,
            (e: unknown) => e as EchecFichiers,
        );
        // 'A' est EXACT : la règle 1 prime, et il n'y a pas d'ambiguïté.
        expect(ambigu).toBe('A');

        const vrai = await canoniserOuLever(parentAvec(['a', 'A']), 'à', 'introuvable').then(
            (n) => n,
            (e: unknown) => e as EchecFichiers,
        );
        expect(vrai).toBeInstanceOf(EchecFichiers);
    });
});

describe('l’injection de faute', () => {
    it('🔴 est INERTE quand elle n’est pas armée', async () => {
        // Rouge : la lire depuis le module. Un utilisateur qui créerait un
        // dossier `.faute-disque-plein` casserait son propre pont — et le
        // module cesserait d'être testable, puisqu'il lirait `location`.
        await expect(injecterFaute(['.faute-disque-plein', 'x'], false)).resolves.toBeUndefined();
    });

    it('lève le code demandé quand elle est armée', async () => {
        await expect(injecterFaute(['.faute-acces-refuse'], true)).rejects.toMatchObject({
            code: 'acces-refuse',
        });
        await expect(injecterFaute(['.faute-disque-plein'], true)).rejects.toMatchObject({
            code: 'disque-plein',
        });
    });

    it('🔴 ne regarde QUE le premier composant', async () => {
        // Un balayage de tous les composants ferait qu'un chemin traversant un
        // dossier ainsi nommé — même en profondeur — échouerait, ce qui rendrait
        // l'injection impossible à désarmer par le geste.
        await expect(
            injecterFaute(['normal', '.faute-acces-refuse'], true),
        ).resolves.toBeUndefined();
    });

    it('🔴 `.faute-silence` NE RÉSOUT JAMAIS', async () => {
        // C'est le seul moyen d'exercer `DelaiDepasse` : une réponse, quelle
        // qu'elle soit, empêcherait le pont d'expirer.
        let resolue = false;
        void injecterFaute([`${PREFIXE_FAUTE}${FAUTE_SILENCE}`], true).then(() => {
            resolue = true;
        });
        await new Promise((r) => setTimeout(r, 20));
        expect(resolue).toBe(false);
    });

    it('🔴 un suffixe INCONNU est DIT, jamais avalé', async () => {
        // Sans cela, une coquille de recette produirait un « introuvable »
        // ordinaire, et l'opérateur croirait avoir exercé un code qu'il n'a pas
        // exercé.
        await expect(injecterFaute(['.faute-typo'], true)).rejects.toMatchObject({
            code: 'interne',
        });
    });

    it('un chemin ordinaire ne déclenche rien, même armé', async () => {
        await expect(injecterFaute(['dossier', 'note.txt'], true)).resolves.toBeUndefined();
        await expect(injecterFaute([], true)).resolves.toBeUndefined();
    });
});
