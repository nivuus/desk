// La MESURE qui fonde la borne du critère ③, prise AVANT que la borne ne soit
// jugée digne de foi.
//
//     tsx mesure-borne-3.ts <sqlite|postgres> <commit>
//
// 🔴 ELLE NE MESURE PAS « LA LATENCE DU PRODUIT », et le dire importe : elle
// mesure un aller-retour HTTP sur la boucle locale, contre un service en cours
// de démarrage à froid, sur une machine qui porte par ailleurs une VM et un
// chantier concurrent. Ce qu'elle établit est un ORDRE DE GRANDEUR — celui
// d'un refus qui ne comporte ni attente, ni tentative de réveil, ni
// scrutation. La borne du critère est posée deux ordres de grandeur au-dessus
// du pire cas RELEVÉ HORS DÉMARRAGE À FROID, pour qu'elle ne puisse pas rougir
// sur une machine chargée tout en restant huit fois sous la rouge prescrite
// (`setTimeout(2000)`). Les deux rapports sont écrits dans le journal.

import { appeler, creerCompte, demarrerService, enroler, jetonDe, ligne, type Moteur } from './socle';
import { inventaireStatique } from '../../../../../plateforme/src/orchestration/inventaire-statique';

const [moteur, commit] = process.argv.slice(2) as [Moteur, string];
const N = 100;

const service = await demarrerService(moteur, 'mesure3');
const carol = await creerCompte(service.base, 'carol@essai.local');
const autre = await creerCompte(service.base, 'autre@essai.local');
const vm = await enroler(service.base, 'w-autre');
await inventaireStatique(service.base, Date.now).attribuer(vm.vmId, autre);
const jeton = jetonDe(carol);

const durees: number[] = [];
for (let i = 0; i < N; i += 1) {
    const r = await appeler(service.port, '/session', 'POST', jeton);
    if (r.code !== 409) throw new Error(`code inattendu ${r.code} — la mesure ne porterait pas sur un refus`);
    durees.push(r.dureeMs);
}
await service.arreter();

const tri = [...durees].sort((a, b) => a - b);
const q = (p: number) => Number(tri[Math.floor(p * (N - 1))].toFixed(3));
process.stdout.write(
    [
        `# MESURE DE LA BORNE DU CRITÈRE ③ — ${N} refus 409 consécutifs, moteur ${moteur}`,
        `# Prise le ${new Date().toISOString()}, commit ${commit}`,
        '#',
        ligne('appels', N),
        // 🔴 LE PREMIER APPEL EST SORTI DU LOT, et ce n'est pas pour flatter le
        // chiffre : il paie le démarrage à froid (compilation du chemin,
        // première connexion), et le CRITÈRE ne le mesure jamais — ses vingt
        // appels chronométrés viennent tous après un premier appel déjà fait.
        // Le confondre avec les autres majorerait la borne pour une raison qui
        // ne se produit pas dans ce qu'on juge.
        ligne('PREMIER appel, à froid (ms)', Number(durees[0].toFixed(3))),
        ligne('maximum des 99 SUIVANTS (ms)', Number(Math.max(...durees.slice(1)).toFixed(3))),
        ligne('minimum (ms)', q(0)),
        ligne('médiane (ms)', q(0.5)),
        ligne('p90 (ms)', q(0.9)),
        ligne('p99 (ms)', q(0.99)),
        ligne('MAXIMUM (ms)', q(1)),
        '',
    ].join('\n'),
);
