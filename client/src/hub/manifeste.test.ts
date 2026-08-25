import { describe, expect, it } from 'vitest';
import tokensCss from '../design/tokens/couleurs.css?raw';
import { accepterDepuis, batirManifeste, cotePng, mimeDe, versDataUrl, type Sujet } from './manifeste';

/// Un vrai PNG d'un côté donné — signature, IHDR, et rien d'autre. Il suffit à
/// `cotePng`, qui ne lit que l'en-tête, et il est CONSTRUIT plutôt que collé en
/// base64 : un littéral opaque ne dirait pas ce qu'il porte.
function pngDe(cote: number): Uint8Array {
    const o = new Uint8Array(24);
    o.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], 0);
    o.set([0x49, 0x48, 0x44, 0x52], 12);
    const ecrire = (d: number, v: number) => {
        o[d] = (v >>> 24) & 0xff;
        o[d + 1] = (v >>> 16) & 0xff;
        o[d + 2] = (v >>> 8) & 0xff;
        o[d + 3] = v & 0xff;
    };
    ecrire(16, cote);
    ecrire(20, cote);
    return o;
}

const APP: Sujet = { id: 'u-1', nom: 'Bloc-notes' };

/// 🔴 AUCUNE COULEUR N'EST ÉCRITE DANS CE FICHIER, ET C'EST §7.2 QUI L'EXIGE :
/// son balayage couvre les `.ts` autant que les `.css`, et son exclusion ne
/// couvre que `client/src/design/*.test.ts` — pas ce fichier-ci. Un premier
/// jet y posait deux littérales et le contrôle les a relevées. **Élargir son
/// exclusion aurait satisfait le contrôle en le VIDANT** ; les valeurs sont
/// donc LUES sur `tokens/couleurs.css` (`tokens.css` avant l'extraction de la
/// tâche 6, 25 août 2026), ce qui est un meilleur test : il rougirait
/// aussi le jour où le token changerait de valeur sans que ce fichier bouge.
function tokenSombre(nom: string): string {
    const racine = tokensCss.slice(tokensCss.indexOf(':root'));
    const trouve = new RegExp(`${nom}:\\s*([^;]+);`).exec(racine);
    if (trouve === null) throw new Error(`tokens/couleurs.css ne declare plus ${nom}`);
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

    it("porte l'icône en data: et déclare le côté LU DANS SES OCTETS", () => {
        const m = batirManifeste({ ...APP, icone: pngDe(256) }, 'https://x', FOND);
        expect(m.icons).toHaveLength(1);
        expect(m.icons[0].sizes).toBe('256x256');
        expect(m.icons[0].type).toBe('image/png');
        expect(m.icons[0].purpose).toBe('any');
        expect(m.icons[0].src.startsWith('data:image/png;base64,')).toBe(true);
    });

    it('déclare 128x128 sur un PNG de 128 — ce que la ROUGE du critère ① exige', () => {
        // 🔴 LE DÉFAUT QUE LA RECETTE A TROUVÉ : un premier jet prenait la
        //    taille de l'APPELANT, qui la laissait à 256 par défaut, et le
        //    manifeste du témoin annonçait donc `256x256` en portant un PNG de
        //    128. Chromium l'a attrapé (`no-acceptable-icon`) — mais un
        //    manifeste qui ment sur ce qu'il porte est un défaut même rattrapé.
        expect(batirManifeste({ ...APP, icone: pngDe(128) }, 'https://x', FOND).icons[0].sizes).toBe('128x128');
    });

    it("ignore une icône VIDE plutôt que de déclarer une entrée sans image", () => {
        expect(batirManifeste({ ...APP, icone: new Uint8Array([]) }, 'https://x', FOND).icons).toEqual([]);
    });

    it("N'AFFIRME RIEN sur des octets qui ne sont pas un PNG : aucune icône déclarée", () => {
        // Poser `256x256` par défaut serait affirmer ce qu'on ne sait pas.
        expect(batirManifeste({ ...APP, icone: new Uint8Array([1, 2, 3]) }, 'https://x', FOND).icons).toEqual([]);
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

describe('cotePng', () => {
    it("lit le côté dans l'IHDR", () => {
        expect(cotePng(pngDe(256))).toBe(256);
        expect(cotePng(pngDe(128))).toBe(128);
    });

    it("rend undefined sur une signature qui n'est pas celle d'un PNG", () => {
        const faux = pngDe(256);
        faux[1] = 0x00;
        expect(cotePng(faux)).toBeUndefined();
    });

    it("rend undefined quand le premier morceau n'est pas IHDR", () => {
        const faux = pngDe(256);
        faux[12] = 0x58;
        expect(cotePng(faux)).toBeUndefined();
    });

    it('rend undefined sur un tampon trop court pour porter un en-tête', () => {
        expect(cotePng(new Uint8Array([0x89, 0x50, 0x4e, 0x47]))).toBeUndefined();
    });

    it("rend undefined sur une image NON CARRÉE plutôt que d'en décrire une fausse", () => {
        // ⚠️ Le manifeste emploie la largeur pour les DEUX dimensions : une
        //    image non carrée y serait mal décrite. Que le magasin n'en produise
        //    que des carrées est une propriété de l'AGENT, pas de ce module.
        const rect = pngDe(256);
        rect[23] = 0x80; // hauteur 128, largeur 256
        expect(cotePng(rect)).toBeUndefined();
    });

    it('lit une taille sur QUATRE octets, pas seulement sur le dernier', () => {
        // 4096 = 0x1000 : le troisième octet porte l'information.
        expect(cotePng(pngDe(4096))).toBe(4096);
    });
});

describe('les file_handlers par application (tranche F)', () => {
    it("OMET `file_handlers` quand l'application n'ouvre rien", () => {
        // ⚠️ Le cas le plus fréquent. Déclarer un handler qui n'accepte rien
        //    serait une entrée sans objet, et Chromium ANALYSE ce membre.
        expect('file_handlers' in batirManifeste(APP, 'https://x', FOND)).toBe(false);
        expect(
            'file_handlers' in batirManifeste({ ...APP, associations: [] }, 'https://x', FOND),
        ).toBe(false);
    });

    it("pose une `action` DANS LE SCOPE, ce que Chromium exige", () => {
        // 🔴 MESURÉ : une `action` hors scope fait rendre à Chromium
        //    « property 'action' ignored, should be within scope of the
        //    manifest. » puis « FileHandler ignored. » — c'est la sonde qui a
        //    établi que Chromium analyse bel et bien ce membre.
        const m = batirManifeste({ ...APP, associations: ['.txt'] }, 'https://x', FOND);
        expect(m.file_handlers).toHaveLength(1);
        expect(m.file_handlers![0].action.startsWith(m.scope)).toBe(true);
    });

    it('REGROUPE les extensions qui partagent un MIME', () => {
        // 🔴 `.txt` et `.log` sont tous deux `text/plain`. Une entrée par
        //    extension écraserait la précédente, et une application qui ouvre
        //    les deux n'en verrait qu'une.
        // ROUGE : `accept[mime] = [extension]` sans le regroupement ⟹ ne reste
        //    que `.log`.
        const m = batirManifeste({ ...APP, associations: ['.txt', '.log', '.pdf'] }, 'https://x', FOND);
        expect(m.file_handlers![0].accept).toEqual({
            'text/plain': ['.txt', '.log'],
            'application/pdf': ['.pdf'],
        });
    });
});

describe('mimeDe et accepterDepuis', () => {
    it('rend le type connu des extensions de la table', () => {
        expect(mimeDe('.msi')).toBe('application/x-msi');
        expect(mimeDe('.exe')).toBe('application/vnd.microsoft.portable-executable');
        expect(mimeDe('.bat')).toBe('application/x-bat');
    });

    it('replie la casse', () => {
        expect(mimeDe('.TXT')).toBe('text/plain');
    });

    it("rend le type des octets INCONNUS plutot que d'omettre l'entree", () => {
        // ⚠️ `application/octet-stream` est le type HONNÊTE pour « des octets
        //    dont on ne sait rien ». Omettre l'entrée ferait disparaître
        //    l'extension du manifeste sans que rien ne le dise.
        expect(mimeDe('.qqch')).toBe('application/octet-stream');
        expect(accepterDepuis(['.qqch', '.autre'])).toEqual({
            'application/octet-stream': ['.qqch', '.autre'],
        });
    });

    it('ne double pas une extension repetee', () => {
        expect(accepterDepuis(['.txt', '.txt'])).toEqual({ 'text/plain': ['.txt'] });
    });

    it('rend une carte VIDE sur une liste vide', () => {
        expect(accepterDepuis([])).toEqual({});
    });
});
