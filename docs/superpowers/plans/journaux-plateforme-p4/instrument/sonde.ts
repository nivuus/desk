// The four criteria of acceptance run P4, played against a REAL service.
//
//     tsx sonde.ts <1|2|3|4> <sqlite|postgres> <commit>
//
// 🔴 EACH ASSERTION HAS ITS OWN VERDICT, AND ITS OWN RED RUN. It is the lesson
// P2 paid for: its red run ①A had to bring down "BOTH assertions" of
// criterion ①, and only brought down one, `expect` stopping at the
// first. Here nothing stops: `Verdicts.juger` is not an `expect`,
// it records and carries on, so that a mutation shows EVERYTHING it
// breaks rather than the first thing.

import { lister, attribuerSiLibre } from '../../../../../plateforme/src/depot/vm';
import type { LigneVm } from '../../../../../plateforme/src/depot/vm';
import type { Pilote } from '../../../../../plateforme/src/base/pilote';
import { inventaireStatique } from '../../../../../plateforme/src/orchestration/inventaire-statique';
import { CODE_HTTP, BACKEND_STATIQUE } from '../../../../../plateforme/src/orchestration/refus';
import { SEUIL_INJOIGNABLE_MS } from '../../../../../plateforme/src/agents/fraicheur';
import {
    appeler, createAccount, startService, enroler, entete, INSTANT, jetonDe, ligne, poserVuA,
    Verdicts, type Moteur,
} from './socle';

const [critere, moteur, commit] = process.argv.slice(2) as [string, Moteur, string];

/// 🔴 CRITERION ③'S BOUND IS MEASURED, NOT CHOSEN, and the ratios that
/// justify it are written rather than asserted. The reading that grounds it is filed
/// — `mesure-borne-3.log`, 100 refusals per engine — and it gives a worst case of
/// 7.5 ms (sqlite) and 12.9 ms (postgres) OUTSIDE cold start, the latter
/// costing 78.7 and 39.9 ms on its own. The twenty timed calls below
/// all come after a first call already made: they never pay that
/// start. 250 ms is therefore about 19 times the comparable worst case — enough
/// not to turn red on a loaded machine —, and eight times BELOW the prescribed
/// red run (`setTimeout(2000)`). It is the latter ratio that makes the criterion
/// discriminating; the former only says it will not turn red without reason.
/// ⚠️ "TWO ORDERS OF MAGNITUDE" had been written here first, and the reading
/// refuted it. Fixed on the evidence.
/// ⚠️ IT DOES NOT MEASURE THE PRODUCT'S LATENCY — it only establishes that the
/// refusal is IMMEDIATE, that is, that it involves neither waiting, nor wake-up
/// attempt, nor polling. That, and nothing else, is what the criterion states.
const BORNE_REFUS_MS = 250;

/// The tee of `console.warn`. It FORWARDS to the real error output — which
/// `jouer.sh` pours into the log — and remembers what goes by. It is
/// observation, not substitution: the real line is in the log,
/// and the counter allows turning it into a verdict.
const journalises: string[] = [];
const warnReel = console.warn.bind(console);
console.warn = (...args: unknown[]) => {
    journalises.push(args.map(String).join(' '));
    warnReel(...args);
};

/// A driver replaying the RACE of decision D5, on the REAL engine.
///
/// 🔴 WHAT IS SIMULATED, AND WHAT IS NOT. Simulated: the STALENESS of the
/// prior read — its first answer is a snapshot taken before
/// the user acquires their VM, which no transaction can see
/// otherwise. Real: EVERYTHING else — the `UPDATE` hits the real table, it is
/// the real partial index `vm_un_utilisateur` that throws, it is the real message of the
/// real engine, and it is the real reread after `ROLLBACK` that explains it.
///
/// Without this staleness, the path is UNREACHABLE: the prior read
/// sees the VM the user has just acquired and refuses before writing. A
/// mutation making the `catch` entirely rethrowing would therefore stay GREEN
/// — it is measured, and it is the reason this probe exists.
function enCourse(reelRacine: Pilote, perime: LigneVm[]): { pilote: Pilote; lectures: () => number } {
    let lectures = 0;
    const habiller = (reel: Pilote): Pilote => ({
        async interroger<T>(sql: string, params: unknown[]): Promise<T[]> {
            lectures += 1;
            if (lectures === 1) return perime as unknown as T[];
            return reel.interroger<T>(sql, params);
        },
        executer: (sql: string, params: unknown[]) => reel.executer(sql, params),
        transaction: <T>(corps: (p: Pilote) => Promise<T>) =>
            reel.transaction((t) => corps(habiller(t))),
        fermer: () => reel.fermer(),
    });
    return { pilote: habiller(reelRacine), lectures: () => lectures };
}

const v = new Verdicts();
const service = await startService(moteur, `c${critere}`);
const base = service.base;
const horloge = () => Date.now();
const orch = inventaireStatique(base, horloge);

try {
    if (critere === '1') {
        v.dire(entete("CRITÈRE ① — `instantane` REFUSE explicitement, en 501, et le JOURNALISE", moteur, commit));
        const alice = await createAccount(base, 'alice@essai.local');
        const vm = await enroler(base, 'w-alice');
        v.juger('attribution préalable', await orch.attribuer(vm.vmId, alice), { ok: true });

        const before = journalises.length;
        const r = await appeler(service.port, `/vm/${vm.vmId}/instantane`, 'POST', jetonDe(alice));

        // ①a — THE CODE AND THE TYPED BODY.
        v.dire(ligne('code HTTP rendu', r.code));
        v.dire(ligne('corps rendu', r.corps));
        v.juger('①a le code est 501, jamais 500', r.code, 501);
        v.juger('①a le corps est le refus TYPÉ, entier', r.corps, {
            motif: 'non-supporte', operation: 'instantane', backend: BACKEND_STATIQUE,
        });
        v.juger('①a et 501 est bien ce que la table de codes dit de ce motif',
            CODE_HTTP['non-supporte'], 501);

        // ①b — THE LOG LINE. An `it()` distinct from ①a: otherwise the
        // first verdict would mask the second.
        const neuves = journalises.slice(before);
        v.dire(ligne('lignes de journal émises pendant l’appel', neuves.length));
        for (const l of neuves) v.dire(ligne('  ligne', l));
        v.juger('①b le refus a produit EXACTEMENT une ligne de journal', neuves.length, 1);
        v.juger('①b elle nomme l’opération refusée', neuves[0]?.includes('instantane') ?? false, true);
        v.juger('①b elle nomme le backend qui refuse', neuves[0]?.includes(BACKEND_STATIQUE) ?? false, true);
    }

    if (critere === '2') {
        v.dire(entete("CRITÈRE ② — deux utilisateurs ne partagent pas une VM, un utilisateur n’en a pas deux, et JAMAIS un 500", moteur, commit));
        const alice = await createAccount(base, 'alice@essai.local');
        const bob = await createAccount(base, 'bob@essai.local');
        const v1 = await enroler(base, 'w-1');
        const v2 = await enroler(base, 'w-2');

        // ②a — THE THEFT. The orchestrator's prior read would refuse
        // BEFORE writing: it is therefore `attribuerSiLibre` that is hit
        // directly, the only place where the conditional clause is tested.
        // Without that, removing `AND utilisateur_id IS NULL` would stay GREEN.
        v.juger('mise en place : v1 est à alice', await orch.attribuer(v1.vmId, alice), { ok: true });
        const volees = await attribuerSiLibre(base, v1.vmId, bob);
        const apresVol = (await lister(base)).find((l) => l.id === v1.vmId)!;
        v.dire(ligne('lignes touchées par l’UPDATE de vol', volees));
        v.dire(ligne('propriétaire de v1 après le vol', apresVol.utilisateur_id === alice ? 'alice' : apresVol.utilisateur_id));
        v.juger('②a l’UPDATE de vol ne touche AUCUNE ligne', volees, 0);
        v.juger('②a et la ligne porte TOUJOURS son propriétaire', apresVol.utilisateur_id, alice);
        v.juger('②a l’orchestrateur, lui, refuse par un TYPE', await orch.attribuer(v1.vmId, bob),
            { ok: false, motif: 'vm-deja-attribuee', operation: 'attribuer', backend: BACKEND_STATIQUE });

        // ②b — THE SECOND VM. Same reason: at the repository, it is the partial index
        // alone that guards, and its removal must turn red.
        let leve: string | undefined;
        let lignes: number | undefined;
        try {
            lignes = await attribuerSiLibre(base, v2.vmId, alice);
        } catch (cause) {
            leve = String(cause);
        }
        v.dire(ligne('l’UPDATE d’une SECONDE VM pour alice', leve ?? `AUCUNE exception, ${lignes} ligne(s)`));
        v.juger('②b l’index partiel LÈVE plutôt que d’attribuer', leve !== undefined, true);
        v.juger('②b et v2 est resté LIBRE', (await lister(base)).find((l) => l.id === v2.vmId)!.utilisateur_id, null);
        v.juger('②b l’orchestrateur, lui, refuse par un TYPE et NE LÈVE PAS',
            await orch.attribuer(v2.vmId, alice),
            { ok: false, motif: 'utilisateur-servi', operation: 'attribuer', backend: BACKEND_STATIQUE });

        // ②c — THE RACE, the only path that reaches the `catch`. See `enCourse`.
        const v3 = await enroler(base, 'w-3');
        const perime = (await lister(base)).map((l) =>
            l.id === v3.vmId || l.id === v1.vmId ? { ...l, utilisateur_id: null } : l);
        const course = enCourse(base, perime);
        let issue: unknown;
        let echappee: string | undefined;
        try {
            issue = await inventaireStatique(course.pilote, horloge).attribuer(v3.vmId, alice);
        } catch (cause) {
            echappee = String(cause);
        }
        v.dire(ligne('lectures faites par la course', course.lectures()));
        v.dire(ligne('issue de la course', echappee ?? issue));
        v.juger('②c AUCUNE exception ne s’échappe — c’est elle qui ferait le 500', echappee, undefined);
        v.juger('②c la violation d’index est traduite en refus TYPÉ', issue,
            { ok: false, motif: 'utilisateur-servi', operation: 'attribuer', backend: BACKEND_STATIQUE });
        v.juger('②c la RELECTURE a bien eu lieu — le motif est lu, pas deviné', course.lectures(), 2);
        const codeDuMotif = CODE_HTTP[(issue as { motif: 'utilisateur-servi' }).motif];
        v.dire(ligne('code HTTP de ce motif', codeDuMotif));
        v.juger('②c et son code HTTP n’est PAS 500', codeDuMotif !== 500, true);
        v.juger('②c aucun des motifs de refus ne vaut 500',
            Object.values(CODE_HTTP).filter((c) => c === 500).length, 0);
    }

    if (critere === '3') {
        v.dire(entete("CRITÈRE ③ — un utilisateur sans VM reçoit un refus IMMÉDIAT", moteur, commit));
        const carol = await createAccount(base, 'carol@essai.local');
        // A VM exists, and it does NOT belong to carol: the refusal must be
        // `aucune-vm`, not "no VM in the world". Without this VM, the criterion
        // would pass on an empty inventory, a weaker case.
        const autre = await createAccount(base, 'autre@essai.local');
        const vm = await enroler(base, 'w-autre');
        v.juger('mise en place : la VM est à quelqu’un d’AUTRE', await orch.attribuer(vm.vmId, autre), { ok: true });

        const jeton = jetonDe(carol);
        const r = await appeler(service.port, '/session', 'POST', jeton);
        v.dire(ligne('code HTTP rendu', r.code));
        v.dire(ligne('corps rendu', r.corps));
        // ③a — THE TYPED REFUSAL.
        v.juger('③a le code est 409, jamais 200', r.code, 409);
        v.juger('③a et 409 est ce que la table de codes dit d’`aucune-vm`', CODE_HTTP['aucune-vm'], 409);
        v.juger('③a le corps porte le motif, et AUCUN préfixe', r.corps, { motif: 'aucune-vm' });
        v.juger('③a le corps ne porte pas de champ `prefixe` — un préfixe vide au coffre serait la panne muette',
            'prefixe' in (r.corps ?? {}), false);

        // ③b — IMMEDIACY. Twenty calls, and it is the WORST that is judged.
        const durees: number[] = [];
        for (let i = 0; i < 20; i += 1) {
            durees.push((await appeler(service.port, '/session', 'POST', jeton)).dureeMs);
        }
        const pire = Math.max(...durees);
        const median = [...durees].sort((a, b) => a - b)[10];
        v.dire(ligne('20 appels — médiane (ms)', Number(median.toFixed(2))));
        v.dire(ligne('20 appels — pire cas (ms)', Number(pire.toFixed(2))));
        v.dire(ligne('borne MESURÉE du critère (ms)', BORNE_REFUS_MS));
        v.judgeBelow('③b le pire des 20 refus reste sous la borne', pire, BORNE_REFUS_MS);
    }

    if (critere === '4') {
        v.dire(entete("CRITÈRE ④ — une VM dont l’agent n’a pas été vu est ANNONCÉE injoignable, et l’API AVOUE ne pas savoir la redémarrer", moteur, commit));
        const dave = await createAccount(base, 'dave@essai.local');
        const vm = await enroler(base, 'w-dave');
        v.juger('mise en place : la VM est à dave', await orch.attribuer(vm.vmId, dave), { ok: true });
        const jeton = jetonDe(dave);
        v.dire(ligne('SEUIL_INJOIGNABLE_MS', SEUIL_INJOIGNABLE_MS));

        // ④c, first side of the bound: the agent has just been seen.
        await poserVuA(base, vm.vmId, Date.now());
        const frais = await appeler(service.port, '/session', 'POST', jeton);
        v.dire(ligne('vu_a = maintenant — code HTTP', frais.code));
        v.dire(ligne('vu_a = maintenant — corps', frais.corps));
        v.juger('④c avant la transition : 200', frais.code, 200);
        v.juger('④c avant la transition : `prete`', frais.corps?.etat, 'prete');
        v.juger('④c et le préfixe est délivré', frais.corps?.prefixe, vm.prefixe);

        // ④a / ④b: the same agent, silent for one millisecond too many.
        await poserVuA(base, vm.vmId, Date.now() - SEUIL_INJOIGNABLE_MS - 1);
        const muet = await appeler(service.port, '/session', 'POST', jeton);
        v.dire(ligne('vu_a = maintenant − SEUIL − 1 — code HTTP', muet.code));
        v.dire(ligne('vu_a = maintenant − SEUIL − 1 — corps', muet.corps));
        v.juger('④a l’état annoncé est `injoignable`', muet.corps?.etat, 'injoignable');
        v.juger('④a et le code est 503 — un état du monde, pas une erreur de requête', muet.code, 503);
        v.juger('④a le préfixe est rendu QUAND MÊME — il est connu et juste', muet.corps?.prefixe, vm.prefixe);
        // ④b — THE ADMISSION. An `it()` distinct from ④a: the plan requires it by name,
        // otherwise the first verdict would stop before this one.
        v.juger('④b le champ `redemarrage` EXISTE', muet.corps?.redemarrage !== undefined, true);
        v.juger('④b et il AVOUE, avec son motif et son backend', muet.corps?.redemarrage,
            { possible: false, motif: 'non-supporte', backend: BACKEND_STATIQUE });

        // ④c — THE TRANSITION SEEN, and the bound besieged EXACTLY. `Date.now`
        // is not injectable through the service; it is at the
        // orchestrator, on the SAME real database and the SAME row.
        // A REALISTIC epoch, never a convenient `1_000`: it is P1's most
        // expensive lesson.
        const vuA = INSTANT;
        await poserVuA(base, vm.vmId, vuA);
        const aLaBorne = inventaireStatique(base, () => vuA + SEUIL_INJOIGNABLE_MS);
        const uneMsApres = inventaireStatique(base, () => vuA + SEUIL_INJOIGNABLE_MS + 1);
        const etatBorne = await aLaBorne.etat(vm.vmId);
        const etatApres = await uneMsApres.etat(vm.vmId);
        v.dire(ligne('t = vu_a + SEUIL (la borne EXACTE)', etatBorne));
        v.dire(ligne('t = vu_a + SEUIL + 1 ms', etatApres));
        v.juger('④c à la borne exacte, encore `prete`', etatBorne, 'prete');
        v.juger('④c une milliseconde plus tard, `injoignable`', etatApres, 'injoignable');
        v.juger('④c la TRANSITION est donc VUE, des deux côtés',
            `${etatBorne}->${etatApres}`, 'prete->injoignable');
        // And the case no clock catches up with: never seen at all.
        await poserVuA(base, vm.vmId, null);
        v.juger('④c une VM JAMAIS vue est `injoignable`, pas « peut-être »',
            await orch.etat(vm.vmId), 'injoignable');
    }
} finally {
    await service.arreter();
}

process.exit(v.conclure());
