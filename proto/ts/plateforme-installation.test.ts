// The INSTALLATION messages, on the TypeScript side.
//
// 🔴 THIS FILE EXISTS FOR A GUARD THE VECTORS CANNOT
// TEST. `plateforme-vectors.json` is a set of ROUND TRIPS: it pins the
// strings both languages must produce and read back. It says nothing of
// what must be REFUSED — and the most fragile guard of v4 is precisely
// a refusal: a `termine` whose `motif` key is MISSING.

import { describe, expect, it } from 'vitest';
import {
    PLATEFORME_VERSION,
    encodeTermine,
    parseVersLaPlateforme,
    type Issue,
} from './plateforme';

/** The reference `termine`, from which each case below removes one thing. */
function termineComplet(): Record<string, unknown> {
    return {
        type: 'termine',
        v: PLATEFORME_VERSION,
        installation: 'i-1',
        issue: 'reussie' as Issue,
        motif: null,
        code_sortie: 0,
        journal: '',
        journal_tronque: false,
    };
}

describe('`termine`: the optional fields are REQUIRED ON THE WIRE', () => {
    it('reads a complete `termine`, reason and code at null', () => {
        const lu = parseVersLaPlateforme(JSON.stringify(termineComplet()));
        expect(lu).toEqual({ ok: true, message: termineComplet() });
    });

    // 🔴 THE GUARD THAT COUNTS, AND IT IS THE EXACT TWIN OF
    // `champs::option_obligatoire` ON THE RUST SIDE. `serde_derive` treats every
    // `Option<T>` field as carrying an IMPLICIT `#[serde(default)]`, and
    // `JSON.parse` returns `undefined` for "absent key" as well as for
    // "key set to undefined". Without a guard, a `termine` from an EARLIER version —
    // which does not have these fields — would be accepted with a silently
    // absent reason: it is the precise disguise the version bump exists to
    // prevent, and `deny_unknown_fields` can do nothing about it, looking as it does at
    // EXTRA fields.
    it.each(['motif', 'code_sortie'])('🔴 REFUSES a `termine` without the key `%s`', (cle) => {
        const ampute = termineComplet();
        delete ampute[cle];
        expect(parseVersLaPlateforme(JSON.stringify(ampute))).toEqual({
            ok: false,
            motif: 'forme',
        });
    });

    it('accepts `null` on these two keys, and tells them apart from absence', () => {
        const withNull = { ...termineComplet(), motif: null, code_sortie: null };
        const lu = parseVersLaPlateforme(JSON.stringify(withNull));
        expect(lu).toEqual({ ok: true, message: withNull });
    });

    // ⚠️ `encodeTermine` REQUIRES `| null`, NOT `?`, and it is the only thing that
    // prevents the omission from being writable: `JSON.stringify` OMITS an
    // `undefined` and WRITES a `null`. An encoder with an optional parameter
    // would therefore produce a string the Rust twin would refuse — and the only
    // symptom would be a `forme` refusal very far from its cause.
    it('🔴 encodes `motif: null` by writing the key, never by omitting it', () => {
        const chain = encodeTermine('i-1', 'reussie', null, null, '', false);
        expect(chain).toContain('"motif":null');
        expect(chain).toContain('"code_sortie":null');
        expect(parseVersLaPlateforme(chain).ok).toBe(true);
    });

    it('refuses a non-integer exit code', () => {
        expect(
            parseVersLaPlateforme(
                JSON.stringify({ ...termineComplet(), code_sortie: 1.5 }),
            ),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    // 🔴 A NEGATIVE EXIT CODE IS LEGITIMATE on Windows: failure `HRESULT`s
    // have the high bit set, and read as a signed `i32`. Refusing
    // it would confuse "non-standard code" with "ordinary failure".
    it('accepts a NEGATIVE exit code', () => {
        const negatif = { ...termineComplet(), code_sortie: -1073741510 };
        expect(parseVersLaPlateforme(JSON.stringify(negatif))).toEqual({
            ok: true,
            message: negatif,
        });
    });
});

describe('`progression`: the counts are natural integers', () => {
    function progression(sur: Record<string, unknown> = {}): Record<string, unknown> {
        return {
            type: 'progression',
            v: PLATEFORME_VERSION,
            installation: 'i-1',
            phase: 'transfert',
            octets_faits: 1,
            octets_total: 2,
            ecoule_ms: 3,
            ...sur,
        };
    }

    it('reads a well-formed progress', () => {
        expect(parseVersLaPlateforme(JSON.stringify(progression()))).toEqual({
            ok: true,
            message: progression(),
        });
    });

    // ⚠️ `typeof x === 'number'` IS NOT ENOUGH: it lets `NaN`,
    // `Infinity` and `1.5` through. A `NaN` would go through to the database, where it
    // would become a `NULL` in a `NOT NULL` column — that is, an
    // SQL error very far from its cause.
    it.each([
        ['a negative count', { octets_faits: -1 }],
        ['a fractional count', { octets_total: 1.5 }],
        ['an unknown phase', { phase: 'empreinte' }],
        ['an empty installation', { installation: '' }],
    ])('🔴 REFUSES %s', (_nom, sur) => {
        expect(parseVersLaPlateforme(JSON.stringify(progression(sur)))).toEqual({
            ok: false,
            motif: 'forme',
        });
    });

    // 🔴 `empreinte` IS NOT A PHASE OF THIS CHANNEL, and the case above
    // pins it: it takes place in the BROWSER, before the platform has
    // the slightest row to write. Accepting it there would suggest the agent
    // can report it.
    it('accepts an `execution` phase without a total', () => {
        const sansTotal = progression({ phase: 'execution', octets_faits: 0, octets_total: 0 });
        expect(parseVersLaPlateforme(JSON.stringify(sansTotal))).toEqual({
            ok: true,
            message: sansTotal,
        });
    });
});
