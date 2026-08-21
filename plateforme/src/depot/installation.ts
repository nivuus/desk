// Le dépôt `installation` : la demande, son avancement, et son issue.
//
// 🔴 L'HORLOGE EST UN PARAMÈTRE, jamais lue ici. 🔴 AUCUNE VALEUR LITTÉRALE
// dans le SQL. Mêmes règles que ses quatre voisins, et pour les mêmes raisons.
//
// 🔴 CE MODULE NE DÉCIDE D'AUCUNE ISSUE. Le verdict est calculé par
// `agent/src/apps/installation/verdict.rs`, PUR, sur la VM : c'est lui qui
// connaît le compte d'applications apparues pendant la fenêtre, et lui seul.
// Ici on écrit ce que l'agent rapporte.
//
// 🔴 ET LE CODE DE SORTIE N'EST PAS INTERPRÉTÉ ICI NON PLUS. `msiexec` rend
// 3010 pour un succès qui demande un redémarrage, et beaucoup d'installeurs
// rendent 0 après une annulation : une colonne `reussie BOOLEAN` dérivée du
// code serait fausse dans les deux sens. Le code est RAPPORTÉ, à côté de
// l'issue.

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

/// L'état de la LIGNE, distinct de l'issue.
///
/// 🔴 C'EST LUI QUI GOUVERNE LA RÉÉMISSION : la plateforme cesse de pousser
/// l'ordre dès que l'état n'est plus `en_attente`. C'est la PREMIÈRE des deux
/// ceintures contre une double exécution ; la seconde est le marqueur sur le
/// disque de la VM, et elle protège du cas où la première a perdu sa base.
export type EtatInstallation = 'en_attente' | 'en_cours' | 'terminee';

export interface LigneInstallation {
    id: string;
    vm_id: string;
    televersement_id: string;
    demandee_a: number;
    etat: EtatInstallation;
    /// `transfert` | `execution` | `reconciliation`, ou la chaîne vide tant que
    /// rien n'a commencé. ⚠️ `empreinte` n'est PAS une phase de ce canal : elle
    /// se déroule dans le navigateur.
    phase: string;
    octets_faits: number;
    /// Vaut zéro en phase `execution`, où il n'y a rien à totaliser.
    octets_total: number;
    ecoule_ms: number;
    /// 🔴 `null` = LE CODE N'A PAS PU ÊTRE RECUEILLI, et c'est un fait distinct
    /// de tout code entier. Une sentinelle `-1` les confondrait.
    code_sortie: number | null;
    issue: string | null;
    motif: string | null;
    journal: string | null;
    /// ⚠️ Un entier et non un booléen : SQLite n'a pas de type booléen, et
    /// c'est la convention du reste du schéma.
    journal_tronque: number;
    terminee_a: number | null;
    maj_a: number;
}

export async function creer(
    p: Pilote,
    entree: { vmId: string; televersementId: string },
    maintenant: number,
): Promise<LigneInstallation> {
    const ligne: LigneInstallation = {
        id: randomUUID(),
        vm_id: entree.vmId,
        televersement_id: entree.televersementId,
        demandee_a: maintenant,
        etat: 'en_attente',
        phase: '',
        octets_faits: 0,
        octets_total: 0,
        ecoule_ms: 0,
        code_sortie: null,
        issue: null,
        motif: null,
        journal: null,
        journal_tronque: 0,
        terminee_a: null,
        maj_a: maintenant,
    };
    await p.executer(
        'INSERT INTO installation(id,vm_id,televersement_id,demandee_a,etat,phase,octets_faits,'
            + 'octets_total,ecoule_ms,code_sortie,issue,motif,journal,journal_tronque,terminee_a,maj_a)'
            + ' VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)',
        [
            ligne.id, ligne.vm_id, ligne.televersement_id, ligne.demandee_a, ligne.etat,
            ligne.phase, ligne.octets_faits, ligne.octets_total, ligne.ecoule_ms,
            ligne.code_sortie, ligne.issue, ligne.motif, ligne.journal,
            ligne.journal_tronque, ligne.terminee_a, ligne.maj_a,
        ],
    );
    return ligne;
}

const COLONNES =
    'id, vm_id, televersement_id, demandee_a, etat, phase, octets_faits, octets_total,'
    + ' ecoule_ms, code_sortie, issue, motif, journal, journal_tronque, terminee_a, maj_a';

export async function lireParId(
    p: Pilote,
    id: string,
): Promise<LigneInstallation | undefined> {
    const lignes = await p.interroger<LigneInstallation>(
        `SELECT ${COLONNES} FROM installation WHERE id = ?`,
        [id],
    );
    return lignes[0];
}

/// Les installations qu'une VM doit encore recevoir.
///
/// 🔴 C'EST LA REQUÊTE DE LA RÉÉMISSION À L'ENRÔLEMENT, et l'index
/// `installation_par_vm_et_etat` existe pour elle. Un `push` WebSocket n'a
/// AUCUNE garantie de livraison : sans cette réémission, un ordre émis pendant
/// une coupure serait perdu SANS TERME. C'est le même filet que le
/// `complet = true` du catalogue, et la recette de G1 l'a vu fonctionner sur le
/// chemin réel.
export async function lireEnAttentePourVm(
    p: Pilote,
    vmId: string,
): Promise<LigneInstallation[]> {
    return p.interroger<LigneInstallation>(
        `SELECT ${COLONNES} FROM installation WHERE vm_id = ? AND etat = ? ORDER BY demandee_a`,
        [vmId, 'en_attente'],
    );
}

/// Enregistre une progression rapportée par l'agent.
///
/// ⚠️ ELLE FAIT PASSER L'ÉTAT À `en_cours`, ET C'EST CE QUI ARRÊTE LA
/// RÉÉMISSION. Sans ce passage, la plateforme rejouerait l'ordre au prochain
/// enrôlement alors que l'installateur tourne déjà — et l'agent aurait à
/// s'en défendre seul, par son marqueur de disque.
export async function avancer(
    p: Pilote,
    id: string,
    avancement: { phase: string; octetsFaits: number; octetsTotal: number; ecouleMs: number },
    maintenant: number,
): Promise<void> {
    await p.executer(
        'UPDATE installation SET etat = ?, phase = ?, octets_faits = ?, octets_total = ?,'
            + ' ecoule_ms = ?, maj_a = ? WHERE id = ? AND etat <> ?',
        [
            'en_cours', avancement.phase, avancement.octetsFaits, avancement.octetsTotal,
            avancement.ecouleMs, maintenant, id, 'terminee',
        ],
    );
}

/// ⚠️ `AND etat <> 'terminee'` SUR LES DEUX ÉCRITURES, et ce n'est pas une
/// précaution de style : la plateforme RÉÉMET, donc un agent peut rapporter
/// deux fois. Sans ce garde, une progression tardive écraserait une issue déjà
/// posée et une installation terminée redeviendrait « en cours » — c'est-à-dire
/// que la réémission recommencerait.
export async function terminer(
    p: Pilote,
    id: string,
    issue: {
        issue: string;
        motif: string | null;
        codeSortie: number | null;
        journal: string;
        journalTronque: boolean;
    },
    maintenant: number,
): Promise<void> {
    await p.executer(
        'UPDATE installation SET etat = ?, issue = ?, motif = ?, code_sortie = ?, journal = ?,'
            + ' journal_tronque = ?, terminee_a = ?, maj_a = ? WHERE id = ? AND etat <> ?',
        [
            'terminee', issue.issue, issue.motif, issue.codeSortie, issue.journal,
            issue.journalTronque ? 1 : 0, maintenant, maintenant, id, 'terminee',
        ],
    );
}
