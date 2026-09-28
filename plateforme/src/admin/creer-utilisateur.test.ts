// La partie PURE de la création de compte en ligne de commande.
//
// 🔴 La rouge de ce fichier est `--mot-de-passe` : l'accepter « pour la
// commodité » exposerait le mot de passe à TOUT utilisateur de la machine,
// `ps` donnant l'argv de tout processus. Le refus est explicite et porte son
// motif, pour que l'administrateur sache quoi faire à la place.

import { describe, expect, it } from 'vitest';
import { analyserArguments } from './creer-utilisateur';

describe('analyserArguments', () => {
    it('reads --email', () => {
        expect(analyserArguments(['--email', 'ada@exemple.test']))
            .toEqual({ email: 'ada@exemple.test' });
    });

    it('REFUSES --mot-de-passe on the command line, with its reason', () => {
        // 🔴 La rouge : l'accepter. `ps` expose l'argv de tout processus à tout
        // utilisateur de la machine ; le mot de passe se lit sur l'entrée
        // standard, et là seulement.
        const r = analyserArguments(['--email', 'ada@exemple.test', '--mot-de-passe', 'secret']);
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/standard input/i);
        // Le motif ne RECOPIE PAS le secret qu'on vient de refuser : ce serait
        // le réécrire dans un journal après l'avoir refusé dans un argv.
        expect(r.refus).not.toContain('secret');
    });

    it('also refuses the variants of the same flag', () => {
        for (const drapeau of ['--motdepasse', '--password', '-p']) {
            const r = analyserArguments(['--email', 'ada@exemple.test', drapeau, 'secret']);
            expect('refus' in r).toBe(true);
        }
    });

    it('refuses a missing --email, rather than returning undefined', () => {
        // Rendre `undefined` laisserait l'appelant planter plus loin, avec un
        // diagnostic sans rapport.
        const r = analyserArguments([]);
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/--email/);
    });

    it('refuses an empty email', () => {
        expect('refus' in analyserArguments(['--email', ''])).toBe(true);
        expect('refus' in analyserArguments(['--email'])).toBe(true);
    });
});
