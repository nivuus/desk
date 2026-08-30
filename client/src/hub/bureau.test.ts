import { describe, expect, it } from 'vitest';
import { NOM_FENETRE_BUREAU, PAGE_DU_BUREAU, ouvrirLeBureau } from './bureau';

describe('le chemin du hub vers le bureau', () => {
    it('ouvre la page du bureau sous un NOM, jamais dans une fenêtre neuve', () => {
        const appels: { url: string; nom: string }[] = [];
        const ouvert = ouvrirLeBureau({
            ouvrir(url, nom) {
                appels.push({ url, nom });
                return {} as Window;
            },
        });
        expect(ouvert).toBe(true);
        expect(appels).toEqual([{ url: PAGE_DU_BUREAU, nom: NOM_FENETRE_BUREAU }]);
    });

    it('ramène au premier plan un bureau déjà ouvert', () => {
        // Sans ce `focus`, un second clic réutiliserait la fenêtre existante
        // SANS la montrer : l'utilisateur croirait son clic perdu.
        let focalisee = 0;
        ouvrirLeBureau({ ouvrir: () => ({ focus: () => { focalisee += 1; } }) as unknown as Window });
        expect(focalisee).toBe(1);
    });

    it('dit NON quand le navigateur a bloqué l’ouverture, et ne lève pas', () => {
        // 🔴 LA ROUGE DU MÉCANISME. `window.open` rend `null` quand la pop-up
        // est bloquée ; l'appelant doit pouvoir le dire à l'utilisateur plutôt
        // que de le laisser devant un hub qui ne réagit pas.
        expect(ouvrirLeBureau({ ouvrir: () => null })).toBe(false);
    });

    it('ne suppose pas que la fenêtre rendue porte un focus', () => {
        expect(ouvrirLeBureau({ ouvrir: () => ({}) as Window })).toBe(true);
    });
});
