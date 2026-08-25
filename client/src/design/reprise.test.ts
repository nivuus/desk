/// <reference types="vite/client" />
import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme } from './tokens';
import couleursCss from './tokens/couleurs.css?raw';
import echellesCss from './tokens/echelles.css?raw';
import baseCss from './base.css?raw';

// 🔴 DEPUIS L'EXTRACTION DE LA TÂCHE 6 (25 août 2026) : les couleurs et les
// échelles vivent dans deux fichiers distincts, chacun son propre `:root {}`
// sans condition. Ce test-ci a BESOIN des deux dans le MÊME bloc « racine »
// — il compare `--fond-0` (couleur) ET `--e-3` (échelle) sur `racine` — donc
// c'est ICI, et non dans un test isolé, que la fusion de `lireBlocsDeTheme`
// est exercée sur du contenu RÉEL plutôt que sur une démonstration.
const tokensCss = `${couleursCss}\n${echellesCss}`;

/**
 * LA COMPARAISON QUI PROUVE LA REPRISE — pas l'affirmation.
 *
 * ⚠️ CE QUE CES ASSERTIONS ÉTABLISSENT : l'égalité des valeurs DÉCLARÉES, à
 * racine 16 px. PAS l'égalité des pixels rendus — cela demanderait un moteur
 * de rendu, et la spec §7.8 écarte la comparaison d'images, sa première raison
 * étant éliminatoire : les polices système rendent différemment d'une machine
 * à l'autre, donc une référence prise sur un poste échouerait sur le suivant
 * pour une raison qui n'est pas un défaut.
 *
 * Les valeurs de référence ci-dessous sont celles de `client/src/style.css`
 * AVANT ce socle, au commit `7e9b438` : c'est pourquoi elles sont écrites en
 * littéral ici, et c'est la seule raison pour laquelle le contrôle §7.2
 * exclut `client/src/design/*.test.ts` — sans quoi cette tâche serait
 * impossible.
 */

const blocs = lireBlocsDeTheme(tokensCss);
const racine = blocs.find((b) => b.nom === 'racine');

describe('les couleurs déjà en place sont reprises caractère pour caractère', () => {
    it('`--fond-0` sombre vaut exactement l’ancien `--surface`', () => {
        expect(racine?.tokens.get('--fond-0')).toBe('#0b0d10');
    });

    it('`--texte-fort` sombre vaut exactement l’ancien `--text`', () => {
        expect(racine?.tokens.get('--texte-fort')).toBe('#e6e8eb');
    });
});

describe('les longueurs reprises rendent le MÊME nombre de pixels', () => {
    it('chaque cran employé rend, à racine 16 px, le littéral d’avant', () => {
        // 🔴 Poser `--e-3: 0.8rem` rendrait 12,8 px : plausible, et faux. C'est
        // exactement le genre d'erreur qu'aucun des neuf contrôles n'attrape.
        // ❌ « puisque aucun ne mesure une longueur » : plus vrai depuis le
        // sous-bloc S4 — §7.10 en mesure une. Mais `tokens.css` est son
        // exception ③, NOMMÉE : ses littéraux SONT l'échelle, et un contrôle
        // qui les refuserait refuserait l'échelle elle-même. C'est ce test-ci,
        // et lui seul, qui tient la valeur des crans.
        const attendus: Array<[string, string, number]> = [
            ['--e-2', '0.5rem', 8],
            ['--e-3', '0.75rem', 12],
            ['--e-8', '4rem', 64],
            ['--t-s', '0.75rem', 12],
            ['--t-m', '0.875rem', 14],
        ];
        for (const [token, litteral, pixels] of attendus) {
            const valeur = racine?.tokens.get(token);
            expect(valeur, token).toBe(litteral);
            expect(Number.parseFloat(litteral) * 16, `${token} en pixels`).toBe(pixels);
        }
    });

    it('les crans sans unité `rem` valent le littéral d’avant, tels quels', () => {
        expect(racine?.tokens.get('--r-2')).toBe('6px');
        expect(racine?.tokens.get('--duree-2')).toBe('300ms');
        expect(racine?.tokens.get('--lh-normal')).toBe('1.5');
    });
});

describe('les six voiles hors thème reprennent les littérales verbatim', () => {
    it('chacune vaut la valeur exacte qu’elle remplace', () => {
        const attendus: Record<string, string> = {
            '--video-letterbox': '#000',
            '--voile-flottant': 'rgb(0 0 0 / 0.72)',
            '--voile-bouton': 'rgb(0 0 0 / 0.55)',
            '--voile-bouton-survol': 'rgb(0 0 0 / 0.75)',
            '--voile-micro-actif': 'rgb(220 38 38 / 0.85)',
            '--voile-micro-refuse': 'rgb(120 53 15 / 0.85)',
        };
        for (const [token, valeur] of Object.entries(attendus)) {
            expect(racine?.tokens.get(token), token).toBe(valeur);
        }
    });
});

describe('la précondition de tout ce qui précède : la racine est LIBRE', () => {
    it('aucune règle dont le sélecteur contient `html` ne pose de taille de police', () => {
        // 🔴 C'EST LE SEUL LIEN MÉCANIQUE ENTRE `base.css` ET LES CHIFFRES
        // CI-DESSUS. `--e-3` ne vaut 12 px que si `1rem` vaut 16 px, donc que
        // si la racine n'est pas forcée. Avec le `font: 14px/1.5` que
        // `style.css` posait sur `html, body` avant ce socle, `0.75rem`
        // vaudrait 10,5 px — et AUCUN des neuf contrôles ne le dirait.
        const regles = [...baseCss.matchAll(/([^{}]+)\{([^{}]*)\}/g)];
        const fautives = regles
            .filter(([, selecteur]) => /(^|[\s,>+~])html\b/.test(selecteur))
            .filter(([, , corps]) => /(^|[;\s])font-size\s*:|(^|[;\s])font\s*:/.test(corps))
            .map(([, selecteur]) => selecteur.trim());
        expect(fautives).toEqual([]);
        // Et le contrôle peut échouer : il DOIT voir la règle `html, body`.
        expect(regles.some(([, s]) => /(^|[\s,>+~])html\b/.test(s))).toBe(true);
    });
});
