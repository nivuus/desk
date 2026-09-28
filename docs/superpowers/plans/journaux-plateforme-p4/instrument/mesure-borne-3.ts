// The MEASUREMENT that grounds criterion ③'s bound, taken BEFORE the bound is
// deemed trustworthy.
//
//     tsx mesure-borne-3.ts <sqlite|postgres> <commit>
//
// 🔴 IT DOES NOT MEASURE "THE PRODUCT'S LATENCY", and saying so matters: it
// measures an HTTP round trip on the loopback, against a service in the middle
// of a cold start, on a machine that also carries a VM and a
// concurrent workstream. What it establishes is an ORDER OF MAGNITUDE — that
// of a refusal involving neither waiting, nor wake-up attempt, nor
// polling. The criterion's bound is set two orders of magnitude above
// the worst case RECORDED OUTSIDE COLD START, so that it cannot turn red
// on a loaded machine while staying eight times below the prescribed red run
// (`setTimeout(2000)`). Both ratios are written in the log.

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
        // 🔴 THE FIRST CALL IS TAKEN OUT OF THE BATCH, and not to flatter the
        // figure: it pays the cold start (path compilation,
        // first connection), and the CRITERION never measures it — its twenty
        // timed calls all come after a first call already made.
        // Mixing it with the others would inflate the bound for a reason that
        // does not happen in what is judged.
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
