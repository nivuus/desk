import { describe, expect, it } from 'vitest';
import vecteurs from '../plateforme-vectors.json';
import {
    PLATEFORME_VERSION,
    encodeInstaller,
    encodeProgression,
    encodeTermine,
    type Issue,
    type Phase,
    encodeEnroler,
    encodeBattement,
    encodeEnrole,
    encodeBattementRecu,
    encodeRefus,
    parseDepuisLaPlateforme,
    parseVersLaPlateforme,
    type MotifCanal,
    type Application,
    type IssueLancement,
    encodeCatalogue,
    encodeIconesManquantes,
    encodeLancee,
    encodeLancer,
    typesDepuis,
    typesVers,
} from './plateforme';

/**
 * Local typing of the cases: the JSON mixes fields specific to each `kind`,
 * all optional here since no case carries them all. Same choice as
 * `input.test.ts`, for the same reason — avoiding an imprecise union without
 * scattering `as any`.
 */
interface CasVecteur {
    name: string;
    sens: string;
    kind: string;
    installation?: string;
    url?: string;
    nom?: string;
    taille?: number;
    sha256?: string;
    phase?: string;
    octets_faits?: number;
    octets_total?: number;
    ecoule_ms?: number;
    code_sortie?: number | null;
    journal?: string;
    journal_tronque?: boolean;
    json: string;
    vm?: string;
    secret?: string;
    prefixe?: string;
    jeton?: string;
    expire_a?: number;
    motif?: string;
    complet?: boolean;
    applications?: Application[];
    disparues?: string[];
    demande?: string;
    issue?: string;
    cle?: string;
    empreintes?: string[];
}

const cas: CasVecteur[] = vecteurs.cases as CasVecteur[];

/**
 * 🔴 AN UNKNOWN `kind` THROWS, it does not fall back to a default. The dispatch
 * was a ternary whose last branch served as a catch-all: a
 * misspelled `kind` would have been encoded there by the wrong function, and the
 * only symptom would have been a wrong string — impossible to tell from a
 * real encoding divergence. It is the `panic!` of the Rust side, transposed.
 */
function encodeVers(c: CasVecteur): string {
    switch (c.kind) {
        case 'enroler': return encodeEnroler(c.vm!, c.secret!);
        case 'battement': return encodeBattement();
        case 'catalogue': return encodeCatalogue(c.complet!, c.applications!, c.disparues!);
        case 'lancee': return encodeLancee(c.demande!, c.issue as IssueLancement);
        case 'progression':
            return encodeProgression(
                c.installation!, c.phase as Phase,
                c.octets_faits!, c.octets_total!, c.ecoule_ms!,
            );
        case 'termine':
            return encodeTermine(
                c.installation!, c.issue as Issue,
                // ⚠️ `?? null` WOULD BE A MISTAKE HERE: it would confuse "the key
                // is missing from the vector" with "the key carries null", and a truncated
                // vector would still encode. The JSON tells them apart, and
                // `c.motif` is literally `null` in the cases that
                // carry it — it is what the untyped read returns.
                c.motif as string | null, c.code_sortie as number | null,
                c.journal!, c.journal_tronque!,
            );
        default: throw new Error(`unknown kind in the vers direction: ${c.kind}`);
    }
}

function encodeDepuis(c: CasVecteur): string {
    switch (c.kind) {
        case 'enrole': return encodeEnrole(c.prefixe!, c.jeton!, c.expire_a!);
        case 'battement-recu': return encodeBattementRecu(c.jeton!, c.expire_a!);
        case 'refus': return encodeRefus(c.motif as MotifCanal);
        case 'lancer': return encodeLancer(c.demande!, c.cle!);
        case 'icones-manquantes': return encodeIconesManquantes(c.empreintes!);
        case 'installer':
            return encodeInstaller(c.installation!, c.url!, c.nom!, c.taille!, c.sha256!);
        default: throw new Error(`unknown kind in the depuis direction: ${c.kind}`);
    }
}

describe('shared vectors of the platform channel', () => {
    it('🔴 declares the SAME version as the protocol', () => {
        // 🔴 The red: omitting it. It is exactly the gap that
        // `vectors.json` drags along on the Rust side — `input.rs` never checks
        // `doc["version"]` —, fixed here ON BOTH SIDES for the new file.
        expect(PLATEFORME_VERSION).toBe(vecteurs.version);
    });

    it('🔴 carries at least one case', () => {
        // 🔴 ANTI-TAUTOLOGY: an empty file would make the whole loop
        // below pass without testing anything. Same guard as `input.rs:326` and as
        // `sous-ensemble.test.ts`.
        expect(cas.length).toBeGreaterThan(0);
    });

    it.each(cas.filter((c) => c.sens === 'vers'))(
        'encodes « $name » exactly like the vector',
        (c) => {
            expect(encodeVers(c)).toBe(c.json);
        },
    );

    it.each(cas.filter((c) => c.sens === 'depuis'))(
        're-reads « $name » and finds each of its fields',
        (c) => {
            const lu = parseDepuisLaPlateforme(c.json) as unknown as Record<string, unknown>;
            expect(lu.type).toBe(c.kind);
            expect(lu.v).toBe(PLATEFORME_VERSION);
            // The fields specific to each variant, compared one by one: a
            // `toBe(c.kind)` alone would pass on a truncated message.
            for (const champ of ['prefixe', 'jeton', 'expire_a', 'motif', 'demande', 'cle'] as const) {
                if (c[champ] !== undefined) expect(lu[champ]).toBe(c[champ]);
            }
            // ⚠️ `empreintes` is an ARRAY: `toBe` would compare the
            // references there and would pass for the wrong reason on `undefined`.
            if (c.empreintes !== undefined) expect(lu.empreintes).toEqual(c.empreintes);
        },
    );

    it.each(cas.filter((c) => c.sens === 'depuis'))(
        '🔴 ENCODES « $name » exactly like the vector',
        (c) => {
            // 🔴 THIS CHECK WAS MISSING, and its absence was a real
            // asymmetry: Rust serialises BOTH directions and compares the exact
            // string (`plateforme.rs`), whereas TypeScript only READ BACK
            // the `depuis` direction. Yet it is the PLATFORM that emits those
            // messages, in TypeScript. Nobody therefore pinned the bytes
            // it really puts on the wire — a field order that
            // diverged from the Rust one would have shown nowhere.
            expect(encodeDepuis(c)).toBe(c.json);
        },
    );

    it('exercises BOTH directions, and no case is skipped', () => {
        // 🔴 Without this count, a misspelled `sens` would make cases disappear
        // silently: the two `it.each` above would simply yield fewer
        // tests, and nothing would say so. Same guard as on the Rust side.
        const vers = cas.filter((c) => c.sens === 'vers').length;
        const depuis = cas.filter((c) => c.sens === 'depuis').length;
        expect(vers).toBeGreaterThan(0);
        expect(depuis).toBeGreaterThan(0);
        expect(vers + depuis).toBe(cas.length);
    });
});

describe('TypeScript mirror of the platform channel', () => {
    it('encodes `enroler` exactly like Rust', () => {
        // The EXACT string, compared with the one `plateforme.rs` asserts on
        // its side. A divergence of one character and the two ends no longer
        // talk.
        expect(encodeEnroler('w1', 'chut'))
            .toBe('{"type":"enroler","v":5,"vm":"w1","secret":"chut"}');
    });

    it('encode `battement`', () => {
        expect(encodeBattement()).toBe('{"type":"battement","v":5}');
    });

    it('reads a well-formed `enrole`', () => {
        const m = parseDepuisLaPlateforme(
            '{"type":"enrole","v":5,"prefixe":"PPP","jeton":"jjj","expire_a":1787136773742}',
        );
        expect(m).toEqual({
            type: 'enrole', v: PLATEFORME_VERSION, prefixe: 'PPP', jeton: 'jjj', expire_a: 1787136773742,
        });
    });

    it('🔴 REJECTS a version PLATEFORME_VERSION + 1', () => {
        // 🔴 The red: only comparing `parsed.type`. The message would pass, and
        // it is the TypeScript half of criterion ③.
        //
        // ⚠️ THIS CASE CARRIED A `refus` UNTIL 20 AUGUST 2026. The refusal is
        // now the only variant outside versioning — see
        // `RefusMessage` —, so it can no longer carry this property. A
        // `lancer` carries it, and it is the message where it bites the most: a
        // future version could give `cle` an entirely different meaning.
        expect(() => parseDepuisLaPlateforme(
            `{"type":"lancer","v":${PLATEFORME_VERSION + 1},"demande":"d","cle":"c"}`,
        )).toThrow(/unsupported platform version/);
    });

    it('🔴 REJECTS an ABSENT version', () => {
        // 🔴 The red named by the plan is comparing with `!=` instead of
        // `!==`. ⚠️ IT DOES NOT TURN RED, and it is checked rather than assumed:
        // `undefined != 1` is `true` in JavaScript, so the refusal happens
        // anyway. The mutation that REALLY turns this test red is
        // the reverse — accepting the absence, for example `parsed.v ?? VERSION`.
        // The plan asked us to check it before declaring it: it is
        // done, and it was right to doubt.
        //
        // ⚠️ THE VEHICLE STAYS A `refus`, AND IT IS NOW THE BEST
        // PLACE FOR THIS TEST: since 20 August 2026 the refusal tolerates any
        // VALUE of `v`, and this assertion is what guards the boundary —
        // "any value" is not "no field at all".
        expect(() => parseDepuisLaPlateforme('{"type":"refus","motif":"version"}'))
            .toThrow(/platform version absent or not numeric/);
    });

    it('🔴 REJECTS a NULL version, which `?? ` would let through', () => {
        // Complement to the previous test: `null ?? 1` is `1`, so a
        // default-value implementation would accept this message without
        // anything else moving.
        expect(() => parseDepuisLaPlateforme('{"type":"refus","v":null,"motif":"version"}'))
            .toThrow(/platform version absent or not numeric/);
    });

    it('REJECTS an unknown `type`', () => {
        expect(() => parseDepuisLaPlateforme('{"type":"vol","v":5}'))
            .toThrow(/unknown platform message type/);
    });

    it('🔴 REJECTS a `type` of the AGENT -> PLATFORM direction', () => {
        // 🔴 The parser ONLY reads the platform -> agent direction. Accepting
        // `enroler` here would make an agent treat its own message as
        // an answer — a confusion of direction no other test would see.
        expect(() => parseDepuisLaPlateforme('{"type":"enroler","v":5,"vm":"w","secret":"s"}'))
            .toThrow(/unknown platform message type/);
    });
});

describe('the parser of the AGENT -> PLATFORM direction', () => {
    // ⚠️ THIS ONE READS WHAT A REAL THIRD PARTY WRITES. `parseDepuisLaPlateforme`
    // validates at the boundary then casts, because its emitter is the service
    // itself; here the emitter is a network peer, which has no reason
    // to be well behaved. The fields are therefore CHECKED, one by one.
    //
    // ⚠️ IT RETURNS A VERDICT, IT DOES NOT THROW, and that is what distinguishes it from
    // its twin: the platform must ANSWER a typed reason to the peer, not
    // merely fail.

    it('reads a well-formed `enroler`', () => {
        expect(parseVersLaPlateforme('{"type":"enroler","v":5,"vm":"w1","secret":"chut"}')).toEqual({
            ok: true,
            message: { type: 'enroler', v: PLATEFORME_VERSION, vm: 'w1', secret: 'chut' },
        });
    });

    it('reads a `battement`', () => {
        expect(parseVersLaPlateforme('{"type":"battement","v":5}')).toEqual({
            ok: true,
            message: { type: 'battement', v: PLATEFORME_VERSION },
        });
    });

    it('🔴 REJECTS a version PLATEFORME_VERSION + 1, reason `version`', () => {
        // 🔴 It is the TypeScript half of criterion ③ seen from the PLATFORM.
        // Only comparing `type` would let in a message from a future
        // version, whose fields could say something entirely different.
        expect(
            parseVersLaPlateforme(
                `{"type":"battement","v":${PLATEFORME_VERSION + 1}}`,
            ),
        ).toEqual({ ok: false, motif: 'version' });
        // 🔴 AND THE VERSION GOES BEFORE THE TYPE: a message that is bad
        // on BOTH counts must say `version`, not `forme`. Without this
        // assertion, the order of the two checks would be fixed by nothing, and the
        // peer that reads the reason to decide whether to UPDATE or
        // CORRECT itself would one day receive the wrong answer.
        expect(parseVersLaPlateforme(`{"type":"vol","v":${PLATEFORME_VERSION + 1}}`)).toEqual({
            ok: false,
            motif: 'version',
        });
    });

    it('🔴 REJECTS an ABSENT version and a NULL version', () => {
        // The real red is writing `parsed.v ?? PLATEFORME_VERSION`:
        // `undefined ?? 1` and `null ?? 1` both yield `1`. (The `!=` instead
        // of `!==` does NOT turn red — measured on the other parser.)
        expect(parseVersLaPlateforme('{"type":"battement"}')).toEqual({
            ok: false,
            motif: 'version',
        });
        expect(parseVersLaPlateforme('{"type":"battement","v":null}')).toEqual({
            ok: false,
            motif: 'version',
        });
    });

    it('🔴 REJECTS a `type` of the PLATFORM -> AGENT direction, reason `forme`', () => {
        // 🔴 The confusion of direction, guarded in BOTH directions. Accepting
        // `enrole` here would make the platform treat its own answer
        // as a request.
        for (const brut of [
            '{"type":"enrole","v":5,"prefixe":"P","jeton":"j","expire_a":1}',
            '{"type":"battement-recu","v":5,"jeton":"j","expire_a":1}',
            '{"type":"refus","v":5,"motif":"forme"}',
        ]) {
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });

    it('REJECTS what is not a JSON object, reason `forme`', () => {
        // `null` is the dangerous case: `null.type` THROWS, where a number or
        // a string would return `undefined`. Same guard as the relay's
        // `isJsonObject`, and for the same reason — an uncaught exception in a
        // `ws` `message` handler takes down the whole Node process.
        for (const brut of ['null', '"a string"', '42', '[]', 'not json']) {
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });

    it('🔴 REJECTS an `enroler` missing `vm` or `secret`, reason `forme`', () => {
        // 🔴 WITHOUT THIS CHECK, `undefined` would go through to the SQL
        // query: `lireParVm(p, undefined)` returns nothing on SQLite but is
        // not the same query on Postgres, and above all the refusal that would
        // come out of it would say `enrolement` — hence "wrong secret" — for a
        // message that never carried a VM.
        for (const brut of [
            '{"type":"enroler","v":5,"secret":"chut"}',
            '{"type":"enroler","v":5,"vm":"w1"}',
            '{"type":"enroler","v":5,"vm":"","secret":"chut"}',
            '{"type":"enroler","v":5,"vm":42,"secret":"chut"}',
        ]) {
            expect(parseVersLaPlateforme(brut)).toEqual({ ok: false, motif: 'forme' });
        }
    });
});


describe('the protocol version, and the allow-lists DERIVED from the union', () => {
    // ⚠️ THE NUMBER IS WRITTEN ONLY ONCE, in the assertion below. It
    // also lived in the title of the `describe` and in that of the `it`, which
    // announced "version 3": three places for a single fact, two of which
    // no test could turn red. G3's bump found them
    // stale — they said "3" on a protocol at 4.
    it('🔴 announces its version, and REFUSES a v1 message', () => {
        // 🔴 Forgetting the bump on the TypeScript side would make the two ends diverge
        // SILENTLY: the Rust would emit `v:3`, this parser would expect `v:2`, and
        // only the shared vectors would say so.
        // 🔴 DELIBERATE LITERAL, AND IT IS A TRIPWIRE: `toBe(PLATEFORME_VERSION)`
        // would be a tautology. This number exists so that a bump FORCES a
        // human hand to come by here, and the comment above says why.
        // ⚠️ BUMPED FROM 4 TO 5 BY SUB-BLOCK G5 (slice F), and THE TRIPWIRE
        // WORKED: this test failed, which forced a hand to come here
        // and acknowledge the bump rather than suffer it. It is the third bump
        // this literal catches.
        expect(PLATEFORME_VERSION).toBe(5);
        // ⚠️ THIS CASE CARRIED A `refus` UNTIL 20 AUGUST 2026, and it pinned
        // the defect instead of guarding against it: the refusal is now the ONLY
        // variant outside versioning, precisely so that a v1 agent can
        // read the one that tells it it is out of date. The property guarded here
        // — "a v1 message is refused" — remains tested, on a variant that
        // still carries it.
        expect(() =>
            parseDepuisLaPlateforme('{"type":"enrole","v":1,"prefixe":"P","jeton":"j","expire_a":1}'),
        ).toThrow(/unsupported platform version/);
        expect(parseVersLaPlateforme('{"type":"battement","v":1}')).toEqual({
            ok: false,
            motif: 'version',
        });
    });

    it('🔴 both allow-lists cover EXACTLY their union', () => {
        // 🔴 STRUCTURAL REMEDY, AND THIS IS ITS HALF OBSERVABLE AT RUNTIME.
        // The other half is the typecheck: `ALL_FROM` and `ALL_TO` are
        // `Record<Union['type'], true>`, and `tsc` refuses a
        // missing key. It is the twin of `TYPES_AGENT` (`control.ts`), written
        // by hand, which nothing confronts with its union — and whose omission
        // breaks "neither compilation nor tests". Here the omission breaks the typecheck.
        expect([...typesDepuis()].sort()).toEqual(
            ['battement-recu', 'enrole', 'icones-manquantes', 'installer', 'lancer', 'refus'],
        );
        expect([...typesVers()].sort()).toEqual([
            'battement', 'catalogue', 'enroler', 'lancee', 'progression', 'termine',
        ]);
    });
});

// ---------------------------------------------------------------------------
// Fix of 20 August 2026 — A REFUSAL MUST BE READABLE BY ITS RECIPIENT.
// ---------------------------------------------------------------------------

describe('the refusal is an envelope OUTSIDE versioning', () => {
    // 🔴 THE RED OF DEFECT 2, on the mirror side. Measured at G1's acceptance: a v1 agent
    // facing a v2 platform cannot read the refusal that tells it WHY
    // it is refused, because the version check also applies to the
    // refusal. Both ends must tolerate it, otherwise the mirror would diverge
    // from the Rust silently.
    it('🔴 is read whatever the version of its sender', () => {
        const futur = parseDepuisLaPlateforme('{"type":"refus","v":97,"motif":"version"}') as
            unknown as Record<string, unknown>;
        expect(futur.type).toBe('refus');
        expect(futur.motif).toBe('version');
        const passe = parseDepuisLaPlateforme('{"type":"refus","v":1,"motif":"enrolement"}') as
            unknown as Record<string, unknown>;
        expect(passe.motif).toBe('enrolement');
    });

    it('🔴 keeps a reason that no version of this repository knows', () => {
        const lu = parseDepuisLaPlateforme('{"type":"refus","v":98,"motif":"quota-depasse"}') as
            unknown as Record<string, unknown>;
        expect(lu.motif).toBe('quota-depasse');
    });

    it('🔴 and the OTHER types stay refused on a diverging version', () => {
        // Without this half, the tolerance above could be obtained by no longer
        // checking anything at all.
        for (const brut of [
            '{"type":"enrole","v":97,"prefixe":"P","jeton":"j","expire_a":1}',
            '{"type":"battement-recu","v":97,"jeton":"j","expire_a":1}',
            '{"type":"lancer","v":97,"demande":"d","cle":"c"}',
        ]) {
            expect(() => parseDepuisLaPlateforme(brut)).toThrow(
                /unsupported platform version/,
            );
        }
    });
});

describe('the readable refusals of the shared vectors file', () => {
    // 🔴 THE SAME STRINGS AS `conformance_to_the_shared_readable_refusals` ON THE
    // RUST SIDE. Without this shared vector, the remedy could live on only one
    // side — it is exactly the silent divergence that
    // `plateforme-vectors.json` exists to close.
    const lisibles = (vecteurs as unknown as {
        refus_lisibles: { name: string; json: string; v: number; motif: string }[];
    }).refus_lisibles;

    it('🔴 carries four of them, and not zero', () => {
        // Anti-tautology: an empty array would make the loop below pass
        // without testing anything. Same guard as on the Rust side.
        expect(lisibles).toHaveLength(4);
    });

    for (const cas of lisibles) {
        it(`reads « ${cas.name} » whatever its version`, () => {
            const lu = parseDepuisLaPlateforme(cas.json) as unknown as Record<string, unknown>;
            expect(lu.type).toBe('refus');
            expect(lu.v).toBe(cas.v);
            expect(lu.motif).toBe(cas.motif);
        });
    }
});
