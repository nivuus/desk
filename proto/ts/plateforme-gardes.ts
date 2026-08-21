/**
 * Les gardes de FORME du canal plateforme <-> agent : ce qui décide qu'une
 * valeur venue du fil est bien ce qu'elle prétend être.
 *
 * 🔴 EXTRAITES DE `plateforme.ts` VERBATIM (sous-bloc G2), PARCE QUE LE
 * PLAFOND DE 500 LIGNES A ÉTÉ FRANCHI — 528 — ET QUE LA DOCTRINE DU DÉPÔT EST
 * DE RATTRAPER PAR UNE EXTRACTION, JAMAIS PAR UNE COMPRESSION.
 *
 * ⚠️ **L'EXTRACTION AURAIT DÛ PRÉCÉDER L'ADDITION, ET ELLE NE L'A PAS FAIT.**
 * Le plan de G2 avait nommé trois extractions à jouer d'avance ; les trois ont
 * été jouées, et celle-ci n'était pas prévue — le fichier était annoncé à 429
 * lignes pour « le miroir ». Le franchissement est DÉCLARÉ plutôt que
 * dissimulé.
 *
 * ⚠️ AUCUNE LIGNE DE COMPORTEMENT N'A ÉTÉ AJOUTÉE, RETIRÉE NI REFORMULÉE.
 */
import type { Application, IssueLancement, SourceMax } from './plateforme';

/** Les quatre issues, énumérées — voir `estIssue`. */
const ISSUES: ReadonlyArray<IssueLancement> = ['raccourci', 'cible', 'inconnue', 'echec'];

/**
 * Le CHAMP `string` de `Application` à valider, un par un — voir
 * `estApplication`.
 *
 * 🔴 `icone` ET `source_max` N'Y SONT PAS, ET LES Y METTRE SERAIT UN DÉFAUT
 * SILENCIEUX. Cette liste est parcourue par `estChaine` : y ajouter `icone`
 * ferait REFUSER TOUT CATALOGUE dont une seule application n'a pas d'icône —
 * `null` n'est pas une chaîne —, avec le motif `forme`, c'est-à-dire un
 * catalogue entier perdu sans qu'aucune trace ne dise pourquoi. Les deux
 * champs neufs ont donc leurs propres gardes.
 */
const CHAMPS_APPLICATION: ReadonlyArray<keyof Application> = [
    'cle', 'nom', 'chemin', 'cible', 'arguments', 'repertoire',
];

export function estObjetJson(valeur: unknown): valeur is Record<string, unknown> {
    return typeof valeur === 'object' && valeur !== null && !Array.isArray(valeur);
}

export function chaineNonVide(valeur: unknown): valeur is string {
    return typeof valeur === 'string' && valeur.length > 0;
}

/** ⚠️ `arguments` est LÉGITIMEMENT VIDE : la garde est `string`, pas `chaineNonVide`. */
export function estChaine(valeur: unknown): valeur is string {
    return typeof valeur === 'string';
}

/**
 * ⚠️ `null` EST UNE VALEUR ATTENDUE, PAS UNE ABSENCE. La garde exige que la
 * clé soit PRÉSENTE — `'icone' in valeur` — puis que sa valeur soit `null` ou
 * une chaîne. Se contenter de `=== null || typeof === 'string'` accepterait
 * un objet SANS le champ, `valeur.icone` valant alors `undefined`… qui n'est
 * ni `null` ni une chaîne, donc le cas serait refusé par accident. Écrire la
 * présence explicitement rend la propriété lisible plutôt qu'heureuse.
 */
export function estIcone(valeur: Record<string, unknown>): boolean {
    if (!('icone' in valeur)) return false;
    return valeur.icone === null || estChaine(valeur.icone);
}

/**
 * 🔴 UN OBJET QUELCONQUE NE PASSE PAS. `{"pixels":"gros"}` est refusé, et
 * `{"pixels":256,"bonus":1}` aussi : la forme est exactement l'une des deux
 * que le Rust sait émettre, et rien d'autre.
 */
export function estSourceMax(valeur: unknown): valeur is SourceMax {
    if (valeur === 'non-mesuree') return true;
    if (!estObjetJson(valeur)) return false;
    const cles = Object.keys(valeur);
    if (cles.length !== 1 || cles[0] !== 'pixels') return false;
    return typeof valeur.pixels === 'number' && Number.isInteger(valeur.pixels);
}

export function estApplication(valeur: unknown): valeur is Application {
    if (!estObjetJson(valeur)) return false;
    if (!CHAMPS_APPLICATION.every((champ) => estChaine(valeur[champ]))) return false;
    return (
        estIcone(valeur)
        && estSourceMax(valeur.source_max)
        && estAccent(valeur)
        && estAssociations(valeur.associations)
    );
}

/**
 * ⚠️ MÊME FORME QUE `estIcone`, ET POUR LA MÊME RAISON : `null` est une valeur
 * LÉGITIME — « pas de dominante » —, mais le champ doit être PRÉSENT. Un champ
 * absent serait un catalogue d'une autre version, et l'accepter en silence est
 * exactement ce que le versionnement de ce protocole existe pour empêcher.
 */
export function estAccent(valeur: Record<string, unknown>): boolean {
    if (!('accent' in valeur)) return false;
    return valeur.accent === null || estChaine(valeur.accent);
}

/**
 * 🔴 UN TABLEAU DE CHAÎNES, ET VIDE EST VALIDE. Refuser le vide ferait rejeter
 * la très grande majorité des applications, qui n'ouvrent aucun type de
 * fichier.
 */
export function estAssociations(valeur: unknown): valeur is string[] {
    return Array.isArray(valeur) && valeur.every((e) => estChaine(e));
}

export function estIssue(valeur: unknown): valeur is IssueLancement {
    return ISSUES.includes(valeur as IssueLancement);
}

