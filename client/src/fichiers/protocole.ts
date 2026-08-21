// Le serveur du protocole fichiers, côté navigateur. **PUR** — ni DOM, ni
// WebRTC, ni File System Access API : il reçoit des octets et en rend, et
// l'adaptateur lui est injecté.
//
// 🔴 LE NAVIGATEUR EST UN SERVEUR, ET RIEN D'AUTRE. Il ne demande jamais rien :
// aucune corrélation ne lui appartient. Une trame portant un type de RÉPONSE
// (`TYPE_ENTREES`, `TYPE_META`, `TYPE_DONNEES`, `TYPE_ECHEC`) ne peut donc être
// qu'un écho, une boucle, ou un pair confus — elle est IGNORÉE et journalisée,
// jamais interprétée comme une requête.
//
// C'est le symétrique exact du défaut que la tâche 17 corrige côté agent, où
// `transport/evenements.rs` aiguillait sur le seul drapeau `data.binary` et
// prenait donc toute trame binaire pour une entrée souris. Un aiguillage qui ne
// nomme pas ses cas se paie à chaque message neuf — ce dépôt l'a payé quatre
// fois sur le bras catch-all de `capteur/pont_media.rs`.
//
// ⚠️ TOUTE **REQUÊTE** REÇOIT UNE RÉPONSE, y compris quand elle échoue. Ne rien
// répondre laisserait la commande en vol côté agent jusqu'à son expiration, et
// l'Explorateur se figerait sur une panne pourtant immédiate.
//
// 🔴 UNE **ANNONCE** N'EN REÇOIT AUCUNE, ET LA LISTE DES ANNONCES EST CLOSE.
// L'invariant ci-dessus était écrit en majuscules et sans exception — « UNE
// REQUÊTE REÇOIT TOUJOURS UNE RÉPONSE » — et F2 introduit une TROISIÈME famille
// de messages : `TYPE_DUES`, l'annonce des écritures dues. Elle n'attend rien,
// et n'y pas répondre ne laisse RIEN en vol : aucune entrée de table ne lui
// correspond côté pont.
//
// L'invariant est donc RÉÉCRIT, jamais contourné. Un bras qui rendrait `null`
// SANS que la famille soit nommée serait exactement le bras catch-all
// silencieux que ce dépôt a payé QUATRE fois sur `capteur/pont_media.rs` (D5
// `Sommeil`, D6 `Part`, D7 `Audio`, D8 `PleinEcran`).
//
// Les trois familles, et ce qu'on en fait :
//   REQUÊTES  (1..5, 7, 8) → une réponse, toujours ;
//   ANNONCES  (6)          → un rappel injecté, et `null` ;
//   RÉPONSES  (64..)       → ignorées : le navigateur ne demande rien.
//
// ⚠️ LA NUMÉROTATION N'EST PAS CONTIGUË PAR FAMILLE : 6 est une ANNONCE, 7 et 8
// des REQUÊTES. F2 a sauté 7 et 8 pour F3, ce qui a évité une renumérotation
// tardive — et c'est cet aiguillage NOMMÉ qui dit la famille, jamais la valeur.
//
// ⚠️ L'INVARIANT DE F3 EST CELUI DE F1, ET NON L'EXCEPTION DE F2 :
// `TYPE_RENOMMER` et `TYPE_SUPPRIMER` sont des REQUÊTES. Elles reçoivent
// `Fait` ou `Echec`, toujours. Ne rien répondre laisserait la commande en vol
// côté pont jusqu'à son expiration, et l'Explorateur se figerait sur une panne
// pourtant immédiate.
//
// Le seul cas où l'on ne répond pas SANS que ce soit une annonce est celui où
// l'on n'a pas de requête — trame illisible, type de réponse, type inconnu.

import {
    TYPE_ATTRIBUTS,
    TYPE_CREER,
    TYPE_RENOMMER,
    TYPE_SUPPRIMER,
    TYPE_DONNEES,
    TYPE_BONJOUR,
    TYPE_DUES,
    TYPE_ECHEC,
    TYPE_ECRIRE,
    TYPE_ENTREES,
    TYPE_FAIT,
    TYPE_LIRE,
    TYPE_LISTER,
    TYPE_META,
    TYPE_RAFRAICHIR,
    decoder,
    encoderTexte,
} from '../../../proto/ts/fichiers';
import {
    encodeDonnees,
    encodeEchec,
    encodeEntrees,
    encodeBonjour,
    encodeMeta,
    parseChemin,
    parseCreer,
    parseDues,
    parseEcrire,
    parseLire,
    parseRenommer,
    parseSupprimer,
    type Due,
} from '../../../proto/ts/fichiers-entetes';
import { EchecFichiers, type Adaptateur } from './adaptateur';
import type { Ecrivain } from './ecriture';
import type { Mutateur } from './mutation-service';

/** Où partent les trames qu'on n'a pas su traiter. Injecté, donc observable. */
export type Journal = (message: string) => void;

export interface Serveur {
    /**
     * Traite une trame reçue et rend la trame à réémettre, ou `null` s'il n'y
     * a rien à répondre.
     */
    traiter(octets: ArrayBuffer): Promise<ArrayBuffer | null>;
}

/** Ce que le serveur sait faire en plus de lire, depuis F2. */
export interface OptionsServeur {
    /** L'écrivain. **Absent = lecture seule**, c'est-à-dire le serveur de F1. */
    ecrivain?: Ecrivain;
    /**
     * Le rappel de l'annonce `TYPE_DUES`. **INJECTÉ**, donc observable : c'est
     * la page-shell qui décide ce qu'elle en fait, et ce module reste PUR.
     */
    onDues?: (dues: Due[], retenues: boolean) => void;
    /**
     * Le mutateur — renommage et suppression. **Absent = lecture seule.**
     *
     * ⚠️ **DISTINCT de l'écrivain, et le rester est le point** : `PONT_MUTATION`
     * et `PONT_ECRITURE` sont deux variables de banc distinctes côté agent, et
     * les confondre ferait qu'une recette du renommage couperait aussi l'idiome
     * temp+rename qu'elle veut exercer.
     */
    mutateur?: Mutateur;
    /**
     * Une MUTATION a échoué ici. Même raison que [`onEchecEcriture`] : le
     * navigateur est le seul à connaître la cause, et il n'a personne d'autre à
     * qui la dire.
     *
     * ⚠️ **Un renommage porte DEUX chemins**, et le message doit les nommer
     * tous deux : « impossible de renommer X » ne dit pas vers quoi, et c'est
     * précisément ce que l'utilisateur doit vérifier.
     */
    onEchecMutation?: (quoi: string, code: string) => void;
    /**
     * Le repli de renommage a COPIÉ. **L'instrumentation que la spec §3.5.1
     * exige.**
     *
     * ⚠️ **DIVERGENCE DÉCLARÉE AVEC LE PLAN**, qui la fait « rendre au pont
     * dans l'en-tête de la réponse `Fait` ». `TYPE_FAIT` n'a **aucune forme
     * propre** — son en-tête est `{}`, F2 l'écrit en toutes lettres, et lui en
     * donner une exigerait un vecteur partagé de plus pour une donnée purement
     * diagnostique. La trace part donc par le journal injecté, **là où le
     * navigateur SAIT ce qu'il a fait** ; le pont, lui, journalise ce que LUI
     * sait — c'est la doctrine « une trace dit ce qu'elle SAIT ».
     */
    onRenommagePorCopie?: (de: string, vers: string, octets: number, entrees: number) => void;
    /**
     * Une écriture a échoué ICI, côté navigateur.
     *
     * 🔴 **LE NAVIGATEUR EST LE SEUL À CONNAÎTRE LA CAUSE, et il n'a personne à
     * qui la dire.** Le code traverse bien le fil jusqu'à l'agent, qui le
     * journalise — mais **il n'atteint AUCUNE application Windows** : le handle
     * est refermé depuis longtemps (voir `pont::notifications`). Ce rappel est
     * donc le chemin le plus COURT vers la seule personne que cela concerne :
     * l'utilisateur, devant sa page-shell.
     */
    onEchecEcriture?: (chemin: string, code: string) => void;
}

export function creerServeur(
    adaptateur: Adaptateur,
    journal: Journal = () => {},
    options: OptionsServeur = {},
): Serveur {
    return {
        async traiter(octets) {
            let trame;
            try {
                trame = decoder(octets);
            } catch (e) {
                journal(`trame illisible, ignorée : ${(e as Error).message}`);
                return null;
            }

            switch (trame.type) {
                // ── REQUÊTES : elles reçoivent une réponse, toujours.
                case TYPE_LISTER:
                case TYPE_ATTRIBUTS:
                case TYPE_LIRE:
                case TYPE_ECRIRE:
                case TYPE_CREER:
                case TYPE_RENOMMER:
                case TYPE_SUPPRIMER:
                    break;
                // ── ANNONCE : elle ne reçoit RIEN, et la famille est NOMMÉE.
                case TYPE_DUES: {
                    try {
                        const annonce = parseDues(trame.entete);
                        options.onDues?.(annonce.dues, annonce.retenues);
                    } catch (e) {
                        journal(`annonce de dues illisible : ${(e as Error).message}`);
                    }
                    return null;
                }
                // ── RÉPONSES : le navigateur ne demande rien.
                case TYPE_FAIT:
                case TYPE_ENTREES:
                case TYPE_META:
                case TYPE_DONNEES:
                case TYPE_ECHEC:
                    journal(
                        `réponse ignorée : le navigateur ne demande rien ` +
                            `(type=${trame.type}, corrélation=${trame.correlation})`,
                    );
                    return null;
                default:
                    journal(
                        `type inconnu ignoré : type=${trame.type}, ` +
                            `corrélation=${trame.correlation}`,
                    );
                    return null;
            }

            try {
                return await servir(
                    adaptateur,
                    options,
                    trame.type,
                    trame.correlation,
                    trame.entete,
                    trame.charge,
                );
            } catch (e) {
                // Le code est celui de l'adaptateur quand il en porte un ; tout
                // le reste — en-tête malformé compris — est `interne`. Inventer
                // un code plus précis ferait traduire à l'agent un HRESULT faux
                // plutôt qu'un HRESULT vague.
                const code = e instanceof EchecFichiers ? e.code : 'interne';
                journal(`échec ${code} sur la corrélation ${trame.correlation} : ${(e as Error).message}`);
                if (trame.type === TYPE_RENOMMER || trame.type === TYPE_SUPPRIMER) {
                    // ⚠️ Le chemin est relu de l'en-tête plutôt que retenu :
                    // l'échec a pu venir de son ANALYSE, auquel cas il n'y a
                    // rien à nommer.
                    const quoi = mutationDe(trame.type, trame.entete);
                    if (quoi !== undefined) options.onEchecMutation?.(quoi, code);
                } else if (trame.type === TYPE_ECRIRE || trame.type === TYPE_CREER) {
                    // ⚠️ Le chemin est relu de l'en-tête plutôt que retenu :
                    // l'échec a pu venir de son ANALYSE, auquel cas il n'y a
                    // rien à nommer, et deviner serait pire que se taire.
                    const chemin = cheminDe(trame.entete);
                    if (chemin !== undefined) options.onEchecEcriture?.(chemin, code);
                }
                return encoderTexte(TYPE_ECHEC, trame.correlation, encodeEchec(code));
            }
        },
    };
}

/**
 * Ce qu'une mutation en échec doit NOMMER, si l'en-tête est lisible.
 *
 * ⚠️ **Un renommage porte DEUX chemins**, et les deux comptent : « impossible
 * de renommer X » ne dit pas vers quoi, et c'est précisément ce que
 * l'utilisateur doit vérifier — la destination existe peut-être déjà.
 */
function mutationDe(type: number, entete: unknown): string | undefined {
    if (typeof entete !== 'object' || entete === null) return undefined;
    const o = entete as { chemin?: unknown; de?: unknown; vers?: unknown };
    if (type === TYPE_SUPPRIMER) {
        return typeof o.chemin === 'string' ? o.chemin : undefined;
    }
    if (typeof o.de === 'string' && typeof o.vers === 'string') {
        return `${o.de} → ${o.vers}`;
    }
    return undefined;
}

/** Le chemin d'un en-tête d'écriture, s'il est lisible. */
function cheminDe(entete: unknown): string | undefined {
    if (typeof entete !== 'object' || entete === null) return undefined;
    const chemin = (entete as { chemin?: unknown }).chemin;
    return typeof chemin === 'string' ? chemin : undefined;
}

async function servir(
    adaptateur: Adaptateur,
    options: OptionsServeur,
    type: number,
    correlation: number,
    entete: unknown,
    charge: Uint8Array,
): Promise<ArrayBuffer> {
    const ecrivain: Ecrivain | undefined = options.ecrivain;
    if (type === TYPE_RENOMMER || type === TYPE_SUPPRIMER) {
        const mutateur = options.mutateur;
        if (mutateur === undefined) {
            // ⚠️ **PAS `interne` : `protege-en-ecriture`.** Un lecteur monté
            // sans mutateur et un lecteur en panne n'appellent pas le même
            // geste — le contre-exemple est l'ancien pont, qui rendait `EPERM`
            // à neuf sites distincts.
            throw new EchecFichiers(
                'protege-en-ecriture',
                'ce lecteur ne sait pas renommer ni supprimer',
            );
        }
        if (type === TYPE_RENOMMER) {
            const r = parseRenommer(entete);
            const trace = await mutateur.renommer(r.de, r.vers, r.repertoire);
            if (!trace.parMove) {
                options.onRenommagePorCopie?.(r.de, r.vers, trace.octets, trace.entrees);
            }
        } else {
            const s = parseSupprimer(entete);
            await mutateur.supprimer(s.chemin, s.repertoire);
        }
        return encoderTexte(TYPE_FAIT, correlation, '{}');
    }
    if (type === TYPE_ECRIRE || type === TYPE_CREER) {
        if (ecrivain === undefined) {
            // ⚠️ **PAS `interne` : `protege-en-ecriture`.** Un serveur monté en
            // lecture seule et un serveur en panne n'appellent pas le même
            // geste, et c'est tout l'objet de `CodeEchec` — le contre-exemple
            // est l'ancien pont, qui rendait `EPERM` à neuf sites distincts.
            throw new EchecFichiers(
                'protege-en-ecriture',
                'ce lecteur est monté en lecture seule',
            );
        }
        if (type === TYPE_ECRIRE) {
            const e = parseEcrire(entete);
            // 🔴 **L'EN-TÊTE ET LA CHARGE DOIVENT SE CORROBORER.** Écrire une
            // quantité d'octets que l'émetteur ne croyait pas envoyer est le
            // genre de divergence qu'aucun contrôle en aval ne rattrape : seul
            // un condensat le dirait. C'est le symétrique exact du contrôle que
            // l'agent applique déjà aux réponses `Donnees`.
            if (e.longueur !== charge.length) {
                throw new EchecFichiers(
                    'interne',
                    `en-tête Ecrire incohérent : ${e.longueur} annoncés, ${charge.length} reçus`,
                );
            }
            await ecrivain.ecrire(e.chemin, e.position, charge, e.premier, e.dernier);
        } else {
            const c = parseCreer(entete);
            await ecrivain.creer(c.chemin, c.repertoire);
        }
        // ⚠️ **EN-TÊTE VIDE `{}`.** `TYPE_FAIT` n'a pas de forme propre : ce qui
        // identifie l'écriture acquittée est la CORRÉLATION, pas l'en-tête.
        return encoderTexte(TYPE_FAIT, correlation, '{}');
    }
    if (type === TYPE_LISTER) {
        const { chemin } = parseChemin(entete);
        const entrees = await adaptateur.lister(chemin);
        // Charge binaire vide : les entrées tiennent dans l'en-tête.
        return encoderTexte(TYPE_ENTREES, correlation, encodeEntrees(entrees));
    }
    if (type === TYPE_ATTRIBUTS) {
        const { chemin } = parseChemin(entete);
        const m = await adaptateur.attributs(chemin);
        return encoderTexte(
            TYPE_META,
            correlation,
            encodeMeta(m.nom, m.repertoire, m.taille, m.modifie),
        );
    }
    const { chemin, position, longueur } = parseLire(entete);
    const octets = await adaptateur.lire(chemin, position, longueur);
    // 🔴 LA LONGUEUR ANNONCÉE EST CELLE RÉELLEMENT LUE, jamais celle demandée.
    // Un fichier lu jusqu'à sa fin en rend moins ; recopier la demande ferait
    // mentir la trame, et l'agent la refuserait pour incohérence en-tête/charge
    // — c'est le seul contrôle qui empêche d'écrire dans le tampon de ProjFS
    // une quantité que l'émetteur ne croyait pas envoyer.
    return encoderTexte(TYPE_DONNEES, correlation, encodeDonnees(position, octets.length), octets);
}

/**
 * La trame de l'annonce `Bonjour` — **navigateur → pont**.
 *
 * 🔴 **Elle n'attend AUCUNE réponse, et sa corrélation est IGNORÉE** : le pont
 * l'aiguille **avant** de chercher une corrélation en table, précisément parce
 * qu'elle n'en a pas. La valeur `0` est donc un remplissage, pas un identifiant.
 *
 * ⚠️ **`racine` est le `name` de la poignée de répertoire, et c'est un INDICE,
 * pas une preuve** : `isSameEntry()` compare deux poignées vivantes, jamais une
 * poignée à un souvenir. Deux répertoires homonymes sur deux disques différents
 * passeraient pour un seul, et rien ici ne le dirait.
 */
export function trameBonjour(racine: string, forcer: boolean): ArrayBuffer {
    return encoderTexte(TYPE_BONJOUR, 0, encodeBonjour(racine, forcer));
}

/**
 * La trame de l'annonce `Rafraichir` — **navigateur → pont**.
 *
 * Son en-tête est `{}` : ce qui l'identifie est son TYPE. Lui donner une forme
 * ferait une structure à épingler qui n'épingle rien — le précédent de
 * `TYPE_FAIT`, écrit dans `proto/src/fichiers/entetes.rs`.
 */
export function trameRafraichir(): ArrayBuffer {
    return encoderTexte(TYPE_RAFRAICHIR, 0, '{}');
}
