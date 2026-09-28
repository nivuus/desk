// ⚠️ AUCUNE directive `@vitest-environment` : ces deux fonctions sont PURES,
// et `client/` n'a ni jsdom ni happy-dom — par convention, pas par oubli
// (`accent-dom.test.ts`). C'est pour cela qu'elles sont exportées séparément
// du reste du module, qui, lui, touche le DOM et n'est pas testé.
import { describe, expect, it } from 'vitest';
import { diffuserSiChange, nomDuVerrou, type DepsBureauPage } from './porteur-dom';

describe('nomDuVerrou', () => {
    it('carries the VM PREFIX', () => {
        // 🔴 SANS LUI, DEUX VMs OUVERTES DANS DEUX ONGLETS S EXCLURAIENT L UNE
        // L AUTRE -- le defaut que P3 a corrige sur le nom de session,
        // reintroduit par la porte de derriere.
        expect(nomDuVerrou('vm-7')).toBe('vm-7:nivuus-bureau');
    });

    it('without a known prefix, returns the bare name', () => {
        expect(nomDuVerrou('')).toBe('nivuus-bureau');
    });
});

describe('diffuserSiChange', () => {
    it('broadcasts NOTHING when the state is identical', () => {
        // ⚠️ Le porteur redessine a 1 Hz : diffuser a chaque tour reveillerait
        // tous les onglets une fois par seconde pour rien.
        const envoyes: unknown[] = [];
        const canal = { postMessage: (m: unknown) => void envoyes.push(m) };
        const list = [{ session: 's', titre: 'x', ouverte: true }];
        let last = '';
        last = diffuserSiChange(canal, list, last);
        last = diffuserSiChange(canal, list, last);
        expect(envoyes.length).toBe(1);
        // ⚠️ MINOR round 1 : `last` reaffecte et jamais relu suggerait une
        // assertion absente. Elle porte l empreinte -- verifier qu elle EST
        // celle de la liste stable, jamais une chaine vide oubliee.
        expect(last).toBe(JSON.stringify(list));
    });

    it('broadcasts when a window changes state', () => {
        const envoyes: unknown[] = [];
        const canal = { postMessage: (m: unknown) => void envoyes.push(m) };
        let last = diffuserSiChange(canal, [{ session: 's', titre: 'x', ouverte: true }], '');
        last = diffuserSiChange(canal, [{ session: 's', titre: 'x', ouverte: false }], last);
        expect(envoyes.length).toBe(2);
        // Meme raison que ci-dessus : l empreinte rendue suit la DERNIERE
        // diffusion, pas la premiere.
        expect(last).toBe(JSON.stringify([{ session: 's', titre: 'x', ouverte: false }]));
    });
});

/* ══ CE QUE LA REVUE FINALE DU 31 AOUT 2026 A AJOUTE ═════════════════════ */

describe('DepsBureauPage: the token is a PROVIDER, never a string', () => {
    // 🔴 CE QUI EST FIGE ICI N EST PAS LA REGLE DE FRAICHEUR -- `jeton.test.ts`
    // la tient depuis la tache 1 -- MAIS LA JONCTION. Le champ portait une
    // CHAINE, capturee au chargement de la page ; or `ouvrirLaSession` ne
    // court, pour un suiveur, qu au moment de sa PROMOTION, potentiellement
    // des heures plus tard, et `DUREE_JETON_ACCES_MS` vaut DIX MINUTES
    // (`plateforme/src/identite/jeton.ts`). Un test d `assurerAccesFrais`
    // n aurait rien vu : c est ce couplage-la qui etait faux.
    //
    // ⚠️ CES ELEMENTS NE SONT JAMAIS TOUCHES : `installerLeBureau` n est PAS
    // appele ici, et ne peut pas l etre -- `client/` n a ni jsdom ni
    // happy-dom, par convention. Ce test fige un CONTRAT DE TYPE, et son juge
    // est `tsc --noEmit`, pas Vitest (qui transpile sans verifier les types).
    const elements = {} as DepsBureauPage['elements'];

    it('exposes `jetonFrais`, a FUNCTION the promotion can call again', async () => {
        const deps: DepsBureauPage = {
            signalingUrl: 'ws://exemple/signal',
            jetonFrais: () => Promise.resolve('frais'),
            prefixe: 'vm-7',
            fautesArmees: false,
            elements,
        };
        expect(typeof deps.jetonFrais).toBe('function');
        await expect(deps.jetonFrais()).resolves.toBe('frais');
    });

    it('REFUSES a scalar token -- assertion held by `tsc --noEmit`', () => {
        const deps: DepsBureauPage = {
            signalingUrl: 'ws://exemple/signal',
            jetonFrais: () => Promise.resolve(undefined),
            prefixe: '',
            fautesArmees: false,
            elements,
            // 🔴 LA DIRECTIVE EST L ASSERTION. `tsc` ECHOUE sur un
            // « Unused '@ts-expect-error' directive » le jour ou ce champ
            // redeviendrait licite -- c est-a-dire le jour ou l on
            // reintroduirait le defaut. Vitest, lui, ne verifie aucun type :
            // c est `npm run typecheck` qui juge, et il est obligatoire.
            // @ts-expect-error un jeton FIGE n a plus sa place dans ces deps
            jeton: 'a string captured at load time',
        };
        expect('jeton' in deps).toBe(true);
    });
});
