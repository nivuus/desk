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
// ⚠️ UNE REQUÊTE REÇOIT TOUJOURS UNE RÉPONSE, y compris quand elle échoue. Ne
// rien répondre laisserait la commande en vol côté agent jusqu'à son expiration,
// et l'Explorateur se figerait sur une panne pourtant immédiate. Le seul cas où
// l'on ne répond pas est celui où l'on n'a pas de requête — trame illisible,
// type de réponse, type inconnu.

import {
    TYPE_ATTRIBUTS,
    TYPE_DONNEES,
    TYPE_ECHEC,
    TYPE_ENTREES,
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
    parseLire,
} from '../../../proto/ts/fichiers-entetes';
import { EchecFichiers, type Adaptateur } from './adaptateur';

/** Où partent les trames qu'on n'a pas su traiter. Injecté, donc observable. */
export type Journal = (message: string) => void;

export interface Serveur {
    /**
     * Traite une trame reçue et rend la trame à réémettre, ou `null` s'il n'y
     * a rien à répondre.
     */
    traiter(octets: ArrayBuffer): Promise<ArrayBuffer | null>;
}

export function creerServeur(adaptateur: Adaptateur, journal: Journal = () => {}): Serveur {
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
                case TYPE_LISTER:
                case TYPE_ATTRIBUTS:
                case TYPE_LIRE:
                    break;
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
                return await servir(adaptateur, trame.type, trame.correlation, trame.entete);
            } catch (e) {
                // Le code est celui de l'adaptateur quand il en porte un ; tout
                // le reste — en-tête malformé compris — est `interne`. Inventer
                // un code plus précis ferait traduire à l'agent un HRESULT faux
                // plutôt qu'un HRESULT vague.
                const code = e instanceof EchecFichiers ? e.code : 'interne';
                journal(`échec ${code} sur la corrélation ${trame.correlation} : ${(e as Error).message}`);
                return encoderTexte(TYPE_ECHEC, trame.correlation, encodeEchec(code));
            }
        },
    };
}

async function servir(
    adaptateur: Adaptateur,
    type: number,
    correlation: number,
    entete: unknown,
): Promise<ArrayBuffer> {
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
