// Les en-têtes JSON des trames du pont fichiers — le jumeau TypeScript de
// `proto/src/fichiers/entetes.rs`.
//
// ⚠️ LE DÉCOUPAGE EST LE MÊME DES DEUX CÔTÉS : la trame dans `fichiers.{rs,ts}`,
// les en-têtes ici et dans `fichiers/entetes.rs`. Une asymétrie de découpage
// rendrait le jumelage plus difficile à relire qu'à écrire. Ce fichier est
// séparé de `fichiers.ts` pour la même raison que son jumeau Rust l'est de
// `fichiers.rs` — et parce que les deux réunis passeraient la porte de 250
// lignes que le plan arme sur `fichiers.ts`.
//
// 🔴 CE QUI RATTRAPE UN RENOMMAGE : `proto/fichiers-vectors.json`, lu par
// `fichiers-entetes.test.ts` ICI et par `fichiers/entetes/tests.rs` LÀ-BAS.
// Tant que ces formes vivaient dans l'agent seul, ce fichier aurait dû les
// reproduire à la main, et un champ renommé d'un côté aurait cassé le pont sans
// casser un seul test — le patron exact de `TYPES_AGENT` (`control.ts`) et de
// la variante `battement-recu` restée verte sur cinquante tests.
//
// ⚠️ `position`, `taille` et `longueur` sont des entiers 64 bits côté Rust et
// des `number` ici : au-delà de 2^53 les deux implémentations divergeraient en
// silence. F1 est en lecture seule sur un répertoire local ouvert par la File
// System Access API, où un fichier de 9 pétaoctets n'existe pas ; la borne est
// nommée, pas gardée.

import { CODES_ECHEC, type CodeEchec } from './fichiers';

/** L'en-tête de `TYPE_LISTER` et de `TYPE_ATTRIBUTS`. */
export interface EnteteChemin {
    /** Chemin logique, composants séparés par `/`. Vide = la racine. */
    chemin: string;
}

/** L'en-tête de `TYPE_LIRE`. La plage est **un morceau**, jamais le fichier. */
export interface EnteteLire {
    chemin: string;
    position: number;
    longueur: number;
}

/** Une entrée de répertoire, dans la réponse `TYPE_ENTREES`. */
export interface EntreeJson {
    nom: string;
    repertoire: boolean;
    taille: number;
    /** `File.lastModified` : millisecondes depuis l'époque Unix, **signé**. */
    modifie: number;
}

/** L'en-tête de `TYPE_ENTREES`. Charge binaire **vide**. */
export interface EnteteEntrees {
    entrees: EntreeJson[];
}

/** L'en-tête de `TYPE_META`. Charge binaire **vide**. */
export interface EnteteMeta {
    repertoire: boolean;
    taille: number;
    modifie: number;
}

/** L'en-tête de `TYPE_DONNEES`. **La charge porte les octets.** */
export interface EnteteDonnees {
    position: number;
    longueur: number;
}

/** L'en-tête de `TYPE_ECHEC`. */
export interface EnteteEchec {
    code: CodeEchec;
}

/* ── ENCODAGE ─────────────────────────────────────────────────────────────
   ⚠️ L'ORDRE DES CLÉS EST CELUI DE LA DÉCLARATION RUST, et il compte : les
   vecteurs figent la chaîne exacte, `JSON.stringify` suit l'ordre d'insertion,
   `serde_json` celui de la déclaration. Réordonner un littéral ici rend le
   vecteur rouge — c'est voulu. */

export function encodeChemin(chemin: string): string {
    return JSON.stringify({ chemin } satisfies EnteteChemin);
}

export function encodeLire(chemin: string, position: number, longueur: number): string {
    return JSON.stringify({ chemin, position, longueur } satisfies EnteteLire);
}

export function encodeEntrees(entrees: EntreeJson[]): string {
    // Chaque entrée est reconstruite champ par champ : un objet venu de
    // l'appelant pourrait porter des clés en trop, ou dans un autre ordre.
    return JSON.stringify({
        entrees: entrees.map((e) => ({
            nom: e.nom,
            repertoire: e.repertoire,
            taille: e.taille,
            modifie: e.modifie,
        })),
    } satisfies EnteteEntrees);
}

export function encodeMeta(repertoire: boolean, taille: number, modifie: number): string {
    return JSON.stringify({ repertoire, taille, modifie } satisfies EnteteMeta);
}

export function encodeDonnees(position: number, longueur: number): string {
    return JSON.stringify({ position, longueur } satisfies EnteteDonnees);
}

export function encodeEchec(code: CodeEchec): string {
    return JSON.stringify({ code } satisfies EnteteEchec);
}

/* ── ANALYSE ──────────────────────────────────────────────────────────────
   Aucun champ n'est complété par défaut : un en-tête incomplet est REJETÉ. La
   doctrine de version de `control`, appliquée aux en-têtes — et le jumeau Rust
   n'a aucun `#[serde(default)]` pour la même raison.

   Le TYPE est vérifié autant que la PRÉSENCE : un `taille` en chaîne passerait
   un contrôle de présence et donnerait à ProjFS une taille de fichier absurde. */

function objet(valeur: unknown, forme: string): Record<string, unknown> {
    if (typeof valeur !== 'object' || valeur === null || Array.isArray(valeur)) {
        throw new Error(`en-tête ${forme} : objet attendu, reçu ${typeof valeur}`);
    }
    return valeur as Record<string, unknown>;
}

function chaine(o: Record<string, unknown>, cle: string, forme: string): string {
    const v = o[cle];
    if (typeof v !== 'string') {
        throw new Error(`en-tête ${forme} : champ « ${cle} » absent ou non textuel`);
    }
    return v;
}

function entier(o: Record<string, unknown>, cle: string, forme: string): number {
    const v = o[cle];
    if (typeof v !== 'number' || !Number.isInteger(v)) {
        throw new Error(`en-tête ${forme} : champ « ${cle} » absent ou non entier`);
    }
    return v;
}

function booleen(o: Record<string, unknown>, cle: string, forme: string): boolean {
    const v = o[cle];
    if (typeof v !== 'boolean') {
        throw new Error(`en-tête ${forme} : champ « ${cle} » absent ou non booléen`);
    }
    return v;
}

export function parseChemin(brut: unknown): EnteteChemin {
    const o = objet(brut, 'Chemin');
    return { chemin: chaine(o, 'chemin', 'Chemin') };
}

export function parseLire(brut: unknown): EnteteLire {
    const o = objet(brut, 'Lire');
    return {
        chemin: chaine(o, 'chemin', 'Lire'),
        position: entier(o, 'position', 'Lire'),
        longueur: entier(o, 'longueur', 'Lire'),
    };
}

export function parseEntrees(brut: unknown): EnteteEntrees {
    const o = objet(brut, 'Entrees');
    const liste = o.entrees;
    if (!Array.isArray(liste)) {
        throw new Error('en-tête Entrees : champ « entrees » absent ou non tableau');
    }
    return {
        entrees: liste.map((e) => {
            const item = objet(e, 'EntreeJson');
            return {
                nom: chaine(item, 'nom', 'EntreeJson'),
                repertoire: booleen(item, 'repertoire', 'EntreeJson'),
                taille: entier(item, 'taille', 'EntreeJson'),
                modifie: entier(item, 'modifie', 'EntreeJson'),
            };
        }),
    };
}

export function parseMeta(brut: unknown): EnteteMeta {
    const o = objet(brut, 'Meta');
    return {
        repertoire: booleen(o, 'repertoire', 'Meta'),
        taille: entier(o, 'taille', 'Meta'),
        modifie: entier(o, 'modifie', 'Meta'),
    };
}

export function parseDonnees(brut: unknown): EnteteDonnees {
    const o = objet(brut, 'Donnees');
    return {
        position: entier(o, 'position', 'Donnees'),
        longueur: entier(o, 'longueur', 'Donnees'),
    };
}

export function parseEchec(brut: unknown): EnteteEchec {
    const o = objet(brut, 'Echec');
    const code = chaine(o, 'code', 'Echec');
    // 🔴 La liste est celle de `CODES_ECHEC`, donc de l'énumération Rust : un
    // code inconnu est refusé plutôt que propagé comme une chaîne libre.
    if (!(CODES_ECHEC as readonly string[]).includes(code)) {
        throw new Error(`en-tête Echec : code inconnu « ${code} »`);
    }
    return { code: code as CodeEchec };
}
