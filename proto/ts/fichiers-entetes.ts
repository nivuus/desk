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
// silence. Le pont sert un répertoire local ouvert par la File System Access
// API, où un fichier de 9 pétaoctets n'existe pas ; la borne est nommée, pas
// gardée.
//
// ⚠️ *Cette phrase disait « F1 est en lecture seule » : F2 a ouvert l'écriture,
// et la borne vaut désormais aussi pour la `position` d'un morceau écrit et
// pour les `octets` d'une écriture due. Elle n'est pas davantage gardée.*

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

/**
 * L'en-tête de `TYPE_META`. Charge binaire **vide**.
 *
 * 🔴 `nom` EST LE NOM CANONIQUE — CELUI QUI EST STOCKÉ, jamais celui que
 * l'application a tapé (F3). Vide pour la RACINE, qui n'a pas de nom.
 */
export interface EnteteMeta {
    nom: string;
    repertoire: boolean;
    taille: number;
    modifie: number;
}

/** L'en-tête de `TYPE_DONNEES`. **La charge porte les octets.** */
export interface EnteteDonnees {
    position: number;
    longueur: number;
}

/**
 * L'en-tête de `TYPE_ECRIRE`. **La charge porte les octets.**
 *
 * ⚠️ `premier` et `dernier` NE SONT PAS DÉDUCTIBLES de `position` et
 * `longueur` : c'est `premier` qui commande l'ouverture du flux SANS
 * `keepExistingData`, et `dernier` qui déclenche le `close()`, donc la
 * COMMITTAISON.
 */
export interface EnteteEcrire {
    chemin: string;
    position: number;
    longueur: number;
    premier: boolean;
    dernier: boolean;
}

/** L'en-tête de `TYPE_CREER`. Charge binaire **vide**. */
export interface EnteteCreer {
    chemin: string;
    repertoire: boolean;
}

/**
 * L'en-tête de `TYPE_RENOMMER`. Charge binaire **vide**.
 *
 * 🔴 L'ORDRE DES DEUX CHAMPS EST LE SENS DE L'OPÉRATION, ET S'Y TROMPER
 * DÉTRUIT. `de` est la source, `vers` la destination. Inverser les deux ne
 * produirait aucune erreur : le renommage aurait lieu, à l'envers.
 *
 * ⚠️ `repertoire` est TRANSPORTÉ, jamais redécouvert : c'est l'`isdirectory`
 * que le rappel ProjFS reçoit du système. Le navigateur le redemanderait au
 * prix d'un aller-retour, et se tromperait sur une entrée disparue entre-temps.
 */
export interface EnteteRenommer {
    de: string;
    vers: string;
    repertoire: boolean;
}

/**
 * L'en-tête de `TYPE_SUPPRIMER`. Charge binaire **vide**.
 *
 * ⚠️ La suppression n'est PAS récursive côté navigateur, contre la lettre de la
 * spec §3.5. Un geste dans la VM ne doit pas déclencher une destruction
 * récursive du disque du poste local, sur la foi d'un miroir qu'aucune preuve
 * ne dit à jour.
 */
export interface EnteteSupprimer {
    chemin: string;
    repertoire: boolean;
}

/** Une écriture DUE : des octets qui vivent sur la VM et pas encore ici. */
export interface Due {
    chemin: string;
    octets: number;
}

/**
 * L'en-tête de `TYPE_DUES`. Charge binaire **vide**.
 *
 * ⚠️ C'est une ANNONCE : elle n'attend AUCUNE réponse.
 */
export interface EnteteDues {
    dues: Due[];
    /**
     * **F5** — vrai quand le pont a des écritures dues et **refuse de les
     * pousser**, le navigateur ayant annoncé une racine dont le nom diffère de
     * celui mémorisé (`Bonjour`, spec §6.4 cas 2).
     *
     * 🔴 **CHAMP REQUIS**, comme tout champ de ce module. Un défaut à `false`
     * vaudrait « le pont pousse », c'est-à-dire l'inverse de ce que `Bonjour`
     * existe pour empêcher. **Un défaut doit tomber du côté sûr, ou ne pas
     * exister.**
     */
    retenues: boolean;
}

/**
 * L'en-tête de `TYPE_BONJOUR`. Charge binaire **vide**.
 *
 * ⚠️ C'est une ANNONCE, et elle REMONTE : du navigateur vers le pont, sans que
 * celui-ci l'ait demandée, et sa corrélation est IGNORÉE.
 *
 * ⚠️ `racine` est un INDICE, jamais une preuve : `isSameEntry()` compare deux
 * poignées VIVANTES, jamais une poignée à un souvenir (spec §6.4 cas 2). Deux
 * répertoires homonymes sur deux disques différents passeraient pour un seul.
 * La v1 compare le nom faute de mieux.
 */
export interface EnteteBonjour {
    racine: string;
    forcer: boolean;
}

/* ⚠️ `TYPE_RAFRAICHIR` n'a PAS d'en-tête propre : sa trame porte `{}`, comme
   `TYPE_FAIT`. Lui donner une interface vide ferait une forme à épingler qui
   n'épingle rien, et un vecteur partagé qui ne peut pas casser. */

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

export function encodeMeta(
    nom: string,
    repertoire: boolean,
    taille: number,
    modifie: number,
): string {
    // ⚠️ L'ORDRE DES CLÉS EST CELUI DE LA DÉCLARATION RUST, et le vecteur le
    // fige : `nom` vient EN PREMIER.
    return JSON.stringify({ nom, repertoire, taille, modifie } satisfies EnteteMeta);
}

export function encodeDonnees(position: number, longueur: number): string {
    return JSON.stringify({ position, longueur } satisfies EnteteDonnees);
}

export function encodeEchec(code: CodeEchec): string {
    return JSON.stringify({ code } satisfies EnteteEchec);
}

export function encodeEcrire(
    chemin: string,
    position: number,
    longueur: number,
    premier: boolean,
    dernier: boolean,
): string {
    return JSON.stringify({
        chemin,
        position,
        longueur,
        premier,
        dernier,
    } satisfies EnteteEcrire);
}

export function encodeCreer(chemin: string, repertoire: boolean): string {
    return JSON.stringify({ chemin, repertoire } satisfies EnteteCreer);
}

export function encodeRenommer(de: string, vers: string, repertoire: boolean): string {
    return JSON.stringify({ de, vers, repertoire } satisfies EnteteRenommer);
}

export function encodeSupprimer(chemin: string, repertoire: boolean): string {
    return JSON.stringify({ chemin, repertoire } satisfies EnteteSupprimer);
}

export function encodeBonjour(racine: string, forcer: boolean): string {
    return JSON.stringify({ racine, forcer } satisfies EnteteBonjour);
}

export function encodeDues(dues: Due[], retenues: boolean): string {
    // Chaque due est reconstruite champ par champ, comme `encodeEntrees` : un
    // objet venu de l'appelant pourrait porter des clés en trop, ou dans un
    // autre ordre — et l'ordre est ce que le vecteur fige.
    return JSON.stringify({
        dues: dues.map((d) => ({ chemin: d.chemin, octets: d.octets })),
        retenues,
    } satisfies EnteteDues);
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
        nom: chaine(o, 'nom', 'Meta'),
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

export function parseEcrire(brut: unknown): EnteteEcrire {
    const o = objet(brut, 'Ecrire');
    return {
        chemin: chaine(o, 'chemin', 'Ecrire'),
        position: entier(o, 'position', 'Ecrire'),
        longueur: entier(o, 'longueur', 'Ecrire'),
        // 🔴 Les DEUX drapeaux sont exigés. Sans `premier`, le flux s'ouvrirait
        // avec `keepExistingData` et un fichier réécrit plus court garderait sa
        // queue d'octets — le défaut EXACT de l'ancien pont. Sans `dernier`, le
        // `close()` ne viendrait jamais et rien ne serait jamais commis.
        premier: booleen(o, 'premier', 'Ecrire'),
        dernier: booleen(o, 'dernier', 'Ecrire'),
    };
}

export function parseCreer(brut: unknown): EnteteCreer {
    const o = objet(brut, 'Creer');
    return {
        chemin: chaine(o, 'chemin', 'Creer'),
        repertoire: booleen(o, 'repertoire', 'Creer'),
    };
}

export function parseRenommer(brut: unknown): EnteteRenommer {
    const o = objet(brut, 'Renommer');
    return {
        de: chaine(o, 'de', 'Renommer'),
        vers: chaine(o, 'vers', 'Renommer'),
        repertoire: booleen(o, 'repertoire', 'Renommer'),
    };
}

export function parseSupprimer(brut: unknown): EnteteSupprimer {
    const o = objet(brut, 'Supprimer');
    return {
        chemin: chaine(o, 'chemin', 'Supprimer'),
        repertoire: booleen(o, 'repertoire', 'Supprimer'),
    };
}

export function parseDues(brut: unknown): EnteteDues {
    const o = objet(brut, 'Dues');
    const liste = o.dues;
    if (!Array.isArray(liste)) {
        throw new Error('en-tête Dues : champ « dues » absent ou non tableau');
    }
    return {
        dues: liste.map((d) => {
            const item = objet(d, 'Due');
            return {
                chemin: chaine(item, 'chemin', 'Due'),
                octets: entier(item, 'octets', 'Due'),
            };
        }),
        retenues: booleen(o, 'retenues', 'Dues'),
    };
}

export function parseBonjour(brut: unknown): EnteteBonjour {
    const o = objet(brut, 'Bonjour');
    return {
        racine: chaine(o, 'racine', 'Bonjour'),
        forcer: booleen(o, 'forcer', 'Bonjour'),
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
