import { describe, expect, it } from 'vitest';
import { encoderTexte, TYPE_LIRE, TYPE_RENOMMER } from '../../../proto/ts/fichiers';
import { cleDOrdre, creerSequenceur } from './ordre';

/** A promise resolved from outside, to decide WHEN each piece of work ends. */
function differee<T>() {
    let resoudre!: (v: T) => void;
    const promesse = new Promise<T>((r) => (resoudre = r));
    return { promesse, resoudre };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe('the order of the answers', () => {
    it('sends the answers of one path in the order of their requests, whatever finishes first', async () => {
        const s = creerSequenceur();
        const envoyes: number[] = [];
        const travaux = [differee<number>(), differee<number>(), differee<number>()];
        travaux.forEach((t) => void s.enchainer('f.bin', t.promesse, (n) => envoyes.push(n)));
        // The LAST chunk — the short one — finishes first, then the second.
        travaux[2].resoudre(2);
        travaux[1].resoudre(1);
        await tick();
        expect(envoyes).toEqual([]);
        travaux[0].resoudre(0);
        await tick();
        expect(envoyes).toEqual([0, 1, 2]);
        expect(s.enAttente()).toBe(0);
    });

    it('does not make a path wait behind another one', async () => {
        const s = creerSequenceur();
        const envoyes: string[] = [];
        const listage = differee<string>();
        void s.enchainer('dossier', listage.promesse, (v) => envoyes.push(v));
        void s.enchainer('f.bin', Promise.resolve('morceau'), (v) => envoyes.push(v));
        await tick();
        expect(envoyes).toEqual(['morceau']);
        listage.resoudre('entrees');
        await tick();
        expect(envoyes).toEqual(['morceau', 'entrees']);
    });

    it('keeps going after an emission that threw', async () => {
        const s = creerSequenceur();
        const envoyes: number[] = [];
        const premier = s.enchainer('f', Promise.resolve(0), () => {
            throw new Error('send refused');
        });
        void s.enchainer('f', Promise.resolve(1), (n) => envoyes.push(n));
        await expect(premier).rejects.toThrow('send refused');
        await tick();
        expect(envoyes).toEqual([1]);
    });
});

describe('cleDOrdre', () => {
    it('is the path a request names, `de` for a rename, empty otherwise', () => {
        expect(cleDOrdre(encoderTexte(TYPE_LIRE, 1, '{"chemin":"a/b.txt","position":0,"longueur":4}'))).toBe(
            'a/b.txt',
        );
        expect(cleDOrdre(encoderTexte(TYPE_RENOMMER, 2, '{"de":"x","vers":"y","repertoire":false}'))).toBe('x');
        expect(cleDOrdre(encoderTexte(TYPE_LIRE, 3, '{}'))).toBe('');
        expect(cleDOrdre(new ArrayBuffer(2))).toBe('');
    });
});
