import { describe, expect, it } from 'vitest';

import { estUnRaccourciDeCollage, type ToucheObservee } from './raccourcis';

/// Construit une touche observée : tout est faux par défaut, seul ce qu'on
/// nomme est vrai. C'est ce qui rend chaque cas de la table lisible.
function touche(partiel: Partial<ToucheObservee> & { code: string }): ToucheObservee {
    return { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...partiel };
}

describe('estUnRaccourciDeCollage', () => {
    it('reconnaît Ctrl+V', () => {
        expect(estUnRaccourciDeCollage(touche({ ctrlKey: true, code: 'KeyV' }))).toBe(true);
    });

    // La sonde du 20 août 2026 établit que `Shift+Insert` produit le MÊME
    // `paste` de confiance, aux quatre cellules où il est éprouvé et aux deux
    // exécutions (`journaux-presse-papier-p2/p2-paste-video-{1,2}.json`). D6 le
    // nommait sans l'avoir mesuré.
    it('reconnaît Shift+Insert', () => {
        expect(estUnRaccourciDeCollage(touche({ shiftKey: true, code: 'Insert' }))).toBe(true);
    });

    // 🔴 C'EST LE RISQUE R5 DE LA SPEC, ET CES TROIS CAS SONT TOUTE LA DÉFENSE.
    // ROUGE si la condition teste `e.ctrlKey` seul : `Ctrl+W` fermerait la
    // fenêtre de session, `Ctrl+T` ouvrirait un onglet, `Ctrl+N` une fenêtre —
    // parce que P2 retire alors le `preventDefault` qui les retenait.
    it.each(['KeyW', 'KeyT', 'KeyN'])('refuse Ctrl+%s (risque R5)', (code) => {
        expect(estUnRaccourciDeCollage(touche({ ctrlKey: true, code }))).toBe(false);
    });

    // ROUGE si l'on omet `!e.shiftKey`. C'est « coller sans mise en forme »
    // dans plusieurs applications, et il DOIT rester au produit Windows.
    it('refuse Ctrl+Shift+V', () => {
        expect(
            estUnRaccourciDeCollage(touche({ ctrlKey: true, shiftKey: true, code: 'KeyV' })),
        ).toBe(false);
    });

    // ROUGE si l'on omet `!e.altKey`.
    it('refuse Ctrl+Alt+V', () => {
        expect(estUnRaccourciDeCollage(touche({ ctrlKey: true, altKey: true, code: 'KeyV' }))).toBe(
            false,
        );
    });

    // ROUGE si l'on omet `!e.metaKey`. Sur macOS ce serait le collage natif ;
    // le produit ne le traite PAS en v1, et ce test fige la décision plutôt que
    // de la laisser à l'appréciation du prochain lecteur.
    it('refuse Meta+V', () => {
        expect(estUnRaccourciDeCollage(touche({ metaKey: true, code: 'KeyV' }))).toBe(false);
    });

    // ROUGE si l'on teste le seul `code`.
    it('refuse V seul', () => {
        expect(estUnRaccourciDeCollage(touche({ code: 'KeyV' }))).toBe(false);
    });

    // ROUGE si la branche `Insert` n'exige pas `!e.ctrlKey`.
    it('refuse Ctrl+Shift+Insert', () => {
        expect(
            estUnRaccourciDeCollage(touche({ ctrlKey: true, shiftKey: true, code: 'Insert' })),
        ).toBe(false);
    });

    // ROUGE si la branche `Insert` n'exige pas `!e.altKey` / `!e.metaKey`.
    it('refuse Shift+Alt+Insert et Shift+Meta+Insert', () => {
        expect(
            estUnRaccourciDeCollage(touche({ shiftKey: true, altKey: true, code: 'Insert' })),
        ).toBe(false);
        expect(
            estUnRaccourciDeCollage(touche({ shiftKey: true, metaKey: true, code: 'Insert' })),
        ).toBe(false);
    });

    // Les modificateurs eux-mêmes ne sont PAS des raccourcis de collage : leur
    // `preventDefault` est conservé, et la sonde établit que cela n'empêche
    // PAS le `paste` d'arriver (`ControlLeft` porte `dp: true`, `KeyV` porte
    // `dp: false`, et le `paste` est reçu — deux exécutions).
    it.each(['ControlLeft', 'ControlRight', 'ShiftLeft', 'ShiftRight'])(
        'refuse le modificateur %s seul',
        (code) => {
            expect(estUnRaccourciDeCollage(touche({ ctrlKey: true, code }))).toBe(false);
            expect(estUnRaccourciDeCollage(touche({ shiftKey: true, code }))).toBe(false);
        },
    );
});
