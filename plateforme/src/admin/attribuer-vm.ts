// L'attribution d'une VM à un utilisateur, par ligne de commande.
//
//     npm run admin:attribuer -- --email ada@exemple.test --vm w1
//     npm run admin:attribuer -- --detacher --vm w1
//
// 🔴 POURQUOI CE N'EST PAS UNE ROUTE HTTP. Il n'existe AUCUN rôle
// d'administration dans ce service : `identite/jeton.ts` ne connaît que
// `utilisateur` et `agent`, et `config.ts` n'a aucune variable
// d'administrateur. Une route qui attribuerait une VM serait donc, au mieux,
// ouverte à tout utilisateur authentifié — une escalade de privilège offerte.
// Le précédent est exact : `admin:utilisateur` et `admin:agent` (D8).
//
// 🔴 ET C'EST POURQUOI ELLE NOMME LA CAUSE, à l'inverse des routes.
// `http/routes-vm.ts` rend le même refus pour « VM inconnue » et « VM
// d'autrui », parce qu'une route publique qui les distinguerait serait un
// oracle d'énumération. Ici l'appelant a déjà l'accès à la base et au secret de
// configuration : l'énumération n'est pas un risque, et lui cacher la cause le
// ferait chercher ailleurs (E8).
//
// ⚠️ CETTE COMMANDE NE CRÉE AUCUNE VM. Le seul chemin de création reste
// `admin:agent`. Elle pose un propriétaire sur une VM déjà enrôlée, et rien de
// plus — c'est tout ce que « statique » autorise (D1).

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

/// Les drapeaux qui tenteraient de faire passer un secret par l'argv.
///
/// ⚠️ AUCUN SECRET N'EST EN JEU DANS CETTE COMMANDE, et la liste des deux
/// autres est reprise TELLE QUELLE quand même : une commande qui accepterait
/// `--mot-de-passe` sans s'en servir laisserait tout de même la chaîne dans
/// `ps`, où tout utilisateur de la machine la lirait, puis dans l'historique du
/// shell. Ils sont ÉNUMÉRÉS plutôt que devinés : un motif large refuserait un
/// jour un drapeau légitime sans qu'on sache pourquoi.
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

/// PURE, et testée seule.
export function analyserArguments(argv: string[]): Arguments {
    for (const drapeau of DRAPEAUX_INTERDITS) {
        if (argv.includes(drapeau)) {
            // ⚠️ Le motif ne RECOPIE PAS la valeur refusée.
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

    // 🔴 `--detacher` N'EXIGE PAS DE COURRIEL : on détache une VM DE quelqu'un,
    // et exiger de nommer ce quelqu'un obligerait l'opérateur à savoir d'avance
    // ce que la commande va lui apprendre.
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
    /// 0 = fait, 2 = refus nommé. ⚠️ 1 est réservé à l'imprévu, comme dans les
    /// deux autres commandes.
    code: number;
    sortie?: string;
    erreur?: string;
}

/// Résout `--vm` : le NOM d'abord, l'identifiant ensuite.
///
/// ⚠️ L'ORDRE EST DÉLIBÉRÉ : l'administrateur connaît le nom qu'il a donné à
/// `admin:agent`, et l'identifiant est un UUID que la commande a tiré au sort.
/// ⚠️ `vm.nom` n'étant pas UNIQUE (`0001-socle.sql`), deux VMs homonymes font
/// rendre la première — c'est acceptable pour un opérateur qui voit le
/// résultat, et ce ne le serait pas sur une route.
async function resoudre(p: Pilote, designation: string): Promise<LigneVm | undefined> {
    return (await lireParNom(p, designation)) ?? (await lireParId(p, designation));
}

/// Le cœur, testable : il prend un `Pilote` et rend l'issue, sans lire aucune
/// configuration ni écrire sur aucun flux. Même découpage que
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

    // 🔴 L'ATTRIBUTION PASSE PAR L'ORCHESTRATEUR, jamais par un `UPDATE` écrit
    // ici. C'est lui qui porte l'ordre « lire, écrire sous clause, traduire
    // l'exception » (D4), et le dupliquer ferait diverger les deux chemins le
    // jour où l'un changerait — la commande d'administration étant précisément
    // celle qu'on relit le moins souvent.
    //
    // ⚠️ L'horloge n'est employée par aucun chemin d'`attribuer` ; elle est
    // passée parce que l'interface l'exige, et `Date.now` est honnête ici.
    const orchestrateur = inventaireStatique(p, Date.now);
    const issue = await orchestrateur.attribuer(ligne.id, utilisateur.id);
    if (issue.ok) {
        return {
            code: 0,
            sortie: `vm=${ligne.id}\nnom=${ligne.nom}\nutilisateur=${utilisateur.id}\nemail=${args.email}\n`,
        };
    }

    // 🔴 LE REFUS TYPÉ EST TRADUIT EN UNE PHRASE, JAMAIS RELAYÉ TEL QUEL NI
    // LAISSÉ SOUS FORME D'EXCEPTION. Une trace de pile portant `UNIQUE
    // constraint failed` — ou `duplicate key value violates unique constraint`,
    // l'autre moteur n'écrivant pas la même chose — n'apprend pas à
    // l'administrateur quoi faire.
    switch (issue.motif) {
        case 'vm-deja-attribuee': {
            // On NOMME le propriétaire : sans lui, l'opérateur saurait que ça a
            // échoué sans savoir qui détacher, et devrait ouvrir la base à la
            // main — ce que cette commande existe pour éviter.
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
            // Inatteignable avec les entrées ci-dessus (la VM a été résolue,
            // donc jamais `vm-inconnue`), écrit quand même : un motif ajouté un
            // jour ne doit pas tomber dans un silence.
            return { code: 2, erreur: `attribution refusée : ${issue.motif}.` };
    }
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
        // sur une base neuve, et un `UPDATE` sur une table absente rendrait un
        // diagnostic sans rapport avec la cause. Même choix que les deux autres
        // commandes.
        await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, Date.now());
        const issue = await appliquer(base, args);
        if (issue.erreur !== undefined) process.stderr.write(`${issue.erreur}\n`);
        if (issue.sortie !== undefined) process.stdout.write(issue.sortie);
        return issue.code;
    } catch (cause) {
        // Code 1, réservé à l'imprévu : un refus nommé rend 2.
        process.stderr.write(`attribution impossible : ${String(cause)}\n`);
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
