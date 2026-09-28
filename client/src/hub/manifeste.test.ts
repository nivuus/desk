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
    const write = (d: number, v: number) => {
        o[d] = (v >>> 24) & 0xff;
        o[d + 1] = (v >>> 16) & 0xff;
        o[d + 2] = (v >>> 8) & 0xff;
        o[d + 3] = v & 0xff;
    };
    write(16, cote);
    write(20, cote);
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
    if (trouve === null) throw new Error(`tokens/couleurs.css no longer declares ${nom}`);
    return trouve[1].trim();
}
const FOND = tokenSombre('--fond-0');
const ACCENT = tokenSombre('--accent');

describe('batirManifeste', () => {
    it('returns ABSOLUTE URLs — the constraint the P0 gate measured', () => {
        const m = batirManifeste(APP, 'https://exemple.test', FOND);
        // 🔴 SI CES TROIS-LÀ REDEVIENNENT RELATIVES, le manifeste `blob:` est
        //    refusé par Chromium et l'application cesse d'être installable.
        //    Mesuré 2 exécutions, sonde `f` de `instrument/porte-p0.mjs`.
        expect(m.start_url).toBe('https://exemple.test/?app=u-1');
        expect(m.scope).toBe('https://exemple.test/');
        expect(m.id).toBe('https://exemple.test/shell.html?app=u-1');
        for (const url of [m.start_url, m.scope, m.id]) {
            expect(url.startsWith('https://')).toBe(true);
        }
    });

    it('tolerates an origin with a trailing slash without doubling the slash', () => {
        const m = batirManifeste(APP, 'https://exemple.test/', FOND);
        expect(m.scope).toBe('https://exemple.test/');
        expect(m.start_url).toBe('https://exemple.test/?app=u-1');
    });

    it("escapes the identifier in start_url and id", () => {
        const m = batirManifeste({ id: 'a/b?c', nom: 'X' }, 'https://x', FOND);
        expect(m.start_url).toBe('https://x/?app=a%2Fb%3Fc');
        expect(m.id).toBe('https://x/shell.html?app=a%2Fb%3Fc');
    });

    it('gives TWO applications DISTINCT `id`s under a SHARED `scope`', () => {
        const a = batirManifeste({ id: 'u-1', nom: 'A' }, 'https://x', FOND);
        const b = batirManifeste({ id: 'u-2', nom: 'B' }, 'https://x', FOND);
        expect(a.id).not.toBe(b.id);
        expect(a.scope).toBe(b.scope);
    });

    it('sets display_override with the Window Controls Overlay first', () => {
        expect(batirManifeste(APP, 'https://x', FOND).display_override).toEqual([
            'window-controls-overlay',
            'standalone',
        ]);
    });

    it("OMITS theme_color when no accent is known, rather than making one up", () => {
        const m = batirManifeste(APP, 'https://x', FOND);
        expect('theme_color' in m).toBe(false);
    });

    it("sets theme_color when an accent is given", () => {
        expect(batirManifeste({ ...APP, accent: ACCENT }, 'https://x', FOND).theme_color).toBe(ACCENT);
    });

    it("has NO icon when none is supplied", () => {
        expect(batirManifeste(APP, 'https://x', FOND).icons).toEqual([]);
    });

    it("carries the icon as data: and declares the side READ FROM ITS BYTES", () => {
        const m = batirManifeste({ ...APP, icone: pngDe(256) }, 'https://x', FOND);
        expect(m.icons).toHaveLength(1);
        expect(m.icons[0].sizes).toBe('256x256');
        expect(m.icons[0].type).toBe('image/png');
        expect(m.icons[0].purpose).toBe('any');
        expect(m.icons[0].src.startsWith('data:image/png;base64,')).toBe(true);
    });

    it('declares 128x128 on a 128 PNG — what the RED of criterion ① requires', () => {
        // 🔴 LE DÉFAUT QUE LA RECETTE A TROUVÉ : un premier jet prenait la
        //    taille de l'APPELANT, qui la laissait à 256 par défaut, et le
        //    manifeste du témoin annonçait donc `256x256` en portant un PNG de
        //    128. Chromium l'a attrapé (`no-acceptable-icon`) — mais un
        //    manifeste qui ment sur ce qu'il porte est un défaut même rattrapé.
        expect(batirManifeste({ ...APP, icone: pngDe(128) }, 'https://x', FOND).icons[0].sizes).toBe('128x128');
    });

    it("ignores an EMPTY icon rather than declaring an entry without an image", () => {
        expect(batirManifeste({ ...APP, icone: new Uint8Array([]) }, 'https://x', FOND).icons).toEqual([]);
    });

    it("ASSERTS NOTHING about bytes that are not a PNG: no icon declared", () => {
        // Poser `256x256` par défaut serait affirmer ce qu'on ne sait pas.
        expect(batirManifeste({ ...APP, icone: new Uint8Array([1, 2, 3]) }, 'https://x', FOND).icons).toEqual([]);
    });
});

describe('start_url and id DIVERGE since 31 August 2026', () => {
    it('start_url leads to the ROOT, where the hub holds the session', () => {
        const m = batirManifeste({ id: 'u-1', nom: 'x' }, 'https://exemple.test', FOND);
        expect(m.start_url).toBe('https://exemple.test/?app=u-1');
    });

    it('id DOES NOT MOVE: it carries the identity of the installed PWA', () => {
        // 🔴 CHANGER `id` N EST PAS UNE MISE A JOUR : c est une SECONDE
        // application, la premiere devenant orpheline. Et comme le manifeste est
        // publie en blob:, une PWA installee ne le relit JAMAIS -- elle
        // continuera d ouvrir shell.html, que la redirection rattrape.
        const m = batirManifeste({ id: 'u-1', nom: 'x' }, 'https://exemple.test', FOND);
        expect(m.id).toBe('https://exemple.test/shell.html?app=u-1');
    });

    it('id and start_url now DIVERGE, and that is intended', () => {
        const m = batirManifeste({ id: 'u-1', nom: 'x' }, 'https://exemple.test', FOND);
        expect(m.id === m.start_url).toBe(false);
    });

    it('start_url stays INSIDE the scope, a condition Chromium checks', () => {
        const m = batirManifeste({ id: 'u-1', nom: 'x' }, 'https://exemple.test', FOND);
        expect(m.start_url.startsWith(m.scope)).toBe(true);
    });

    it('the application identifier stays encoded in BOTH', () => {
        const m = batirManifeste({ id: 'a/b?c', nom: 'x' }, 'https://x', FOND);
        expect(m.start_url).toBe('https://x/?app=a%2Fb%3Fc');
        expect(m.id).toBe('https://x/shell.html?app=a%2Fb%3Fc');
    });
});

describe('versDataUrl', () => {
    it('encodes in base64 without a dependency', () => {
        // « PNG\r » — les quatre premiers octets d'un vrai PNG.
        expect(versDataUrl(new Uint8Array([0x89, 0x50, 0x4e, 0x47]))).toBe(
            'data:image/png;base64,iVBORw==',
        );
    });

    it('encodes a buffer longer than the packet without overflowing the stack', () => {
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

describe('the background colour', () => {
    it("is a PARAMETER, and the manifest returns EXACTLY what it is given", () => {
        // 🔴 IL N'Y A PLUS DE CONSTANTE DE COULEUR À CONFRONTER : `manifeste.ts`
        //    n'en porte aucune, et c'est la page qui lit le thème vivant par
        //    `getComputedStyle`. Ce test éprouve donc ce qui reste éprouvable —
        //    que la valeur traverse sans être réécrite —, et il le fait avec
        //    une valeur LUE sur `tokens.css`, jamais écrite ici.
        expect(batirManifeste(APP, 'https://x', FOND).background_color).toBe(FOND);
    });

    it("does not confuse the background and the accent", () => {
        const m = batirManifeste({ ...APP, accent: ACCENT }, 'https://x', FOND);
        expect(m.background_color).toBe(FOND);
        expect(m.theme_color).toBe(ACCENT);
        expect(FOND).not.toBe(ACCENT);
    });
});

describe('cotePng', () => {
    it("reads the side in the IHDR", () => {
        expect(cotePng(pngDe(256))).toBe(256);
        expect(cotePng(pngDe(128))).toBe(128);
    });

    it("returns undefined on a signature that is not a PNG one", () => {
        const faux = pngDe(256);
        faux[1] = 0x00;
        expect(cotePng(faux)).toBeUndefined();
    });

    it("returns undefined when the first chunk is not IHDR", () => {
        const faux = pngDe(256);
        faux[12] = 0x58;
        expect(cotePng(faux)).toBeUndefined();
    });

    it('returns undefined on a buffer too short to carry a header', () => {
        expect(cotePng(new Uint8Array([0x89, 0x50, 0x4e, 0x47]))).toBeUndefined();
    });

    it("returns undefined on a NON-SQUARE image rather than describing a false one", () => {
        // ⚠️ Le manifeste emploie la largeur pour les DEUX dimensions : une
        //    image non carrée y serait mal décrite. Que le magasin n'en produise
        //    que des carrées est une propriété de l'AGENT, pas de ce module.
        const rect = pngDe(256);
        rect[23] = 0x80; // hauteur 128, largeur 256
        expect(cotePng(rect)).toBeUndefined();
    });

    it('reads a size over FOUR bytes, not only over the last one', () => {
        // 4096 = 0x1000 : le troisième octet porte l'information.
        expect(cotePng(pngDe(4096))).toBe(4096);
    });
});

describe('the per-application file_handlers (slice F)', () => {
    it("OMITS `file_handlers` when the application opens nothing", () => {
        // ⚠️ Le cas le plus fréquent. Déclarer un handler qui n'accepte rien
        //    serait une entrée sans objet, et Chromium ANALYSE ce membre.
        expect('file_handlers' in batirManifeste(APP, 'https://x', FOND)).toBe(false);
        expect(
            'file_handlers' in batirManifeste({ ...APP, associations: [] }, 'https://x', FOND),
        ).toBe(false);
    });

    it("sets an `action` INSIDE THE SCOPE, which Chromium requires", () => {
        // 🔴 MESURÉ : une `action` hors scope fait rendre à Chromium
        //    « property 'action' ignored, should be within scope of the
        //    manifest. » puis « FileHandler ignored. » — c'est la sonde qui a
        //    établi que Chromium analyse bel et bien ce membre.
        const m = batirManifeste({ ...APP, associations: ['.txt'] }, 'https://x', FOND);
        expect(m.file_handlers).toHaveLength(1);
        expect(m.file_handlers![0].action.startsWith(m.scope)).toBe(true);
    });

    it('GROUPS the extensions that share a MIME', () => {
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

describe('mimeDe and accepterDepuis', () => {
    it('returns the known type of the extensions in the table', () => {
        expect(mimeDe('.msi')).toBe('application/x-msi');
        expect(mimeDe('.exe')).toBe('application/vnd.microsoft.portable-executable');
        expect(mimeDe('.bat')).toBe('application/x-bat');
    });

    it('folds the case', () => {
        expect(mimeDe('.TXT')).toBe('text/plain');
    });

    it("returns the UNKNOWN-bytes type rather than omitting the entry", () => {
        // ⚠️ `application/octet-stream` est le type HONNÊTE pour « des octets
        //    dont on ne sait rien ». Omettre l'entrée ferait disparaître
        //    l'extension du manifeste sans que rien ne le dise.
        expect(mimeDe('.qqch')).toBe('application/octet-stream');
        expect(accepterDepuis(['.qqch', '.autre'])).toEqual({
            'application/octet-stream': ['.qqch', '.autre'],
        });
    });

    it('does not double a repeated extension', () => {
        expect(accepterDepuis(['.txt', '.txt'])).toEqual({ 'text/plain': ['.txt'] });
    });

    it('returns an EMPTY map on an empty list', () => {
        expect(accepterDepuis([])).toEqual({});
    });
});
