// L'enrôlement d'une VM par ligne de commande d'administration.
//
//     npm run admin:agent -- --vm w1 --adresse 192.168.3.2
//
// 🔴 LE SECRET EST TIRÉ AU SORT PAR LA COMMANDE, ET ÉCRIT UNE SEULE FOIS SUR
// STDOUT. Il n'est jamais relisible : seule son empreinte va en base. Un
// `--secret` sur l'argv est REFUSÉ explicitement, avec son motif — `ps` expose
// la ligne de commande de tout processus à tout utilisateur de la machine, et
// un secret passé ainsi serait lisible par n'importe qui pendant toute la
// durée de l'appel, puis dans l'historique du shell. C'est le précédent exact
// de `creer-utilisateur.ts`, repris à la lettre.
//
// ⚠️ CE QU'ON EN FAIT ENSUITE N'EST PAS PROTÉGÉ, et il faut le dire ici :
// le secret est destiné à `AGENT_SECRET` dans `scripts/run-agent.sh`, qui
// l'écrit EN CLAIR dans `C:\dev\run-agent.ps1` sur un partage CIFS lisible
// depuis l'hôte — comme les cinquante-sept autres variables. C'est acceptable
// pour une VM de développement, et c'est à rouvrir au sous-bloc P5.
//
// ⚠️ Une VM déjà enrôlée fait LEVER, par la clé primaire de `agent_enrole`.
// Il n'y a PAS d'oracle d'énumération ici, contrairement au canal `/agent` :
// l'appelant est l'administrateur, et lui cacher l'échec lui ferait croire à
// un enrôlement qui n'existe pas.

import { randomBytes, randomUUID } from 'node:crypto';
import { lireConfig } from '../config';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from '../base/migrations';
import { ouvrirBase } from '../base/ouvrir';
import type { Pilote } from '../base/pilote';
import { enroler } from '../depot/agent';
import { hacher } from '../identite/mot-de-passe';
import { nouveauPrefixe } from '../agents/prefixe';

export type Arguments = { vm: string; adresse: string } | { refus: string };

/// Les drapeaux qui tenteraient de faire passer un secret par l'argv. Ils sont
/// ÉNUMÉRÉS plutôt que devinés : un motif large refuserait un jour un drapeau
/// légitime sans qu'on sache pourquoi. Même choix que `creer-utilisateur.ts`.
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

/// 32 octets, soit 43 caractères de `base64url`. ⚠️ NON CALIBRÉE : c'est la
/// longueur usuelle d'un secret de 256 bits, pas un seuil mesuré. Elle rejoint
/// la liste des constantes non calibrées du dépôt.
const OCTETS_SECRET = 32;

/// PURE, et testée seule. Rend la paire, ou un refus qui porte son motif.
export function analyserArguments(argv: string[]): Arguments {
    for (const drapeau of DRAPEAUX_INTERDITS) {
        if (argv.includes(drapeau)) {
            // ⚠️ Le motif ne RECOPIE PAS la valeur refusée.
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
    const adresse = lire('--adresse');
    if (adresse === undefined || adresse === '') {
        return { refus: "--adresse <hôte> est obligatoire, et n'a aucun défaut." };
    }
    return { vm, adresse };
}

export interface Enrolement {
    vmId: string;
    prefixe: string;
    /// 🔴 Rendu UNE SEULE FOIS. Il n'existe nulle part ailleurs : seule son
    /// empreinte est écrite, et rien ne permet de le retrouver ensuite.
    secret: string;
}

/// Crée la VM et son enrôlement, et rend le secret en clair À L'APPELANT SEUL.
///
/// L'horloge est un PARAMÈTRE, comme partout dans ce dépôt : c'est ce qui
/// rendrait une assertion sur une valeur exacte possible si un jour cette
/// fonction en écrivait une.
export async function enrolerLaVm(
    p: Pilote,
    nom: string,
    adresse: string,
    _maintenant: number,
): Promise<Enrolement> {
    const vmId = randomUUID();
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [vmId, nom, adresse]);

    // 🔴 TIRÉ AU SORT, jamais dérivé du nom de VM : un secret dérivé serait
    // devinable par quiconque connaît ce nom, et l'enrôlement
    // n'authentifierait plus rien.
    const secret = randomBytes(OCTETS_SECRET).toString('base64url');
    await enroler(p, vmId, await hacher(secret), nouveauPrefixe());

    const ligne = await p.interroger<{ prefixe_session: string }>(
        'SELECT prefixe_session FROM agent_enrole WHERE vm_id = ?',
        [vmId],
    );
    return { vmId, prefixe: ligne[0].prefixe_session, secret };
}

/// Le corps impur. Rend le code de sortie.
export async function executer(argv: string[]): Promise<number> {
    const args = analyserArguments(argv);
    if ('refus' in args) {
        process.stderr.write(`${args.refus}\n`);
        return 2;
    }

    const config = lireConfig(process.env);
    const base = await ouvrirBase(config);
    try {
        // Les migrations d'abord : la commande peut être le tout premier geste
        // sur une base neuve, et un `INSERT` sur une table absente rendrait un
        // diagnostic sans rapport avec la cause.
        await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, Date.now());
        const { vmId, prefixe, secret } = await enrolerLaVm(
            base,
            args.vm,
            args.adresse,
            Date.now(),
        );
        // 🔴 LA SEULE ET UNIQUE FOIS où le secret est écrit quelque part. Il
        // va sur la sortie standard avec ce dont l'exploitant a besoin pour
        // poser `AGENT_VM` et `AGENT_SECRET` ; l'avertissement, lui, va sur
        // stderr, pour que la sortie standard reste utilisable dans un tube.
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

// Exécuté seulement quand ce fichier EST le point d'entrée : sans cette garde,
// l'importer depuis un test lancerait la commande.
if (process.argv[1] && import.meta.url.endsWith(process.argv[1].replace(/\\/g, '/'))) {
    process.exitCode = await executer(process.argv.slice(2));
}
