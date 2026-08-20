// Le VERDICT de la corroboration sur VM : chaque assertion, son obtenu, son
// attendu, et un code de sortie.
//
// 🔴 IL LIT DES PIÈCES ÉCRITES PAR LE SERVICE, JAMAIS DES VARIABLES DU SCRIPT.
// Les corps des trois `POST /session` et des deux `GET /vm` sont sur le disque ;
// le seul argument venu d'ailleurs est le préfixe ANNONCÉ par l'enrôlement,
// et il n'est là que pour être CONFRONTÉ à celui que la route rend. Une
// assertion qui comparerait une variable à elle-même ne pourrait pas échouer,
// et ce dépôt en a attrapé quatre au seul sous-bloc D10.
import { readFileSync } from 'node:fs';
import path from 'node:path';

const [sortie, prefixeEnrole] = process.argv.slice(2);
const lire = (f) => {
    try { return JSON.parse(readFileSync(path.join(sortie, f), 'utf8')); }
    catch { return undefined; }
};
const texte = (f) => {
    try { return readFileSync(path.join(sortie, f), 'utf8'); } catch { return ''; }
};

const avant = lire('session-avant-attribution.json');
const muet = lire('session-agent-muet.json');
const vivant = lire('session-agent-vivant.json');
const vms = lire('vm.json');
const vmsApres = lire('vm-apres.json');
const shell = texte('shell.log');
const agent = texte('agent-plat.log');

let echecs = 0;
const dire = (libelle, obtenu, attendu) => {
    const tenu = JSON.stringify(obtenu) === JSON.stringify(attendu);
    if (!tenu) echecs += 1;
    const verdict = tenu
        ? 'TENU'
        : `🔴 NON TENU — obtenu ${JSON.stringify(obtenu)}, attendu ${JSON.stringify(attendu)}`;
    console.log(`    ${libelle.padEnd(58)}: ${verdict}`);
};

console.log('');
console.log('    --- ⓐ la chaîne de refus, sur le service réel ---');
dire('ⓐ sans attribution, la route refuse `aucune-vm`', avant?.motif, 'aucune-vm');
dire('ⓐ et elle ne délivre AUCUN préfixe', avant?.prefixe, undefined);
console.log('    --- ⓑ attribuée mais agent MUET : l’aveu, pas la fonction ---');
dire('ⓑ l’état annoncé est `injoignable`', muet?.etat, 'injoignable');
dire('ⓑ la plateforme AVOUE ne pas savoir redémarrer', muet?.redemarrage?.possible, false);
dire('ⓑ et le préfixe est rendu QUAND MÊME', typeof muet?.prefixe, 'string');
console.log('    --- ⓒ agent VIVANT : c’est le battement réel qui décide ---');
dire('ⓒ l’état annoncé est `prete`', vivant?.etat, 'prete');
dire('ⓒ aucun aveu de redémarrage sur une VM prête', vivant?.redemarrage, undefined);
dire('ⓒ le préfixe RENDU est celui de la VM enrôlée', vivant?.prefixe, prefixeEnrole);
dire('ⓒ GET /vm annonce le même état', vms?.vms?.[0]?.etat, 'prete');
console.log('    --- ⓓ la page-shell, sur le préfixe RENDU PAR LA ROUTE ---');
const nomBureau = `${prefixeEnrole}:bureau`;
// ⚠️ CE QUE CETTE ASSERTION ÉTABLIT, ET RIEN DE PLUS : que la poignée de main
// du client HUMAIN est acceptée sur un nom de session composé à partir du
// préfixe QUE LA ROUTE A RENDU. Elle ne dit RIEN de la présence de l'agent en
// face — une première rédaction cherchait la sous-chaîne « bureau » n'importe
// où dans le journal, ce qui l'aurait rendue vraie sur une shell parlant toute
// seule. Le pair d'en face est jugé par ⓕ, sur le journal de l'AGENT.
dire('ⓓ la poignée de main humaine est acceptée sur <préfixe>:bureau',
    shell.includes(`poignée de main client sur ${nomBureau}`), true);
dire('ⓓ et le relais lui délivre son `ice-config` pour CETTE session',
    shell.includes(`"username":"`) && shell.includes(`:${nomBureau}"`), true);
console.log('    --- ⓕ l’agent, en face : enrôlé, ou refusé ? ---');
const refusVersion = (agent.match(/version de plateforme non support/g) ?? []).length;
const reprises = (agent.match(/reprise du canal \/agent/g) ?? []).length;
console.log(`    ${'ⓕ refus de version relevés au journal de l’agent'.padEnd(58)}: ${refusVersion}`);
console.log(`    ${'ⓕ reprises du canal /agent relevées'.padEnd(58)}: ${reprises}`);
dire('ⓕ l’agent est enrôlé auprès de la plateforme',
    /agent enr[oô]l[ée] aupr/i.test(agent), true);
dire('ⓕ aucune ligne ERROR au journal de l’agent',
    (agent.match(/\bERROR\b/g) ?? []).length, 0);
console.log(`    ${'ⓕ (pour mémoire) lignes WARN au journal de l’agent'.padEnd(58)}: ${(agent.match(/\bWARN\b/g) ?? []).length}`);
console.log('    --- ⓔ le lecteur de session.utilisateur_id (legs n°3 de P3) ---');
dire('ⓔ GET /vm porte le champ `sessions_ouvertes`',
    typeof vmsApres?.vms?.[0]?.sessions_ouvertes, 'number');

console.log('');
console.log(`    VERDICT : ${echecs === 0 ? 'TOUT TENU' : `🔴 ${echecs} assertion(s) NON TENUE(S)`}`);
process.exit(echecs === 0 ? 0 : 1);
