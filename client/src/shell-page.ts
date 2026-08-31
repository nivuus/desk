// L'ANCIENNE PAGE-SHELL — devenue une simple redirection le 31 août 2026.
//
// 🔴 CE FICHIER PORTAIT 500 LIGNES ET LA MOITIÉ DU PRODUIT. Son contenu vit
// désormais dans `bureau/porteur-dom.ts`, `bureau/fenetres-dom.ts` et
// `bureau/fichiers-dom.ts`, employés par le hub — la SEULE surface depuis
// cette date. La règle métier `shell.ts`, elle, n'a pas bougé d'une ligne :
// elle était déjà pure et testée, et elle est réutilisée telle quelle.
//
// ⚠️ NE PAS SUPPRIMER CE FICHIER NI SA PAGE. Voir `bureau/redirection.ts`
// pour la raison — elle tient aux PWA déjà installées, pas à la prudence.

import { cibleDeRedirection } from './bureau/redirection';

// `replace` et non `href` : un retour arrière ramènerait sur cette page qui
// redirigerait de nouveau, et l'utilisateur serait piégé dans l'historique.
window.location.replace(cibleDeRedirection(window.location.search));
