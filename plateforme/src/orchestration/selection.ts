// Which VMs of an inventory belong to a user.
//
// 🔴 THIS MODULE IS PURE, and that is the point: no database, no socket, no clock. A
// filter defect living in `http/routes-vm.ts` would leak the WHOLE
// inventory, and there would be no place to make it fail without standing up a server.
//
// 🔴 "TO NOBODY" IS NOT "TO EVERYBODY". A VM whose
// `userId` is `null` is IN THE POOL: it is returned to no
// requester. The opposite would show any authenticated user every VM
// not yet assigned — that is, the fleet inventory. ⚠️
// Sub-block G1 deliberately keeps the other behaviour for ITS routes; the
// two efforts diverge, and it is declared rather than quietly reconciled
// by the second branch to arrive.

import type { Vm } from './interface';

/// All the VMs of this user. The inventory order is preserved.
///
/// ⚠️ THE COMPARISON IS AN EQUALITY, never a prefix or an inclusion:
/// `alice` and `alice-bis` are two users. Same rule as
/// `http/cors.ts`, and for the same reason — a `startsWith` opens a family
/// of matches that nobody decided on.
export function vmsDe(inventaire: readonly Vm[], userId: string): Vm[] {
    // `null !== userId` by construction, so the pool is excluded without
    // any branch saying so — but the test names it, because it is
    // a property and not a side effect of the comparison.
    return inventaire.filter((v) => v.userId === userId);
}

/// THE VM of this user, or `undefined`.
///
/// 🔴 IT THROWS ON A DUPLICATE, and it is not a style preference.
/// The partial index `vm_un_utilisateur` (`0001-socle.sql`) makes the case (policy: allow-fr - SQLite index and file name)
/// impossible IN THE DATABASE — but this function receives an array, and nothing in its
/// signature says where it comes from. Silently returning the first would pick
/// a VM at random and do it without a trace; the exception, on the other hand, says where
/// to look the day the index has disappeared.
export function laVmDe(inventaire: readonly Vm[], userId: string): Vm | undefined {
    const siennes = vmsDe(inventaire, userId);
    if (siennes.length > 1) {
        throw new Error(
            `the inventory carries ${siennes.length} VMs for one user, which ` +
                "the partial index `vm_un_utilisateur` must make impossible: the database " +
                'or its schema must be examined, this refusal cannot be bypassed.',
        );
    }
    return siennes[0];
}
