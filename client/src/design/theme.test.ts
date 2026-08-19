import { describe, expect, it } from 'vitest';
import {
    CLE_THEME,
    appliquer,
    choisir,
    surStockageModifie,
    themeStocke,
} from './theme';
import amorce from './amorce-theme.js?raw';

/** Doublure de `localStorage` — voir l'en-tête de `theme.ts`. */
function coffreFactice(initial: Record<string, string> = {}) {
    const contenu = new Map(Object.entries(initial));
    return {
        contenu,
        ecritures: 0,
        getItem(cle: string) {
            return contenu.get(cle) ?? null;
        },
        setItem(this: { ecritures: number }, cle: string, valeur: string) {
            contenu.set(cle, valeur);
            this.ecritures += 1;
        },
    };
}

/** Doublure de `document.documentElement`. */
function racineFactice() {
    return {
        attributs: new Map<string, string>(),
        setAttribute(this: { attributs: Map<string, string> }, nom: string, valeur: string) {
            this.attributs.set(nom, valeur);
        },
        removeAttribute(this: { attributs: Map<string, string> }, nom: string) {
            this.attributs.delete(nom);
        },
    };
}

describe('choisir — la fenêtre ÉCRIVANTE', () => {
    // 🔴 Les deux tests suivants sont DEUX `it()` séparés, jamais deux `expect`
    // du même test. `expect` interrompt le test à la première assertion : un
    // seul `it()` qui vérifierait l'écriture PUIS l'attribut s'arrêterait à
    // l'écriture, et l'oubli de l'application locale — le défaut naturel de ce
    // mécanisme, puisque `storage` ne se déclenche pas chez l'écrivain — se
    // cacherait derrière elle. C'est la leçon que le sous-bloc P2 a payée.

    it('écrit le thème dans le coffre, sous la clé préfixée', () => {
        const coffre = coffreFactice();
        choisir(coffre, racineFactice(), 'clair');
        expect(coffre.getItem(CLE_THEME)).toBe('clair');
    });

    it("applique le thème LOCALEMENT, parce que `storage` ne revient pas chez l'écrivain", () => {
        const racine = racineFactice();
        choisir(coffreFactice(), racine, 'clair');
        expect(racine.attributs.get('data-theme')).toBe('clair');
    });
});

describe('surStockageModifie — une fenêtre VOISINE', () => {
    it("pose `data-theme` à partir de l'événement d'une autre fenêtre", () => {
        const racine = racineFactice();
        surStockageModifie(racine, CLE_THEME, 'sombre');
        expect(racine.attributs.get('data-theme')).toBe('sombre');
    });

    it("retire l'attribut quand la voisine est repassée à « systeme »", () => {
        // ⚠️ CE TEST REMPLACE celui que le plan prescrivait — « `surStockageModifie`
        // n'écrit rien dans le coffre ». Ce dernier est INSATISFIABLE COMME
        // TEST : la signature ne reçoit AUCUN `Coffre`, donc la fonction n'a
        // rien à écrire et l'assertion ne peut pas tomber. La propriété est
        // garantie par le TYPE, ce qui est plus fort qu'un test — elle est
        // déclarée dans l'en-tête de `theme.ts` plutôt que mise en scène ici.
        // Le cas ci-dessous, lui, est réel et peut échouer : une voisine
        // remise sur « systeme » efface la clé, et l'attribut doit disparaître.
        const racine = racineFactice();
        racine.attributs.set('data-theme', 'clair');
        surStockageModifie(racine, CLE_THEME, null);
        expect(racine.attributs.has('data-theme')).toBe(false);
    });

    it("ignore un événement portant une AUTRE clé — un jeton d'accès", () => {
        // La clé employée ici n'est pas inventée : `client/src/connexion.ts:57`
        // écrit RÉELLEMENT `guac.jeton.acces` au moment de la connexion, donc
        // toute fenêtre voisine reçoit cet événement. Un gestionnaire qui ne
        // filtrerait pas la clé poserait `data-theme` à partir d'un JWT.
        const racine = racineFactice();
        racine.attributs.set('data-theme', 'clair');
        surStockageModifie(racine, 'guac.jeton.acces', 'eyJhbGciOiJIUzI1NiJ9.charge.signature');
        expect(racine.attributs.get('data-theme')).toBe('clair');
    });
});

describe('themeStocke — robustesse', () => {
    it("retombe sur « systeme » sur une valeur inconnue", () => {
        expect(themeStocke(coffreFactice({ [CLE_THEME]: 'bleu' }))).toBe('systeme');
    });

    it("retombe sur « systeme » quand la clé est absente", () => {
        expect(themeStocke(coffreFactice())).toBe('systeme');
    });
});

describe('appliquer', () => {
    it("RETIRE `data-theme` pour « systeme », au lieu de poser l'attribut", () => {
        // Son ABSENCE signifie « systeme ». Un `data-theme="systeme"` passerait
        // le sélecteur `:root:not([data-theme="sombre"])` de la requête média,
        // mais pas `:root[data-theme="clair"]` — et rien ne casserait
        // VISIBLEMENT. Un attribut inventé qui ne casse rien est exactement le
        // genre d'écart qui survit dix sous-blocs.
        const racine = racineFactice();
        racine.attributs.set('data-theme', 'sombre');
        appliquer(racine, 'systeme');
        expect(racine.attributs.has('data-theme')).toBe(false);
    });
});

/**
 * LE GARDE ENTRE `theme.ts` ET `amorce-theme.js` — et pourquoi il existe.
 *
 * `client/src/design/amorce-theme.js` s'exécute AVANT tout module, en ligne
 * dans le `<head>` : il ne peut donc RIEN importer, et il redit la chaîne
 * `guac.theme` en littéral. C'est le seul recouvrement entre les deux
 * fichiers, et il est assumé — mais un recouvrement assumé qui n'est gardé par
 * rien devient une divergence muette : l'amorce lirait une clé que plus
 * personne n'écrit, et le seul symptôme serait un éclair de mauvais thème que
 * personne ne regarde en revue.
 *
 * ⚠️ Ce test est le patron exact que P2 a employé entre `client/src/jeton.ts`
 * et `client/recette/jeton-recette.mjs`, et le garde de P2 A ÉTÉ VU LEVER.
 * Celui-ci l'a été aussi : voir le document de résultats de S1, tâche 10.
 *
 * ⚠️ Il compare la CHAÎNE, jamais le comportement. Que l'amorce pose bien
 * `data-theme` avant la première peinture n'est prouvé par aucun test — c'est
 * le build qui prouve l'INJECTION (tâche 10, Step 3) et rien ne prouve
 * l'absence d'éclair.
 */
describe('l’amorce anti-FOUC et `theme.ts` ne peuvent pas diverger sur la clé', () => {
    it('`amorce-theme.js` LIT littéralement la valeur de `CLE_THEME`', () => {
        // 🔴 L'assertion porte sur l'APPEL, pas sur la présence de la chaîne
        // quelque part dans le fichier. Une première rédaction disait
        // `toContain("'guac.theme'")` : elle était satisfaite par le
        // commentaire d'en-tête, si bien qu'un amorce remplacé par
        // `var t = null;` restait VERT sur les deux assertions. Vu, mesuré,
        // et c'est pourquoi la clé ne s'écrit plus dans ce commentaire-là.
        expect(amorce).toContain(`getItem('${CLE_THEME}')`);
    });

    it('`amorce-theme.js` ne porte AUCUNE autre clé `guac.*`', () => {
        // Sans cette seconde assertion, ajouter une clé à l'amorce sans retirer
        // l'ancienne passerait : `toContain` ne dit rien de ce qu'il y a autour.
        const cles = [...amorce.matchAll(/'(guac\.[a-z.]+)'/g)].map((m) => m[1]);
        expect([...new Set(cles)]).toEqual([CLE_THEME]);
    });
});
