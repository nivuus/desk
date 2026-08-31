// ⚠️ AUCUNE directive `@vitest-environment` : ces deux fonctions sont PURES,
// et `client/` n'a ni jsdom ni happy-dom — par convention, pas par oubli
// (`accent-dom.test.ts`). C'est pour cela qu'elles sont exportées séparément
// du reste du module, qui, lui, touche le DOM et n'est pas testé.
import { describe, expect, it } from 'vitest';
import { diffuserSiChange, nomDuVerrou } from './porteur-dom';

describe('nomDuVerrou', () => {
    it('porte le PREFIXE de VM', () => {
        // 🔴 SANS LUI, DEUX VMs OUVERTES DANS DEUX ONGLETS S EXCLURAIENT L UNE
        // L AUTRE -- le defaut que P3 a corrige sur le nom de session,
        // reintroduit par la porte de derriere.
        expect(nomDuVerrou('vm-7')).toBe('vm-7:nivuus-bureau');
    });

    it('sans prefixe connu, rend le nom nu', () => {
        expect(nomDuVerrou('')).toBe('nivuus-bureau');
    });
});

describe('diffuserSiChange', () => {
    it('ne diffuse RIEN quand l etat est identique', () => {
        // ⚠️ Le porteur redessine a 1 Hz : diffuser a chaque tour reveillerait
        // tous les onglets une fois par seconde pour rien.
        const envoyes: unknown[] = [];
        const canal = { postMessage: (m: unknown) => void envoyes.push(m) };
        const liste = [{ session: 's', titre: 'x', ouverte: true }];
        let dernier = '';
        dernier = diffuserSiChange(canal, liste, dernier);
        dernier = diffuserSiChange(canal, liste, dernier);
        expect(envoyes.length).toBe(1);
        // ⚠️ MINOR round 1 : `dernier` reaffecte et jamais relu suggerait une
        // assertion absente. Elle porte l empreinte -- verifier qu elle EST
        // celle de la liste stable, jamais une chaine vide oubliee.
        expect(dernier).toBe(JSON.stringify(liste));
    });

    it('diffuse quand une fenetre change d etat', () => {
        const envoyes: unknown[] = [];
        const canal = { postMessage: (m: unknown) => void envoyes.push(m) };
        let dernier = diffuserSiChange(canal, [{ session: 's', titre: 'x', ouverte: true }], '');
        dernier = diffuserSiChange(canal, [{ session: 's', titre: 'x', ouverte: false }], dernier);
        expect(envoyes.length).toBe(2);
        // Meme raison que ci-dessus : l empreinte rendue suit la DERNIERE
        // diffusion, pas la premiere.
        expect(dernier).toBe(JSON.stringify([{ session: 's', titre: 'x', ouverte: false }]));
    });
});
