// La chaîne des routeurs HTTP, extraite de `serveur.ts` le 22 août 2026.
//
// 🔴 POURQUOI CE FICHIER EXISTE : `serveur.ts` était à 475 lignes sur 500 au
// moment où le lot « page derrière Pomerium » devait y ajouter un onzième
// routeur. `CLAUDE.md` prescrit l'extraction en tâche DÉDIÉE, AVANT celle qui
// ajoute — « extraire, jamais comprimer » — parce que la marge regagnée par
// une extraction se reperd si on la traite comme acquise (payé six fois).
//
// ⚠️ CETTE EXTRACTION NE CHANGE AUCUN COMPORTEMENT. L'ordre des routeurs, les
// commentaires qui l'expliquent et le `false` final sont repris VERBATIM. Le
// seul changement est que `deps` arrive en paramètre au lieu d'être capturé
// par fermeture.

import type { IncomingMessage, ServerResponse } from 'node:http';
import { servirAuth, type DependancesAuth } from './routes-auth';
import { servirIdentite, type DependancesIdentite } from './routes-identite';
import { servirVm, type DependancesVm } from './routes-vm';
import { servirSession, type DependancesSession } from './routes-session';
import { servirApplications, type DependancesApplications } from './routes-applications';
import { servirIcone, type DependancesIcone } from './routes-icone';
import { servirTeleversement, type DependancesTeleversement } from './routes-televersement';
import { servirInstallation, type DependancesInstallation } from './routes-installation';
import { servirSante, type DependancesSante } from './routes-sante';

/// Tout ce que la chaîne consomme, réuni.
///
/// ⚠️ UNE INTERSECTION, PAS UNE INTERFACE QUI ÉTEND : deux `Dependances*` qui
/// déclareraient la même clé avec des types incompatibles feraient ÉCHOUER un
/// `extends` à la déclaration, alors que l'intersection laisse le conflit se
/// révéler à l'APPEL, sur l'objet réel — c'est-à-dire là où il se corrige.
export type DependancesRoutage = DependancesIdentite &
    DependancesAuth &
    DependancesVm &
    DependancesApplications &
    DependancesIcone &
    DependancesTeleversement &
    DependancesInstallation &
    DependancesSession &
    DependancesSante;

/// Essaie les routeurs dans l'ordre, et rend `false` si aucun n'a servi.
///
/// ⚠️ L'ORDRE EST SIGNIFIANT MAIS NON CONTRAIGNANT ICI : les quatre jeux de
/// chemins sont DISJOINTS (`/auth/*`, `/vm*`, `/session`, `/application*`),
/// et chacun compare exactement plutôt que par préfixe. Un `await` de plus
/// ne coûte donc rien à personne — mais le jour où deux routeurs se
/// disputeraient un chemin, c'est cet ordre qui trancherait, en silence.
///
/// 🔴 LE ROUTEUR DES APPLICATIONS EST CHAÎNÉ AVANT LE 404, ET C'EST LA
/// SEULE LIGNE QUI LE FAIT VIVRE. Sans elle, ses deux routes rendraient le
/// 404 générique — c'est-à-dire la panne la plus discrète possible : le
/// service répond, écoute, et sert les trois autres. `serveur.test.ts` la
/// tient par un test dédié, comme il tient déjà le canal `/agent`.
///
/// ⚠️ LE CORPS DU 404 N'EST PAS TOUCHÉ : « rien ne le testait avant P2, et
/// le changer serait un effet de bord non déclaré ».
export async function servirTout(
    requete: IncomingMessage,
    reponse: ServerResponse,
    deps: DependancesRoutage,
): Promise<boolean> {
    // 🔴 `servirIdentite` EST CHAÎNÉ EN TÊTE, ET CE N'EST PAS INDIFFÉRENT :
    // `/auth/moi` et les deux chemins de `servirAuth` sont DISJOINTS
    // aujourd'hui, mais les trois partagent le préfixe `/auth/`. Le jour
    // où l'un comparerait par préfixe, c'est cet ordre qui trancherait —
    // en silence.
    if (await servirIdentite(requete, reponse, deps)) return true;
    if (await servirAuth(requete, reponse, deps)) return true;
    if (await servirVm(requete, reponse, deps)) return true;
    if (await servirApplications(requete, reponse, deps)) return true;
    if (await servirIcone(requete, reponse, deps)) return true;
    // 🔴 LES DEUX ROUTEURS DE G3, ET CE SONT LES SEULES LIGNES QUI LES FONT
    // VIVRE. Sans elles, leurs sept routes tomberaient dans le 404
    // générique : la panne la plus discrète qui soit, puisque le service
    // répond, écoute, et sert correctement les six autres routeurs.
    // ROUGE JOUÉE : retirer la première fait tomber le test (6ter) de
    // `entetes-routeurs.test.ts`, et LUI SEUL — `1 failed | 10 passed`.
    //
    // ⚠️ Les deux se partagent le préfixe `/televersement/` : le premier
    // sert `…/tranche/:n`, `…/sceller` et l'état, le second `…/contenu`
    // seul. Les jeux restent DISJOINTS — chacun découpe par SEGMENTS et
    // compare leur NOMBRE exactement, jamais par `startsWith` —, donc aucun
    // ne peut voler le chemin de l'autre. L'ordre est une ceinture, pas la
    // garantie.
    if (await servirTeleversement(requete, reponse, deps)) return true;
    if (await servirInstallation(requete, reponse, deps)) return true;
    if (await servirSession(requete, reponse, deps)) return true;
    // ⚠️ `/sante` EST CHAÎNÉE EN DERNIER, et l'ordre n'est pas indifférent
    // ici : c'est la seule route NON AUTHENTIFIÉE du service, et la placer
    // en tête ferait courir sa comparaison de chemin avant celles des
    // routes gardées. Les SIX jeux de chemins restent DISJOINTS, donc
    // aucun ne peut voler le chemin d'un autre ; l'ordre est une ceinture,
    // pas une garantie.
    return servirSante(requete, reponse, deps);
}
