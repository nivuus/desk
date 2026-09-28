// The only `Orchestrateur` implementation of v1, under BOTH engines.
//
// 🔴 THE CLOCK IS INJECTED, and it is what makes the bound of criterion ④
// besiegeable from both sides. A `Date.now()` read in the module would leave
// only one observable instant, and the threshold would never be crossed in a
// test run.
//
// ⚠️ SIXTEEN TESTS AND NOT THE PLAN'S FIFTEEN, announced before being read: the plan
// gives no line to `lister`, whereas the `LigneVm` -> `Vm` conversion
// (snake_case to camelCase) is code that can break silently.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { SEUIL_INJOIGNABLE_MS } from '../agents/fraicheur';
import { enroler, marquerVu } from '../depot/agent';
import { lireParId } from '../depot/vm';
import { createUser } from '../depot/utilisateur';
import { BACKEND_STATIQUE } from './refus';
import { inventaireStatique } from './inventaire-statique';

const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

async function poserVm(p: Pilote, id: string, nom: string): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, nom, '192.168.3.2']);
}

async function seedUser(p: Pilote, email: string): Promise<string> {
    return createUser(p, email, 'empreinte-opaque-de-test', MS);
}

/// A FROZEN but adjustable clock: each test sets the instant it tests.
function horlogeA(instant: number): () => number {
    return () => instant;
}

describe(`inventaireStatique, engine=${MOTEUR}`, () => {
    it('🔴 `instantane` REFUSES with a type — reason, operation, backend', async () => {
        // 🔴 The red: replacing it with a silent `return`. It is
        // LITERALLY criterion ① — "an operation the backend does not know
        // how to do returns a typed refusal, never silence".
        base = await baseNeuve('inv-instantane');
        await poserVm(base, 'v1', 'w1');
        const o = inventaireStatique(base, horlogeA(MS));
        expect(await o.instantane('v1', 'before-update')).toEqual({
            ok: false,
            motif: 'non-supporte',
            operation: 'instantane',
            backend: BACKEND_STATIQUE,
        });
    });

    it('🔴 `instantane` LOGS its refusal', async () => {
        // 🔴 The red: removing the `console.warn`. ⚠️ `it()` DISTINCT from the
        // previous one, and it is P2's lesson ①A/①A-bis: `expect` interrupts
        // a test at its first false assertion, so that a second
        // assertion placed here would be tested by NOTHING.
        base = await baseNeuve('inv-instantane-journal');
        await poserVm(base, 'v1', 'w1');
        const avertir = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const o = inventaireStatique(base, horlogeA(MS));
        await o.instantane('v1', 'before-update');
        expect(avertir).toHaveBeenCalledTimes(1);
        // The line names the operation AND the backend: a refusal read in a
        // log without knowing what it is about does not inform.
        const ligne = String(avertir.mock.calls[0][0]);
        expect(ligne).toContain('instantane');
        expect(ligne).toContain(BACKEND_STATIQUE);
    });

    it('`demarrer` refuses the same way', async () => {
        // ⚠️ The three verbs share ONE SINGLE `refuser…` function, so
        // that this test tests the same line as the previous one. It exists
        // anyway: a verb that one day stopped going through it would not be
        // seen otherwise. The cost is two almost identical tests, and
        // it is paid.
        base = await baseNeuve('inv-start');
        await poserVm(base, 'v1', 'w1');
        const o = inventaireStatique(base, horlogeA(MS));
        expect(await o.start('v1')).toEqual({
            ok: false,
            motif: 'non-supporte',
            operation: 'demarrer',
            backend: BACKEND_STATIQUE,
        });
    });

    it('`arreter` refuses the same way', async () => {
        // ⚠️ `arreter` is named by NO criterion of the spec — `instantane`
        // alone is. It is refused anyway, for the reason of §3.6: an
        // operation the backend does not know how to do returns a typed refusal. Saying
        // so here prevents a reader from believing it an oversight.
        base = await baseNeuve('inv-arreter');
        await poserVm(base, 'v1', 'w1');
        const o = inventaireStatique(base, horlogeA(MS));
        expect(await o.arreter('v1')).toEqual({
            ok: false,
            motif: 'non-supporte',
            operation: 'arreter',
            backend: BACKEND_STATIQUE,
        });
    });

    it('🔴 `lister` converts the row into a `Vm`, without losing a field', async () => {
        // 🔴 The red: forgetting a field in the `LigneVm` -> `Vm` conversion.
        // `prefixe_session` -> `prefixe` and `vu_a` -> `vuA` are two
        // renames, hence two opportunities to return `undefined` silently —
        // and `undefined` is not `null`: `etatDe` distinguishes the latter.
        base = await baseNeuve('inv-lister');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        await marquerVu(base, 'v1', MS);
        const alice = await seedUser(base, 'alice@exemple.test');
        await inventaireStatique(base, horlogeA(MS)).attribuer('v1', alice);

        const [vm] = await inventaireStatique(base, horlogeA(MS)).lister();
        expect(vm).toEqual({
            id: 'v1',
            nom: 'w1',
            adresse: '192.168.3.2',
            userId: alice,
            prefixe: 'PREFIXEv1',
            vuA: MS,
        });
    });

    it('🔴 `etat`: `prete` at vu_a + THRESHOLD EXACTLY, `injoignable` one ms later', async () => {
        // 🔴 The red: freezing the injected clock (for example reading `Date.now()`
        // in the module). The bound would no longer be besieged from both sides, and
        // a threshold never reached proves nothing. The bound is STRICT and on the
        // `prete` side — `agents/fraicheur.ts` writes it that way precisely
        // for this.
        base = await baseNeuve('inv-etat-borne');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        await marquerVu(base, 'v1', MS);

        expect(await inventaireStatique(base, horlogeA(MS + SEUIL_INJOIGNABLE_MS)).etat('v1'))
            .toBe('prete');
        expect(await inventaireStatique(base, horlogeA(MS + SEUIL_INJOIGNABLE_MS + 1)).etat('v1'))
            .toBe('injoignable');
    });

    it('🔴 `etat` of a NEVER SEEN VM returns `injoignable`', async () => {
        // 🔴 The red: returning `prete`. `vu_a` is born `null` at enrolment
        // (`depot/agent.ts`): a VM enrolled but never started would be
        // announced ready, and the error would only show when opening a
        // session on a switched-off machine.
        base = await baseNeuve('inv-etat-jamais-vue');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        expect(await inventaireStatique(base, horlogeA(MS)).etat('v1')).toBe('injoignable');
    });

    it('`etat` of an UNKNOWN VM returns `injoignable`, never an exception', async () => {
        // 🔴 The red: throwing. An exception would bubble up as a 500 where there is
        // nothing abnormal — and on a route it would be an enumeration oracle.
        base = await baseNeuve('inv-etat-inconnue');
        expect(await inventaireStatique(base, horlogeA(MS)).etat('v-inexistante'))
            .toBe('injoignable');
    });

    it('🔴 `attribuer` on a free VM returns `{ok:true}`', async () => {
        base = await baseNeuve('inv-attrib-ok');
        await poserVm(base, 'v1', 'w1');
        const alice = await seedUser(base, 'alice@exemple.test');
        expect(await inventaireStatique(base, horlogeA(MS)).attribuer('v1', alice))
            .toEqual({ ok: true });
    });

    it('🔴 …and the ROW carries the owner', async () => {
        // ⚠️ `it()` DISTINCT: the verdict and the reread state are two assertions,
        // and an `attribuer` that returned `{ok:true}` without writing anything would pass
        // the first.
        base = await baseNeuve('inv-attrib-ok-etat');
        await poserVm(base, 'v1', 'w1');
        const alice = await seedUser(base, 'alice@exemple.test');
        await inventaireStatique(base, horlogeA(MS)).attribuer('v1', alice);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(alice);
    });

    it('🔴 `attribuer` on an ALREADY TAKEN VM returns `vm-deja-attribuee`', async () => {
        // 🔴 The red: removing the `AND utilisateur_id IS NULL` clause from
        // `depot/vm.ts`. MEASURED on both engines: the theft then passes.
        base = await baseNeuve('inv-attrib-prise');
        await poserVm(base, 'v1', 'w1');
        const alice = await seedUser(base, 'alice@exemple.test');
        const bob = await seedUser(base, 'bob@exemple.test');
        const o = inventaireStatique(base, horlogeA(MS));
        await o.attribuer('v1', alice);
        expect(await o.attribuer('v1', bob)).toEqual({
            ok: false,
            motif: 'vm-deja-attribuee',
            operation: 'attribuer',
            backend: BACKEND_STATIQUE,
        });
    });

    it('🔴 …and the owner has NOT changed', async () => {
        // ⚠️ `it()` DISTINCT: it is property ②a itself, and it bears
        // on the STATE, not on the verdict.
        base = await baseNeuve('inv-attrib-prise-etat');
        await poserVm(base, 'v1', 'w1');
        const alice = await seedUser(base, 'alice@exemple.test');
        const bob = await seedUser(base, 'bob@exemple.test');
        const o = inventaireStatique(base, horlogeA(MS));
        await o.attribuer('v1', alice);
        await o.attribuer('v1', bob);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(alice);
    });

    it('🔴 `attribuer` to someone who ALREADY has a VM returns `utilisateur-servi` and DOES NOT THROW', async () => {
        // 🔴 The red: removing the translation. The uniqueness exception
        // would bubble up, and the HTTP layer would return **500** — it is the third
        // assertion of criterion ②, "index violation translated into a typed refusal,
        // never into a 500".
        base = await baseNeuve('inv-attrib-servi');
        await poserVm(base, 'v1', 'w1');
        await poserVm(base, 'v2', 'w2');
        const alice = await seedUser(base, 'alice@exemple.test');
        const o = inventaireStatique(base, horlogeA(MS));
        await o.attribuer('v1', alice);
        expect(await o.attribuer('v2', alice)).toEqual({
            ok: false,
            motif: 'utilisateur-servi',
            operation: 'attribuer',
            backend: BACKEND_STATIQUE,
        });
        expect((await lireParId(base, 'v2'))?.utilisateur_id).toBeNull();
    });

    it('`attribuer` on an UNKNOWN VM returns `vm-inconnue`', async () => {
        base = await baseNeuve('inv-attrib-inconnue');
        const alice = await seedUser(base, 'alice@exemple.test');
        expect(await inventaireStatique(base, horlogeA(MS)).attribuer('v-inexistante', alice))
            .toEqual({
                ok: false,
                motif: 'vm-inconnue',
                operation: 'attribuer',
                backend: BACKEND_STATIQUE,
            });
    });

    it('🔴 an exception FOREIGN to uniqueness is RETHROWN, never translated', async () => {
        // 🔴 The red: translating EVERY exception into `utilisateur-servi`. An
        // unreachable database would then be presented as a business refusal — the exact
        // silent failure spec §6 forbids ("it does not start
        // degraded"). It is the ONLY place in the service where an exception is
        // caught, and this `catch` must never become silent.
        //
        // The fake driver answers reads and THROWS on write.
        const factice: Pilote = {
            async interroger<T>(): Promise<T[]> {
                // A free VM, and the user has none: the three
                // prior checks pass, and the write is reached.
                return [
                    {
                        id: 'v1',
                        nom: 'w1',
                        adresse: '192.168.3.2',
                        utilisateur_id: null,
                        prefixe_session: 'PREFIXEv1',
                        vu_a: MS,
                    },
                ] as unknown as T[];
            },
            async executer() {
                throw new Error('database unreachable');
            },
            async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
                return corps(factice);
            },
            async fermer() {},
        };
        await expect(
            inventaireStatique(factice, horlogeA(MS)).attribuer('v1', 'u-ada'),
        ).rejects.toThrow(/database unreachable/);
    });

    it('🔴 the INDEX VIOLATION is translated into `utilisateur-servi`, never rethrown', async () => {
        // 🔴 THIS TEST EXISTS BECAUSE A MUTATION STAYED GREEN WITHOUT IT, and
        // it is the only one that reaches the `catch`. The test "assigning to someone who
        // already has a VM" goes through the PRIOR READ and never reaches
        // the write: making the `catch` entirely rethrowing therefore left
        // all sixteen tests green, and the third assertion of criterion ② —
        // "index violation translated into a typed refusal, NEVER into a 500" —
        // was tested by NOTHING. MEASURED, then repaired here.
        //
        // The case is that of the RACE: between the read and the write,
        // the user acquired another VM elsewhere. The prior read
        // structurally cannot see it; it is the partial index that THROWS, and
        // it is the reread after `ROLLBACK` that explains it.
        //
        // 🔴 The red: making the `catch` rethrow without translating. The HTTP layer
        // would then return 500 on what is a business refusal.
        let lectures = 0;
        const enCourse: Pilote = {
            async interroger<T>(): Promise<T[]> {
                lectures += 1;
                const v1 = {
                    id: 'v1', nom: 'w1', adresse: '192.168.3.2',
                    utilisateur_id: null, prefixe_session: 'PREFIXEv1', vu_a: MS,
                };
                // The FIRST read, that of the transaction, sees nothing for the
                // user: the three checks pass.
                // The SECOND, that after the `ROLLBACK`, sees the VM they
                // have just acquired — and it is what explains the exception.
                const v2 = {
                    id: 'v2', nom: 'w2', adresse: '192.168.3.2',
                    utilisateur_id: lectures === 1 ? null : 'u-ada',
                    prefixe_session: 'PREFIXEv2', vu_a: MS,
                };
                return [v1, v2] as unknown as T[];
            },
            async executer() {
                // The text imitates an engine, and NO code reads it: the two
                // engines do not write the same one, and it is the reread state that
                // decides.
                throw new Error('UNIQUE constraint failed: vm.utilisateur_id');
            },
            async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
                return corps(enCourse);
            },
            async fermer() {},
        };

        expect(await inventaireStatique(enCourse, horlogeA(MS)).attribuer('v1', 'u-ada')).toEqual({
            ok: false,
            motif: 'utilisateur-servi',
            operation: 'attribuer',
            backend: BACKEND_STATIQUE,
        });
        // The reread did TAKE PLACE: without it, the reason would be guessed.
        expect(lectures).toBe(2);
    });

    it('the LOSER of a sequential race receives `vm-deja-attribuee`', async () => {
        // ⚠️ THIS TEST IS SEQUENTIAL AND DOES NOT MEASURE SERIALISATION: the
        // loser plays AFTER the winner. It tests the TRANSLATION of the refusal, not
        // the lock. The serialisation, for its part, was measured outside the suite, on
        // PostgreSQL 16.15 alone, on ONE pair of transactions — B blocks on
        // A's row lock then returns `0 rows` after its `COMMIT`. Nothing
        // is established beyond two concurrent ones, and the case cannot be
        // usefully measured on `node:sqlite`, opened as `:memory:` in a
        // single process.
        base = await baseNeuve('inv-course');
        await poserVm(base, 'v1', 'w1');
        const alice = await seedUser(base, 'alice@exemple.test');
        const bob = await seedUser(base, 'bob@exemple.test');
        const gagnant = inventaireStatique(base, horlogeA(MS));
        const perdant = inventaireStatique(base, horlogeA(MS));
        expect(await gagnant.attribuer('v1', alice)).toEqual({ ok: true });
        const issue = await perdant.attribuer('v1', bob);
        expect(issue).toEqual({
            ok: false,
            motif: 'vm-deja-attribuee',
            operation: 'attribuer',
            backend: BACKEND_STATIQUE,
        });
    });
});
