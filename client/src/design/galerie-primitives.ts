/**
 * L'ENTRÉE DE `client/primitives.html` — la galerie des primitives.
 *
 * ⚠️ CE MODULE N'EST PAS DU PRODUIT, et il n'a pas de test : même statut que
 * `galerie.ts` et `selecteur-theme.ts`, pour la même raison.
 *
 * 🔴 IL NE LIT AUCUN TOKEN, et c'est ce qui le distingue de `galerie.ts` :
 * cette page-ci ne montre pas des VALEURS, elle montre des COMPOSANTS. Il n'y
 * a donc aucun appel à `getComputedStyle`, et rien à redessiner quand le thème
 * change — le rappel `apres` est délibérément vide, les primitives suivant le
 * thème par leurs seuls `var(--…)`.
 */
import { installerSelecteurDeTheme } from './selecteur-theme';

const hote = document.getElementById('themes');
if (!hote) throw new Error('la galerie des primitives attend un élément #themes');

installerSelecteurDeTheme(hote, document.documentElement, localStorage, () => {});
