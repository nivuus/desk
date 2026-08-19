/**
 * Les trois états du thème — PUR : dépendances INJECTÉES, aucun DOM.
 *
 * ⚠️ NI `window`, NI `localStorage`, NI `document` NE SONT DISPONIBLES ICI
 * sous Vitest : `client/` n'a aucun `vitest.config.*` et aucun `jsdom`
 * (`ls client/node_modules/@types/` rend `estree` seul). `Coffre` et `Racine`
 * sont donc des PARAMÈTRES, sur le patron exact de `client/src/jeton.ts:4-10`.
 * Un `const racine = document.documentElement` en tête de module suffirait à
 * rendre ce fichier impossible à charger sous Node, donc impossible à tester.
 *
 * ⚠️ TypeScript EFFAÇABLE (aucun `enum`, aucun `namespace`) : ce module est
 * dans le même répertoire que ceux qu'un `.mjs` importe, et la contrainte s'y
 * applique par cohérence. Voir l'en-tête de `tokens.ts` pour la mesure.
 *
 * ────────────────────────────────────────────────────────────────────────────
 * CE QUE CES FONCTIONS NE PROUVENT PAS, mot pour mot d'après la spec §7.5 :
 * « il n'établit pas que le navigateur déclenche bien `storage` entre deux
 * fenêtres réelles. Il éprouve NOTRE gestionnaire, pas la plateforme. » La
 * confirmation à deux fenêtres réelles est prévue HORS CRITÈRE, en
 * corroboration — même statut que les confirmations sur VM réelle du
 * sous-projet ⑤.
 *
 * ⚠️ « `surStockageModifie` n'écrit rien dans le coffre » est garanti par la
 * SIGNATURE, pas par un test : la fonction ne reçoit aucun `Coffre`, donc elle
 * n'a rien à écrire. Un test de cette propriété ne pourrait jamais tomber, et
 * ce dépôt ne garde pas de contrôle incapable d'échouer. C'est ce qui ferme la
 * boucle entre fenêtres : une voisine qui réécrirait en réagissant
 * déclencherait un `storage` chez ses propres voisines, sans terme.
 */

export type Theme = 'systeme' | 'clair' | 'sombre';

/**
 * ⚠️ La clé est PRÉFIXÉE, là où la spec §4.2 écrivait `theme` nu. Le dépôt a
 * déjà la convention : `client/src/jeton.ts:37-38` déclare
 * `guac.jeton.acces` et `guac.jeton.rafraichissement`. Une clé nue sur la même
 * origine que le hub, la page-shell et N fenêtres de session est une collision
 * qui attend ; le préfixe ne coûte rien.
 */
export const CLE_THEME = 'guac.theme';

const ATTRIBUT = 'data-theme';

export interface Coffre {
    getItem(cle: string): string | null;
    setItem(cle: string, valeur: string): void;
}

export interface Racine {
    setAttribute(nom: string, valeur: string): void;
    removeAttribute(nom: string): void;
}

function estTheme(valeur: string | null): valeur is Theme {
    return valeur === 'systeme' || valeur === 'clair' || valeur === 'sombre';
}

/** Lit le coffre. Toute valeur inconnue — `null` compris — rend `'systeme'`. */
export function themeStocke(coffre: Coffre): Theme {
    const valeur = coffre.getItem(CLE_THEME);
    return estTheme(valeur) ? valeur : 'systeme';
}

/**
 * Pose `data-theme`, ou le RETIRE pour `'systeme'` : son ABSENCE signifie
 * « systeme ». Voir le test correspondant pour ce qu'un `data-theme="systeme"`
 * casserait — et surtout pour ce qu'il ne casserait pas, visiblement.
 */
export function appliquer(racine: Racine, theme: Theme): void {
    if (theme === 'systeme') racine.removeAttribute(ATTRIBUT);
    else racine.setAttribute(ATTRIBUT, theme);
}

/**
 * Écrit ET applique localement.
 *
 * 🔴 LES DEUX MOITIÉS SONT NÉCESSAIRES, et c'est le piège que la spec §4.2
 * nomme : l'événement `storage` NE SE DÉCLENCHE PAS dans le document qui a
 * écrit. La fenêtre qui change le thème est précisément la seule que
 * l'utilisateur regarde ; si elle se contentait d'écrire, elle serait la seule
 * à ne pas changer d'apparence.
 */
export function choisir(coffre: Coffre, racine: Racine, theme: Theme): void {
    coffre.setItem(CLE_THEME, theme);
    appliquer(racine, theme);
}

/**
 * Réagit à un événement `storage` venu d'une AUTRE fenêtre. N'écrit RIEN —
 * voir l'en-tête.
 *
 * ⚠️ Le filtre de clé n'est pas une précaution théorique :
 * `client/src/connexion.ts:57` écrit réellement `guac.jeton.acces`, donc toute
 * fenêtre voisine reçoit cet événement-là. Sans le filtre, `data-theme`
 * vaudrait un JWT.
 */
export function surStockageModifie(racine: Racine, cle: string | null, valeur: string | null): void {
    if (cle !== CLE_THEME) return;
    appliquer(racine, estTheme(valeur) ? valeur : 'systeme');
}
