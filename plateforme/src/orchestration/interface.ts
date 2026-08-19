// Le vocabulaire de l'orchestration : ce qu'est une VM pour le service, dans
// quel état elle peut être, et quels verbes existent.
//
// 🔴 CE MODULE EST PUR. Ni base, ni socket, ni horloge : il ne porte que des
// types et deux tableaux `as const`.
//
// 🔴 LE SENS DE LA DÉRIVATION EST « TABLEAU → TYPE », JAMAIS L'INVERSE. Écrire
// d'abord `type Operation = 'lister' | …` puis un tableau de valeurs à côté
// produit DEUX objets qu'on espère égaux ; ici la liste d'exécution et la
// liste de types sont LE MÊME OBJET. Ce dépôt a payé quatre fois un catch-all
// silencieux (`pont_media.rs`, sous-blocs D5 à D8), et `proto/ts/control.ts`
// porte encore un `TYPES_AGENT` écrit à la main sans être confronté à son
// union — c'est cette forme-là qu'on ne reproduit pas.
//
// ⚠️ `satisfies readonly Operation[]` ne suffit PAS, et il faut le dire : il
// interdit d'écrire un verbe qui n'existe pas, jamais d'en OUBLIER un. C'est
// le test d'union d'`interface.test.ts` qui interdit l'oubli, et il tourne
// sous `vitest`. Chacun couvre l'angle mort de l'autre — la doctrine de
// `base/sous-ensemble.test.ts`.

import type { EtatAgent } from '../agents/fraicheur';

/// L'état d'une VM, tel qu'un backend v1 peut le connaître.
///
/// 🔴 C'EST UN RÉEXPORT DE `EtatAgent`, ET IL A EXACTEMENT DEUX MEMBRES —
/// alors que la spec §3.6 en pose quatre (`arretee`, `demarrage`, `prete`,
/// `injoignable`). Les deux premiers supposent un hyperviseur, qu'AUCUN
/// backend de P4 ne pilote (`InventaireStatique` ne fait qu'inventorier ce
/// qu'un administrateur a enrôlé) : les écrire produirait deux variantes que
/// rien n'émet, c'est-à-dire du code mort DANS UN TYPE — l'espèce la plus
/// difficile à retirer, parce qu'aucun test ne rougit sur elle.
///
/// Le jour où un backend d'hyperviseur les produira, il élargira l'union — et
/// toute exhaustivité qui en dépend cassera À LA COMPILATION. C'est la bonne
/// panne : bruyante, et c'est la raison pour laquelle la table de codes de
/// `refus.ts` est un `Record<Motif, number>` et non un objet libre.
export type EtatVm = EtatAgent;

/// Une VM de l'inventaire, telle que l'orchestrateur la rend.
///
/// ⚠️ ELLE PORTE `adresse`, ET LES ROUTES HTTP NE LA RECOPIENT PAS. L'adresse
/// est de la topologie interne : le navigateur parle au signaling, jamais à la
/// VM. L'administration en a besoin, un utilisateur authentifié non — c'est
/// pourquoi elle vit ici et pas dans le corps d'une réponse (D7).
export interface Vm {
    id: string;
    nom: string;
    adresse: string;
    /// `null` = au vivier, à personne. ⚠️ « À personne » n'est PAS « à tout le
    /// monde » : `orchestration/selection.ts` ne la rend à aucun utilisateur.
    utilisateurId: string | null;
    /// `null` si la VM n'a jamais été enrôlée comme agent.
    prefixe: string | null;
    /// Le dernier battement, `null` si l'agent n'a jamais battu. C'est la
    /// seule entrée de `fraicheur.etatDe`.
    vuA: number | null;
}

/// TOUS les verbes de l'orchestrateur, sans exception.
export const OPERATIONS = [
    'lister',
    'etat',
    'demarrer',
    'arreter',
    'instantane',
    'attribuer',
] as const;
export type Operation = (typeof OPERATIONS)[number];

/// Les opérations que `POST /vm/:id/:operation` reconnaît.
///
/// 🔴 C'EST UNE LISTE BLANCHE, jamais une liste noire — le mot que
/// `http/serveur.ts` emploie déjà pour le routage des montées WebSocket. Un
/// chemin portant un verbe absent d'ici n'est pas « refusé » : il n'est PAS
/// SERVI, et le 404 générique s'applique. Un verbe inconnu qui recevrait un
/// 501 mentirait sur l'existence de l'opération.
///
/// 🔴 `attribuer` N'Y FIGURE PAS, ET C'EST TENU PAR UN TEST NOMMÉ. Il
/// n'existe aucun rôle d'administration dans ce service (`identite/jeton.ts`
/// ne connaît que `utilisateur` et `agent`) : une route d'attribution serait
/// ouverte à tout utilisateur authentifié, donc une escalade de privilège
/// offerte. L'attribution passe par `npm run admin:attribuer` (D8).
export const OPERATIONS_HTTP = [
    'demarrer',
    'arreter',
    'instantane',
] as const satisfies readonly Operation[];

/// Le complément exact : les verbes qui existent et que HTTP n'expose pas.
///
/// ⚠️ IL EST ÉCRIT PLUTÔT QUE CALCULÉ, et c'est délibéré : un complément
/// calculé serait vrai par construction, donc ne pourrait jamais rougir. Écrit
/// à la main, il est confronté à `OPERATIONS` par le test d'union — c'est ce
/// qui rend l'oubli d'un verbe DÉTECTABLE.
export const OPERATIONS_HORS_HTTP = [
    'lister',
    'etat',
    'attribuer',
] as const satisfies readonly Operation[];
