// Enrolling a VM from the administration command line.
//
//     npm run admin:agent -- --vm w1 --adresse 192.168.3.2      (enrol)
//     npm run admin:agent -- --vm <id> --roter                  (rotate)
//
// 🔴 THE SECRET IS DRAWN AT RANDOM BY THE COMMAND, AND WRITTEN ONLY ONCE TO
// STDOUT. It can never be read back: only its digest goes to the database. A
// `--secret` on argv is REFUSED explicitly, with its reason — `ps` exposes
// the command line of every process to every user of the machine, and
// a secret passed that way would be readable by anyone for the whole
// duration of the call, then in the shell history. It is the exact precedent
// of `creer-utilisateur.ts`, taken to the letter.
//
// ⚠️ WHAT IS DONE WITH IT NEXT IS NOT PROTECTED, and it must be said here:
// the secret is meant for `AGENT_SECRET` in `scripts/run-agent.sh`, which
// writes it IN PLAIN TEXT into `C:\dev\run-agent.ps1` on a CIFS share readable
// from the host — like the fifty-seven other variables.
//
// 🔴 SUB-BLOCK P5 REOPENED THIS POINT, AND IT DOES NOT FIX IT: IT MAKES IT
// REPAIRABLE. Removing the secret from that file would require changing `scripts/`,
// making `agent/` read a Windows vault (DPAPI), and testing the
// result ON THE VM — three things outside its scope. Delivering a half
// remedy, untested, would be worse than declaring the gap.
//
// **The counterpart is `--roter`, and it did not exist.** Before it, an
// operator who learned that a secret had leaked had NO way to
// replace it: `enroler` only knows how to INSERT, `vm_id` is a primary key
// (`0003-agents.sql`), so re-enrolling an already enrolled VM RAISES. All that was left
// was a manual `DELETE` in the database. The theft of a secret is now repairable.
//
// ⚠️ An already enrolled VM makes it RAISE, through the primary key of `agent_enrole`.
// There is NO enumeration oracle here, unlike the `/agent` channel:
// the caller is the administrator, and hiding the failure would make them believe in
// an enrolment that does not exist.

import { randomBytes, randomUUID } from 'node:crypto';
import { lireConfig } from '../config';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from '../base/migrations';
import { ouvrirBase } from '../base/ouvrir';
import type { Pilote } from '../base/pilote';
import { enroler, lireParVm, remplacerEmpreinte } from '../depot/agent';
import { hacher } from '../identite/mot-de-passe';
import { nouveauPrefixe } from '../agents/prefixe';

/// The two actions of this command, and the refusal.
///
/// ⚠️ `mode` IS EXPLICIT rather than inferred from the presence of `adresse`: the
/// day a third action appeared, the inference would go wrong
/// silently, whereas a named field forces a decision.
export type Arguments =
    | { mode: 'enroler'; vm: string; adresse: string }
    | { mode: 'roter'; vm: string }
    | { refus: string };

/// The flags that would try to pass a secret through argv. They are
/// ENUMERATED rather than guessed: a broad pattern would one day refuse a
/// legitimate flag without anyone knowing why. Same choice as `creer-utilisateur.ts`.
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

/// 32 bytes, i.e. 43 `base64url` characters. ⚠️ NOT CALIBRATED: it is the
/// usual length of a 256-bit secret, not a measured threshold. It joins
/// the list of uncalibrated constants of the repository.
const OCTETS_SECRET = 32;

/// PURE, and tested on its own. Returns the pair, or a refusal carrying its reason.
export function analyserArguments(argv: string[]): Arguments {
    for (const drapeau of DRAPEAUX_INTERDITS) {
        if (argv.includes(drapeau)) {
            // ⚠️ The reason does NOT COPY the refused value.
            return {
                refus:
                    `${drapeau} est refusé : le secret d'enrôlement est TIRÉ AU SORT par ` +
                    "cette commande et écrit une seule fois sur la sortie standard, jamais " +
                    "reçu sur la ligne de commande — `ps` l'exposerait à tout utilisateur " +
                    'de la machine.',
            };
        }
    }

    const lire = (drapeau: string): string | undefined => {
        const i = argv.indexOf(drapeau);
        return i === -1 ? undefined : argv[i + 1];
    };

    const vm = lire('--vm');
    if (vm === undefined || vm === '') {
        return { refus: "--vm <nom> est obligatoire, et n'a aucun défaut." };
    }

    // 🔴 ROTATION DOES NOT REQUIRE `--adresse`, and that is not a convenience:
    // it does NOT touch the `vm` table. Requiring an address would invite
    // typing a random one, which would be ignored — a parameter asked for
    // without being used ends up being believed used.
    //
    // ⚠️ THE CHECK FOR FORBIDDEN FLAGS IS UPSTREAM OF THIS BRANCH, so
    // it covers `--roter` too. That is the path taken precisely
    // when a secret has leaked: letting a `--secret` on argv through there
    // would replay the leak we are in the middle of repairing.
    if (argv.includes('--roter')) {
        return { mode: 'roter', vm };
    }

    const adresse = lire('--adresse');
    if (adresse === undefined || adresse === '') {
        return { refus: "--adresse <hôte> est obligatoire, et n'a aucun défaut." };
    }
    return { mode: 'enroler', vm, adresse };
}

export interface Enrolement {
    vmId: string;
    prefixe: string;
    /// 🔴 Returned ONLY ONCE. It exists nowhere else: only its
    /// digest is written, and nothing allows finding it again afterwards.
    secret: string;
}

/// Creates the VM and its enrolment, and returns the plain secret TO THE CALLER ONLY.
///
/// The clock is a PARAMETER, as everywhere in this repository: that is what
/// would make an assertion on an exact value possible if one day this
/// function wrote one.
export async function enrolerLaVm(
    p: Pilote,
    nom: string,
    adresse: string,
    _maintenant: number,
): Promise<Enrolement> {
    const vmId = randomUUID();
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [vmId, nom, adresse]);

    // 🔴 DRAWN AT RANDOM, never derived from the VM name: a derived secret would be
    // guessable by anyone who knows that name, and the enrolment
    // would no longer authenticate anything.
    const secret = randomBytes(OCTETS_SECRET).toString('base64url');
    await enroler(p, vmId, await hacher(secret), nouveauPrefixe());

    const ligne = await p.interroger<{ prefixe_session: string }>(
        'SELECT prefixe_session FROM agent_enrole WHERE vm_id = ?',
        [vmId],
    );
    return { vmId, prefixe: ligne[0].prefixe_session, secret };
}

/// What a rotation returns: the NEW secret, or a reasoned refusal.
export type Rotation = { vmId: string; prefixe: string; secret: string } | { refus: string };

/// Rotates the enrolment secret of an ALREADY enrolled VM.
///
/// 🔴 THE SESSION PREFIX IS NOT TOUCHED, AND THAT IS DELIBERATE. It makes up the
/// name of the LIVE sessions of this VM (`agents/prefixe.ts`, spec §3.4):
/// rotating it would cut all ongoing sessions, at the very moment when
/// the operator is reacting to a leak and least needs one more
/// outage. **Rotating the secret is not rotating the identity.** It is
/// moreover RETURNED to the caller, unchanged, so they see it with their own eyes.
///
/// 🔴 A NON-ENROLLED VM RETURNS A REASONED REFUSAL, never a silent success.
/// The `UPDATE` alone would touch zero rows without saying anything, and the administrator
/// would believe they had repaired a leak while the old secret stayed valid —
/// the worst possible result for a command used ONLY in that
/// case. ⚠️ There is NO enumeration oracle here, unlike the
/// `/agent` channel: the caller is the administrator, and the refusal tells them.
///
/// ⚠️ WHAT ROTATION DOES NOT DO: revoke the agent tokens ALREADY
/// issued, which stay valid until they expire. Same property as
/// human tokens (spec §3.5); the window is bounded by
/// `DUREE_JETON_ACCES_MS`, and the runbook says so.
export async function roterLeSecret(p: Pilote, vmId: string): Promise<Rotation> {
    const ligne = await lireParVm(p, vmId);
    if (ligne === undefined) {
        return {
            refus:
                `la VM ${vmId} n'est pas enrôlée : il n'y a aucun secret à faire ` +
                "tourner. Vérifier l'identifiant — c'est `vm_id`, celui qu'`--vm " +
                "<nom> --adresse <hôte>` a imprimé à l'enrôlement, pas le nom " +
                "d'affichage de la VM.",
        };
    }

    // 🔴 DRAWN AT RANDOM, exactly as at enrolment, and by the same call:
    // a rotation secret derived from anything at all would be guessable, and the
    // rotation would repair nothing.
    const secret = randomBytes(OCTETS_SECRET).toString('base64url');
    await remplacerEmpreinte(p, vmId, await hacher(secret));
    return { vmId, prefixe: ligne.prefixe_session, secret };
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
        // on a new database, and an `INSERT` on a missing table would give a
        // diagnosis unrelated to the cause.
        await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, Date.now());

        if (args.mode === 'roter') {
            const r = await roterLeSecret(base, args.vm);
            if ('refus' in r) {
                process.stderr.write(`rotation refusée : ${r.refus}\n`);
                return 2;
            }
            // ⚠️ THE SAME SPLIT AS AT ENROLMENT: the warning on
            // stderr, the value on stdout, so that standard output stays
            // usable in a pipe.
            process.stderr.write(
                'Le secret ci-dessous ne sera JAMAIS réaffiché : seule son empreinte est en base.\n' +
                    "Le préfixe de session est INCHANGÉ — les sessions en cours de cette VM ne sont pas coupées.\n" +
                    "⚠️ Les jetons d'agent DÉJÀ délivrés restent valides jusqu'à leur expiration.\n",
            );
            process.stdout.write(
                `vm_id=${r.vmId}\nprefixe=${r.prefixe}\nAGENT_SECRET=${r.secret}\n`,
            );
            return 0;
        }

        const { vmId, prefixe, secret } = await enrolerLaVm(
            base,
            args.vm,
            args.adresse,
            Date.now(),
        );
        // 🔴 THE ONE AND ONLY TIME the secret is written anywhere. It
        // goes to standard output with what the operator needs to
        // set `AGENT_VM` and `AGENT_SECRET`; the warning goes to
        // stderr, so that standard output stays usable in a pipe.
        process.stderr.write(
            'Le secret ci-dessous ne sera JAMAIS réaffiché : seule son empreinte est en base.\n',
        );
        process.stdout.write(`vm_id=${vmId}\nprefixe=${prefixe}\nAGENT_SECRET=${secret}\n`);
        return 0;
    } catch (cause) {
        process.stderr.write(`enrôlement refusé : ${String(cause)}\n`);
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
