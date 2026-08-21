/**
 * Le branchement de la couleur d'accent sur le DOM — sous-bloc **A1**.
 *
 * **La seule ligne de DOM du mécanisme**, et rien d'autre : la décision — juger
 * la lisibilité, refuser, rendre le thème — vit dans `accent.ts`, **pur** et
 * testé. Le patron est `presse-papier-dom.ts` (P1), lui-même adossé à
 * `presse-papier.ts`.
 *
 * **Écrit AVANT de toucher `main.ts`**, et pour la raison que
 * `presse-papier-dom.ts` documente : `main.ts` était à **466 lignes au moment
 * d'écrire ce module** — il en fait 483 depuis que les trois lignes de câblage
 * y sont — pour un plafond
 * de projet à 500, il **n'a AUCUN test**, et `client/` n'a **ni jsdom ni
 * happy-dom**. Ce module-ci en a une, parce qu'il ne touche ni `document` ni
 * `window` directement mais reçoit `lireToken` et `poserToken` par injection —
 * le patron d'`attachFullscreenAuDOM`, d'`armerLeSon` et de `theme.ts`.
 *
 * 🔴 **A1 POSE LE PREMIER `setProperty` DU DÉPÔT.** Relevé avant d'écrire :
 * `grep -rn 'setProperty' client/src/` rendait **zéro**. La règle que ce
 * précédent pose, et qu'il faut donc écrire : **une écriture de token à
 * l'exécution ne se fait que sur `document.documentElement`, jamais sur un
 * élément.** Un token posé sur `document.body` serait invisible à
 * `getComputedStyle(document.documentElement)`, et c'est exactement la rouge du
 * critère ② de la recette.
 *
 * 🔴 **ET LE PREMIER APPELANT DE PRODUIT DE `getComputedStyle`.** Le seul autre
 * est `design/galerie.ts`, qui déclare lui-même n'être **pas** du produit —
 * aucune surface ne l'importe, il n'a aucun test. La règle du §4.1 de la spec ⑥
 * — « à un changement de thème, jamais par image » — est respectée et même
 * dépassée : on lit **à l'arrivée d'un message**, soit au plus une fois toutes
 * les `PERIODE_ACCENT` (5 s), et **seulement quand l'icône a changé**, le
 * capteur n'annonçant qu'au changement.
 *
 * ⚠️ **`--accent-fenetre` n'est déclaré NULLE PART dans `tokens.css`, et c'est
 * une décision MESURÉE** (D-A1-2, sonde H1 du 21 août 2026) : le déclarer y
 * rend §7.6 **ROUGE** (« NOUVEL ORPHELIN — déclaré et appelé par personne »),
 * parce que le périmètre « employé » de ce contrôle est le `.css` seulement et
 * que sa détection ne reconnaît que `var(--…)` — un `setProperty` TypeScript y
 * est invisible **deux fois**. Et la voie « ligne d'attente » que la spec
 * déclare acceptable n'est tenable **que si A1 refuse de se déclarer clos**,
 * ce qui est pire. Les cinq cellules de la sonde sont versées dans
 * `journaux-accent-a1/01-sonde-h1.log`.
 *
 * ⚠️ **Conséquence à écrire, puisque personne ne la verra ailleurs** : avant le
 * premier message, `var(--accent-fenetre)` est **indéfini**. Aucune feuille ne
 * le référence aujourd'hui — relevé, `grep -rn 'accent-fenetre' client/src/` ne
 * rend que ce fichier —, donc rien n'en souffre ; mais **toute référence future
 * doit porter un repli (`var(--accent-fenetre, var(--accent))`) ou déclarer le
 * token**, et §7.6 le dira par sa PREMIÈRE inclusion (« aucun `var(--…)` non
 * déclaré »). **La déclaration part avec son appelant, et les deux
 * appartiennent à G5**, qui posera le manifeste PWA.
 */

import { conformer } from './accent';

/** Le token que ce module écrit, et le seul. */
export const TOKEN_ACCENT = '--accent-fenetre';

/**
 * Les trois fonds du thème courant, plus l'accent du thème.
 *
 * ⚠️ **LES TROIS, et pas le seul `--fond-0`** : la spec D10 point 2 dit « les
 * trois fonds du thème courant », et l'accent d'une fenêtre peut se poser sur
 * n'importe lequel selon la surface.
 */
const FONDS = ['--fond-0', '--fond-1', '--fond-2'] as const;
const ACCENT_DU_THEME = '--accent';

/** Ce dont ce module a besoin du document, et rien d'autre. */
export interface AccesTokens {
    /** Lit la valeur COURANTE d'un token sur la racine. */
    lireToken(nom: string): string;
    /** Écrit un token sur la racine — **jamais sur un élément**. */
    poserToken(nom: string, valeur: string): void;
}

/**
 * Reçoit une couleur annoncée par l'agent, la conforme, et pose le résultat.
 *
 * 🔴 **Les fonds sont relus À CHAQUE MESSAGE, jamais mémorisés au montage.**
 * Sans cela, une bascule de thème laisserait l'accent jugé contre l'ANCIEN
 * thème : les fonds du clair et du sombre n'ont rien à voir, et une couleur
 * lisible sur `#0b0d10` ne l'est pas forcément sur `#ffffff`. C'est un test.
 *
 * ⚠️ **Sur un REFUS, on pose `--accent` — on ne laisse pas le token en place.**
 * Ne rien poser lui laisserait sa valeur PRÉCÉDENTE, c'est-à-dire la teinte
 * d'une icône qui n'est plus celle de cette fenêtre : un état périmé, plus
 * trompeur qu'un repli visible. C'est aussi un test.
 */
export function attacherAccentAuDOM(acces: AccesTokens): { recevoir(couleur: string): void } {
    return {
        recevoir(couleur: string): void {
            const fonds = FONDS.map((nom) => acces.lireToken(nom));
            const accentDuTheme = acces.lireToken(ACCENT_DU_THEME);
            acces.poserToken(TOKEN_ACCENT, conformer(couleur, fonds, accentDuTheme));
        },
    };
}
