import { describe, expect, it } from 'vitest';
import {
    creerAdaptateur,
    EchecFichiers,
    type FichierLu,
    type PoigneeFichier,
    type PoigneeRepertoire,
    type TrancheLisible,
} from './adaptateur';

/* ── UN FAUX SYSTÈME DE FICHIERS EN MÉMOIRE ───────────────────────────────
   La File System Access API n'existe pas sous Node : sans l'injection de la
   racine (spec §4.4), AUCUN de ces tests n'existerait.

   🔴 LE FAUX `File` COMPTE SES APPELS, et c'est ce qui donne sa valeur au test
   de plage. Un faux qui rendrait simplement les bons octets passerait aussi
   bien avec `slice(o, o+n).arrayBuffer()` qu'avec `arrayBuffer()` suivi d'une
   découpe côté appelant — c'est-à-dire serait VACUEUX. Le défaut visé est
   relevé, pas imaginé : `web/index.js:562-564` lisait le fichier ENTIER pour en
   rendre une plage. */

interface Compteurs {
    arrayBufferEntier: number;
    slice: number;
}

function fauxFichier(octets: Uint8Array, modifie: number, compteurs: Compteurs): FichierLu {
    return {
        size: octets.length,
        lastModified: modifie,
        slice(debut: number, fin: number): TrancheLisible {
            compteurs.slice += 1;
            const tranche = octets.slice(debut, fin);
            return { arrayBuffer: async () => tranche.buffer.slice(tranche.byteOffset, tranche.byteOffset + tranche.byteLength) as ArrayBuffer };
        },
        arrayBuffer: async () => {
            compteurs.arrayBufferEntier += 1;
            return octets.buffer.slice(octets.byteOffset, octets.byteOffset + octets.byteLength) as ArrayBuffer;
        },
    };
}

type Arbre = { [nom: string]: Arbre | { octets: Uint8Array; modifie: number } };

function estFichier(n: Arbre[string]): n is { octets: Uint8Array; modifie: number } {
    return 'octets' in n && n.octets instanceof Uint8Array;
}

/** `DOMException` est disponible sous Node ≥ 17 ; on s'en sert telle quelle. */
function absent(nom: string): never {
    throw new DOMException(`« ${nom} » est introuvable`, 'NotFoundError');
}

function mauvaisType(nom: string): never {
    throw new DOMException(`« ${nom} » n'est pas du type demandé`, 'TypeMismatchError');
}

function repertoire(nom: string, arbre: Arbre, compteurs: Compteurs): PoigneeRepertoire {
    return {
        kind: 'directory',
        name: nom,
        async getDirectoryHandle(enfant: string): Promise<PoigneeRepertoire> {
            const n = arbre[enfant];
            if (n === undefined) absent(enfant);
            if (estFichier(n)) mauvaisType(enfant);
            return repertoire(enfant, n, compteurs);
        },
        async getFileHandle(enfant: string): Promise<PoigneeFichier> {
            const n = arbre[enfant];
            if (n === undefined) absent(enfant);
            if (!estFichier(n)) mauvaisType(enfant);
            return {
                kind: 'file',
                name: enfant,
                getFile: async () => fauxFichier(n.octets, n.modifie, compteurs),
            };
        },
        async *values() {
            for (const [enfant, n] of Object.entries(arbre)) {
                yield estFichier(n)
                    ? {
                          kind: 'file' as const,
                          name: enfant,
                          getFile: async () => fauxFichier(n.octets, n.modifie, compteurs),
                      }
                    : repertoire(enfant, n, compteurs);
            }
        },
    };
}

const CONTENU = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

function monter() {
    const compteurs: Compteurs = { arrayBufferEntier: 0, slice: 0 };
    const arbre: Arbre = {
        'note.txt': { octets: new Uint8Array([65, 66, 67]), modifie: 1_690_000_000_000 },
        'gros.bin': { octets: CONTENU, modifie: 42 },
        dossier: {
            'dedans.txt': { octets: new Uint8Array([88]), modifie: -86_400_000 },
        },
    };
    return { compteurs, adaptateur: creerAdaptateur(repertoire('', arbre, compteurs)) };
}

describe('adaptateur de la File System Access API', () => {
    it('lister rend les entrées avec leur nature', async () => {
        const { adaptateur } = monter();
        const entrees = await adaptateur.lister('');
        const parNom = new Map(entrees.map((e) => [e.nom, e]));
        expect(parNom.get('dossier')?.repertoire).toBe(true);
        expect(parNom.get('note.txt')?.repertoire).toBe(false);
        expect(parNom.get('note.txt')?.taille).toBe(3);
        expect(parNom.get('note.txt')?.modifie).toBe(1_690_000_000_000);
    });

    it('lister la racine prend le chemin vide', async () => {
        const { adaptateur } = monter();
        // La racine n'a pas de nom : le chemin logique vide la désigne, et
        // c'est ce que `pont::chemins` normalise côté agent.
        expect((await adaptateur.lister('')).length).toBe(3);
        expect((await adaptateur.lister('dossier')).map((e) => e.nom)).toEqual(['dedans.txt']);
    });

    it('attributs rend la taille et l’horodatage', async () => {
        const { adaptateur } = monter();
        expect(await adaptateur.attributs('note.txt')).toEqual({
            repertoire: false,
            taille: 3,
            modifie: 1_690_000_000_000,
        });
        expect(await adaptateur.attributs('dossier')).toEqual({
            repertoire: true,
            taille: 0,
            modifie: 0,
        });
        expect(await adaptateur.attributs('')).toEqual({
            repertoire: true,
            taille: 0,
            modifie: 0,
        });
    });

    it('🔴 lire une plage ne lit QUE cette plage', async () => {
        const { adaptateur, compteurs } = monter();
        const octets = await adaptateur.lire('gros.bin', 4, 3);
        expect([...octets]).toEqual([4, 5, 6]);
        // 🔴 LE CŒUR DU TEST : le fichier entier n'est JAMAIS matérialisé.
        expect(compteurs.arrayBufferEntier).toBe(0);
        expect(compteurs.slice).toBe(1);
    });

    it('lire au-delà de la fin rend moins d’octets, sans lever', async () => {
        const { adaptateur } = monter();
        const octets = await adaptateur.lire('gros.bin', 8, 100);
        expect([...octets]).toEqual([8, 9]);
        // Entièrement au-delà : zéro octet, toujours sans lever.
        expect((await adaptateur.lire('gros.bin', 50, 10)).length).toBe(0);
    });

    it('🔴 un chemin inexistant rend le code Introuvable', async () => {
        const { adaptateur } = monter();
        await expect(adaptateur.attributs('absent.txt')).rejects.toBeInstanceOf(EchecFichiers);
        await expect(adaptateur.attributs('absent.txt')).rejects.toMatchObject({
            code: 'introuvable',
        });
        await expect(adaptateur.lire('absent.txt', 0, 1)).rejects.toMatchObject({
            code: 'introuvable',
        });
    });

    it('distingue le composant final absent d’un PARENT absent', async () => {
        const { adaptateur } = monter();
        // ProjFS distingue ERROR_FILE_NOT_FOUND d'ERROR_PATH_NOT_FOUND, et
        // l'Explorateur ne dit pas la même chose des deux.
        await expect(adaptateur.attributs('nulle-part/note.txt')).rejects.toMatchObject({
            code: 'chemin-introuvable',
        });
        await expect(adaptateur.lister('note.txt/encore')).rejects.toMatchObject({
            code: 'chemin-introuvable',
        });
    });

    it('🔴 une erreur de la FSA devient un CODE, jamais une chaîne', async () => {
        const { adaptateur } = monter();
        // 🔴 LE DÉFAUT EXACT DE L'ANCIEN PONT, éprouvé ici même :
        // `web/index.js:669` faisait `JSON.stringify(e)` d'une `Error`, ce qui
        // rend `"{}"`, et `src/file.js:127` le reconstruisait en
        // `new Error("{}")`. Toute la cause était détruite À L'ÉMISSION.
        expect(JSON.stringify(new Error('permission refusée'))).toBe('{}');

        const echec = await adaptateur.attributs('absent.txt').catch((e: unknown) => e);
        expect(echec).toBeInstanceOf(EchecFichiers);
        // Ce que l'adaptateur produit à la place : un membre de l'énumération
        // PARTAGÉE, qui traverse le fil sans rien perdre.
        expect((echec as EchecFichiers).code).toBe('introuvable');
        // Et le message reste lisible pour un humain, côté navigateur — il ne
        // traverse pas le fil, mais il est ce qu'on lit dans la console.
        expect((echec as EchecFichiers).message).toMatch(/absent\.txt/);
    });

    it('un refus de permission devient acces-refuse, pas interne', async () => {
        const refusante: PoigneeRepertoire = {
            kind: 'directory',
            name: '',
            async getDirectoryHandle() {
                throw new DOMException('permission révoquée', 'NotAllowedError');
            },
            async getFileHandle() {
                throw new DOMException('permission révoquée', 'NotAllowedError');
            },
            async *values(): AsyncGenerator<PoigneeFichier | PoigneeRepertoire> {
                throw new DOMException('permission révoquée', 'NotAllowedError');
            },
        };
        const adaptateur = creerAdaptateur(refusante);
        await expect(adaptateur.lister('')).rejects.toMatchObject({ code: 'acces-refuse' });
        await expect(adaptateur.attributs('x')).rejects.toMatchObject({ code: 'acces-refuse' });
    });

    it('une panne imprévue devient interne, jamais un code inventé', async () => {
        const cassee: PoigneeRepertoire = {
            kind: 'directory',
            name: '',
            async getDirectoryHandle() {
                throw new Error('quelque chose a explosé');
            },
            async getFileHandle() {
                throw new Error('quelque chose a explosé');
            },
            async *values(): AsyncGenerator<PoigneeFichier | PoigneeRepertoire> {
                throw new Error('quelque chose a explosé');
            },
        };
        await expect(creerAdaptateur(cassee).lister('')).rejects.toMatchObject({
            code: 'interne',
        });
    });
});
