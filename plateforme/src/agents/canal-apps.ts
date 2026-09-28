// The UPSTREAM branches of sub-project ④: what an enrolled agent reports about
// its catalogue, its launches and its installations.
//
// 🔴 EXTRACTED BEFORE THE ADDITION, NEVER AFTER. `canal.ts` was at 462 lines,
// margin 38; sub-block G3 adds two branches and the re-emission at
// enrolment to it. The plan put it at 423 on 20 August: G2 is what consumed
// the difference. The `CLAUDE.md` doctrine is to give the margin back through an
// extraction played in advance, never through compression — and this repository says
// four times that the margin won back by an extraction is lost again if it is
// treated as settled.
//
// 🔴 THE BOUNDARY IS THE ONE THREE OTHER FILES ALREADY USE: life
// cycle on one side (enrolment, heartbeat, refusal, brake), app management
// on the other. It is the cut of `proto/src/plateforme/apps.rs`, of
// `proto/src/plateforme/tests_apps.rs` and of `proto/ts/plateforme-apps.test.ts`.
// A single cut for four files is what makes it memorable.
//
// 🔴 AND IT IS A TYPE GUARD, NOT A FALL-THROUGH. `traiter` is called behind
// the predicate `estMontantDeQuatre`, which NAMES the four types. `canal.ts`
// previously let `enroler` be the REMAINDER of an `if/else` — and the widening
// of the union by G3 made `progression` and `termine` two members of that
// remainder, hence two messages that the destructuring `const { vm, secret }`
// would have read as an enrolment. **`tsc` is what said so, and it got
// lucky: the same fragility on a value rather than a type would have gone through
// silently.** The predicate closes the class.

import type { WebSocket } from 'ws';
import {
    encodeIconesManquantes,
    encodeInstaller,
    type CatalogueMessage,
    type LanceeMessage,
    type ProgressionMessage,
    type TermineMessage,
    type VersLaPlateforme,
} from '../../../proto/ts/plateforme';
import type { Magasin } from '../apps/icones';
import type { Pilote } from '../base/pilote';
import { fusionner } from '../apps/catalogue';
import { appliquer, lireConnues } from '../depot/application';
import { avancer, lireEnAttentePourVm, terminer } from '../depot/installation';
import { lireParId as lireTeleversement } from '../depot/televersement';
import type { RegistreAgents } from './registre';

/// The four upstream types that belong to ④.
///
/// 🔴 DERIVED FROM THE UNION THROUGH THE RETURN TYPE OF THE PREDICATE: a variant
/// added to `VersLaPlateforme` without its key here would not be refused by
/// `tsc`, but it would fall into the `enroler` of `canal.ts`, where the
/// destructuring would refuse it LOUDLY. That is the wanted behaviour — a
/// message of ④ one forgets to route must show up, not get lost.
export type MontantDeQuatre =
    | CatalogueMessage
    | LanceeMessage
    | ProgressionMessage
    | TermineMessage;

const TYPES_DE_QUATRE: readonly MontantDeQuatre['type'][] = [
    'catalogue',
    'lancee',
    'progression',
    'termine',
];

export function estMontantDeQuatre(message: VersLaPlateforme): message is MontantDeQuatre {
    return (TYPES_DE_QUATRE as readonly string[]).includes(message.type);
}

export interface DependancesMontantes {
    base: Pilote;
    registre: RegistreAgents;
    magasin: Magasin | undefined;
    socket: WebSocket;
    vmId: string;
    maintenant: () => number;
    envoyer: (brut: string) => void;
}

/// The inventory of icons the platform lacks, requested AFTER the catalogue
/// is written.
///
/// 🔴 THE INVENTORY QUERIES THE DISK, NOT A TABLE. A bookkeeping
/// table would diverge from the store the day a file got lost — and
/// that is PRECISELY the day one needs to know.
function reclamerLesIcones(
    deps: DependancesMontantes,
    message: CatalogueMessage,
): void {
    if (deps.magasin === undefined) return;
    const annoncees = message.applications
        .map((a) => a.icone)
        .filter((e): e is string => e !== null);
    if (annoncees.length === 0) return;
    const manque = deps.magasin.manquantes(annoncees);
    if (manque.length === 0) return;
    deps.envoyer(encodeIconesManquantes(manque));
}

/// Handles an upstream message of ④. The caller has ALREADY checked the enrolment.
///
/// ⚠️ NOTHING IS AWAITED HERE, AND THAT IS THE RULE OF THE FILE: an `await` on the
/// path of a message would mean a momentarily unavailable database
/// WOULD TAKE DOWN THE CONNECTION of an agent that is perfectly fine, and a rejected promise
/// without `catch` would take down the whole Node process. It is the rule `canal.ts`
/// already imposes on itself for `marquerVu`.
export function traiter(deps: DependancesMontantes, message: MontantDeQuatre): void {
    const instant = deps.maintenant();

    if (message.type === 'lancee') {
        // ⚠️ SYNCHRONOUS, and nothing to write: `resoudre` only touches an in-memory
        // `Map`, and IGNORES an unknown request rather than raising.
        deps.registre.resoudre(message.demande, message.issue);
        return;
    }

    if (message.type === 'progression') {
        // ⚠️ THE COST IS NAMED: a lost progress report only shows in the
        // log. It catches up on its own — the next one arrives a
        // second later, and the `termine` carries the final state.
        void avancer(
            deps.base,
            message.installation,
            {
                phase: message.phase,
                octetsFaits: message.octets_faits,
                octetsTotal: message.octets_total,
                ecouleMs: message.ecoule_ms,
            },
            instant,
        ).catch((cause) => {
            console.error(
                `progression non écrite pour l'installation ${message.installation} : ${String(cause)}`,
            );
        });
        return;
    }

    if (message.type === 'termine') {
        // 🔴 THIS ONE, IF LOST, DOES NOT CATCH UP ON ITS OWN — and that must be said
        // rather than left to be believed. The agent emits `termine` only
        // once; if the write fails, the row stays `en_cours` forever
        // and the re-emission does not pick it up either, since it only targets
        // the `en_attente` ones. The log is then the only trace, and that is
        // why it carries the identifier.
        //
        // ⚠️ What protects the user from a replayed installation is NOT
        // this write but the marker on the VM disk: the second
        // belt exists for the case where the first one lost its database.
        void terminer(
            deps.base,
            message.installation,
            {
                issue: message.issue,
                motif: message.motif,
                codeSortie: message.code_sortie,
                journal: message.journal,
                journalTronque: message.journal_tronque,
            },
            instant,
        ).catch((cause) => {
            console.error(
                `issue non écrite pour l'installation ${message.installation} : ${String(cause)}`,
            );
        });
        return;
    }

    const identifiant = deps.vmId;
    void lireConnues(deps.base, identifiant)
        .then((connues) => appliquer(deps.base, identifiant, fusionner(connues, message), instant))
        .then(() => reclamerLesIcones(deps, message))
        .catch((cause) => {
            console.error(`catalogue non écrit pour la VM ${identifiant} : ${String(cause)}`);
        });
}

export interface DependancesReemission {
    base: Pilote;
    vmId: string;
    /// 🔴 THE EMISSION IS INJECTED, AND THAT IS WHAT GIVES THIS FUNCTION TWO
    /// CALLERS: the channel, which writes to the socket it just enrolled, and
    /// `POST /installation`, which goes through the registry to reach an agent
    /// ALREADY connected. Without that second caller, an order placed while the VM
    /// is online was only delivered at the next enrolment — that is, at the
    /// next restart of the agent. See §5quater of the results.
    ///
    /// ⚠️ A `socket: WebSocket` FIELD SAT HERE AND WAS READ BY NOBODY.
    /// It was not removed out of taste: as long as it was there, only a holder of a
    /// `WebSocket` could call this function, and the HTTP route — which has
    /// none — had to duplicate building the message. A dead field can
    /// therefore cost a duplication, not only a line.
    envoyer: (brut: string) => void;
}

/// Pushes again the installation orders a VM has not acknowledged yet.
///
/// 🔴 IT IS THE SAFETY NET OF THE `push` WITHOUT DELIVERY GUARANTEE, and it has a twin
/// that the G1 acceptance run saw working on the real path: the
/// `complet = true` the agent sends back at every re-enrolment. Without it, an
/// order emitted during an outage would be lost WITH NO END.
///
/// ⚠️ **IT ONLY TARGETS THE `en_attente` ONES.** As soon as an agent has reported a
/// progress, the row turns `en_cours` and stops being re-emitted: that is the
/// FIRST of the two belts against a double run. The second is the
/// marker on the VM disk, and it protects from the case where the first one
/// lost its database.
///
/// ⚠️ **THE URL IS DERIVED, NOT CONFIGURED.** It is relative — `/televersement/
/// :id/contenu` — and the agent resolves it against the address of its own channel,
/// as it already derives that of the icon upload. Two variables for the
/// same address would diverge the day one of the two got changed.
export async function reemettreLesInstallations(
    deps: DependancesReemission,
): Promise<void> {
    const enAttente = await lireEnAttentePourVm(deps.base, deps.vmId);
    if (enAttente.length === 0) return;
    for (const ligne of enAttente) {
        const tel = await lireTeleversement(deps.base, ligne.televersement_id);
        if (tel === undefined) {
            // ⚠️ UNREACHABLE THROUGH THE FOREIGN KEY, and guarded anyway: the
            // assumption "the foreign key prevents it" is true of the database,
            // not of this code. Skipping it while SAYING so is better than emitting an
            // order whose URL would lead nowhere.
            console.error(
                `installation ${ligne.id} sans téléversement ${ligne.televersement_id} : ordre non réémis`,
            );
            continue;
        }
        deps.envoyer(
            encodeInstaller(
                ligne.id,
                `/televersement/${tel.id}/contenu`,
                tel.nom,
                tel.taille,
                tel.sha256,
            ),
        );
    }
}
