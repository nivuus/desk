// Which VMs of an inventory belong to this user. PURE, hence testable
// without a database or socket.
//
// 🔴 WHY THIS MODULE EXISTS RATHER THAN A `filter` IN THE ROUTE: a filter
// defect living in `http/routes-vm.ts` would leak the WHOLE
// inventory, and there would be no place to turn it red without mounting a server.
// Here, the mutation "return the whole inventory" fails within a few
// microseconds.
//
// ⚠️ FIVE TESTS AND NOT FOUR, AND IT IS ANNOUNCED: the plan puts "returns
// the only VM" and "throws on a duplicate" in a single table row,
// while its own doctrine requires one `it()` per assertion (P2's lesson ①A/①A-bis).
// The two are therefore separated.

import { describe, expect, it } from 'vitest';
import type { Vm } from './interface';
import { laVmDe, vmsDe } from './selection';

const MS = 1_787_136_773_742;

function vm(id: string, userId: string | null): Vm {
    return {
        id,
        nom: `w-${id}`,
        adresse: '192.168.3.2',
        userId,
        prefixe: `prefixe-${id}`,
        // An epoch in milliseconds, never a convenient small number: it is
        // the most expensive lesson of P1.
        vuA: MS,
    };
}

const INVENTAIRE: readonly Vm[] = [
    vm('v1', 'alice'),
    vm('v2', 'bob'),
    vm('v3', null),
    vm('v4', 'alice-bis'),
];

describe('selection of the VMs of a user', () => {
    it('🔴 `vmsDe` returns ONLY the VMs of the requester', () => {
        // 🔴 The red: returning the whole inventory. It is the exact mutation
        // this module exists to make visible — and it is undetectable
        // if the filter lives in the route.
        expect(vmsDe(INVENTAIRE, 'alice').map((v) => v.id)).toEqual(['v1']);
        expect(vmsDe(INVENTAIRE, 'bob').map((v) => v.id)).toEqual(['v2']);
        // And a PREFIX comparison would give `v4` to alice.
        expect(vmsDe(INVENTAIRE, 'alice').map((v) => v.id)).not.toContain('v4');
    });

    it('🔴 a VM with a null `utilisateurId` is returned to NOBODY', () => {
        // 🔴 The red: treating `null` as "free for all". Every
        // authenticated user would then see every unassigned VM.
        // ⚠️ It is the behaviour sub-block G1 deliberately keeps
        // for ITS routes ("served, and logged"); P4 does not keep it
        // — see D8. The two workstreams diverge here, and it is declared.
        for (const qui of ['alice', 'bob', 'personne', '']) {
            expect(vmsDe(INVENTAIRE, qui).map((v) => v.id)).not.toContain('v3');
        }
        expect(laVmDe(INVENTAIRE, '')).toBeUndefined();
    });

    it('🔴 `laVmDe` returns `undefined` when the user has none', () => {
        // 🔴 The red: returning `inventaire[0]`. Criterion ③ — "a
        // user without a VM receives 409 `aucune-vm`" — would become
        // unverifiable, the route always believing it holds one.
        expect(laVmDe(INVENTAIRE, 'carole')).toBeUndefined();
        expect(laVmDe([], 'alice')).toBeUndefined();
    });

    it('`laVmDe` returns the single VM when there is one', () => {
        const trouvee = laVmDe(INVENTAIRE, 'bob');
        expect(trouvee?.id).toBe('v2');
    });

    it('🔴 `laVmDe` THROWS if the inventory carries TWO for the same user', () => {
        // 🔴 The red: returning the first one silently. The partial index
        // `vm_un_utilisateur` makes this case impossible IN THE DATABASE — but this
        // function receives an array, and nothing in its signature says where it
        // comes from. The day the index were removed (which red ②b of the
        // acceptance does on purpose), it is this exception that would say where to look;
        // silence would make it pick a VM at random.
        const double: readonly Vm[] = [vm('v1', 'alice'), vm('v9', 'alice')];
        // The message NAMES the index whose disappearance would explain the duplicate:
        // it is what says where to look, and it is what is asserted.
        expect(() => laVmDe(double, 'alice')).toThrow(/vm_un_utilisateur/);
        // `vmsDe`, for its part, does NOT throw: it returns what it sees. It is
        // `laVmDe` that carries the "at most one" invariant.
        expect(vmsDe(double, 'alice')).toHaveLength(2);
    });
});
