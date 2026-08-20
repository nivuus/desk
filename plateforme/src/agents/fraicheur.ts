// La fraîcheur d'un agent : décider `prete` ou `injoignable` à partir de la
// seule colonne `vu_a`, que le battement du canal `/agent` avance.
//
// 🔴 CE MODULE EST PUR. Aucune base, aucun `Date.now()` : l'instant est un
// PARAMÈTRE. C'est la règle du dépôt (`depot/session.ts`, `identite/jeton.ts`,
// `signaling/ice.ts`), et ici elle porte davantage qu'ailleurs — le critère ④
// de la spec exige que le test VOIE la transition, ce qu'une horloge lue à
// l'intérieur rendrait impossible : il n'y aurait qu'un seul instant
// observable, et le seuil ne serait jamais franchi dans une exécution de test.
//
// ✅ IL A SON APPELANT DE PRODUCTION DEPUIS LE SOUS-BLOC P4 (20 août 2026), ET
// LE PARAGRAPHE CI-DESSOUS EST DEVENU DE L'HISTOIRE. `etatDe` est appelée par
// `orchestration/inventaire-statique.ts::etat`, que `GET /vm` et
// `POST /session` lisent toutes deux ; c'est le legs n°2 de P3, fermé. La
// phrase « ses deux lecteurs à ce jour sont son propre test et la recette du
// critère ④ » N'EST DONC PLUS VRAIE, et la déclaration d'orphelinat non plus.
// Ce qui reste vrai, et qui est la raison d'être de ce module : il est PUR,
// son instant est un PARAMÈTRE, et c'est l'orchestrateur qui lui donne son
// horloge — ce qui rend la transition du critère ④ observable.
//
// 🔴 IL N'A AUCUN APPELANT DE PRODUCTION DANS P3, ET C'EST DÉCLARÉ PLUTÔT QUE
// DISSIMULÉ. Le plan de P3 ne lui en prescrit aucun : ce qu'il décide — l'état
// d'une VM — n'est lu par personne tant qu'aucune vue ne liste les VMs, ce qui
// est le sujet de P4. Ses deux lecteurs à ce jour sont son propre test et la
// recette du critère ④ (« un agent muet est vu comme tel »). Lui inventer un
// appelant ici reviendrait à décider à la place de P4 où l'état s'affiche ;
// l'écrire pur et testé le rend disponible sans rien préempter. ⚠️ Le dépôt
// n'a pas de doctrine sur le code orphelin — deux fonctions orphelinées dans
// la même branche y ont été traitées différemment au sous-bloc D10 —, donc ce
// paragraphe vaut déclaration, pas justification par précédent.
//
// ⚠️ CE MODULE NE LIT PAS `agent_enrole` NON PLUS, et `depot/agent.ts` l'avait
// annoncé : « IL NE SAIT RIEN DE LA FRAÎCHEUR : il rend `vu_a` tel qu'il est,
// `null` compris. Décider `prete` / `injoignable` est le travail d'un module
// PUR, avec son horloge en paramètre. » C'est ce module.

/// Au-delà de ce silence, une VM est tenue pour injoignable.
///
/// ⚠️ ELLE N'EST PAS CALIBRÉE. Aucune mesure ne l'a jugée, aucun jugement
/// d'usage n'a été porté sur elle : elle rejoint la liste déjà longue des
/// constantes non calibrées de ce dépôt — `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`,
/// `DUREE_JETON_ACCES_MS`, `DUREE_SECONDES`, `OCTETS_PREFIXE`. Elle vaut une
/// minute et demie parce que c'est plusieurs fois la période d'un battement,
/// pas parce qu'un banc l'a établie ; le jour où la période du battement sera
/// choisie ailleurs, les deux se recalibreront ENSEMBLE.
export const SEUIL_INJOIGNABLE_MS = 90_000;

/// Ce qu'on sait d'une VM enrôlée. Deux états seulement : ni « peut-être »,
/// ni « inconnue ». Une VM qui n'a jamais battu est `injoignable`, ce qui est
/// vrai et suffisant pour tout appelant — il n'y a rien à faire de plus d'une
/// VM jamais vue que d'une VM qui s'est tue.
export type EtatAgent = 'prete' | 'injoignable';

/// Décide l'état d'une VM dont on connaît le dernier battement.
///
/// 🔴 `vuA === null` REND `injoignable`, jamais `prete` : la colonne naît
/// `null` à l'enrôlement (`depot/agent.ts`), donc une VM enrôlée mais jamais
/// démarrée serait annoncée prête à quiconque la demanderait, et l'erreur ne
/// se verrait qu'au moment d'ouvrir une session sur une VM éteinte.
///
/// ⚠️ LA BORNE EST FRANCHE ET DU CÔTÉ DE `prete` : un silence de très
/// exactement `SEUIL_INJOIGNABLE_MS` est encore toléré, un silence d'une
/// milliseconde de plus ne l'est plus. Elle est écrite ainsi pour que le test
/// puisse l'assiéger des deux côtés — même forme que l'expiration de
/// `identite/jeton.ts` (`maintenant >= exp`).
///
/// ⚠️ CETTE FONCTION A SURVÉCU PAR ACCIDENT À UN DÉFAUT DE TYPE, et le noter
/// vaut plus que la correction elle-même. Jusqu'à la recette de P3, `pg`
/// rendait `agent_enrole.vu_a` en **chaîne** là où `node:sqlite` rendait un
/// `number` : `vuA` recevait donc une `string` en production. Rien ne
/// rougissait — `maintenant - vuA` convertit son opérande, et la comparaison
/// qui suit porte sur deux nombres. **Mais `vuA === null` restait juste par
/// chance, et tout `+`, tout `===` ou tout `>` posé ici aurait divergé selon
/// le moteur** : `'1787136773742' + 90000` vaut une concaténation. Le défaut
/// est corrigé au pilote (`base/pilote-postgres.ts`) ; le signataire du type
/// est désormais vrai, et il ne l'était pas.
///
/// ⚠️ UN `vuA` DANS LE FUTUR REND `prete`, et ce n'est pas un cas construit :
/// la plateforme et la VM n'ont pas la même horloge, et `vu_a` est écrit par
/// la plateforme au reçu du battement. Un écart négatif est donc du silence
/// négatif, c'est-à-dire aucun silence.
export function etatDe(vuA: number | null, maintenant: number): EtatAgent {
    if (vuA === null) return 'injoignable';
    return maintenant - vuA > SEUIL_INJOIGNABLE_MS ? 'injoignable' : 'prete';
}
