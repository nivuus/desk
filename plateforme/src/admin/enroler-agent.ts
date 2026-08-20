// L'enrôlement d'une VM par ligne de commande d'administration.
//
//     npm run admin:agent -- --vm w1 --adresse 192.168.3.2      (enrôler)
//     npm run admin:agent -- --vm <id> --roter                  (faire tourner)
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
// depuis l'hôte — comme les cinquante-sept autres variables.
//
// 🔴 LE SOUS-BLOC P5 A ROUVERT CE POINT, ET IL NE LE CORRIGE PAS : IL LE REND
// RÉPARABLE. Retirer le secret de ce fichier exigerait de modifier `scripts/`,
// de faire lire à `agent/` un coffre Windows (DPAPI), et d'éprouver le
// résultat SUR LA VM — trois choses hors de son périmètre. Livrer un demi-
// remède non éprouvé serait pire que de déclarer le manque.
//
// **La contrepartie est `--roter`, et elle n'existait pas.** Avant elle, un
// exploitant qui apprenait qu'un secret avait fuité n'avait AUCUN moyen de le
// remplacer : `enroler` ne sait qu'INSÉRER, `vm_id` est clé primaire
// (`0003-agents.sql`), donc réenrôler une VM déjà enrôlée LÈVE. Il ne restait
// que le `DELETE` manuel en base. Le vol d'un secret est désormais réparable.
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
import { enroler, lireParVm, remplacerEmpreinte } from '../depot/agent';
import { hacher } from '../identite/mot-de-passe';
import { nouveauPrefixe } from '../agents/prefixe';

/// Les deux gestes de cette commande, et le refus.
///
/// ⚠️ `mode` EST EXPLICITE plutôt que déduit de la présence d'`adresse` : un
/// jour où un troisième geste apparaîtrait, la déduction se tromperait en
/// silence, là où un champ nommé oblige à trancher.
export type Arguments =
    | { mode: 'enroler'; vm: string; adresse: string }
    | { mode: 'roter'; vm: string }
    | { refus: string };

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

    // 🔴 LA ROTATION N'EXIGE PAS `--adresse`, et ce n'est pas une commodité :
    // elle ne touche PAS la table `vm`. Exiger une adresse inviterait à en
    // saisir une au hasard, qui serait ignorée — un paramètre qu'on demande
    // sans l'employer finit par être cru employé.
    //
    // ⚠️ LE CONTRÔLE DES DRAPEAUX INTERDITS EST EN AMONT DE CETTE BRANCHE, donc
    // il couvre `--roter` aussi. C'est le chemin qu'on emprunte précisément
    // quand un secret a fuité : y laisser passer un `--secret` sur l'argv
    // rejouerait la fuite qu'on est en train de réparer.
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

/// Ce qu'une rotation rend : le secret NEUF, ou un refus motivé.
export type Rotation = { vmId: string; prefixe: string; secret: string } | { refus: string };

/// Fait tourner le secret d'enrôlement d'une VM DÉJÀ enrôlée.
///
/// 🔴 LE PRÉFIXE DE SESSION N'EST PAS TOUCHÉ, ET C'EST DÉLIBÉRÉ. Il compose le
/// nom des sessions VIVANTES de cette VM (`agents/prefixe.ts`, spec §3.4) : le
/// faire tourner couperait toutes les sessions en cours, au moment même où
/// l'exploitant réagit à une fuite et où il a le moins besoin d'une panne de
/// plus. **Rotation du secret n'est pas rotation de l'identité.** Il est
/// d'ailleurs RENDU à l'appelant, inchangé, pour qu'il le voie de ses yeux.
///
/// 🔴 UNE VM NON ENRÔLÉE REND UN REFUS MOTIVÉ, jamais un succès silencieux.
/// L'`UPDATE` seul toucherait zéro ligne sans rien dire, et l'administrateur
/// croirait avoir réparé une fuite alors que l'ancien secret resterait valide —
/// le pire résultat possible pour une commande qu'on n'emploie QUE dans ce
/// cas-là. ⚠️ Il n'y a PAS d'oracle d'énumération ici, contrairement au canal
/// `/agent` : l'appelant est l'administrateur, et le refus le lui dit.
///
/// ⚠️ CE QUE LA ROTATION NE FAIT PAS : révoquer les jetons d'agent DÉJÀ
/// délivrés, qui restent valides jusqu'à leur expiration. Même propriété que
/// les jetons humains (spec §3.5) ; la fenêtre est bornée par
/// `DUREE_JETON_ACCES_MS`, et le runbook l'écrit.
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

    // 🔴 TIRÉ AU SORT, exactement comme à l'enrôlement, et par le même appel :
    // un secret de rotation dérivé de quoi que ce soit serait devinable, et la
    // rotation ne réparerait rien.
    const secret = randomBytes(OCTETS_SECRET).toString('base64url');
    await remplacerEmpreinte(p, vmId, await hacher(secret));
    return { vmId, prefixe: ligne.prefixe_session, secret };
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

        if (args.mode === 'roter') {
            const r = await roterLeSecret(base, args.vm);
            if ('refus' in r) {
                process.stderr.write(`rotation refusée : ${r.refus}\n`);
                return 2;
            }
            // ⚠️ LE MÊME PARTAGE QU'À L'ENRÔLEMENT : l'avertissement sur
            // stderr, la valeur sur stdout, pour que la sortie standard reste
            // utilisable dans un tube.
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
