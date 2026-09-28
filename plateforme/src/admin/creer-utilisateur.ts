// Account creation from the administration command line.
//
// Spec §4 says "account creation from the administration command line
// (no public sign-up in v1)" and stops there: the entry point and
// the convention are decided here.
//
//     npm run admin:utilisateur -- --email ada@exemple.test
//
// 🔴 THE PASSWORD IS READ FROM STANDARD INPUT, AND ONLY THERE. A
// `--mot-de-passe` on argv is REFUSED explicitly, with its reason:
// `ps` exposes the command line of every process to every user of the
// machine, and a secret passed that way would be readable by anyone for
// the whole duration of the call — then in the shell history.
//
// ⚠️ An email already taken returns a clear message and a non-zero exit code.
// There is NO enumeration oracle here, unlike the HTTP routes:
// the caller is the administrator, and hiding the failure would make them believe in
// an account that does not exist.

import { createInterface } from 'node:readline';
import { lireConfig } from '../config';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from '../base/migrations';
import { ouvrirBase } from '../base/ouvrir';
import { createUser } from '../depot/utilisateur';
import { hacher } from '../identite/mot-de-passe';

export type Arguments = { email: string } | { refus: string };

/// The flags that would try to pass a secret through argv. They are
/// enumerated rather than guessed: a broad pattern would one day refuse a
/// legitimate flag without anyone knowing why.
const DRAPEAUX_INTERDITS = ['--mot-de-passe', '--motdepasse', '--password', '--mdp', '-p'];

/// PURE, and tested on its own. Returns the address, or a refusal carrying its reason.
export function analyserArguments(argv: string[]): Arguments {
    for (const drapeau of DRAPEAUX_INTERDITS) {
        if (argv.includes(drapeau)) {
            // ⚠️ The reason does NOT copy the refused value: writing it again into
            // a log after refusing it in an argv would make no
            // sense.
            return {
                refus:
                    `${drapeau} est refusé : le mot de passe se lit sur l'entrée standard, ` +
                    "jamais sur la ligne de commande — `ps` l'exposerait à tout utilisateur " +
                    'de la machine.',
            };
        }
    }

    const i = argv.indexOf('--email');
    if (i === -1) {
        return { refus: "--email <adresse> est obligatoire, et n'a aucun défaut." };
    }
    const email = argv[i + 1];
    if (email === undefined || email === '') {
        return { refus: '--email attend une adresse non vide.' };
    }
    return { email };
}

/// Reads one line from standard input. Nothing is echoed back — a bare
/// `readline` would echo the password to the terminal.
function lireMotDePasse(invite: string): Promise<string> {
    const rl = createInterface({ input: process.stdin, terminal: false });
    process.stderr.write(invite);
    return new Promise((resolve) => {
        rl.once('line', (ligne) => {
            rl.close();
            resolve(ligne);
        });
    });
}

/// The impure body: read, hash, write. Returns the exit code.
export async function executer(argv: string[]): Promise<number> {
    const args = analyserArguments(argv);
    if ('refus' in args) {
        process.stderr.write(`${args.refus}\n`);
        return 2;
    }

    const motDePasse = await lireMotDePasse('mot de passe (entrée standard) : ');
    if (motDePasse === '') {
        process.stderr.write('mot de passe vide : aucun compte créé.\n');
        return 2;
    }

    const config = lireConfig(process.env);
    const base = await ouvrirBase(config);
    try {
        // Migrations first: the command may be the very first action
        // on a new database, and an `INSERT` on a missing table would give a
        // diagnosis unrelated to the cause.
        await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, Date.now());
        const id = await createUser(
            base,
            args.email,
            await hacher(motDePasse),
            Date.now(),
        );
        // The created identifier, and NOTHING ELSE, on standard output: that is
        // what makes the command usable in a pipe.
        process.stdout.write(`${id}\n`);
        return 0;
    } catch (cause) {
        process.stderr.write(`création refusée : ${String(cause)}\n`);
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
