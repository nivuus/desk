// Le dépôt `vm` : l'inventaire que P4 lit, et la SEULE colonne dont le
// service soit propriétaire — `vm.utilisateur_id`.
//
// 🔴 CE MODULE NE CRÉE AUCUNE VM, et c'est le sens du mot « statique » de la
// spec §3.6 relu par D1 : le backend ne pilote aucun hyperviseur. Le seul
// chemin de création reste `admin/enroler-agent.ts`, et P4 n'en ajoute pas
// d'autre. Ce qu'il ajoute est `admin:attribuer`, qui ne crée rien non plus :
// il pose un propriétaire sur une VM déjà enrôlée.
//
// 🔴 AUCUNE VALEUR LITTÉRALE dans le SQL : tout passe en paramètre, `null`
// compris, sans quoi `rendreMarqueurs` lèverait côté Postgres
// (`base/pilote.ts`). ⚠️ Cette moitié du lint ne mord QUE sur le chemin
// Postgres — le lint statique de `base/sous-ensemble.test.ts` ne balaie que
// les `.sql`. Une requête fautive écrite ici serait donc verte sous
// `test:sqlite` seul.
//
// 🔴 L'HORLOGE N'EST PAS LUE ICI, et il n'y en a même pas besoin : ce dépôt
// n'écrit aucun horodatage. `vm.vue_a` existe dans le schéma et ce module NE
// LA SÉLECTIONNE PAS — voir `COLONNES`.

import type { Pilote } from '../base/pilote';

/// Une ligne d'inventaire : la VM, et ce que son enrôlement en dit.
export interface LigneVm {
    id: string;
    nom: string;
    adresse: string;
    /// `null` = au vivier. ⚠️ « À personne » n'est pas « à tout le monde » :
    /// c'est `orchestration/selection.ts` qui porte cette règle.
    utilisateur_id: string | null;
    /// `null` si la VM n'a jamais été enrôlée comme agent — d'où le LEFT JOIN.
    prefixe_session: string | null;
    /// Le dernier battement. `null` si jamais vue.
    ///
    /// ✅ `number` EST VRAI SUR LES DEUX MOTEURS, et ce ne l'a pas toujours
    /// été : `pg` rendait les `BIGINT` en chaîne jusqu'au `setTypeParser` de
    /// `base/pilote-postgres.ts` (recette de P3). `interroger<T>` faisant un
    /// `as T[]`, aucun typage ne l'aurait attrapé — c'est `vm.test.ts` qui le
    /// tient, au point d'usage, et `pilotes.test.ts` colonne par colonne.
    vu_a: number | null;
}

/// Les colonnes sont ÉNUMÉRÉES, jamais `SELECT *`.
///
/// 🔴 `vm.vue_a` EN EST DÉLIBÉRÉMENT ABSENTE. Elle existe dans
/// `0001-socle.sql`, et elle est ORPHELINE : P3 a créé `agent_enrole.vu_a` à
/// sa place, et AUCUN code de production n'écrit `vm.vue_a`. La sélectionner
/// ferait croire à un successeur qu'elle est renseignée, et il lirait des
/// `null` en pensant lire un silence. C'est la seule garde bon marché contre
/// cette confusion, et elle vaut d'être écrite ici plutôt qu'espérée.
const COLONNES =
    'v.id, v.nom, v.adresse, v.utilisateur_id, a.prefixe_session, a.vu_a';

/// 🔴 UN `LEFT JOIN`, JAMAIS UN `JOIN`. Une VM créée sans enrôlement — ce que
/// rien n'interdit, les deux tables étant écrites par deux instructions
/// distinctes d'`admin/enroler-agent.ts` — DISPARAÎTRAIT de l'inventaire sous
/// un `JOIN`, sans qu'aucune erreur ne le dise. Un inventaire qui perd des
/// lignes en silence est exactement la panne muette que ce dépôt combat.
const DEPUIS = 'FROM vm v LEFT JOIN agent_enrole a ON a.vm_id = v.id';

/// L'inventaire ENTIER. Le filtrage par utilisateur est le travail d'un module
/// PUR (`orchestration/selection.ts`) : le faire ici mettrait la règle hors de
/// portée d'un test qui n'ouvre pas de base.
export async function lister(p: Pilote): Promise<LigneVm[]> {
    return p.interroger<LigneVm>(`SELECT ${COLONNES} ${DEPUIS} ORDER BY v.nom`, []);
}

/// Rend la ligne, ou `undefined`. JAMAIS une exception sur une VM inconnue :
/// une exception qui remonterait en 500 serait à elle seule un oracle
/// d'énumération. Précédent : `depot/agent.ts::lireParVm`.
export async function lireParId(p: Pilote, id: string): Promise<LigneVm | undefined> {
    const lignes = await p.interroger<LigneVm>(
        `SELECT ${COLONNES} ${DEPUIS} WHERE v.id = ?`,
        [id],
    );
    return lignes[0];
}

/// Idem par le NOM. ⚠️ `vm.nom` n'est PAS unique (`0001-socle.sql`) : deux
/// enrôlements du même nom sont possibles, et cette fonction rend alors la
/// première ligne. C'est acceptable pour l'unique appelant — la commande
/// d'administration, dont l'opérateur connaît le nom qu'il a donné — et ce ne
/// le serait pas sur une route.
export async function lireParNom(p: Pilote, nom: string): Promise<LigneVm | undefined> {
    const lignes = await p.interroger<LigneVm>(
        `SELECT ${COLONNES} ${DEPUIS} WHERE v.nom = ?`,
        [nom],
    );
    return lignes[0];
}

/// Attribue la VM SI ELLE EST LIBRE, et rend le nombre de lignes touchées.
///
/// 🔴 `AND utilisateur_id IS NULL` EST LA GARANTIE, et ce n'est PAS la lecture
/// préalable que fait l'appelant. La clause est RÉÉVALUÉE PAR LE MOTEUR au
/// moment de l'écriture : c'est ce qui rend la course sûre. Mesuré sur
/// PostgreSQL 16.15 en `READ COMMITTED`, deux transactions visant la même VM
/// libre — A obtient `1 ligne`, B BLOQUE sur le verrou de ligne, puis rend
/// `0 ligne` après le `COMMIT` de A, Postgres réévaluant la clause sur la
/// ligne mise à jour. Exactement un gagnant, aucune exception, aucun
/// écrasement.
///
/// 🔴 SANS ELLE, LE VOL PASSE. Mesuré sur les DEUX moteurs : un `UPDATE vm SET
/// utilisateur_id = ? WHERE id = ?` nu rend `1 ligne` sur une VM déjà
/// attribuée à quelqu'un d'autre. ⚠️ L'index partiel `vm_un_utilisateur`
/// N'INTERDIT PAS CE VOL — il rend `utilisateur_id` unique à travers les
/// lignes, donc il interdit qu'un utilisateur ait DEUX VMs, jamais qu'une VM
/// change de main. La spec et `0001-socle.sql` attribuaient tous deux au même
/// index une propriété qu'il n'a pas (divergence E3).
///
/// ⚠️ `0` CONFOND TROIS CAUSES : VM inconnue, VM déjà prise, ré-attribution au
/// même utilisateur. C'est l'appelant qui les distingue, en lisant d'abord —
/// et il ne les distingue QUE là où c'est légitime, jamais sur une route
/// publique où ce serait un oracle (E8, D8).
///
/// ⚠️ ELLE LÈVE quand l'utilisateur a déjà une autre VM : c'est l'index
/// partiel, et le texte de l'exception DIFFÈRE selon le moteur. Aucun appelant
/// ne doit le comparer.
export async function attribuerSiLibre(
    p: Pilote,
    vmId: string,
    utilisateurId: string,
): Promise<number> {
    const r = await p.executer(
        'UPDATE vm SET utilisateur_id = ? WHERE id = ? AND utilisateur_id IS NULL',
        [utilisateurId, vmId],
    );
    return r.lignes;
}

/// Rend la VM au vivier, et rend le nombre de lignes touchées.
///
/// ⚠️ AUCUNE CLAUSE SUR LE PROPRIÉTAIRE ACTUEL, à dessein : le seul appelant
/// est la commande d'administration, dont l'objet est précisément de reprendre
/// une VM à quelqu'un. Le jour où un utilisateur pourrait rendre SA VM
/// lui-même, cette fonction aurait besoin d'un `AND utilisateur_id = ?` —
/// sans quoi il rendrait celle d'un autre.
///
/// Le `null` passe en PARAMÈTRE comme toute valeur.
export async function detacher(p: Pilote, vmId: string): Promise<number> {
    const r = await p.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [null, vmId]);
    return r.lignes;
}
