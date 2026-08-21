import { describe, expect, it } from 'vitest';
import tokensCss from '../design/tokens.css?raw';
import { batirManifeste, versDataUrl, type Sujet } from './manifeste';

const APP: Sujet = { id: 'u-1', nom: 'Bloc-notes' };

/// 🔴 AUCUNE COULEUR N'EST ÉCRITE DANS CE FICHIER, ET C'EST §7.2 QUI L'EXIGE :
/// son balayage couvre les `.ts` autant que les `.css`, et son exclusion ne
/// couvre que `client/src/design/*.test.ts` — pas ce fichier-ci. Un premier
/// jet y posait deux littérales et le contrôle les a relevées. **Élargir son
/// exclusion aurait satisfait le contrôle en le VIDANT** ; les valeurs sont
/// donc LUES sur `tokens.css`, ce qui est un meilleur test : il rougirait
/// aussi le jour où le token changerait de valeur sans que ce fichier bouge.
function tokenSombre(nom: string): string {
    const racine = tokensCss.slice(tokensCss.indexOf(':root'));
    const trouve = new RegExp(`${nom}:\\s*([^;]+);`).exec(racine);
    if (trouve === null) throw new Error(`tokens.css ne declare plus ${nom}`);
    return trouve[1].trim();
}
const FOND = tokenSombre('--fond-0');
const ACCENT = tokenSombre('--accent');

describe('batirManifeste', () => {
    it('rend des URL ABSOLUES — la contrainte que la porte P0 a mesurée', () => {
        const m = batirManifeste(APP, 'https://exemple.test', FOND);
        // 🔴 SI CES TROIS-LÀ REDEVIENNENT RELATIVES, le manifeste `blob:` est
        //    refusé par Chromium et l'application cesse d'être installable.
        //    Mesuré 2 exécutions, sonde `f` de `instrument/porte-p0.mjs`.
        expect(m.start_url).toBe('https://exemple.test/shell.html?app=u-1');
        expect(m.scope).toBe('https://exemple.test/');
        expect(m.id).toBe('https://exemple.test/shell.html?app=u-1');
        for (const url of [m.start_url, m.scope, m.id]) {
            expect(url.startsWith('https://')).toBe(true);
        }
    });

    it('tolère une origine à barre oblique finale sans doubler la barre', () => {
        const m = batirManifeste(APP, 'https://exemple.test/', FOND);
        expect(m.scope).toBe('https://exemple.test/');
        expect(m.start_url).toBe('https://exemple.test/shell.html?app=u-1');
    });

    it("échappe l'identifiant dans start_url et id", () => {
        const m = batirManifeste({ id: 'a/b?c', nom: 'X' }, 'https://x', FOND);
        expect(m.start_url).toBe('https://x/shell.html?app=a%2Fb%3Fc');
        expect(m.id).toBe('https://x/shell.html?app=a%2Fb%3Fc');
    });

    it('donne à DEUX applications des `id` DISTINCTS sous un `scope` PARTAGÉ', () => {
        const a = batirManifeste({ id: 'u-1', nom: 'A' }, 'https://x', FOND);
        const b = batirManifeste({ id: 'u-2', nom: 'B' }, 'https://x', FOND);
        expect(a.id).not.toBe(b.id);
        expect(a.scope).toBe(b.scope);
    });

    it('pose display_override avec le Window Controls Overlay en tête', () => {
        expect(batirManifeste(APP, 'https://x', FOND).display_override).toEqual([
            'window-controls-overlay',
            'standalone',
        ]);
    });

    it("OMET theme_color quand aucun accent n'est connu, plutôt que d'en inventer un", () => {
        const m = batirManifeste(APP, 'https://x', FOND);
        expect('theme_color' in m).toBe(false);
    });

    it("pose theme_color quand un accent est donné", () => {
        expect(batirManifeste({ ...APP, accent: ACCENT }, 'https://x', FOND).theme_color).toBe(ACCENT);
    });

    it("n'a AUCUNE icône quand aucune n'est fournie", () => {
        expect(batirManifeste(APP, 'https://x', FOND).icons).toEqual([]);
    });

    it("porte l'icône en data: et déclare le côté qu'on lui donne", () => {
        const m = batirManifeste({ ...APP, icone: new Uint8Array([1, 2, 3]), coteIcone: 256 }, 'https://x', FOND);
        expect(m.icons).toHaveLength(1);
        expect(m.icons[0].sizes).toBe('256x256');
        expect(m.icons[0].type).toBe('image/png');
        expect(m.icons[0].purpose).toBe('any');
        expect(m.icons[0].src.startsWith('data:image/png;base64,')).toBe(true);
    });

    it('déclare 128x128 quand on lui passe 128 — ce que la ROUGE du critère ① exige', () => {
        // 🔴 SANS CE MEMBRE, LA ROUGE DE ① SERAIT INJOUABLE : le magasin ne
        //    connaît qu'une taille (256), et redimensionner côté plateforme est
        //    refusé par `0005-icones.sql:43-47`.
        const m = batirManifeste({ ...APP, icone: new Uint8Array([1]), coteIcone: 128 }, 'https://x', FOND);
        expect(m.icons[0].sizes).toBe('128x128');
    });

    it('ignore une icône VIDE plutôt que de déclarer une entrée sans image', () => {
        expect(batirManifeste({ ...APP, icone: new Uint8Array([]) }, 'https://x', FOND).icons).toEqual([]);
    });
});

describe('versDataUrl', () => {
    it('encode en base64 sans dépendance', () => {
        // « PNG\r » — les quatre premiers octets d'un vrai PNG.
        expect(versDataUrl(new Uint8Array([0x89, 0x50, 0x4e, 0x47]))).toBe(
            'data:image/png;base64,iVBORw==',
        );
    });

    it('encode un tampon plus long que le paquet sans déborder la pile', () => {
        // 0x2000 est la taille de paquet : on la dépasse franchement, comme le
        // fait une vraie icône de 256×256.
        const gros = new Uint8Array(0x2000 * 3 + 7).fill(0x41);
        const url = versDataUrl(gros);
        expect(url.startsWith('data:image/png;base64,')).toBe(true);
        // Le décodage rend EXACTEMENT ce qu'on a encodé : c'est ce qui
        // éprouve la découpe, et pas seulement l'absence d'exception.
        const decode = atob(url.slice('data:image/png;base64,'.length));
        expect(decode.length).toBe(gros.length);
        expect(decode.charCodeAt(0)).toBe(0x41);
        expect(decode.charCodeAt(decode.length - 1)).toBe(0x41);
    });
});

describe('la couleur de fond', () => {
    it("est un PARAMÈTRE, et le manifeste rend EXACTEMENT ce qu'on lui donne", () => {
        // 🔴 IL N'Y A PLUS DE CONSTANTE DE COULEUR À CONFRONTER : `manifeste.ts`
        //    n'en porte aucune, et c'est la page qui lit le thème vivant par
        //    `getComputedStyle`. Ce test éprouve donc ce qui reste éprouvable —
        //    que la valeur traverse sans être réécrite —, et il le fait avec
        //    une valeur LUE sur `tokens.css`, jamais écrite ici.
        expect(batirManifeste(APP, 'https://x', FOND).background_color).toBe(FOND);
    });

    it("ne confond pas le fond et l'accent", () => {
        const m = batirManifeste({ ...APP, accent: ACCENT }, 'https://x', FOND);
        expect(m.background_color).toBe(FOND);
        expect(m.theme_color).toBe(ACCENT);
        expect(FOND).not.toBe(ACCENT);
    });
});
