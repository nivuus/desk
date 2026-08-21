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
// ✅ LA CASSE EST TRAITÉE DEPUIS F3, EN LECTURE COMME EN ÉCRITURE.
//
// ❌ Ces lignes disaient : « EN LECTURE, LA CASSE N'EST TRAITÉE NULLE PART, ET
// C'EST UN LEGS DÉCLARÉ », puis décrivaient le défaut mesuré par F1 et
// concluaient « le remède complet, une table de correspondance alimentée par
// l'énumération, reste F3 ». **F3 est arrivé, et le remède n'est PAS une table
// de correspondance** : c'est `fichiers/noms.ts`, qui énumère le parent à
// CHAQUE résolution, **sans aucun cache**. Un cache que rien n'invalide est le
// défaut de l'ancien pont (`src/file.js`, cache SANS TTL), et le seul moyen de
// le vider — `Rafraichir` — est un livrable de F5.
//
// Chaque composant de chemin passe donc par `canoniser`, et ce module rend le
// nom **STOCKÉ**, jamais le nom demandé.
//
// ⚠️ CE QUE F3 NE CORRIGE PAS, ET QUI N'EST PAS RÉPARABLE ICI : la moitié VM du
// phénomène. NTFS résout la casse sur un fichier DÉJÀ HYDRATÉ sans jamais
// atteindre ce module — et quand NTFS répond, nous ne sommes pas consultés.
// C'est le comportement NORMAL de Windows, et l'en-tête de `noms.ts` le
// détaille.
//
// ⚠️ LE COÛT EST RÉEL ET IL EST DÉCLARÉ : une énumération du parent par
// composant résolu, en plus du `getFile()` par entrée que le listage paie déjà.
// **F3 échange de la latence contre une correction**, et c'est F4 qui dira ce
// que l'échange coûte.
//
// ⛔ **F4 NE L'A PAS DIT** (21 août 2026) : aucun geste de sa campagne n'exerce
// la canonicalisation de casse. **Le coût reste DÛ.**

import type { CodeEchec } from '../../../proto/ts/fichiers';
import { TAILLE_TRAME_MAX } from '../../../proto/ts/fichiers';
import type { EnteteMeta, EntreeJson } from '../../../proto/ts/fichiers-entetes';
import { canoniserOuLever, injecterFaute } from './noms';

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
export function classer(e: unknown, siAbsent: CodeEchec): EchecFichiers {
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
        // ── LES DEUX CAUSES DE F2 ──────────────────────────────────────────
        // Elles n'existaient pas en lecture seule, et sans elles les deux
        // tomberaient dans `interne` : le journal ne dirait plus POURQUOI une
        // écriture a échoué, et l'utilisateur ne saurait pas s'il doit libérer
        // de la place ou rendre une permission.
        //
        // ⚠️ **`TypeMismatchError` N'EST PAS CLASSÉ EN `deja-present`**, contre
        // la lettre du plan de F2 : il est DÉJÀ classé en absence, deux lignes
        // plus haut, et c'est ce qui permet à `attributs` de retenter en
        // fichier après avoir échoué en répertoire. Le classer deux fois est
        // impossible ; le classer ici casserait la lecture.
        case 'QuotaExceededError':
            return new EchecFichiers('disque-plein', texte);
        case 'InvalidModificationError':
            return new EchecFichiers('deja-present', texte);
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

export function creerAdaptateur(racine: Racine, fautesArmees = false): Adaptateur {
    /**
     * Descend les `jusqua` premiers composants, tous des répertoires, **en les
     * CANONICALISANT**.
     */
    async function descendre(parts: string[], jusqua: number): Promise<PoigneeRepertoire> {
        let ici = racine;
        for (let i = 0; i < jusqua; i += 1) {
            // Un composant INTERMÉDIAIRE : le chemin lui-même est en cause, et
            // ProjFS distingue les deux (`ERROR_PATH_NOT_FOUND` contre
            // `ERROR_FILE_NOT_FOUND`).
            const nom = await canoniserOuLever(ici, parts[i], 'chemin-introuvable');
            try {
                ici = await ici.getDirectoryHandle(nom);
            } catch (e) {
                throw classer(e, 'chemin-introuvable');
            }
        }
        return ici;
    }

    async function metaDuFichier(nom: string, f: PoigneeFichier): Promise<EnteteMeta> {
        const fichier = await f.getFile();
        // 🔴 **`nom` EST LE NOM STOCKÉ**, celui que le canonicaliseur a rendu —
        // et c'est lui que `PrjWritePlaceholderInfo` recevra.
        return { nom, repertoire: false, taille: fichier.size, modifie: fichier.lastModified };
    }

    return {
        async lister(chemin) {
            const parts = composants(chemin);
            await injecterFaute(parts, fautesArmees);
            const parent = await descendre(parts, Math.max(parts.length - 1, 0));
            let dossier = parent;
            if (parts.length > 0) {
                const nom = await canoniserOuLever(parent, parts[parts.length - 1], 'introuvable');
                try {
                    dossier = await parent.getDirectoryHandle(nom);
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
                        // mille ouvertures. ✅ **MESURÉ PAR F4** : un listage
                        // de 1 000 entrées coûte **~6,0 s** de bout en bout
                        // (deux exécutions), dont ~3,0 s par traversée et
                        // DEUX traversées par `Get-ChildItem`. Ces mille
                        // `getFile()` sont DEDANS et ne sont pas isolés :
                        // F4 mesure la traversée, jamais ce qui la compose.
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
            await injecterFaute(parts, fautesArmees);
            if (parts.length === 0) {
                // ⚠️ La RACINE n'a pas de nom : `nom` vaut la chaîne vide, et
                // `PrjWritePlaceholderInfo` n'est de toute façon jamais appelée
                // pour elle.
                return { nom: '', repertoire: true, taille: 0, modifie: 0 };
            }
            const parent = await descendre(parts, parts.length - 1);
            // 🔴 **LA RÉSOLUTION EST FAITE UNE FOIS, ICI**, et le nom obtenu
            // sert aux DEUX tentatives — répertoire puis fichier. La refaire
            // deux fois coûterait deux énumérations du parent pour la même
            // question.
            const dernier = await canoniserOuLever(parent, parts[parts.length - 1], 'introuvable');
            try {
                await parent.getDirectoryHandle(dernier);
                return { nom: dernier, repertoire: true, taille: 0, modifie: 0 };
            } catch (e) {
                // On ne retente EN FICHIER que si l'échec est une absence. Un
                // refus de permission retenté serait masqué en « introuvable »,
                // et l'utilisateur chercherait un fichier au lieu de rendre
                // l'accès.
                if (!estAbsence(e)) throw classer(e, 'introuvable');
            }
            try {
                return await metaDuFichier(dernier, await parent.getFileHandle(dernier));
            } catch (e) {
                throw classer(e, 'introuvable');
            }
        },

        async lire(chemin, position, longueur) {
            await injecterFaute(composants(chemin), fautesArmees);
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
            const nom = await canoniserOuLever(parent, parts[parts.length - 1], 'introuvable');
            let fichier: FichierLu;
            try {
                fichier = await (await parent.getFileHandle(nom)).getFile();
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
