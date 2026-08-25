// La chaîne des routeurs HTTP, extraite de `serveur.ts` le 22 août 2026.
//
// 🔴 POURQUOI CE FICHIER EXISTE : `serveur.ts` était à 475 lignes sur 500 au
// moment où le lot « page derrière Pomerium » devait y ajouter un DIXIÈME
// routeur.
// ⚠️ CETTE PHRASE A DIT « ONZIÈME » JUSQU'À LA REVUE FINALE, ici et dans
// `serveur.ts` : le compte était faux des deux côtés, et personne ne l'avait
// relancé. Le voici, avec sa commande — la seule chose qui fasse foi :
//   grep -cE '^    (if \(await servir|return servir)' plateforme/src/http/chaine.ts
//     -> 10
//
// `CLAUDE.md` prescrit l'extraction en tâche DÉDIÉE, AVANT celle qui ajoute —
// « extraire, jamais comprimer » — parce que la marge regagnée par une
// extraction se reperd si on la traite comme acquise (payé six fois).
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
import { servirPage, type DependancesPage } from './page/routes-page';

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
    DependancesSante &
    DependancesPage;

/// Essaie les routeurs dans l'ordre, et rend `false` si aucun n'a servi.
///
/// 🔴 L'ORDRE EST SIGNIFIANT, ET DEPUIS LE 22 AOÛT 2026 IL EST CONTRAIGNANT.
/// La phrase d'origine — « non contraignant ici : les QUATRE jeux de chemins
/// sont DISJOINTS » — était fausse deux fois, et se contredisait avec une
/// autre du même fichier cinquante lignes plus bas (« LE SERVANT EST CHAÎNÉ EN
/// DERNIER, ET C'EST LA GARANTIE, PAS UNE COMMODITÉ »). Le compte, relancé :
///   grep -cE '^    (if \(await servir|return servir)' plateforme/src/http/chaine.ts
///     -> 10
/// **DIX routeurs, dont NEUF aux jeux de chemins DISJOINTS** — chacun compare
/// exactement, ou découpe par SEGMENTS et compare leur NOMBRE, jamais par
/// `startsWith`. **LE DIXIÈME, LE SERVANT DE PAGE, N'EST PAS DISJOINT DES
/// AUTRES : il résout N'IMPORTE QUEL chemin**, son repli SPA repliant tout
/// chemin sans extension sur `index.html`. Sa position n'est donc pas une
/// commodité mais une garantie — voir sa ligne, en fin de fonction.
///
/// 🔴 CE QUE CE DIXIÈME ROUTEUR CHANGE POUR TOUS LES AUTRES, ET QU'AUCUN
/// D'EUX N'AVAIT ÉCRIT : quand `PLATEFORME_PAGE` est armée, un `false` rendu
/// par un routeur NE TOMBE PLUS NÉCESSAIREMENT SUR LE 404 GÉNÉRIQUE. Sur un
/// `GET`/`HEAD`, le servant peut répondre `200 text/html` à sa place ; hors
/// `GET`/`HEAD` il se retire, et le 404 générique reprend la main comme
/// avant. Plusieurs modules de routes portent la phrase « le 404 générique
/// répond alors seul » : elle date d'avant ce servant, et chacun renvoie
/// désormais ici.
///
/// 🔴 LE ROUTEUR DES APPLICATIONS EST CHAÎNÉ AVANT LE 404, ET C'EST LA
/// SEULE LIGNE QUI LE FAIT VIVRE. Sans elle, ses deux routes tomberaient dans
/// le repli ci-dessus — c'est-à-dire la panne la plus discrète possible : le
/// service répond, écoute, et sert les NEUF autres. `serveur.test.ts` la
/// tient par un test dédié, comme il tient déjà le canal `/agent`.
///
/// ⚠️ LE CORPS DU 404 N'EST PAS TOUCHÉ : « rien ne le testait avant P2, et
/// le changer serait un effet de bord non déclaré ». Il vit désormais dans
/// `./introuvable.ts`, d'où les deux gardes de mode le rendent elles-mêmes.
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
    // VIVRE. Sans elles, leurs SEPT routes tomberaient dans le repli : la
    // panne la plus discrète qui soit, puisque le service répond, écoute, et
    // sert correctement les HUIT autres routeurs.
    // ⚠️ « LES SIX AUTRES » ÉTAIT LE MOT, ET IL A VIEILLI EN SILENCE — deux
    // routeurs ont été chaînés depuis. Les deux comptes de cette phrase,
    // remesurés le 22 août 2026 : SEPT routes (les quatre énumérées en tête de
    // `routes-televersement.ts`, les trois en tête de `routes-installation.ts`),
    // et 10 - 2 = HUIT autres routeurs.
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
    // ⚠️ `/sante` EST CHAÎNÉE TARD, et l'ordre n'est pas indifférent ici : la
    // placer en tête ferait courir sa comparaison de chemin avant celles des
    // routes gardées. Son jeu de chemins reste DISJOINT de ceux des huit
    // routeurs qui la précèdent, donc aucun ne peut voler le chemin d'un
    // autre ; l'ordre est une ceinture, pas une garantie.
    //
    // 🔴 TROIS AFFIRMATIONS DE CETTE PHRASE SONT MORTES DANS CE LOT MÊME, ET
    // AUCUNE REVUE PAR TÂCHE NE POUVAIT LE VOIR — l'extraction a déplacé ces
    // lignes VERBATIM, correctement, et la tâche SUIVANTE a chaîné le servant
    // APRÈS. Chaque moitié était juste.
    //   ① « CHAÎNÉE EN DERNIER » : elle ne l'est plus, le servant de page la
    //      suit. C'est `servirPage` qui est en dernier, une ligne plus bas.
    //   ② « LA SEULE ROUTE NON AUTHENTIFIÉE » : le servant en est une SECONDE
    //      — il ne consulte aucun jeton. La propriété qui reste vraie de
    //      `/sante` est d'être la seule route non authentifiée qui TOUCHE LA
    //      BASE, ce qui est exactement ce que son cache existe pour borner
    //      (voir `routes-sante.ts`, où la même phrase est corrigée).
    //   ③ « LES SIX JEUX » : ils sont DIX, dont neuf disjoints — voir le
    //      compte et sa commande en tête de cette fonction.
    if (await servirSante(requete, reponse, deps)) return true;
    // 🔴 LE SERVANT DE PAGE EST CHAÎNÉ EN DERNIER, ET C'EST LA GARANTIE, PAS
    // UNE COMMODITÉ. C'est le SEUL des dix dont le jeu de chemins ne soit pas
    // disjoint de celui des autres : il résout n'importe quel chemin. Chaîné
    // en tête, un fichier nommé `sante` ou `vm` déposé dans la racine volerait
    // le chemin d'un routeur d'API, et le service répondrait 200 avec un corps
    // plausible. Chaîné ici, un routeur d'API a déjà rendu `true` : il ne peut
    // pas être supplanté. `routes-page.test.ts` tient cette ligne par un test
    // dédié.
    //
    // ⚠️ CETTE POSITION NE SUFFIT PAS À TOUT, ET LE LOT L'A PAYÉ : un routeur
    // qui rend `false` ALORS QUE LE CHEMIN EST LE SIEN se fait quand même
    // supplanter — c'est ce qui a tué le `404` porteur de mode des deux gardes
    // d'authentification. Le remède n'est pas ici, il est chez elles : elles
    // répondent désormais leur `404` elles-mêmes (`./introuvable.ts`).
    return servirPage(requete, reponse, deps);
}
