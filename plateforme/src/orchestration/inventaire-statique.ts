// `InventaireStatique`: the one and only `Orchestrateur` implementation of v1.
//
// 🔴 WHAT "STATIC" MEANS, AND WHAT IT DOES NOT MEAN. Spec §3.6
// announced it as "fed by a declarative configuration file"; this
// backend reads THE DATABASE (divergence E1, decision D1). The three fields such a
// file would have carried — name, address, hash of the enrolment secret —
// are already written to the database by `admin/enroler-agent.ts`, and two sources of
// truth for the same thing diverge silently: it is the one nobody
// reads that wins, the day it is relied upon.
//
// "Static" therefore means: IT DRIVES NO HYPERVISOR. It turns on
// nothing, turns off nothing, snapshots nothing, creates no machine. Its
// inventory is what an administrator enrolled, and it can only act on the
// world through the one column it owns:
// `vm.utilisateur_id`. This reading is STRONGER than "reloadable
// file" — a file could have been reloaded, which would have suggested
// some form of management.
//
// 🔴 THE CLOCK IS INJECTED (`maintenant`), never `Date.now()` read here. That is
// what makes the bound of `fraicheur.etatDe` testable from both sides.

import type { Pilote } from '../base/pilote';
import { etatDe } from '../agents/fraicheur';
import { attribuerSiLibre, lister as listerLignes, type LigneVm } from '../depot/vm';
import type { EtatVm, Operation, Orchestrateur, Vm } from './interface';
import { BACKEND_STATIQUE, refuser, type Outcome } from './refus';
import { vmsDe } from './selection';

/// Converts a repository row into a `Vm`.
///
/// ⚠️ TWO RENAMINGS, HENCE TWO CHANCES TO RETURN `undefined` SILENTLY:
/// `prefixe_session` -> `prefixe` and `vu_a` -> `vuA`. And `undefined` is NOT
/// `null` — `etatDe` tells the latter (a VM never seen) apart from what would be
/// a conversion error. The `lister` test compares the WHOLE object.
function enVm(l: LigneVm): Vm {
    return {
        id: l.id,
        nom: l.nom,
        adresse: l.adresse,
        userId: l.utilisateur_id,
        prefixe: l.prefixe_session,
        vuA: l.vu_a,
    };
}

async function inventaireDe(p: Pilote): Promise<Vm[]> {
    return (await listerLignes(p)).map(enVm);
}

/// Refuses an operation this backend cannot perform, AND writes it to the
/// log.
///
/// ⚠️ THE THREE ACTION VERBS SHARE THIS ONE FUNCTION, and it must be
/// said: a test on `instantane` therefore exercises the same line as a test on
/// `start`. Each of the three nevertheless has its own `it()` — not to
/// exercise three different lines, but so that a verb that stopped going
/// through here some day would be noticed.
///
/// 🔴 THE LOG IS A `warn!`, NOT A `debug`. A backend that silently refuses
/// an operation the operator believes they triggered is
/// a silent failure; operations run at the ordinary level, and a
/// silent mitigation is not one.
function refuserNonSupporte(operation: Operation): Outcome {
    console.warn(
        `opération refusée : ${operation} n'est pas supportée par le backend ` +
            `${BACKEND_STATIQUE}, qui ne pilote aucun hyperviseur — il inventorie ce ` +
            "qu'un administrateur a enrôlé, et n'écrit que vm.utilisateur_id.",
    );
    return refuser('non-supporte', operation);
}

export function inventaireStatique(base: Pilote, maintenant: () => number): Orchestrateur {
    return {
        async lister(): Promise<Vm[]> {
            return inventaireDe(base);
        },

        async etat(vm: string): Promise<EtatVm> {
            const inventaire = await inventaireDe(base);
            const cible = inventaire.find((v) => v.id === vm);
            // 🔴 AN UNKNOWN VM RETURNS `injoignable`, NEVER AN EXCEPTION.
            // That is true — we know nothing about it, so we cannot use it —
            // and it is already what `agents/fraicheur.ts` says of a VM never
            // seen. An exception would bubble up as a 500 where there is nothing
            // abnormal, and on a route it would be an enumeration oracle.
            if (cible === undefined) return 'injoignable';
            // 🔴 IT IS THE PRODUCTION CALLER THAT `agents/fraicheur.ts`
            // DECLARES IT HAS BEEN WAITING FOR SINCE P3, and it closes legacy no. 2 of that
            // sub-block: "it has no production caller in P3, and
            // that is declared rather than hidden […] which is the subject of
            // P4".
            return etatDe(cible.vuA, maintenant());
        },

        async start(vm: string): Promise<Outcome> {
            // 🔴 THIS REFUSAL IS NOT A CONVENIENCE: it is the content of
            // criterion ④. The framing promises "VM unreachable -> the hub
            // shows it, OFFERS A RESTART". With this backend, the hub
            // SHOWS it and says it can NOT restart. What P4 delivers is
            // the admission, not the feature — and spec §3.6 calls it a "product
            // consequence to own".
            void vm;
            return refuserNonSupporte('demarrer');
        },

        async arreter(vm: string): Promise<Outcome> {
            void vm;
            return refuserNonSupporte('arreter');
        },

        async instantane(vm: string, nom: string): Promise<Outcome> {
            void vm;
            void nom;
            return refuserNonSupporte('instantane');
        },

        async attribuer(vm: string, user: string): Promise<Outcome> {
            try {
                return await base.transaction(async (t) => {
                    // ① READ FIRST — and it is to NAME the right reason,
                    // never to guarantee anything. `changes = 0`
                    // conflates THREE causes (unknown VM, VM already taken,
                    // reassignment to the same user, divergence E8): a
                    // code that decided on this number alone would return a refusal
                    // that informs nothing.
                    const inventaire = await inventaireDe(t);
                    const cible = inventaire.find((v) => v.id === vm);
                    if (cible === undefined) return refuser('vm-inconnue', 'attribuer');
                    if (cible.userId !== null) {
                        return refuser('vm-deja-attribuee', 'attribuer');
                    }
                    if (vmsDe(inventaire, user).length > 0) {
                        return refuser('utilisateur-servi', 'attribuer');
                    }

                    // ② THEN WRITE, UNDER THE CONDITIONAL CLAUSE. 🔴 It is
                    // THE CLAUSE that guarantees, not the read above: it is
                    // RE-EVALUATED BY THE ENGINE at write time. Code
                    // that read then wrote without a clause would be right in the
                    // tests and wrong in production, and the sequential test would not
                    // see it. Measured on PostgreSQL 16.15: of two
                    // transactions targeting the same free VM, the second blocks
                    // then returns `0 rows` — exactly one winner.
                    const lignes = await attribuerSiLibre(t, vm, user);
                    // Zero rows HERE can only mean one thing: the race
                    // was lost between the read and the write, the two
                    // other causes having already been ruled out.
                    if (lignes === 0) return refuser('vm-deja-attribuee', 'attribuer');
                    return { ok: true };
                });
            } catch (cause) {
                // ③ AND IF THE WRITE THREW: read again, and translate ONLY what
                // the reread explains.
                //
                // 🔴 IT IS THE ONLY PLACE IN THE SERVICE WHERE AN EXCEPTION IS
                // CAUGHT, and it must not become a silent `catch`. A
                // `catch` that translated ANY exception into `utilisateur-servi`
                // would swallow an unreachable database and present it as a
                // business refusal — the exact silent failure spec §6 forbids.
                //
                // 🔴 NO COMPARISON OF THE EXCEPTION TEXT: the two
                // engines do not write the same one (`UNIQUE constraint failed:
                // vm.utilisateur_id` versus `duplicate key value violates unique
                // constraint "vm_un_utilisateur"`). It is the reread STATE that
                // decides, never the message.
                //
                // ⚠️ THE REREAD HAPPENS OUTSIDE THE TRANSACTION, AND THAT IS STRUCTURAL,
                // not a preference: when the exception arrives here,
                // `Pilote.transaction` has ALREADY issued its `ROLLBACK` and released the
                // client — as can be read in `base/pilote-postgres.ts` and
                // `base/pilote-sqlite.ts`. There is no transaction left in
                // which to read; `base` is therefore the only possible route.
                const inventaire = await inventaireDe(base);
                // `vmsDe` and not `laVmDe`: the latter THROWS on a duplicate,
                // and throwing from a `catch` would replace the cause with
                // another.
                const siennes = vmsDe(inventaire, user);
                if (siennes.length > 0 && !siennes.some((v) => v.id === vm)) {
                    // The user does have ANOTHER VM: it is the partial index
                    // `vm_un_utilisateur` that threw, and the refusal is typed.
                    return refuser('utilisateur-servi', 'attribuer');
                }
                // Nothing in the state explains it: the cause is elsewhere, and
                // it bubbles up as is.
                throw cause;
            }
        },
    };
}
