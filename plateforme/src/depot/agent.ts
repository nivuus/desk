// Le dépôt `agent_enrole` : enrôler une VM, la relire par son identifiant ou
// par son préfixe, et marquer qu'on vient de l'entendre battre.
//
// 🔴 L'HORLOGE EST UN PARAMÈTRE, jamais lue ici — même règle que
// `depot/session.ts`, `depot/utilisateur.ts` et `signaling/ice.ts`, et c'est
// ce qui rend `agent.test.ts` capable d'asserter une époque EXACTE.
//
// 🔴 AUCUNE VALEUR LITTÉRALE dans le SQL : tout passe en paramètre, sans quoi
// `rendreMarqueurs` lèverait côté Postgres (`base/pilote.ts`).
//
// ⚠️ CE MODULE NE SAIT RIEN DU HACHAGE : il reçoit et rend une empreinte
// opaque, exactement comme `depot/utilisateur.ts`. C'est ce qui permettra de
// durcir `scrypt` sans le rouvrir — le format porte ses propres paramètres
// (`identite/mot-de-passe.ts`).
//
// ⚠️ IL NE SAIT RIEN DE LA FRAÎCHEUR NON PLUS : il rend `vu_a` tel qu'il est,
// `null` compris. Décider `prete` / `injoignable` est le travail d'un module
// PUR, avec son horloge en paramètre.

import type { Pilote } from '../base/pilote';

export interface LigneAgent {
    vm_id: string;
    empreinte_secret: string;
    prefixe_session: string;
    /// `null` tant que l'agent n'a jamais battu. ⚠️ Ce n'est PAS `0` : zéro se
    /// lirait comme une époque de 1970, donc comme un agent injoignable depuis
    /// cinquante-six ans, et les deux états sont distincts.
    ///
    /// ✅ `number` EST VRAI SUR LES DEUX MOTEURS, et ce ne l'a pas toujours
    /// été : `pg` rend les `BIGINT` en chaîne, et cette déclaration était
    /// FAUSSE en production jusqu'à ce que `base/pilote-postgres.ts` pose son
    /// `setTypeParser` (recette de P3). `interroger<T>` faisant un `as T[]`,
    /// aucun typage ne l'aurait attrapée — c'est `pilotes.test.ts` qui la
    /// tient, colonne par colonne.
    vu_a: number | null;
}

const COLONNES = 'vm_id, empreinte_secret, prefixe_session, vu_a';

/// Enrôle une VM.
///
/// Un préfixe déjà pris fait LEVER, par l'index UNIQUE de `0003-agents.sql` —
/// jamais un retour silencieux : ici l'appelant est l'administrateur, et lui
/// cacher l'échec lui ferait croire à un enrôlement qui n'existe pas. Même
/// raisonnement que `creerUtilisateur` sur le courriel.
export async function enroler(
    p: Pilote,
    vmId: string,
    empreinte: string,
    prefixe: string,
): Promise<void> {
    await p.executer(
        `INSERT INTO agent_enrole(${COLONNES}) VALUES(?, ?, ?, ?)`,
        [vmId, empreinte, prefixe, null],
    );
}

/// Rend la ligne, ou `undefined`. JAMAIS une exception sur une VM inconnue :
/// le canal doit répondre le MÊME refus que pour un secret faux, et une
/// exception qui remonterait en erreur interne serait à elle seule un oracle
/// d'énumération — l'appelant apprendrait par tâtonnement quelles VMs
/// existent. Précédent : `depot/utilisateur.ts::lireParEmail`.
export async function lireParVm(p: Pilote, vmId: string): Promise<LigneAgent | undefined> {
    const lignes = await p.interroger<LigneAgent>(
        `SELECT ${COLONNES} FROM agent_enrole WHERE vm_id = ?`,
        [vmId],
    );
    return lignes[0];
}

/// Rend la ligne dont le préfixe est celui-ci, ou `undefined`.
///
/// C'est la SEULE clé dont dispose qui lit un nom de session : `P:bureau` ne
/// porte pas l'identifiant de la VM, il porte son préfixe. L'unicité de la
/// colonne est ce qui rend ce retour non ambigu.
export async function lireParPrefixe(
    p: Pilote,
    prefixe: string,
): Promise<LigneAgent | undefined> {
    const lignes = await p.interroger<LigneAgent>(
        `SELECT ${COLONNES} FROM agent_enrole WHERE prefixe_session = ?`,
        [prefixe],
    );
    return lignes[0];
}

/// Avance `vu_a`. La valeur est ÉCRASÉE, jamais accumulée : c'est un instant,
/// pas un compteur.
///
/// Une VM inconnue n'écrit rien et ne lève pas — l'`UPDATE` touche zéro ligne.
/// C'est délibéré : ce chemin est celui du battement de cœur, et il ne doit
/// jamais être une raison d'abattre le canal.
export async function marquerVu(p: Pilote, vmId: string, maintenant: number): Promise<void> {
    await p.executer('UPDATE agent_enrole SET vu_a = ? WHERE vm_id = ?', [maintenant, vmId]);
}

/// Remplace l'empreinte du secret d'enrôlement d'une VM, et RIEN D'AUTRE.
///
/// 🔴 `prefixe_session` N'EST PAS TOUCHÉ, ET C'EST LE POINT DE CETTE FONCTION.
/// Le préfixe compose le nom des sessions VIVANTES de cette VM
/// (`agents/prefixe.ts`, spec §3.4) : le faire tourner en même temps que le
/// secret couperait toute session en cours. **Rotation du secret n'est pas
/// rotation de l'identité**, et les deux n'ont pas la même urgence — un secret
/// se remplace le jour où il fuite, une identité ne se remplace jamais en
/// urgence.
///
/// Rend le nombre de lignes touchées. Une VM inconnue en touche ZÉRO et ne lève
/// PAS : c'est ce qui permet à `admin/enroler-agent.ts` de rendre un refus
/// MOTIVÉ plutôt qu'une exception. ⚠️ Le raisonnement d'oracle qui vaut pour
/// `lireParVm` ne s'applique PAS ici — l'appelant est l'administrateur, pas un
/// inconnu au bout d'un canal, et lui cacher qu'il s'est trompé de VM lui
/// ferait croire à une rotation qui n'a pas eu lieu.
///
/// ⚠️ CE QU'ELLE NE FAIT PAS : révoquer les jetons d'agent DÉJÀ délivrés, qui
/// restent valides jusqu'à leur expiration. Même propriété que les jetons
/// humains (spec §3.5), et elle borne la fenêtre à `DUREE_JETON_ACCES_MS`.
export async function remplacerEmpreinte(
    p: Pilote,
    vmId: string,
    empreinte: string,
): Promise<number> {
    const r = await p.executer(
        'UPDATE agent_enrole SET empreinte_secret = ? WHERE vm_id = ?',
        [empreinte, vmId],
    );
    return r.lignes;
}
