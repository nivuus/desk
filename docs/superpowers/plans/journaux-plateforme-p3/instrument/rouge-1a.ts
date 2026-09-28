// 🔴 LA ROUGE GRATUITE DU CRITÈRE ① — jouée sur le SERVICE DE P2, pas sur une
// mutation du nôtre.
//
//     tsx rouge-1a.ts <racine-du-worktree-P2>
//
// La spec §7.2 l'annonce : « P3 ① : le service de P2 refuse le second agent
// sur `bureau` », et « ce sont des ROUGES à JOUER, pas à supposer ». Cette
// sonde monte le service du DERNIER commit de P2 (`f0b2fca`) dans un
// `git worktree`, et y fait exactement ce que fait le chantier D à deux VMs :
// deux superviseurs, chacun ouvrant SA session de contrôle. Sans préfixe, les
// deux la nomment `bureau` — une constante littérale de
// `agent/src/superviseur/protocole.rs` — et se disputent la même entrée de la
// table d'appariement.
//
// ⚠️ AUCUN JETON N'EST PRÉSENTÉ, ET C'EST LE COMPORTEMENT DE P2 : sa garde
// rend `{ok:true}` sans rien exiger pour le rôle `agent` (E2, mesurée par
// `journaux-plateforme-p2/e2-role-agent-toujours-anonyme.log`). Présenter un
// jeton ici serait un anachronisme — le binaire de P2 n'en délivre aucun.
//
// 🔴 LE WORKTREE N'EST PAS UN `git checkout` DE L'ARBRE DE TRAVAIL. Rien de
// non commité n'est touché : `git worktree add` crée un arbre SÉPARÉ, et
// `plateforme/node_modules` y est un lien symbolique vers celui de l'arbre
// principal, le worktree n'en ayant aucun.

import { pathToFileURL } from 'node:url';
import { ligne, Pair, poignee } from './socle';

const racine = process.argv[2];
if (!racine) throw new Error('usage : rouge-1a.ts <racine-du-worktree-P2>');

// Import DYNAMIQUE : le chemin n'est connu qu'à l'exécution, et il pointe
// hors de ce dépôt-ci.
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
