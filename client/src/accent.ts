/**
 * La conformation de la couleur d'accent reçue de l'agent — sous-bloc **A1**.
 *
 * **PUR, sans DOM.** Il ne prend que des chaînes et rend une chaîne. La lecture
 * de `getComputedStyle` et l'écriture de `setProperty` vivent dans
 * `accent-dom.ts` — c'est le patron que P1 a posé pour le presse-papier
 * (`presse-papier.ts` pur / `presse-papier-dom.ts` au DOM).
 *
 * ⚠️ **La spec §7.1 range ce module comme « PUR, sans DOM » ET sa décision D10
 * point 2 lui fait lire les fonds par `getComputedStyle`. Les deux sont
 * inconciliables dans un seul module** (divergence E8 du plan) : les fonds sont
 * donc INJECTÉS, et c'est la testabilité qui tranche — « un plan qui dicte du
 * code intestable cède devant sa propre contrainte » (E15 de P3).
 *
 * 🔴 **C'EST LE SEUL REMPART DU PRODUIT, et ce n'est pas un constat empirique
 * mais une propriété STRUCTURELLE des contrôles du design system (E10) :**
 *
 * - **§7.2** est **syntaxique**, et il le déclare lui-même
 *   (`client/outils/couleurs-litterales.mjs` : « un `el.style.background =
 *   'red'` passerait donc ») ;
 * - **§7.1** parse `tokens.css` et compare une liste de paires **déclarée à la
 *   main** (`design/contraste.ts`), jamais un produit cartésien, et
 *   `client/outils/contraste.mjs` exclut **nommément** les couleurs composées à
 *   l'exécution.
 *
 * **Une couleur reçue du serveur est donc hors de tout contrôle de contraste
 * automatique de ce dépôt.** Son seul juge est `accent.test.ts`, et c'est
 * pourquoi sa rouge devait être VUE.
 */

import { rapportDeContraste } from './design/contraste';

/**
 * Le seuil de lisibilité d'un composant non textuel — WCAG 1.4.11.
 *
 * 🔴 **C'EST UNE DUPLICATION, ET ELLE EST DÉCLARÉE COMME UNE DETTE.** Le plan
 * prescrivait d'importer `SEUIL_COMPOSANT` de `design/contraste.ts` — « un
 * contrôle qui a sa propre copie des valeurs valide sa copie » (spec ⑥ §7.1) —
 * **et il déclarait n'avoir pas vérifié qu'il soit exporté**. Il ne l'est
 * pas : `const SEUIL_COMPOSANT = 3;` y est une constante **de module**, non
 * exportée. La conduite à tenir était écrite d'avance : employer le littéral
 * et **nommer la duplication**, jamais modifier `contraste.ts` pour l'exporter
 * — `client/src/design/` appartient au sous-projet ⑥, clos (D-A1-14).
 *
 * ⚠️ **Le jour où ces deux 3 divergeront, RIEN ne le dira.** Le remède, s'il
 * devient nécessaire, appartient à ⑥ : exporter la constante et l'importer ici.
 */
const SEUIL_COMPOSANT = 3;

/** `#rrggbb`, six chiffres hexadécimaux, et rien d'autre. */
const FORME = /^#[0-9a-f]{6}$/;

/**
 * Rend la couleur reçue si elle est **conforme et lisible**, `accentDuTheme`
 * sinon. Elle **ne lève jamais**, et elle **ne corrige jamais**.
 *
 * Trois étapes, et chacune est un refus possible :
 *
 * 1. **la forme** — `#rrggbb` sur la valeur *trimée et minusculée*. Toute autre
 *    forme est refusée **AVANT** d'atteindre `rapportDeContraste`.
 *    🔴 **C'est ce qui garantit qu'elle ne lève jamais** :
 *    `luminanceRelative` **LÈVE** sur tout ce qui n'est pas `#rgb`, `#rgba`,
 *    `#rrggbb` ou `#rrggbbaa` (divergence E9 — la spec ne le dit nulle part, et
 *    une exception dans un gestionnaire de message de canal de données est le
 *    genre de défaut qui tue une session sans rien dire).
 *    ⚠️ **Un `try/catch` autour de l'appel serait un contrôle PLUS FAIBLE** : il
 *    attraperait aussi une régression de `contraste.ts` en la déguisant en
 *    refus ordinaire. Il n'y en a donc aucun, délibérément.
 * 2. **la lisibilité** — un rapport d'au moins `SEUIL_COMPOSANT` contre
 *    **CHACUN** des fonds passés. ⚠️ **Les trois et non le seul `--fond-0`** :
 *    la spec D10 point 2 dit « les trois fonds du thème courant », et l'accent
 *    d'une fenêtre peut se poser sur n'importe lequel selon la surface. C'est
 *    le choix le plus strict, il est **délibéré**, et il refusera davantage de
 *    couleurs — **un refus est un état SAIN de ce mécanisme**, pas une panne.
 * 3. **aucune correction.** Ni éclaircissement, ni assombrissement, ni
 *    `color-mix` : « éclaircir ou assombrir la couleur d'une application
 *    produirait une teinte que personne n'a choisie » (D10 point 3).
 *    **Refuser EST le comportement**, pas un repli d'échec.
 *
 * ⚠️ **`accentDuTheme` est LU par l'appelant, jamais écrit ici.** Une couleur
 * en dur dans ce fichier ferait rougir §7.2 — mesuré par la cellule E de la
 * sonde H1 : `const REPLI = '#7aa2f7'` y rend
 * « couleurs littérales : 1 », sortie 1.
 */
export function conformer(
    recue: string,
    fonds: readonly string[],
    accentDuTheme: string,
): string {
    const candidate = recue.trim().toLowerCase();
    if (!FORME.test(candidate)) return accentDuTheme;
    if (fonds.length === 0) return accentDuTheme;
    for (const fond of fonds) {
        if (!FORME.test(fond.trim().toLowerCase())) return accentDuTheme;
        if (rapportDeContraste(candidate, fond.trim().toLowerCase()) < SEUIL_COMPOSANT) {
            return accentDuTheme;
        }
    }
    return candidate;
}
