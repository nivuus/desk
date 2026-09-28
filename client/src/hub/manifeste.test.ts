import { describe, expect, it } from 'vitest';
import tokensCss from '../design/tokens/couleurs.css?raw';
import { accepterDepuis, batirManifeste, cotePng, mimeDe, versDataUrl, type Sujet } from './manifeste';

/// A real PNG of a given side — signature, IHDR, and nothing else. It is enough for
/// `cotePng`, which only reads the header, and it is BUILT rather than pasted as
/// base64: an opaque literal would not say what it carries.
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

/// 🔴 NO COLOUR IS WRITTEN IN THIS FILE, AND IT IS §7.2 THAT REQUIRES IT:
/// its scan covers the `.ts` as much as the `.css`, and its exclusion only
/// covers `client/src/design/*.test.ts` — not this file. A first
/// draft put two literals here and the check reported them. **Widening its
/// exclusion would have satisfied the check by EMPTYING it**; the values are
/// therefore READ from `tokens/couleurs.css` (`tokens.css` before the extraction of
/// task 6, August 25th, 2026), which is a better test: it would go red
/// too the day the token changed value without this file moving.
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
        // 🔴 IF THESE THREE BECOME RELATIVE AGAIN, the `blob:` manifest is
        //    refused by Chromium and the application stops being installable.
        //    Measured over 2 runs, probe `f` of `instrument/porte-p0.mjs`.
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
        // 🔴 THE DEFECT ACCEPTANCE FOUND: a first draft took the
        //    size from the CALLER, which left it at 256 by default, and the
        //    witness's manifest therefore announced `256x256` while carrying a PNG of
        //    128. Chromium caught it (`no-acceptable-icon`) — but a
        //    manifest that lies about what it carries is a defect even when caught.
        expect(batirManifeste({ ...APP, icone: pngDe(128) }, 'https://x', FOND).icons[0].sizes).toBe('128x128');
    });

    it("ignores an EMPTY icon rather than declaring an entry without an image", () => {
        expect(batirManifeste({ ...APP, icone: new Uint8Array([]) }, 'https://x', FOND).icons).toEqual([]);
    });

    it("ASSERTS NOTHING about bytes that are not a PNG: no icon declared", () => {
        // Setting `256x256` by default would be claiming what we do not know.
        expect(batirManifeste({ ...APP, icone: new Uint8Array([1, 2, 3]) }, 'https://x', FOND).icons).toEqual([]);
    });
});

describe('start_url and id DIVERGE since 31 August 2026', () => {
    it('start_url leads to the ROOT, where the hub holds the session', () => {
        const m = batirManifeste({ id: 'u-1', nom: 'x' }, 'https://exemple.test', FOND);
        expect(m.start_url).toBe('https://exemple.test/?app=u-1');
    });

    it('id DOES NOT MOVE: it carries the identity of the installed PWA', () => {
        // 🔴 CHANGING `id` IS NOT AN UPDATE: it is a SECOND
        // application, the first one becoming orphaned. And since the manifest is
        // published as blob:, an installed PWA NEVER reads it again -- it
        // will keep opening shell.html, which the redirect catches.
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
        // "PNG\r" — the first four bytes of a real PNG.
        expect(versDataUrl(new Uint8Array([0x89, 0x50, 0x4e, 0x47]))).toBe(
            'data:image/png;base64,iVBORw==',
        );
    });

    it('encodes a buffer longer than the packet without overflowing the stack', () => {
        // 0x2000 is the packet size: we clearly exceed it, as a
        // real 256×256 icon does.
        const gros = new Uint8Array(0x2000 * 3 + 7).fill(0x41);
        const url = versDataUrl(gros);
        expect(url.startsWith('data:image/png;base64,')).toBe(true);
        // Decoding returns EXACTLY what was encoded: that is what
        // exercises the splitting, and not only the absence of an exception.
        const decode = atob(url.slice('data:image/png;base64,'.length));
        expect(decode.length).toBe(gros.length);
        expect(decode.charCodeAt(0)).toBe(0x41);
        expect(decode.charCodeAt(decode.length - 1)).toBe(0x41);
    });
});

describe('the background colour', () => {
    it("is a PARAMETER, and the manifest returns EXACTLY what it is given", () => {
        // 🔴 THERE IS NO COLOUR CONSTANT LEFT TO CONFRONT: `manifeste.ts`
        //    carries none, and it is the page that reads the live theme through
        //    `getComputedStyle`. This test therefore exercises what remains testable —
        //    that the value goes through without being rewritten —, and it does so with
        //    a value READ from `tokens.css`, never written here.
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
        // ⚠️ The manifest uses the width for BOTH dimensions: a
        //    non-square image would be badly described there. That the store only produces
        //    square ones is a property of the AGENT, not of this module.
        const rect = pngDe(256);
        rect[23] = 0x80; // hauteur 128, largeur 256
        expect(cotePng(rect)).toBeUndefined();
    });

    it('reads a size over FOUR bytes, not only over the last one', () => {
        // 4096 = 0x1000: the third byte carries the information.
        expect(cotePng(pngDe(4096))).toBe(4096);
    });
});

describe('the per-application file_handlers (slice F)', () => {
    it("OMITS `file_handlers` when the application opens nothing", () => {
        // ⚠️ The most frequent case. Declaring a handler that accepts nothing
        //    would be a pointless entry, and Chromium PARSES this member.
        expect('file_handlers' in batirManifeste(APP, 'https://x', FOND)).toBe(false);
        expect(
            'file_handlers' in batirManifeste({ ...APP, associations: [] }, 'https://x', FOND),
        ).toBe(false);
    });

    it("sets an `action` INSIDE THE SCOPE, which Chromium requires", () => {
        // 🔴 MEASURED: an `action` outside the scope makes Chromium return
        //    "property 'action' ignored, should be within scope of the
        //    manifest." then "FileHandler ignored." — it is the probe that
        //    established that Chromium does parse this member.
        const m = batirManifeste({ ...APP, associations: ['.txt'] }, 'https://x', FOND);
        expect(m.file_handlers).toHaveLength(1);
        expect(m.file_handlers![0].action.startsWith(m.scope)).toBe(true);
    });

    it('GROUPS the extensions that share a MIME', () => {
        // 🔴 `.txt` and `.log` are both `text/plain`. One entry per
        //    extension would overwrite the previous one, and an application that opens
        //    both would only see one.
        // RED: `accept[mime] = [extension]` without grouping ⟹ only `.log`
        //    remains.
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
        // ⚠️ `application/octet-stream` is the HONEST type for "bytes
        //    we know nothing about". Omitting the entry would make the
        //    extension disappear from the manifest without anything saying so.
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
