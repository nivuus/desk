// Quelles VMs d'un inventaire appartiennent à un utilisateur.
//
// 🔴 CE MODULE EST PUR, et c'est le point : ni base, ni socket, ni horloge. Un
// défaut de filtre qui vivrait dans `http/routes-vm.ts` fuiterait l'inventaire
// ENTIER, et il n'existerait aucun endroit où le rougir sans monter un serveur.
//
// 🔴 « À PERSONNE » N'EST PAS « À TOUT LE MONDE ». Une VM dont
// `utilisateurId` est `null` est AU VIVIER : elle n'est rendue à aucun
// demandeur. Le contraire ferait voir à tout utilisateur authentifié chaque VM
// non encore attribuée — c'est-à-dire l'inventaire de la flotte. ⚠️ Le
// sous-bloc G1 retient délibérément l'autre comportement pour SES routes ; les
// deux chantiers divergent, et c'est déclaré plutôt que réconcilié en douce
// par la seconde branche arrivée.

import type { Vm } from './interface';

/// Toutes les VMs de cet utilisateur. L'ordre de l'inventaire est conservé.
///
/// ⚠️ LA COMPARAISON EST UNE ÉGALITÉ, jamais un préfixe ni une inclusion :
/// `alice` et `alice-bis` sont deux utilisateurs. Même règle que
/// `http/cors.ts`, et pour la même raison — un `startsWith` ouvre une famille
/// de correspondances que personne n'a décidées.
export function vmsDe(inventaire: readonly Vm[], utilisateurId: string): Vm[] {
    // `null !== utilisateurId` par construction, donc le vivier est exclu sans
    // qu'aucune branche ne le dise — mais le test le nomme, parce que c'est
    // une propriété et non un effet de bord de la comparaison.
    return inventaire.filter((v) => v.utilisateurId === utilisateurId);
}

/// LA VM de cet utilisateur, ou `undefined`.
///
/// 🔴 ELLE LÈVE SUR UN DOUBLON, et ce n'est pas une préférence de style.
/// L'index partiel `vm_un_utilisateur` (`0001-socle.sql`) rend le cas
/// impossible EN BASE — mais cette fonction reçoit un tableau, et rien dans sa
/// signature ne dit d'où il vient. Rendre la première en silence choisirait
/// une VM au hasard et le ferait sans trace ; l'exception, elle, dit où
/// regarder le jour où l'index aurait disparu.
export function laVmDe(inventaire: readonly Vm[], utilisateurId: string): Vm | undefined {
    const siennes = vmsDe(inventaire, utilisateurId);
    if (siennes.length > 1) {
        throw new Error(
            `l'inventaire porte ${siennes.length} VM pour un même utilisateur, ce que ` +
                "l'index partiel `vm_un_utilisateur` doit rendre impossible : la base " +
                'ou son schéma est à examiner, ce refus ne se contourne pas.',
        );
    }
    return siennes[0];
}
