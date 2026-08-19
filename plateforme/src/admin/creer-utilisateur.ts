// La création de compte par ligne de commande d'administration.
//
// La spec §4 dit « création de compte par ligne de commande d'administration
// (pas d'inscription publique en v1) » et s'arrête là : le point d'entrée et
// la convention sont décidés ici.
//
//     npm run admin:utilisateur -- --email ada@exemple.test
//
// 🔴 LE MOT DE PASSE SE LIT SUR L'ENTRÉE STANDARD, ET LÀ SEULEMENT. Un
// `--mot-de-passe` sur l'argv est REFUSÉ explicitement, avec son motif :
// `ps` expose la ligne de commande de tout processus à tout utilisateur de la
// machine, et un secret passé ainsi serait lisible par n'importe qui pendant
// toute la durée de l'appel — puis dans l'historique du shell.
//
// ⚠️ Un courriel déjà pris rend un message clair et un code de sortie non nul.
// Il n'y a PAS d'oracle d'énumération ici, contrairement aux routes HTTP :
// l'appelant est l'administrateur, et lui cacher l'échec lui ferait croire à
// un compte qui n'existe pas.

import { createInterface } from 'node:readline';
import { lireConfig } from '../config';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from '../base/migrations';
import { ouvrirBase } from '../base/ouvrir';
import { creerUtilisateur } from '../depot/utilisateur';
import { hacher } from '../identite/mot-de-passe';

export type Arguments = { email: string } | { refus: string };

/// Les drapeaux qui tenteraient de faire passer un secret par l'argv. Ils sont
/// énumérés plutôt que devinés : un motif large refuserait un jour un drapeau
/// légitime sans qu'on sache pourquoi.
const DRAPEAUX_INTERDITS = ['--mot-de-passe', '--motdepasse', '--password', '--mdp', '-p'];

/// PURE, et testée seule. Rend l'adresse, ou un refus qui porte son motif.
export function analyserArguments(argv: string[]): Arguments {
    for (const drapeau of DRAPEAUX_INTERDITS) {
        if (argv.includes(drapeau)) {
            // ⚠️ Le motif ne recopie PAS la valeur refusée : la réécrire dans
            // un journal après l'avoir refusée dans un argv n'aurait aucun
            // sens.
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

/// Lit une ligne sur l'entrée standard. Rien n'est réaffiché — un `readline`
/// nu ferait l'écho du mot de passe au terminal.
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

/// Le corps impur : lecture, hachage, écriture. Rend le code de sortie.
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
        // Les migrations d'abord : la commande peut être le tout premier geste
        // sur une base neuve, et un `INSERT` sur une table absente rendrait un
        // diagnostic sans rapport avec la cause.
        await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, Date.now());
        const id = await creerUtilisateur(
            base,
            args.email,
            await hacher(motDePasse),
            Date.now(),
        );
        // L'identifiant créé, et RIEN D'AUTRE, sur la sortie standard : c'est
        // ce qui rend la commande utilisable dans un tube.
        process.stdout.write(`${id}\n`);
        return 0;
    } catch (cause) {
        process.stderr.write(`création refusée : ${String(cause)}\n`);
        return 1;
    } finally {
        await base.fermer();
    }
}

// Exécuté seulement quand ce fichier EST le point d'entrée : sans cette garde,
// l'importer depuis un test lancerait la commande.
if (process.argv[1] && import.meta.url.endsWith(process.argv[1].replace(/\\/g, '/'))) {
    process.exitCode = await executer(process.argv.slice(2));
}
