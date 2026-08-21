// Les types de charge utile de l'INSTALLATION d'un logiciel téléversé, et les
// gardes de forme qui les jugent.
//
// 🔴 EXTRAIT AVANT L'ADDITION, comme `plateforme-apps.ts` — même raison, même
// mécanisme : `plateforme.ts` était à 426 lignes après la première extraction,
// et le sous-bloc G3 y ajoute trois messages, deux énumérations, trois
// encodeurs et leurs branches de parseur. La doctrine de `CLAUDE.md` est de
// rendre la marge par une extraction jouée D'AVANCE, jamais par une
// compression.
//
// ⚠️ CE FICHIER NE DOIT IMPORTER NI `node:` NI AUCUN DOM : il est chargé par le
// service ET par le navigateur.
//
// 🔴 IL IMPORTE `PLATEFORME_VERSION` DE `plateforme-version.ts`, ET NON DE
// `plateforme.ts` : ce dernier importe CE module, et le cycle qui en résulterait
// serait un cycle de VALEURS — pas de types, que TypeScript efface —, donc un
// vrai cycle à l'exécution, du genre qui rend une constante `undefined` selon
// l'ordre d'évaluation des modules.

import { PLATEFORME_VERSION } from './plateforme-version';
import { chaineNonVide, estChaine } from './plateforme-gardes';

/**
 * Où en est une installation.
 *
 * ⚠️ LA PHASE `empreinte` N'EST PAS ICI, et ce n'est pas un oubli : elle se
 * déroule dans le NAVIGATEUR, avant que la plateforme n'ait la moindre ligne à
 * écrire. Elle ne traverse jamais le canal `/agent`.
 *
 * 🔴 `execution` NE PORTE AUCUN POURCENTAGE : un installeur Windows n'en publie
 * pas, et en inventer un serait mentir sur une progression que personne ne
 * mesure. Elle porte le temps écoulé, et l'interface affiche un état
 * indéterminé.
 */
export type Phase = 'transfert' | 'execution' | 'reconciliation';

/**
 * Ce qu'une installation a produit.
 *
 * 🔴 LE CODE DE SORTIE N'ENTRE PAS DANS CETTE DÉCISION. `msiexec` rend 3010
 * pour un succès qui demande un redémarrage, et beaucoup d'installeurs rendent
 * 0 après une annulation : un produit qui jugerait sur le code se tromperait
 * dans les deux sens. Il est RAPPORTÉ à côté de l'issue, jamais interprété.
 *
 * ⚠️ `refusee` est une ADDITION à la spécification, qui n'en nomme que trois.
 * Empreinte fausse, élévation requise, extension refusée, processus assigné à
 * un job object : ce ne sont ni des succès, ni des « sans effet », ni des
 * ignorances — ce sont des refus, et ils portent leur motif. Les fondre dans
 * `issue-inconnue` ferait lire « on ne sait pas » là où l'on sait très bien.
 */
export type Issue = 'reussie' | 'sans-effet' | 'issue-inconnue' | 'refusee';

/**
 * L'ordre d'installation. **Les octets ne l'empruntent jamais** : il porte une
 * URL, et l'agent va tirer le fichier en HTTP avec son jeton d'agent. Le canal
 * est en JSON et porte le battement de cœur ; une tranche de 8 Mio y coûterait
 * +33 % en base64 tout en bloquant ce battement.
 */
export interface InstallerMessage {
    v: number;
    type: 'installer';
    installation: string;
    url: string;
    nom: string;
    taille: number;
    sha256: string;
}

/** Où en est une installation. ÉCHANTILLONNÉE — voir `cadence.rs` côté agent. */
export interface ProgressionMessage {
    v: number;
    type: 'progression';
    installation: string;
    phase: Phase;
    octets_faits: number;
    octets_total: number;
    ecoule_ms: number;
}

/**
 * L'issue, et ce qui s'est réellement passé.
 *
 * 🔴 `code_sortie` EST `number | null`, JAMAIS UN `-1` SENTINELLE : « pas de
 * code » et « code −1 » sont deux faits différents.
 *
 * ⚠️ UN `journal` VIDE EST LE CAS NORMAL, pas un échec : la plupart des
 * installeurs Windows sont graphiques et n'écrivent rien sur les flux standard.
 */
export interface TermineMessage {
    v: number;
    type: 'termine';
    installation: string;
    issue: Issue;
    motif: string | null;
    code_sortie: number | null;
    journal: string;
    journal_tronque: boolean;
}

const PHASES: readonly Phase[] = ['transfert', 'execution', 'reconciliation'];
const ISSUES: readonly Issue[] = ['reussie', 'sans-effet', 'issue-inconnue', 'refusee'];

export function estPhase(valeur: unknown): valeur is Phase {
    return typeof valeur === 'string' && (PHASES as readonly string[]).includes(valeur);
}

export function estIssueInstallation(valeur: unknown): valeur is Issue {
    return typeof valeur === 'string' && (ISSUES as readonly string[]).includes(valeur);
}

/**
 * Un compte d'octets ou de millisecondes : entier, fini, non négatif, et sous
 * `Number.MAX_SAFE_INTEGER`.
 *
 * ⚠️ `typeof x === 'number'` NE SUFFIT PAS : il laisse passer `NaN`, `Infinity`
 * et `1.5`. Un `NaN` traverserait jusqu'à la base, où il deviendrait un `NULL`
 * sur une colonne `NOT NULL` — c'est-à-dire une erreur SQL très loin de sa
 * cause.
 */
export function estCompte(valeur: unknown): valeur is number {
    return (
        typeof valeur === 'number' &&
        Number.isSafeInteger(valeur) &&
        valeur >= 0
    );
}

/**
 * Un champ facultatif **OBLIGATOIRE SUR LE FIL** : la clé doit être présente,
 * sa valeur peut être `null`.
 *
 * 🔴 C'EST LE JUMEAU EXACT DE `champs::option_obligatoire` CÔTÉ RUST, et sans
 * lui les deux bouts ne diraient pas la même chose. `parsed.motif` vaut
 * `undefined` aussi bien pour « clé absente » que pour « clé à `undefined` » :
 * seul `'motif' in parsed` distingue le champ manquant, et c'est cette
 * distinction que le bump de version existe pour rendre visible. Un `termine`
 * d'une version antérieure, sans `motif`, doit être REFUSÉ — pas complété avec
 * un motif absent.
 */
export function presentEtNulOu<T>(
    objet: Record<string, unknown>,
    cle: string,
    garde: (valeur: unknown) => valeur is T,
): { present: true; valeur: T | null } | { present: false } {
    if (!(cle in objet)) return { present: false };
    const valeur = objet[cle];
    if (valeur === null) return { present: true, valeur: null };
    if (garde(valeur)) return { present: true, valeur };
    return { present: false };
}

export function estEntierSigne(valeur: unknown): valeur is number {
    return typeof valeur === 'number' && Number.isSafeInteger(valeur);
}

// --- Les encodeurs et les lectures, déplacés ici depuis `plateforme.ts` ---
export function encodeProgression(
    installation: string,
    phase: Phase,
    octetsFaits: number,
    octetsTotal: number,
    ecouleMs: number,
): string {
    const message: ProgressionMessage = {
        type: 'progression',
        v: PLATEFORME_VERSION,
        installation,
        phase,
        octets_faits: octetsFaits,
        octets_total: octetsTotal,
        ecoule_ms: ecouleMs,
    };
    return JSON.stringify(message);
}

/**
 * ⚠️ `motif` ET `code_sortie` SONT ÉCRITS MÊME À `null`, et c'est ce que
 * `JSON.stringify` fait d'un `null` — mais PAS d'un `undefined`, qu'il OMET.
 * Passer `undefined` produirait une chaîne sans la clé, que le jumeau Rust
 * refuserait par `option_obligatoire`. La signature exige donc `| null`, pas
 * `?`, et c'est la seule chose qui empêche l'omission d'être écrivable.
 */
export function encodeTermine(
    installation: string,
    issue: Issue,
    motif: string | null,
    codeSortie: number | null,
    journal: string,
    journalTronque: boolean,
): string {
    const message: TermineMessage = {
        type: 'termine',
        v: PLATEFORME_VERSION,
        installation,
        issue,
        motif,
        code_sortie: codeSortie,
        journal,
        journal_tronque: journalTronque,
    };
    return JSON.stringify(message);
}

export function lireProgression(parsed: Record<string, unknown>): ProgressionMessage | null {
    if (!chaineNonVide(parsed.installation)) return null;
    if (!estPhase(parsed.phase)) return null;
    if (
        !estCompte(parsed.octets_faits) ||
        !estCompte(parsed.octets_total) ||
        !estCompte(parsed.ecoule_ms)
    ) {
        return null;
    }
    return {
        type: 'progression',
        v: PLATEFORME_VERSION,
        installation: parsed.installation,
        phase: parsed.phase,
        octets_faits: parsed.octets_faits,
        octets_total: parsed.octets_total,
        ecoule_ms: parsed.ecoule_ms,
    };
}

export function lireTermine(parsed: Record<string, unknown>): TermineMessage | null {
    if (!chaineNonVide(parsed.installation)) return null;
    if (!estIssueInstallation(parsed.issue)) return null;
    // 🔴 `presentEtNulOu` PLUTÔT QU'UN TEST DE VALEUR : la clé doit être
    // PRÉSENTE, sa valeur peut être `null`. C'est le jumeau exact de
    // `champs::option_obligatoire` côté Rust, et sans lui un `termine`
    // d'une version antérieure — sans `motif` — serait accepté avec un
    // motif silencieusement absent. C'est le déguisement précis que le
    // bump de version existe pour empêcher.
    const motif = presentEtNulOu(parsed, 'motif', estChaine);
    if (!motif.present) return null;
    const code = presentEtNulOu(parsed, 'code_sortie', estEntierSigne);
    if (!code.present) return null;
    if (!estChaine(parsed.journal)) return null;
    if (typeof parsed.journal_tronque !== 'boolean') return null;
    return {
        type: 'termine',
        v: PLATEFORME_VERSION,
        installation: parsed.installation,
        issue: parsed.issue,
        motif: motif.valeur,
        code_sortie: code.valeur,
        journal: parsed.journal,
        journal_tronque: parsed.journal_tronque,
    };
}

/**
 * L'ordre d'installation, encodé ICI et nulle part ailleurs.
 *
 * ⚠️ L'ORDRE DES CHAMPS EST `type` PUIS `v`, comme partout dans ce protocole :
 * serde émet le tag interne en premier, `JSON.stringify` respecte l'ordre
 * d'insertion, et `plateforme-vectors.json` fige la chaîne EXACTE que les DEUX
 * langages doivent produire.
 */
export function encodeInstaller(
    installation: string,
    url: string,
    nom: string,
    taille: number,
    sha256: string,
): string {
    const message: InstallerMessage = {
        type: 'installer',
        v: PLATEFORME_VERSION,
        installation,
        url,
        nom,
        taille,
        sha256,
    };
    return JSON.stringify(message);
}
