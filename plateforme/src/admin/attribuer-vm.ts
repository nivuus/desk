// Assigning a VM to a user, from the command line.
//
//     npm run admin:attribuer -- --email ada@exemple.test --vm w1
//     npm run admin:attribuer -- --detacher --vm w1
//
// 🔴 WHY THIS IS NOT AN HTTP ROUTE. There is NO administration
// role in this service: `identite/jeton.ts` only knows
// `utilisateur` and `agent`, and `config.ts` has no administrator
// variable. A route that assigned a VM would therefore, at best,
// be open to any authenticated user — a privilege escalation on offer.
// The precedent is exact: `admin:utilisateur` and `admin:agent` (D8).
//
// 🔴 AND THAT IS WHY IT NAMES THE CAUSE, unlike the routes.
// `http/routes-vm.ts` returns the same refusal for "unknown VM" and "someone
// else's VM", because a public route that told them apart would be an
// enumeration oracle. Here the caller already has access to the database and to the
// configuration secret: enumeration is not a risk, and hiding the cause would
// send them looking elsewhere (E8).
//
// ⚠️ THIS COMMAND CREATES NO VM. The only creation path remains
// `admin:agent`. It sets an owner on an already enrolled VM, and nothing
// more — that is all "static" allows (D1).

import { lireConfig } from '../config';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from '../base/migrations';
import { ouvrirBase } from '../base/ouvrir';
import type { Pilote } from '../base/pilote';
import { detacher, lireParId, lireParNom, type LigneVm } from '../depot/vm';
import { lireParEmail } from '../depot/utilisateur';
import { inventaireStatique } from '../orchestration/inventaire-statique';

export type Arguments =
    | { action: 'attribuer'; email: string; vm: string }
    | { action: 'detacher'; vm: string }
    | { refus: string };

/// The flags that would try to pass a secret through argv.
///
/// ⚠️ NO SECRET IS AT STAKE IN THIS COMMAND, and the list of the two
/// others is taken AS IS anyway: a command that accepted
/// `--mot-de-passe` without using it would still leave the string in
/// `ps`, where any user of the machine would read it, then in the shell
/// history. They are ENUMERATED rather than guessed: a broad pattern would one
/// day refuse a legitimate flag without anyone knowing why.
const DRAPEAUX_INTERDITS = [
    '--secret',
    '--secret-enrolement',
    '--mot-de-passe',
    '--motdepasse',
    '--password',
    '--mdp',
    '-s',
    '-p',
];

/// PURE, and tested on its own.
export function analyserArguments(argv: string[]): Arguments {
    for (const drapeau of DRAPEAUX_INTERDITS) {
        if (argv.includes(drapeau)) {
            // ⚠️ The reason does NOT COPY the refused value.
            return {
                refus:
                    `${drapeau} est refusé : cette commande n'a besoin d'aucun secret, et ` +
                    "un secret passé sur la ligne de commande serait lisible par tout " +
                    'utilisateur de la machine — `ps` expose l’argv de tout processus.',
            };
        }
    }

    const lire = (drapeau: string): string | undefined => {
        const i = argv.indexOf(drapeau);
        return i === -1 ? undefined : argv[i + 1];
    };

    const vm = lire('--vm');
    if (vm === undefined || vm === '') {
        return { refus: "--vm <nom|id> est obligatoire, et n'a aucun défaut." };
    }

    // 🔴 `--detacher` DOES NOT REQUIRE AN EMAIL: we detach a VM FROM someone,
    // and requiring that someone to be named would force the operator to know in advance
    // what the command is about to tell them.
    if (argv.includes('--detacher')) return { action: 'detacher', vm };

    const email = lire('--email');
    if (email === undefined || email === '') {
        return {
            refus:
                "--email <courriel> est obligatoire, et n'a aucun défaut : un défaut " +
                "attribuerait la VM à un compte que l'opérateur n'a pas nommé.",
        };
    }
    return { action: 'attribuer', email, vm };
}

export interface Issue {
    /// 0 = done, 2 = named refusal. ⚠️ 1 is reserved for the unexpected, as in the
    /// two other commands.
    code: number;
    sortie?: string;
    erreur?: string;
}

/// Resolves `--vm`: the NAME first, the identifier next.
///
/// ⚠️ THE ORDER IS DELIBERATE: the administrator knows the name they gave to
/// `admin:agent`, and the identifier is a UUID the command drew at random.
/// ⚠️ `vm.nom` not being UNIQUE (`0001-socle.sql`), two VMs with the same name make
/// it return the first — that is acceptable for an operator who sees the
/// result, and it would not be on a route.
async function resoudre(p: Pilote, designation: string): Promise<LigneVm | undefined> {
    return (await lireParNom(p, designation)) ?? (await lireParId(p, designation));
}

/// The testable core: it takes a `Pilote` and returns the outcome, without reading any
/// configuration or writing to any stream. Same split as
/// `enroler-agent.ts::enrolerLaVm`.
export async function appliquer(p: Pilote, args: Exclude<Arguments, { refus: string }>): Promise<Issue> {
    const ligne = await resoudre(p, args.vm);
    if (ligne === undefined) {
        return { code: 2, erreur: `aucune VM nommée « ${args.vm} », ni par son nom ni par son identifiant.` };
    }

    if (args.action === 'detacher') {
        const lignes = await detacher(p, ligne.id);
        return {
            code: 0,
            sortie:
                lignes === 1
                    ? `vm=${ligne.id} (${ligne.nom}) rendue au vivier.\n`
                    : `vm=${ligne.id} (${ligne.nom}) était déjà au vivier.\n`,
        };
    }

    const utilisateur = await lireParEmail(p, args.email);
    if (utilisateur === undefined) {
        return { code: 2, erreur: `aucun compte pour le courriel « ${args.email} ».` };
    }

    // 🔴 THE ASSIGNMENT GOES THROUGH THE ORCHESTRATOR, never through an `UPDATE` written
    // here. It is the one that carries the order "read, write under a clause, translate
    // the exception" (D4), and duplicating it would make the two paths diverge the
    // day one of them changed — the administration command being precisely
    // the one we reread least often.
    //
    // ⚠️ The clock is used by no path of `attribuer`; it is
    // passed because the interface demands it, and `Date.now` is honest here.
    const orchestrateur = inventaireStatique(p, Date.now);
    const issue = await orchestrateur.attribuer(ligne.id, utilisateur.id);
    if (issue.ok) {
        return {
            code: 0,
            sortie: `vm=${ligne.id}\nnom=${ligne.nom}\nutilisateur=${utilisateur.id}\nemail=${args.email}\n`,
        };
    }

    // 🔴 THE TYPED REFUSAL IS TRANSLATED INTO A SENTENCE, NEVER RELAYED AS IS NOR
    // LEFT AS AN EXCEPTION. A stack trace carrying `UNIQUE
    // constraint failed` — or `duplicate key value violates unique constraint`,
    // the other engine not writing the same thing — does not tell the
    // administrator what to do.
    switch (issue.motif) {
        case 'vm-deja-attribuee': {
            // We NAME the owner: without them, the operator would know it
            // failed without knowing whom to detach, and would have to open the database by
            // hand — which this command exists to avoid.
            const relue = await lireParId(p, ligne.id);
            return {
                code: 2,
                erreur:
                    `la VM ${ligne.id} (${ligne.nom}) appartient déjà à ` +
                    `${relue?.utilisateur_id ?? 'un autre compte'} — la détacher d'abord : ` +
                    `npm run admin:attribuer -- --detacher --vm ${ligne.nom}`,
            };
        }
        case 'utilisateur-servi':
            return {
                code: 2,
                erreur:
                    `${args.email} a déjà une VM, et l'index unique partiel ` +
                    "`vm_un_utilisateur` n'en autorise qu'une — détacher la sienne d'abord.",
            };
        default:
            // Unreachable with the inputs above (the VM was resolved,
            // so never `vm-inconnue`), written anyway: a reason added one
            // day must not fall into silence.
            return { code: 2, erreur: `attribution refusée : ${issue.motif}.` };
    }
}

/// The impure body. Returns the exit code.
export async function executer(argv: string[]): Promise<number> {
    const args = analyserArguments(argv);
    if ('refus' in args) {
        process.stderr.write(`${args.refus}\n`);
        return 2;
    }

    const config = lireConfig(process.env);
    const base = await ouvrirBase(config);
    try {
        // Migrations first: the command may be the very first action
        // on a new database, and an `UPDATE` on a missing table would give a
        // diagnosis unrelated to the cause. Same choice as the two other
        // commands.
        await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, Date.now());
        const issue = await appliquer(base, args);
        if (issue.erreur !== undefined) process.stderr.write(`${issue.erreur}\n`);
        if (issue.sortie !== undefined) process.stdout.write(issue.sortie);
        return issue.code;
    } catch (cause) {
        // Code 1, reserved for the unexpected: a named refusal returns 2.
        process.stderr.write(`attribution impossible : ${String(cause)}\n`);
        return 1;
    } finally {
        await base.fermer();
    }
}

// Run only when this file IS the entry point: without this guard,
// importing it from a test would launch the command.
if (process.argv[1] && import.meta.url.endsWith(process.argv[1].replace(/\\/g, '/'))) {
    process.exitCode = await executer(process.argv.slice(2));
}
