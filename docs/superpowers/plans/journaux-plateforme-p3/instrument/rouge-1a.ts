// 🔴 THE FREE RED RUN OF CRITERION ① — played on P2'S SERVICE, not on a
// mutation of ours.
//
//     tsx rouge-1a.ts <racine-du-worktree-P2>
//
// Spec §7.2 announces it: "P3 ①: P2's service refuses the second agent
// on `bureau`", and "these are RED RUNS to PLAY, not to assume". This
// probe mounts the service of P2's LAST commit (`f0b2fca`) in a
// `git worktree`, and does there exactly what workstream D does with two VMs:
// two supervisors, each opening ITS control session. Without a prefix, both
// name it `bureau` — a literal constant of
// `agent/src/superviseur/protocole.rs` — and fight over the same entry of the
// pairing table.
//
// ⚠️ NO TOKEN IS PRESENTED, AND IT IS P2'S BEHAVIOUR: its guard
// returns `{ok:true}` without requiring anything for the `agent` role (E2, measured by
// `journaux-plateforme-p2/e2-role-agent-toujours-anonyme.log`). Presenting a
// token here would be an anachronism — P2's binary delivers none.
//
// 🔴 THE WORKTREE IS NOT A `git checkout` OF THE WORKING TREE. Nothing
// uncommitted is touched: `git worktree add` creates a SEPARATE tree, and
// `plateforme/node_modules` is a symbolic link there to the main tree's,
// the worktree having none.

import { pathToFileURL } from 'node:url';
import { ligne, Pair, poignee } from './socle';

const racine = process.argv[2];
if (!racine) throw new Error('usage : rouge-1a.ts <racine-du-worktree-P2>');

// DYNAMIC import: the path is only known at runtime, and it points
// outside this very repository.
const { lireConfig } = (await import(
    pathToFileURL(`${racine}/plateforme/src/config.ts`).href
)) as typeof import('../../../../../plateforme/src/config');
const { demarrer } = (await import(
    pathToFileURL(`${racine}/plateforme/src/demarrage.ts`).href
)) as typeof import('../../../../../plateforme/src/demarrage');

const service = await demarrer(
    lireConfig({
        PLATEFORME_HOTE: '127.0.0.1',
        PLATEFORME_PORT: '0',
        PLATEFORME_BASE: 'sqlite',
        PLATEFORME_BASE_URL: ':memory:',
        PLATEFORME_SECRET_JETON: '***RETIRE-DE-L-HISTORIQUE***',
    }),
);

const sortie: string[] = [];
const dire = (t = ''): void => void sortie.push(t);

dire('# 🔴 ROUGE ①A — LA ROUGE GRATUITE, jouée sur le SERVICE DE P2');
dire(`# Commit du service mesuré : f0b2fca (dernier commit du sous-bloc P2)`);
dire(`# Jouée le ${new Date().toISOString()}`);
dire('#');
dire("# Deux VMs, deux superviseurs, aucun préfixe — l'état d'avant P3.");
dire();

const url = `ws://127.0.0.1:${service.port}/`;
dire(ligne('service', url));

const premier = await Pair.ouvrir(url);
premier.envoyer(poignee('agent', 'bureau'));
await premier.attendre(1);
dire(ligne('agent de la VM 1 -> bureau', premier.recues.map((t) => t.brut)));

const second = await Pair.ouvrir(url);
second.envoyer(poignee('agent', 'bureau'));
await second.attendre(1);
const brutSecond = second.recues.map((t) => t.brut);
dire(ligne('agent de la VM 2 -> bureau', brutSecond));

const erreurs = brutSecond
    .map((b) => JSON.parse(b) as { type?: string; reason?: string })
    .filter((m) => m.type === 'error');
const motif = erreurs[0]?.reason;
dire();
dire(ligne('motif reçu par le SECOND, verbatim', motif ?? '<aucun>'));
const attendu = 'un agent est déjà connecté à la session bureau';
dire(ligne('motif attendu (spec §4 P3 ①)', attendu));
const rouge = motif === attendu;
dire(ligne('🔴 LA ROUGE EST VUE', rouge ? 'OUI' : 'NON — la rouge ne se produit pas'));
dire();
dire('# Sur le service de P3, ce même geste ne peut plus se produire : les deux');
dire('# superviseurs nomment leur session <préfixe>:bureau, et les deux préfixes');
dire('# sont tirés à 128 bits (voir critere-1-1.log et critere-1-2.log).');

premier.fermer();
second.fermer();
await service.arreter();
process.stdout.write(`${sortie.join('\n')}\n`);
process.exit(rouge ? 0 : 1);
