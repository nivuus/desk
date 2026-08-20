// LA MITIGATION DE LA CLAUSE ③ DU CONTRÔLE §7.6, ET TOUTE SA DOCTRINE.
//
// 🔴 EXTRAIT DE `attente.mjs` PAR LA TÂCHE 6 DU SOUS-BLOC S4, ET LA DOCTRINE
// EST PARTIE AVEC SA DONNÉE — la règle que `attente.mjs` porte lui-même, et que
// `CLAUDE.md` exige nommément (« extraire, jamais compresser » ;
// `serveur/instances.rs` a emporté `TAMPON` avec le commentaire qui le
// justifie). Ce qui a forcé l'extraction est MESURÉ, et la mesure est écrite
// ici plutôt qu'ailleurs :
//
//   La tâche 6 devait faire MAIGRIR `attente.mjs` en en retirant la dernière
//   entrée. Relevé par la commande le 20 août 2026 :
//
//     227  avant la tâche
//     243  après le retrait de l'entrée ET l'écriture de ce qu'il signifie
//          (+16, alors que le seuil d'extraction conditionnel vaut 240)
//
//   L'entrée sortie pesait ~18 lignes ; ce qu'il fallait écrire pour que le
//   fichier ne se fasse pas supprimer par le sous-bloc suivant en pesait
//   davantage. C'est, EN PETIT, la leçon que ce dépôt a payée en grand — « une
//   addition de commentaire peut annuler une extraction ». Le plan de S4
//   prescrivait l'issue d'avance et sans ambiguïté : « si, contre toute
//   attente, la tâche le fait croître au-delà, ELLE EXTRAIT, ELLE NE COMPRESSE
//   PAS ». Aucune ligne de doctrine n'a été raccourcie pour atteindre un
//   nombre : raboter aurait échangé une vérité contre un compte.
//
// ⚠️ CE FICHIER N'A PAS DE TEST, et il n'en a pas besoin : il ne porte AUCUNE
// logique, seulement une donnée et sa justification. Ce qui l'emploie est
// `tokens-orphelins.mjs`, dont la clause ③ échoue quand une entrée nomme un
// sous-bloc d'ici.
// ═══════════════════════════════════════════════════════════════════════════

/**
 * LES SOUS-BLOCS CLOS DE ⑥ — aucune entrée de la liste ci-dessous n'a le droit
 * d'en nommer un.
 *
 * 🔴 SA CLAUSE DE TENUE EST CELLE DE LA LISTE ELLE-MÊME : « ce nombre est tenu
 * à jour par la tâche qui le rend faux, jamais par une tâche de ménage plus
 * tard ». Un sous-bloc s'y inscrit dans le commit qui achève son
 * implémentation.
 *
 * ⚠️ `S4` S'Y INSCRIT À SON TOUR, ET AVEC LA MÊME PROPRIÉTÉ QUE `S3` : la
 * recette et la revue transverse de S4 n'ont pas encore tourné quand cette
 * ligne s'écrit. Si l'une des deux devait ré-étiqueter une entrée vers « S4 »,
 * le contrôle rougirait — comportement voulu, un token que S4 n'a PAS consommé
 * ne doit pas réclamer S4. ⚠️ ET LA CLAUSE ③ EST DÉSORMAIS LA SEULE DES TROIS
 * QUI PUISSE ENCORE MORDRE SUR CE FICHIER, la liste étant vide : c'est
 * exactement la raison pour laquelle il reste.
 *
 * ⚠️ `S3` S'Y EST INSCRIT LUI-MÊME, ET IL FAUT DIRE CE QUE CELA VEUT DIRE : au
 * moment où cette ligne est écrite, la recette et la revue transverse de S3
 * n'ont pas encore tourné. C'est délibéré, et la propriété obtenue est la
 * bonne — si l'une des deux devait re-étiqueter une entrée vers « S3 », le
 * contrôle rougirait, ce qui est exactement le comportement voulu : un token
 * que S3 n'a PAS consommé ne doit pas réclamer S3. Aucune des deux ne
 * re-étiquette quoi que ce soit ; la tâche 7 elle-même n'en re-étiquette
 * aucune — elle change la FORME, pas le contenu.
 */
const SOUS_BLOCS_CLOS = new Set(['S1', 'S2', 'S3', 'S4']);

export { SOUS_BLOCS_CLOS };
