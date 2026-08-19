// L'adaptateur entre les chemins logiques du pont et la File System Access
// API du navigateur.
//
// 🔴 LA RACINE EST INJECTÉE, JAMAIS IMPORTÉE (spec §4.4). C'est la couture qui
// rend ce fichier testable : Vitest tourne sous Node, qui n'a aucune FSA. Sans
// elle, la résolution de chemin, le découpage des plages et le classement des
// erreurs ne seraient éprouvés par rien — et ce sont exactement les trois
// endroits où l'ancien pont se trompait.
//
// Ce module ne connaît ni le DOM, ni WebRTC, ni la trame binaire : il rend des
// valeurs et lève des `EchecFichiers`. C'est `protocole.ts` qui les met sur le
// fil.
//
// ⚠️ LA CASSE N'EST TRAITÉE NULLE PART, ET C'EST UN LEGS DÉCLARÉ. Windows est
// insensible à la casse, la File System Access API ne l'est pas : l'Explorateur
// peut demander `NOTE.TXT` là où le répertoire local porte `note.txt`, et
// `getFileHandle` lèvera `NotFoundError`. Le plan de F1 le nomme comme un legs
// à OBSERVER en recette, pas à résoudre ici — une correspondance insensible à
// la casse exigerait d'énumérer le répertoire à chaque résolution, ce qui est
// une décision de conception et un coût, pas un correctif.

import type { CodeEchec } from '../../../proto/ts/fichiers';
import { TAILLE_TRAME_MAX } from '../../../proto/ts/fichiers';
import type { EnteteMeta, EntreeJson } from '../../../proto/ts/fichiers-entetes';

/* ── LES POIGNÉES, DÉCRITES PAR CE DONT ON SE SERT ────────────────────────
   Ces interfaces sont un SOUS-ENSEMBLE STRUCTUREL de `FileSystemDirectoryHandle`,
   `FileSystemFileHandle`, `File` et `Blob` : la vraie poignée les satisfait
   sans conversion (`canal.ts` le vérifie à la compilation), et un faux en
   mémoire aussi. Les décrire ici plutôt que d'importer les types du DOM garde
   ce module utilisable sous Node. */

/** Ce qu'on sait faire d'une tranche : en lire les octets. */
export interface TrancheLisible {
    arrayBuffer(): Promise<ArrayBuffer>;
}

/**
 * Le sous-ensemble de `File` dont on se sert.
 *
 * ⚠️ `arrayBuffer()` FIGURE DANS CE TYPE ALORS QUE LE CODE NE DOIT JAMAIS
 * L'APPELER, et c'est délibéré : le vrai `File` l'expose, et un type qui le
 * cacherait mentirait sur ce qui est injecté. Surtout, c'est ce qui permet au
 * faux de `adaptateur.test.ts` de COMPTER ses appels, donc au test de plage
 * d'être vu rouge. Un type qui interdirait l'appel remplacerait un contrôle
 * exécuté par une promesse de compilateur — plus fort en apparence, mais on ne
 * l'aurait jamais vu échouer.
 */
export interface FichierLu {
    readonly size: number;
    readonly lastModified: number;
    slice(debut: number, fin: number): TrancheLisible;
    arrayBuffer(): Promise<ArrayBuffer>;
}

/**
 * Ce que toute poignée porte, quelle que soit sa nature.
 *
 * ⚠️ C'EST CE QUE `values()` REND, ET NON L'UNION DES DEUX NATURES — parce que
 * c'est tout ce que la bibliothèque DOM de TypeScript garantit :
 * `FileSystemDirectoryHandle.values()` y est typée
 * `AsyncIterator<FileSystemHandle>`, la classe de BASE, alors que l'API réelle
 * rend les sous-types concrets. Déclarer l'union ici rendrait la vraie poignée
 * NON assignable, et le contrôle de compatibilité de `canal.ts` échouerait sur
 * une divergence de la bibliothèque, pas du produit. L'adaptateur redescend
 * donc vers `PoigneeFichier` après avoir lu `kind` — la même chose que ferait
 * TypeScript tout seul si l'union était déclarée.
 */
export interface PoigneeBase {
    readonly kind: 'file' | 'directory';
    readonly name: string;
}

export interface PoigneeFichier extends PoigneeBase {
    readonly kind: 'file';
    getFile(): Promise<FichierLu>;
}

export interface PoigneeRepertoire extends PoigneeBase {
    readonly kind: 'directory';
    getDirectoryHandle(nom: string): Promise<PoigneeRepertoire>;
    getFileHandle(nom: string): Promise<PoigneeFichier>;
    values(): AsyncIterable<PoigneeBase>;
}

/** La racine choisie par l'utilisateur : un répertoire, et rien d'autre. */
export type Racine = PoigneeRepertoire;

/**
 * Un échec PORTEUR DE SON CODE.
 *
 * 🔴 C'est la réponse au défaut relevé de l'ancien pont : `web/index.js:669`
 * émettait `JSON.stringify(e)`, qui rend `"{}"` pour toute `Error`, et
 * `src/file.js:127` reconstruisait un `new Error("{}")` à l'arrivée. La cause
 * était détruite à l'émission, et personne ne pouvait la retrouver.
 *
 * Ici la cause voyage sous forme de `CodeEchec`, membre de l'énumération
 * partagée `proto::fichiers::CodeEchec` : elle traverse le fil sans rien
 * perdre, et l'agent la retraduit en `HRESULT`. Le `message`, lui, ne traverse
 * pas — il est ce qu'on lit dans la console du navigateur.
 */
export class EchecFichiers extends Error {
    readonly code: CodeEchec;

    constructor(code: CodeEchec, message: string) {
        super(message);
        this.name = 'EchecFichiers';
        this.code = code;
    }
}

/**
 * Classe une exception venue de la File System Access API.
 *
 * `siAbsent` distingue les deux façons d'être introuvable, que ProjFS
 * distingue aussi (`ERROR_FILE_NOT_FOUND` contre `ERROR_PATH_NOT_FOUND`) et
 * dont l'Explorateur ne dit pas la même chose : un composant INTERMÉDIAIRE
 * manquant rend `chemin-introuvable`, le composant FINAL rend `introuvable`.
 */
function classer(e: unknown, siAbsent: CodeEchec): EchecFichiers {
    if (e instanceof EchecFichiers) return e;
    const nom = e instanceof DOMException ? e.name : '';
    const texte = e instanceof Error ? e.message : String(e);
    switch (nom) {
        case 'NotFoundError':
        case 'TypeMismatchError':
            return new EchecFichiers(siAbsent, texte);
        case 'NotAllowedError':
        case 'SecurityError':
            return new EchecFichiers('acces-refuse', texte);
        default:
            // Tout le reste est `interne` : inventer un code plus précis
            // reviendrait à deviner, et l'agent le traduirait en un HRESULT
            // faux plutôt qu'en un HRESULT vague.
            return new EchecFichiers('interne', texte);
    }
}

/** Un échec d'absence est-il rattrapable en essayant l'autre nature ? */
function estAbsence(e: unknown): boolean {
    return e instanceof DOMException && (e.name === 'NotFoundError' || e.name === 'TypeMismatchError');
}

/** `"a/b/c"` → `["a","b","c"]`, `""` → `[]`. */
function composants(chemin: string): string[] {
    return chemin.split('/').filter((c) => c.length > 0);
}

export interface Adaptateur {
    lister(chemin: string): Promise<EntreeJson[]>;
    attributs(chemin: string): Promise<EnteteMeta>;
    lire(chemin: string, position: number, longueur: number): Promise<Uint8Array>;
}

export function creerAdaptateur(racine: Racine): Adaptateur {
    /** Descend les `jusqua` premiers composants, tous des répertoires. */
    async function descendre(parts: string[], jusqua: number): Promise<PoigneeRepertoire> {
        let ici = racine;
        for (let i = 0; i < jusqua; i += 1) {
            try {
                ici = await ici.getDirectoryHandle(parts[i]);
            } catch (e) {
                // Un composant INTERMÉDIAIRE : le chemin lui-même est en cause.
                throw classer(e, 'chemin-introuvable');
            }
        }
        return ici;
    }

    async function metaDuFichier(f: PoigneeFichier): Promise<EnteteMeta> {
        const fichier = await f.getFile();
        return { repertoire: false, taille: fichier.size, modifie: fichier.lastModified };
    }

    return {
        async lister(chemin) {
            const parts = composants(chemin);
            const parent = await descendre(parts, Math.max(parts.length - 1, 0));
            let dossier = parent;
            if (parts.length > 0) {
                try {
                    dossier = await parent.getDirectoryHandle(parts[parts.length - 1]);
                } catch (e) {
                    throw classer(e, 'introuvable');
                }
            }
            const entrees: EntreeJson[] = [];
            try {
                for await (const enfant of dossier.values()) {
                    if (enfant.kind === 'directory') {
                        // ⚠️ La FSA n'expose NI taille NI horodatage d'un
                        // répertoire. Zéro est ce que ProjFS attend d'un
                        // répertoire pour la taille ; l'horodatage nul est une
                        // perte assumée, et le dire évite qu'on la cherche.
                        entrees.push({ nom: enfant.name, repertoire: true, taille: 0, modifie: 0 });
                    } else {
                        // ⚠️ COÛT ASSUMÉ : un `getFile()` par entrée. La FSA
                        // n'offre aucun moyen d'obtenir taille et date sans
                        // ouvrir le fichier, et ProjFS exige les deux dans son
                        // énumération. Un répertoire à mille entrées coûte
                        // mille ouvertures — mesurable en F4, pas ici.
                        // Narrowing explicite : `kind` vaut `'file'`, donc la
                        // poignée EST une `PoigneeFichier`. Voir la note de
                        // `PoigneeBase` — c'est la bibliothèque DOM qui
                        // sous-type `values()`, pas l'API.
                        const f = await (enfant as PoigneeFichier).getFile();
                        entrees.push({
                            nom: enfant.name,
                            repertoire: false,
                            taille: f.size,
                            modifie: f.lastModified,
                        });
                    }
                }
            } catch (e) {
                throw classer(e, 'introuvable');
            }
            return entrees;
        },

        async attributs(chemin) {
            const parts = composants(chemin);
            if (parts.length === 0) {
                return { repertoire: true, taille: 0, modifie: 0 };
            }
            const parent = await descendre(parts, parts.length - 1);
            const dernier = parts[parts.length - 1];
            try {
                await parent.getDirectoryHandle(dernier);
                return { repertoire: true, taille: 0, modifie: 0 };
            } catch (e) {
                // On ne retente EN FICHIER que si l'échec est une absence. Un
                // refus de permission retenté serait masqué en « introuvable »,
                // et l'utilisateur chercherait un fichier au lieu de rendre
                // l'accès.
                if (!estAbsence(e)) throw classer(e, 'introuvable');
            }
            try {
                return await metaDuFichier(await parent.getFileHandle(dernier));
            } catch (e) {
                throw classer(e, 'introuvable');
            }
        },

        async lire(chemin, position, longueur) {
            if (longueur > TAILLE_TRAME_MAX) {
                // Le pair ne se voit pas accorder de confiance sur la taille
                // qu'il demande : `pont::decoupe` borne déjà côté agent, mais
                // c'est l'agent qui le fait, donc l'autre bout du fil.
                throw new EchecFichiers(
                    'trop-grand',
                    `${longueur} octets demandés, maximum ${TAILLE_TRAME_MAX}`,
                );
            }
            const parts = composants(chemin);
            if (parts.length === 0) {
                throw new EchecFichiers('introuvable', 'la racine n’est pas un fichier');
            }
            const parent = await descendre(parts, parts.length - 1);
            let fichier: FichierLu;
            try {
                fichier = await (await parent.getFileHandle(parts[parts.length - 1])).getFile();
            } catch (e) {
                throw classer(e, 'introuvable');
            }
            // 🔴 `slice` PUIS `arrayBuffer`, JAMAIS L'INVERSE. Lire le fichier
            // entier pour en rendre 4 Ko est le défaut relevé de l'ancien pont
            // (`web/index.js:562-564`) : sur un fichier d'un gigaoctet, chaque
            // lecture de ProjFS le matérialiserait en mémoire.
            const debut = Math.min(position, fichier.size);
            const fin = Math.min(position + longueur, fichier.size);
            try {
                return new Uint8Array(await fichier.slice(debut, fin).arrayBuffer());
            } catch (e) {
                throw classer(e, 'introuvable');
            }
        },
    };
}
