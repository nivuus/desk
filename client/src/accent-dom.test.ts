/// <reference types="vite/client" />
/**
 * Tests du branchement DOM de l'accent.
 *
 * ⚠️ `client/` n'a **ni jsdom ni happy-dom** : les dépendances sont injectées,
 * et c'est ce qui rend ce module éprouvable là où `main.ts` ne l'est pas.
 *
 * 🔴 **Aucune couleur en littéral** — voir l'en-tête d'`accent.test.ts` : §7.2
 * balaie les `.ts` de `client/src/`, et son exclusion ne couvre que le socle.
 * Tout est LU dans `tokens/couleurs.css` (`tokens.css` avant l'extraction de
 * la tâche 6, 25 août 2026).
 */

import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme } from './design/tokens';
import tokensCss from './design/tokens/couleurs.css?raw';
import { attacherAccentAuDOM, TOKEN_ACCENT, type AccesTokens } from './accent-dom';

const blocs = lireBlocsDeTheme(tokensCss);
const bloc = (nom: string) => blocs.find((b) => b.nom === nom)!.tokens;
const sombre = bloc('racine');
const clairBrut = bloc('attribut-clair');

const NOMS = ['--fond-0', '--fond-1', '--fond-2', '--accent'] as const;
const theme = (t: Map<string, string>, repli: Map<string, string>): Record<string, string> =>
    Object.fromEntries(NOMS.map((n) => [n, t.get(n) ?? repli.get(n)!]));

const SOMBRE = theme(sombre, sombre);
const CLAIR = theme(clairBrut, sombre);

/// `--succes` du bloc sombre : **8,867 / 8,186 / 7,446** sur les fonds
/// SOMBRES, et **2,194 / 2,047 / 1,889** sur les fonds CLAIRS — mesuré le
/// 21 août 2026. C'est ce qui fait de lui le fixture exact du test de bascule :
/// la MÊME couleur est acceptée dans un thème et refusée dans l'autre.
const READABLE_IN_DARK_ONLY = sombre.get('--succes')!;

/// Une racine factice qui compte ses lectures : c'est ce compteur qui distingue
/// « les fonds sont relus » de « les fonds ont été mémorisés au montage ».
function racine(depart: Record<string, string>) {
    const poses: Array<[string, string]> = [];
    const lus: string[] = [];
    let current = depart;
    const acces: AccesTokens = {
        lireToken: (nom) => {
            lus.push(nom);
            return current[nom] ?? '';
        },
        poserToken: (nom, value) => {
            poses.push([nom, value]);
        },
    };
    return { acces, poses, lus, basculer: (t: Record<string, string>) => (current = t) };
}

describe('attacherAccentAuDOM', () => {
    it('a COMPLIANT colour is set on the root', () => {
        // ROUGE : l'arbre intact avant que le module n'existe.
        const r = racine(SOMBRE);
        attacherAccentAuDOM(r.acces).recevoir(READABLE_IN_DARK_ONLY);
        expect(r.poses).toEqual([[TOKEN_ACCENT, READABLE_IN_DARK_ONLY]]);
    });

    it('a REFUSED colour makes the theme accent be set, never nothing', () => {
        // ROUGE : ne rien poser sur refus ⟹ le token garderait sa valeur
        // PRÉCÉDENTE, c'est-à-dire la teinte d'une icône qui n'est plus celle
        // de cette fenêtre — un état périmé, plus trompeur qu'un repli visible.
        const r = racine(SOMBRE);
        const a = attacherAccentAuDOM(r.acces);
        a.recevoir(READABLE_IN_DARK_ONLY);
        a.recevoir(sombre.get('--bord')!); // 1,447 / 1,336 / 1,215 : illisible
        expect(r.poses).toEqual([
            [TOKEN_ACCENT, READABLE_IN_DARK_ONLY],
            [TOKEN_ACCENT, SOMBRE['--accent']],
        ]);
    });

    it('the THREE backgrounds and the accent are re-read on EACH message', () => {
        // 🔴 ROUGE : mémoriser les fonds au montage ⟹ une bascule de thème
        // laisserait l'accent jugé contre l'ANCIEN thème. La MÊME couleur est
        // acceptée en sombre et refusée en clair : sans la relecture, le second
        // message poserait la couleur au lieu de l'accent clair.
        const r = racine(SOMBRE);
        const a = attacherAccentAuDOM(r.acces);
        a.recevoir(READABLE_IN_DARK_ONLY);
        expect(r.lus).toEqual(['--fond-0', '--fond-1', '--fond-2', '--accent']);

        r.basculer(CLAIR);
        a.recevoir(READABLE_IN_DARK_ONLY);
        expect(r.lus).toHaveLength(8);
        expect(r.poses[1]).toEqual([TOKEN_ACCENT, CLAIR['--accent']]);
        expect(CLAIR['--accent']).not.toBe(SOMBRE['--accent']);
    });

    it('the token is set on the ROOT, and the module knows no element', () => {
        // 🔴 C'EST LA ROUGE DE REMPLACEMENT DU CRITÈRE ② (E7 du plan) : la rouge
        // que la spec prescrivait — « poser le token sans le déclarer dans les
        // trois blocs, §7.4 échoue » — est VACUEUSE sous D-A1-2, où il n'y a
        // AUCUNE déclaration : §7.4 ne verrait rien.
        //
        // La rouge jouable est de poser sur `document.body`. Alors
        // `getComputedStyle(document.documentElement).getPropertyValue(...)`
        // rend la CHAÎNE VIDE, et le critère ② rougit sur l'assertion qu'il
        // énonce. Ce test-ci en est la moitié éprouvable sur l'hôte : le module
        // ne reçoit AUCUN moyen de désigner un élément — son interface n'en
        // porte pas —, donc il ne peut pas en acquérir un par accident.
        //
        // ⚠️ Une rouge qui n'enverrait AUCUN message serait moins bonne : elle
        // rendrait aussi la chaîne vide, et la chaîne vide est ce que rend un
        // mécanisme entièrement mort. C'est la leçon de la rouge ① de P3.
        const r = racine(SOMBRE);
        attacherAccentAuDOM(r.acces).recevoir(READABLE_IN_DARK_ONLY);
        expect(r.poses.every(([nom]) => nom === TOKEN_ACCENT)).toBe(true);
        expect(Object.keys(r.acces)).toEqual(['lireToken', 'poserToken']);
    });

    it('a root whose tokens are ABSENT makes the empty string be set, without throwing', () => {
        // ROUGE : ne pas contrôler la forme des fonds dans `conformer` ⟹
        // `rapportDeContraste` LÈVE sur la chaîne vide, et une exception dans un
        // gestionnaire de message de canal de données tue la session sans rien
        // dire. Le cas est RÉEL : `getComputedStyle` rend la chaîne vide pour un
        // token absent — donc pour toute page dont le socle n'est pas encore lié.
        const r = racine({});
        attacherAccentAuDOM(r.acces).recevoir(READABLE_IN_DARK_ONLY);
        expect(r.poses).toEqual([[TOKEN_ACCENT, '']]);
    });
});
