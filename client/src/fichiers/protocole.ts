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
//   REQUÊTES  (1..5)  → une réponse, toujours ;
//   ANNONCES  (6)     → un rappel injecté, et `null` ;
//   RÉPONSES  (64..)  → ignorées : le navigateur ne demande rien.
//
// Le seul cas où l'on ne répond pas SANS que ce soit une annonce est celui où
// l'on n'a pas de requête — trame illisible, type de réponse, type inconnu.

import {
    TYPE_ATTRIBUTS,
    TYPE_CREER,
    TYPE_DONNEES,
    TYPE_DUES,
    TYPE_ECHEC,
    TYPE_ECRIRE,
    TYPE_ENTREES,
    TYPE_FAIT,
    TYPE_LIRE,
    TYPE_LISTER,
    TYPE_META,
    decoder,
    encoderTexte,
} from '../../../proto/ts/fichiers';
import {
    encodeDonnees,
    encodeEchec,
    encodeEntrees,
    encodeMeta,
    parseChemin,
    parseCreer,
    parseDues,
    parseEcrire,
    parseLire,
    type Due,
} from '../../../proto/ts/fichiers-entetes';
import { EchecFichiers, type Adaptateur } from './adaptateur';
import type { Ecrivain } from './ecriture';

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
    onDues?: (dues: Due[]) => void;
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
                    break;
                // ── ANNONCE : elle ne reçoit RIEN, et la famille est NOMMÉE.
                case TYPE_DUES: {
                    try {
                        options.onDues?.(parseDues(trame.entete).dues);
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
                    options.ecrivain,
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
                if (trame.type === TYPE_ECRIRE || trame.type === TYPE_CREER) {
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

/** Le chemin d'un en-tête d'écriture, s'il est lisible. */
function cheminDe(entete: unknown): string | undefined {
    if (typeof entete !== 'object' || entete === null) return undefined;
    const chemin = (entete as { chemin?: unknown }).chemin;
    return typeof chemin === 'string' ? chemin : undefined;
}

async function servir(
    adaptateur: Adaptateur,
    ecrivain: Ecrivain | undefined,
    type: number,
    correlation: number,
    entete: unknown,
    charge: Uint8Array,
): Promise<ArrayBuffer> {
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
        return encoderTexte(TYPE_META, correlation, encodeMeta(m.repertoire, m.taille, m.modifie));
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
