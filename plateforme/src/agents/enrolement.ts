// Checking an agent enrolment secret.
//
// 🔴 THE REFUSAL ENUMERATES NOTHING. "Unknown VM" and "wrong secret" return the
// SAME object, word for word. Telling them apart would give anyone who opens the
// `/agent` channel an oracle: they would learn by trial and error which VMs are
// enrolled, merely by comparing two answers. That is what spec
// §4 P3 ② explicitly names as the red of its criterion.
//
// ⚠️ WHAT THE REQUESTER DOES NOT LEARN, THE OPERATOR MUST BE ABLE TO READ. The
// refusal is therefore LOGGED with the requested VM name, through a `journaliser`
// passed as a parameter — same split as `identite/garde.ts`, which carries
// `message` (on the wire) and `journal` (on our side) for the same reason. The
// log NEVER copies the secret: it has just been refused, writing it again
// elsewhere would make no sense.
//
// ⚠️ THE DIGEST REUSES `identite/mot-de-passe.ts`, the one of human
// accounts, format `scrypt$N$r$p$sel$empreinte`. Two derivations in the same
// service would diverge the day one of them got hardened — and `verifier` already
// handles length equalisation, whose absence MAKES
// `timingSafeEqual` RAISE (measured in P2).

import type { Pilote } from '../base/pilote';
import { verifier } from '../identite/mot-de-passe';
import { lireParVm } from '../depot/agent';

export type VerdictEnrolement =
    | { ok: true; vmId: string; prefixe: string }
    | { ok: false; motif: 'enrolement' };

/// The ONLY refusal. A single object, built only once, so that no
/// branch can make a variant of it by accident — that is
/// safer than trusting two literals kept identical by discipline.
const REFUS: VerdictEnrolement = { ok: false, motif: 'enrolement' };

/// Checks that a VM presents the secret of its enrolment.
///
/// 🔴 IT DOES NOT CATCH ALL EXCEPTIONS, and that is deliberate:
/// `mot-de-passe.ts::verifier` RAISES on an unknown algorithm, because a
/// silent refusal there would be indistinguishable from a wrong secret and nobody
/// could diagnose a database written by a future version of the service.
/// That exception therefore goes all the way to the channel, which translates it into an
/// `enrolement` refusal while logging it WITH its cause. A merely
/// MALFORMED digest, on the other hand, returns `false` without raising — the two cases are distinct.
export async function verifierEnrolement(
    p: Pilote,
    vmId: string,
    secret: string,
    journaliser: (ligne: string) => void,
): Promise<VerdictEnrolement> {
    const ligne = await lireParVm(p, vmId);
    if (ligne === undefined) {
        journaliser(`enrôlement refusé pour la VM ${vmId}`);
        return REFUS;
    }

    if (!(await verifier(secret, ligne.empreinte_secret))) {
        // ⚠️ EXACTLY THE SAME TEXT as above, on purpose: two different
        // labels in a log end up finding their way into a
        // response, and the oracle would be reborn through the back door.
        journaliser(`enrôlement refusé pour la VM ${vmId}`);
        return REFUS;
    }

    return { ok: true, vmId, prefixe: ligne.prefixe_session };
}
