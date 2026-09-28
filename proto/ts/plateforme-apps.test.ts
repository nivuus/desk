/**
 * Tests of the TypeScript mirror of the platform channel — APPLICATION MANAGEMENT.
 *
 * Extracted from `proto/ts/plateforme.test.ts` VERBATIM (sub-block G2, task 2):
 * that file was at 512 lines and appeared in the debt table of `CLAUDE.md`
 * WITH NO LANDING POINT. Sub-block G2 works in it, so it split it —
 * the rule of the repository is that retroactive splitting happens when one
 * works in the file, not as a separate workstream.
 *
 * **No test was added, removed or rewritten by this move.** The
 * count of `npx vitest run` was announced BEFORE being measured: 142 tests,
 * 5 files before, 142 tests and 6 files after. ⚠️ The FILE count
 * changes, the TEST count does not — say which one is announced (P3's trap).
 *
 * The boundary is the exact mirror of the Rust one: the lifecycle stays
 * in `plateforme.test.ts`, application management comes here. ⚠️ The `describe`
 * "the protocol version, and the allow-lists DERIVED from the union" STAYED in
 * `plateforme.test.ts`: it tests the version constant and the two allow-lists
 * of the whole union, not the catalogue.
 */
import { describe, expect, it } from 'vitest';
import {
    PLATEFORME_VERSION,
    parseDepuisLaPlateforme,
    parseVersLaPlateforme,
    type Application,
    encodeCatalogue,
    encodeIconesManquantes,
    encodeLancee,
    encodeLancer,
} from './plateforme';

// ---------------------------------------------------------------------------
// Sub-block G1 — application catalogue and launch (v2).
// ---------------------------------------------------------------------------

const APP_TEMOIN: Application = {
    cle: 'a1b2',
    nom: 'Bloc-notes',
    chemin: 'C:\\Users\\u\\Desktop\\Bloc-notes.lnk',
    cible: 'c:\\windows\\system32\\notepad.exe',
    arguments: '',
    repertoire: 'c:\\windows\\system32',
    icone: 'a1b2'.repeat(16),
    source_max: { pixels: 256 },
    // ⚠️ BOTH ARE FILLED IN THE WITNESS, AND NOT LEFT AT THEIR NEUTRAL
    // VALUE: an encoder that OMITTED one of the two would return the same JSON
    // as a witness where they were `null` and `[]`, and the round trip
    // could not see it. The neutral case is tested separately.
    accent: '#3f2a7a',
    associations: ['.txt', '.log'],
};

/**
 * 🔴 THE CASE THAT TRAPS `CHAMPS_APPLICATION`. An application whose icon
 * extraction failed carries `icone: null` — and `null` is not a string.
 * Adding `'icone'` to it would therefore make EVERY catalogue containing one
 * refused, with the `forme` reason, silently.
 */
const APP_SANS_ICONE: Application = {
    ...APP_TEMOIN,
    icone: null,
    source_max: 'non-mesuree',
    // Without an icon, there is no dominant colour to compute: `accent` follows.
    accent: null,
    associations: [],
};

const CATALOGUE_TEMOIN =
    '{"type":"catalogue","v":5,"complet":true,"applications":[{"cle":"a1b2",'
    + '"nom":"Bloc-notes","chemin":"C:\\\\Users\\\\u\\\\Desktop\\\\Bloc-notes.lnk",'
    + '"cible":"c:\\\\windows\\\\system32\\\\notepad.exe","arguments":"",'
    + '"repertoire":"c:\\\\windows\\\\system32",'
    + '"icone":"a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2",'
    + '"source_max":{"pixels":256},"accent":"#3f2a7a",'
    + '"associations":[".txt",".log"]}],"disparues":["disparue-1"]}';

describe('the catalogue and the launch, AGENT -> PLATFORM direction', () => {
    it('encodes `catalogue` exactly like Rust', () => {
        // 🔴 The field order is `type` THEN `v`: serde emits the internal tag
        // first, `JSON.stringify` respects insertion order, and the
        // vector pins the string byte for byte. Writing `v` first — which
        // `control.ts` does — would produce a different string, and the
        // divergence was found by this test in P3, not by rereading.
        expect(encodeCatalogue(true, [APP_TEMOIN], ['disparue-1'])).toBe(CATALOGUE_TEMOIN);
    });

    it('encodes `lancee` exactly like Rust', () => {
        expect(encodeLancee('d-7', 'raccourci')).toBe(
            '{"type":"lancee","v":5,"demande":"d-7","issue":"raccourci"}',
        );
    });

    it('reads a well-formed `catalogue`, and finds each of its fields', () => {
        const lu = parseVersLaPlateforme(CATALOGUE_TEMOIN);
        expect(lu.ok).toBe(true);
        if (!lu.ok) return;
        expect(lu.message.type).toBe('catalogue');
        if (lu.message.type !== 'catalogue') return;
        expect(lu.message.complet).toBe(true);
        expect(lu.message.disparues).toEqual(['disparue-1']);
        expect(lu.message.applications).toEqual([APP_TEMOIN]);
    });

    it('🔴 REJECTS a `catalogue` whose `applications` is not an array, reason `forme`', () => {
        // 🔴 It is the ONLY parser of the file whose bytes come from a
        // third party. Without this guard, an absent or scalar `applications`
        // would go through to the SQL query. And returning `enrolement` would make
        // the peer read "wrong secret" for a perfectly
        // authenticated message: the reason designates the cause, it does not disguise it.
        expect(
            parseVersLaPlateforme('{"type":"catalogue","v":5,"complet":true,"applications":3,"disparues":[]}'),
        ).toEqual({ ok: false, motif: 'forme' });
        expect(
            parseVersLaPlateforme('{"type":"catalogue","v":5,"complet":true,"disparues":[]}'),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    it('🔴 REJECTS a `catalogue` with an incomplete application, reason `forme`', () => {
        expect(
            parseVersLaPlateforme(
                '{"type":"catalogue","v":5,"complet":true,"applications":[{"cle":"a"}],"disparues":[]}',
            ),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    it('🔴 REJECTS a `lancee` whose outcome is unknown, reason `forme`', () => {
        expect(
            parseVersLaPlateforme('{"type":"lancee","v":5,"demande":"d","issue":"peut-etre"}'),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    it('reads a well-formed `lancee`', () => {
        const lu = parseVersLaPlateforme('{"type":"lancee","v":5,"demande":"d-7","issue":"echec"}');
        expect(lu).toEqual({
            ok: true,
            message: { type: 'lancee', v: PLATEFORME_VERSION, demande: 'd-7', issue: 'echec' },
        });
    });
});

describe('the launch, PLATFORM -> AGENT direction', () => {
    it('encodes `lancer` exactly like Rust', () => {
        expect(encodeLancer('d-7', 'a1b2')).toBe(
            '{"type":"lancer","v":5,"demande":"d-7","cle":"a1b2"}',
        );
    });

    it('re-reads a `lancer`', () => {
        // 🔴 The red: omitting it from `TYPES_DEPUIS`. The parser would throw
        // "unknown platform message type" on a valid order.
        const lu = parseDepuisLaPlateforme(
            '{"type":"lancer","v":5,"demande":"d-7","cle":"a1b2"}',
        ) as unknown as Record<string, unknown>;
        expect(lu.type).toBe('lancer');
        expect(lu.demande).toBe('d-7');
        expect(lu.cle).toBe('a1b2');
    });
});

// ---------------------------------------------------------------------------
// Sub-block G2 — the icons, their provenance, and the inventory of missing ones.
// ---------------------------------------------------------------------------

describe('both icon fields go through the parser', () => {
    it('🔴 ACCEPTS a catalogue in which an application has NO icon', () => {
        // 🔴 IT IS THE RED THAT COUNTS, AND ITS STATE IS SILENT IN
        // PRODUCTION. Adding `'icone'` to `CHAMPS_APPLICATION` — the naive
        // gesture — would make this catalogue refused for `forme`, that is a
        // WHOLE catalogue lost without any trace stating the reason.
        const brut = encodeCatalogue(true, [APP_SANS_ICONE], []);
        const lu = parseVersLaPlateforme(brut);
        expect(lu.ok).toBe(true);
        if (!lu.ok || lu.message.type !== 'catalogue') throw new Error('forme');
        expect(lu.message.applications[0].icone).toBeNull();
        expect(lu.message.applications[0].source_max).toBe('non-mesuree');
    });

    it('accepts an application WITH an icon, and returns `source_max` as an object', () => {
        const lu = parseVersLaPlateforme(encodeCatalogue(true, [APP_TEMOIN], []));
        if (!lu.ok || lu.message.type !== 'catalogue') throw new Error('forme');
        expect(lu.message.applications[0].source_max).toEqual({ pixels: 256 });
    });

    it('🔴 REFUSES an application that LACKS one of the two new fields', () => {
        // The ABSENT field is the form in which a v2 agent would
        // present itself. It must be refused, not completed.
        const sansIcone = JSON.stringify({ ...APP_TEMOIN, icone: undefined });
        const sansSource = JSON.stringify({ ...APP_TEMOIN, source_max: undefined });
        for (const app of [sansIcone, sansSource]) {
            const brut = `{"type":"catalogue","v":5,"complet":true,"applications":[${app}],"disparues":[]}`;
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });

    it('🔴 REFUSES a `source_max` that is neither the string nor the exact shape', () => {
        for (const value of [
            '"gros"',
            '{"pixels":"gros"}',
            '{"pixels":256,"bonus":1}',
            '{}',
            '256',
            'null',
            '"non_mesuree"',
        ]) {
            const app = JSON.stringify(APP_TEMOIN).replace('{"pixels":256}', value);
            const brut = `{"type":"catalogue","v":5,"complet":true,"applications":[${app}],"disparues":[]}`;
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });

    it('🔴 REFUSES an `icone` that is neither `null` nor a string', () => {
        for (const value of ['42', 'true', '{}', '[]']) {
            const app = JSON.stringify(APP_TEMOIN).replace(
                '"a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2"',
                value,
            );
            const brut = `{"type":"catalogue","v":5,"complet":true,"applications":[${app}],"disparues":[]}`;
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });
});

describe('`icones-manquantes`, PLATFORM -> AGENT direction', () => {
    it('encodes exactly like Rust', () => {
        expect(encodeIconesManquantes(['a1b2', 'c3d4'])).toBe(
            '{"type":"icones-manquantes","v":5,"empreintes":["a1b2","c3d4"]}',
        );
    });

    it('re-reads itself, and REFUSES a diverging version', () => {
        const lu = parseDepuisLaPlateforme(
            '{"type":"icones-manquantes","v":5,"empreintes":["a1b2"]}',
        );
        if (lu.type !== 'icones-manquantes') throw new Error('type');
        expect(lu.empreintes).toEqual(['a1b2']);
        // 🔴 THE DIVERGING VERSION IS DERIVED. It was `4` hardcoded — "the
        // next one" as it read at G2's time —, and G3's bump
        // made it ours: this test then claimed that OUR version is
        // refused. It failed loudly, which is the right behaviour, but
        // it would have done it again at the next bump.
        expect(() =>
            parseDepuisLaPlateforme(
                `{"type":"icones-manquantes","v":${PLATEFORME_VERSION + 1},"empreintes":[]}`,
            ),
        ).toThrow(/unsupported platform version/);
    });
});
