// Le dépôt `session` : ouvrir une ligne à l'appariement, la clore au départ
// des deux pairs, balayer celles qu'un arrêt brutal a laissées ouvertes.
//
// 🔴 L'HORLOGE EST UN PARAMÈTRE, jamais lue ici. C'est la règle du
// sous-ensemble portable (spec §3.2 : les horodatages sont « toujours écrites
// par l'application ») ET le précédent du dépôt : `src/signaling/ice.ts:32-42`
// prend déjà `maintenant` en paramètre pour la même raison. Ce qui rend le
// choix vérifiable plutôt que déclaratif : `session.test.ts` asserte des
// VALEURS EXACTES, qu'un `Date.now()` caché ferait toutes échouer.
//
// ⚠️ CE QUE CETTE TABLE N'EST PAS, et il faut le lire avant de s'y fier : une
// ligne `session` est une trace de l'APPARIEMENT, pas un état de vérité du
// média. Le flux WebRTC ne dépend plus du signaling une fois l'offre et la
// réponse échangées (`agent/src/demarrage.rs` le journalise explicitement) :
// une session peut être VIVANTE alors que le balayage de démarrage vient de
// clore sa ligne. Le balayage MENT donc sur les sessions qui ont réellement
// survécu. C'est une limite de P1, nommée, pas un défaut à corriger ici.

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

export interface LigneSession {
    id: string;
    nom_session: string;
    utilisateur_id: string | null;
    vm_id: string | null;
    ouverte_a: number;
    fermee_a: number | null;
    motif: string | null;
}

/// Motif posé aux lignes qu'un arrêt brutal a laissées ouvertes.
///
/// Il est passé en PARAMÈTRE de la requête, jamais écrit dans le SQL : une
/// valeur littérale ferait lever `rendreMarqueurs` côté Postgres.
export const MOTIF_BALAYAGE = 'plateforme redémarrée';

/// Ouvre une ligne et rend son identifiant.
///
/// L'identifiant est un UUID v4, jamais le nom de session : ce dernier n'est
/// PAS unique dans le temps — `bureau` revient à chaque démarrage d'agent.
///
/// ⚠️ `utilisateurId` est FACULTATIF, et il doit le rester. Le rendre requis
/// casserait les appelants de P1, et surtout il n'existe pas toujours : une
/// session appariée par un pair `agent` seul — la session de contrôle
/// `bureau` au démarrage d'une VM — n'a personne à inscrire. ⚠️ LA RAISON A
/// CHANGÉ AU SOUS-BLOC P3, la conséquence non : l'agent a désormais une
/// identité (le canal `/agent` la lui délivre), mais il ne REVENDIQUE
/// toujours rien — sa session doit rester revendicable par le client humain
/// qui la rejoindra (`identite/garde.ts`). La colonne naît donc NULL,
/// exactement comme P1 l'écrivait.
///
/// C'est cet argument qui rend le mot « enregistrée » du critère ③
/// littéralement vrai : la DÉCISION est prise par le registre en mémoire
/// (`signaling/propriete.ts`), l'ENREGISTREMENT durable se fait ici, et c'est
/// de lui que P4 aura besoin.
///
/// ⚠️ `vmId` est FACULTATIF POUR LA MÊME RAISON, et il vient APRÈS
/// `utilisateurId` pour ne déplacer aucun appelant existant. C'est la trace
/// (`signaling/trace.ts`) qui le résout, en découpant le préfixe du nom de
/// session puis en le cherchant dans `agent_enrole`. Une session `bureau`
/// SANS préfixe — le mode d'essai local que la spec §10 pose comme légitime —
/// n'a aucune VM honnête à inscrire, et une session à préfixe INCONNU non
/// plus : dans les deux cas la colonne reste `null`, jamais une chaîne vide
/// qui mentirait sur ce qu'on sait.
export async function ouvrirSession(
    p: Pilote,
    nomSession: string,
    maintenant: number,
    utilisateurId?: string,
    vmId?: string,
): Promise<string> {
    const id = randomUUID();
    // Les colonnes sont TOUJOURS nommées, et leurs valeurs TOUJOURS passées en
    // paramètre — `null` compris. Écrire deux requêtes selon la présence de
    // l'identifiant en ferait diverger une le jour où la table changerait.
    await p.executer(
        'INSERT INTO session(id, nom_session, utilisateur_id, vm_id, ouverte_a) VALUES(?, ?, ?, ?, ?)',
        [id, nomSession, utilisateurId ?? null, vmId ?? null, maintenant],
    );
    return id;
}

/// Clôt une ligne. La clause `fermee_a IS NULL` rend l'appel IDEMPOTENT : une
/// seconde clôture ne déplace ni l'instant ni le motif de la première — sans
/// elle, une déconnexion tardive réécrirait une trace déjà juste.
export async function clore(
    p: Pilote,
    id: string,
    maintenant: number,
    motif: string | null,
): Promise<void> {
    await p.executer(
        'UPDATE session SET fermee_a = ?, motif = ? WHERE id = ? AND fermee_a IS NULL',
        [maintenant, motif, id],
    );
}

/// Clôt toutes les lignes restées ouvertes et rend leur NOMBRE.
///
/// Voir l'avertissement de tête : ce balayage ment sur les sessions qui ont
/// réellement survécu à l'arrêt du service.
export async function balayerLesOuvertes(p: Pilote, maintenant: number): Promise<number> {
    const r = await p.executer(
        'UPDATE session SET fermee_a = ?, motif = ? WHERE fermee_a IS NULL',
        [maintenant, MOTIF_BALAYAGE],
    );
    return r.lignes;
}

/// Toutes les lignes portant ce nom de session, de la plus ancienne à la plus
/// récente. Il peut y en avoir plusieurs : le nom n'est pas une identité.
export async function lireParNom(p: Pilote, nomSession: string): Promise<LigneSession[]> {
    return p.interroger<LigneSession>(
        'SELECT id, nom_session, utilisateur_id, vm_id, ouverte_a, fermee_a, motif FROM session WHERE nom_session = ? ORDER BY ouverte_a',
        [nomSession],
    );
}
