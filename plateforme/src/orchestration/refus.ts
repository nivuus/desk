// Le vocabulaire du refus : ce qu'un orchestrateur rend quand il ne peut pas
// faire ce qu'on lui demande.
//
// 🔴 CE MODULE EST PUR.
//
// 🔴 UN REFUS EST UNE VALEUR, JAMAIS UN SILENCE. Ni `Promise<void>`, ni
// `boolean`, ni exception : la spec §3.6 appelle cela « la décision de forme
// la plus importante », et sa raison tient en une phrase — un `Promise<void>`
// qui ne fait rien serait indiscernable d'un `Promise<void>` qui fait le
// travail, c'est-à-dire une panne muette. Un `boolean` ne dirait pas POURQUOI,
// et une exception ferait répondre 500 là où le service doit avouer 501.
//
// 🔴 LE MOTIF EST UN CODE COURT, PAS UNE PHRASE. La spec §3.6 proposait
// `{ refus: 'non supporté par ce backend' }` ; tout le reste du service emploie
// un code (`{refus:'identifiants'}` dans `http/routes-auth.ts`,
// `{refus:'methode'}` et `{refus:'interne'}` dans `http/serveur.ts`, les quatre
// motifs d'`identite/garde.ts`). Une phrase ne se compare pas, ne se traduit
// pas, et se réécrit sans que rien ne casse.

import type { Operation } from './interface';

/// Qui refuse. ⚠️ C'est une CONSTANTE EXPORTÉE, jamais un littéral recopié au
/// point d'usage : son intérêt est le jour où un second backend existera, et
/// deux copies d'un nom divergent dès qu'on renomme l'une.
export const BACKEND_STATIQUE = 'inventaire-statique';

/// Tous les motifs, sans exception.
///
/// 🔴 LE TABLEAU PRODUIT LE TYPE, jamais l'inverse — même raison qu'à
/// `orchestration/interface.ts` : la liste d'exécution et la liste de types
/// sont LE MÊME OBJET.
export const MOTIFS = [
    /// Le backend ne sait pas faire — 501, et journalisé.
    'non-supporte',
    /// Inconnue, OU appartenant à quelqu'un d'autre : le MÊME refus, et c'est
    /// délibéré. Les distinguer ferait un ORACLE D'ÉNUMÉRATION — un
    /// utilisateur apprendrait quelles VMs existent en lisant le code de
    /// retour. Troisième application de la règle, après `routes-auth.ts` et
    /// `agents/enrolement.ts` (D8).
    'vm-inconnue',
    /// La VM a déjà un propriétaire. ⚠️ Ce motif ne sort JAMAIS d'une route
    /// HTTP : il n'y sortirait que d'une attribution, qui n'y est pas exposée.
    'vm-deja-attribuee',
    /// L'utilisateur a déjà une VM — c'est l'index partiel `vm_un_utilisateur`
    /// qui le dit, en LEVANT. Même réserve d'exposition que ci-dessus.
    'utilisateur-servi',
    /// L'utilisateur n'a aucune VM attribuée.
    'aucune-vm',
    /// `vu_a` trop vieux, ou nul (`agents/fraicheur.ts`).
    'agent-injoignable',
] as const;
export type Motif = (typeof MOTIFS)[number];

/// Ce que rend une opération d'orchestration.
///
/// ⚠️ Le succès ne porte AUCUNE donnée, et c'est suffisant : les trois verbes
/// qui réussissent en v1 (`lister`, `etat`, `attribuer`) rendent soit leur
/// propre valeur, soit ce `Resultat`. Y glisser un champ facultatif ferait de
/// `ok:true` un objet dont il faudrait vérifier le contenu.
export type Resultat =
    | { ok: true }
    | { ok: false; motif: Motif; operation: Operation; backend: string };

/// Le code HTTP de chaque motif.
///
/// 🔴 `Record<Motif, number>` EST LE REMÈDE STRUCTUREL, et il est choisi
/// délibérément CONTRE une liste écrite à la main. Ajouter un motif sans lui
/// donner son code est une ERREUR DE COMPILATION, que `npm run typecheck` —
/// une étape de `scripts/verify-all.sh` — attrape. Ce dépôt a payé quatre fois
/// un `match` catch-all qui tuait un fil en silence (`pont_media.rs`,
/// sous-blocs D5 à D8), et `proto/ts/control.ts` porte encore un `TYPES_AGENT`
/// écrit à la main sans être confronté à son union.
///
/// ⚠️ `tsc` NE VOIT PAS UNE CLÉ EN TROP posée par un `as any` : c'est
/// `Object.keys(CODE_HTTP)` dans le test qui la voit. Les deux gardes sont
/// nécessaires, et la rouge de chacune a été jouée.
export const CODE_HTTP: Record<Motif, number> = {
    // 501 et non 500 : le service va bien, c'est son backend qui ne sait pas
    // faire. Un 500 ferait chercher une panne inexistante.
    'non-supporte': 501,
    'vm-inconnue': 404,
    'vm-deja-attribuee': 409,
    'utilisateur-servi': 409,
    'aucune-vm': 409,
    // 503 : la VM est bien à cet utilisateur, elle ne répond pas. C'est un état
    // du monde, pas une erreur de la requête.
    'agent-injoignable': 503,
};

/// Construit un refus. `backend` a `BACKEND_STATIQUE` pour DÉFAUT — jamais un
/// littéral recopié à chaque appel.
export function refuser(
    motif: Motif,
    operation: Operation,
    backend: string = BACKEND_STATIQUE,
): Resultat {
    return { ok: false, motif, operation, backend };
}
