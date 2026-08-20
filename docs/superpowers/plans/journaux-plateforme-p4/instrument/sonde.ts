// Les quatre critères de la recette P4, joués contre un service RÉEL.
//
//     tsx sonde.ts <1|2|3|4> <sqlite|postgres> <commit>
//
// 🔴 CHAQUE ASSERTION A SON PROPRE VERDICT, ET SA PROPRE ROUGE. C'est la leçon
// que P2 a payée : sa rouge ①A devait faire tomber « les DEUX assertions » du
// critère ①, et n'en faisait tomber qu'une, `expect` s'interrompant à la
// première. Ici rien ne s'interrompt : `Verdicts.juger` n'est pas un `expect`,
// il enregistre et poursuit, si bien qu'une mutation montre TOUT ce qu'elle
// casse plutôt que la première chose.

import { lister, attribuerSiLibre } from '../../../../../plateforme/src/depot/vm';
import type { LigneVm } from '../../../../../plateforme/src/depot/vm';
import type { Pilote } from '../../../../../plateforme/src/base/pilote';
import { inventaireStatique } from '../../../../../plateforme/src/orchestration/inventaire-statique';
import { CODE_HTTP, BACKEND_STATIQUE } from '../../../../../plateforme/src/orchestration/refus';
import { SEUIL_INJOIGNABLE_MS } from '../../../../../plateforme/src/agents/fraicheur';
import {
    appeler, creerCompte, demarrerService, enroler, entete, INSTANT, jetonDe, ligne, poserVuA,
    Verdicts, type Moteur,
} from './socle';

const [critere, moteur, commit] = process.argv.slice(2) as [string, Moteur, string];

/// 🔴 LA BORNE DU CRITÈRE ③ EST MESURÉE, PAS CHOISIE, et les rapports qui la
/// justifient sont écrits plutôt qu'affirmés. Le relevé qui la fonde est versé
/// — `mesure-borne-3.log`, 100 refus par moteur — et il donne un pire cas de
/// 7,5 ms (sqlite) et 12,9 ms (postgres) HORS démarrage à froid, ce dernier
/// coûtant à lui seul 78,7 et 39,9 ms. Les vingt appels chronométrés ci-dessous
/// viennent tous après un premier appel déjà fait : ils ne paient jamais ce
/// démarrage. 250 ms est donc environ 19 fois le pire cas comparable — assez
/// pour ne pas rougir sur une machine chargée —, et huit fois SOUS la rouge
/// prescrite (`setTimeout(2000)`). C'est ce dernier rapport qui rend le critère
/// discriminant ; le premier dit seulement qu'il ne rougira pas sans raison.
/// ⚠️ « DEUX ORDRES DE GRANDEUR » avait été écrit ici d'abord, et le relevé le
/// réfutait. Corrigé sur pièces.
/// ⚠️ ELLE NE MESURE PAS LA LATENCE DU PRODUIT — elle établit seulement que le
/// refus est IMMÉDIAT, c'est-à-dire qu'il ne comporte ni attente, ni tentative
/// de réveil, ni scrutation. C'est cela, et rien d'autre, que le critère énonce.
const BORNE_REFUS_MS = 250;

/// Le tee de `console.warn`. Il FORWARDE vers la vraie sortie d'erreur — que
/// `jouer.sh` verse dans le journal — et retient ce qui passe. C'est de
/// l'observation, pas une substitution : la ligne réelle est dans le journal,
/// et le compteur permet d'en faire un verdict.
const journalises: string[] = [];
const warnReel = console.warn.bind(console);
console.warn = (...args: unknown[]) => {
    journalises.push(args.map(String).join(' '));
    warnReel(...args);
};

/// Un pilote qui rejoue la COURSE de la décision D5, sur le moteur RÉEL.
///
/// 🔴 CE QUI EST SIMULÉ, ET CE QUI NE L'EST PAS. Simulé : la PÉREMPTION de la
/// lecture préalable — sa première réponse est un instantané pris avant que
/// l'utilisateur n'acquière sa VM, ce qu'aucune transaction ne peut voir
/// autrement. Réel : TOUT le reste — l'`UPDATE` frappe la vraie table, c'est
/// le vrai index partiel `vm_un_utilisateur` qui lève, c'est le vrai message du
/// vrai moteur, et c'est la vraie relecture d'après `ROLLBACK` qui l'explique.
///
/// Sans cette péremption, le chemin est INATTEIGNABLE : la lecture préalable
/// voit la VM que l'utilisateur vient d'acquérir et refuse avant d'écrire. Une
/// mutation qui rendrait le `catch` entièrement relançant resterait donc VERTE
/// — c'est mesuré, et c'est la raison d'être de cette sonde.
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
const service = await demarrerService(moteur, `c${critere}`);
const base = service.base;
const horloge = () => Date.now();
const orch = inventaireStatique(base, horloge);

try {
    if (critere === '1') {
        v.dire(entete("CRITÈRE ① — `instantane` REFUSE explicitement, en 501, et le JOURNALISE", moteur, commit));
        const alice = await creerCompte(base, 'alice@essai.local');
        const vm = await enroler(base, 'w-alice');
        v.juger('attribution préalable', await orch.attribuer(vm.vmId, alice), { ok: true });

        const avant = journalises.length;
        const r = await appeler(service.port, `/vm/${vm.vmId}/instantane`, 'POST', jetonDe(alice));

        // ①a — LE CODE ET LE CORPS TYPÉ.
        v.dire(ligne('code HTTP rendu', r.code));
        v.dire(ligne('corps rendu', r.corps));
        v.juger('①a le code est 501, jamais 500', r.code, 501);
        v.juger('①a le corps est le refus TYPÉ, entier', r.corps, {
            motif: 'non-supporte', operation: 'instantane', backend: BACKEND_STATIQUE,
        });
        v.juger('①a et 501 est bien ce que la table de codes dit de ce motif',
            CODE_HTTP['non-supporte'], 501);

        // ①b — LA LIGNE DE JOURNAL. `it()` distinct de ①a : sans quoi le
        // premier verdict masquerait le second.
        const neuves = journalises.slice(avant);
        v.dire(ligne('lignes de journal émises pendant l’appel', neuves.length));
        for (const l of neuves) v.dire(ligne('  ligne', l));
        v.juger('①b le refus a produit EXACTEMENT une ligne de journal', neuves.length, 1);
        v.juger('①b elle nomme l’opération refusée', neuves[0]?.includes('instantane') ?? false, true);
        v.juger('①b elle nomme le backend qui refuse', neuves[0]?.includes(BACKEND_STATIQUE) ?? false, true);
    }

    if (critere === '2') {
        v.dire(entete("CRITÈRE ② — deux utilisateurs ne partagent pas une VM, un utilisateur n’en a pas deux, et JAMAIS un 500", moteur, commit));
        const alice = await creerCompte(base, 'alice@essai.local');
        const bob = await creerCompte(base, 'bob@essai.local');
        const v1 = await enroler(base, 'w-1');
        const v2 = await enroler(base, 'w-2');

        // ②a — LE VOL. La lecture préalable de l'orchestrateur refuserait
        // AVANT d'écrire : c'est donc `attribuerSiLibre` qu'on frappe
        // directement, seul endroit où la clause conditionnelle est éprouvée.
        // Sans cela, retirer `AND utilisateur_id IS NULL` resterait VERT.
        v.juger('mise en place : v1 est à alice', await orch.attribuer(v1.vmId, alice), { ok: true });
        const volees = await attribuerSiLibre(base, v1.vmId, bob);
        const apresVol = (await lister(base)).find((l) => l.id === v1.vmId)!;
        v.dire(ligne('lignes touchées par l’UPDATE de vol', volees));
        v.dire(ligne('propriétaire de v1 après le vol', apresVol.utilisateur_id === alice ? 'alice' : apresVol.utilisateur_id));
        v.juger('②a l’UPDATE de vol ne touche AUCUNE ligne', volees, 0);
        v.juger('②a et la ligne porte TOUJOURS son propriétaire', apresVol.utilisateur_id, alice);
        v.juger('②a l’orchestrateur, lui, refuse par un TYPE', await orch.attribuer(v1.vmId, bob),
            { ok: false, motif: 'vm-deja-attribuee', operation: 'attribuer', backend: BACKEND_STATIQUE });

        // ②b — LA SECONDE VM. Même raison : au dépôt, c'est l'index partiel
        // seul qui garde, et son retrait doit rougir.
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

        // ②c — LA COURSE, seul chemin qui atteigne le `catch`. Voir `enCourse`.
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
        const carol = await creerCompte(base, 'carol@essai.local');
        // Une VM existe, et elle N'EST PAS à carol : le refus doit être
        // `aucune-vm`, pas « aucune VM au monde ». Sans cette VM, le critère
        // passerait sur un inventaire vide, cas plus faible.
        const autre = await creerCompte(base, 'autre@essai.local');
        const vm = await enroler(base, 'w-autre');
        v.juger('mise en place : la VM est à quelqu’un d’AUTRE', await orch.attribuer(vm.vmId, autre), { ok: true });

        const jeton = jetonDe(carol);
        const r = await appeler(service.port, '/session', 'POST', jeton);
        v.dire(ligne('code HTTP rendu', r.code));
        v.dire(ligne('corps rendu', r.corps));
        // ③a — LE REFUS TYPÉ.
        v.juger('③a le code est 409, jamais 200', r.code, 409);
        v.juger('③a et 409 est ce que la table de codes dit d’`aucune-vm`', CODE_HTTP['aucune-vm'], 409);
        v.juger('③a le corps porte le motif, et AUCUN préfixe', r.corps, { motif: 'aucune-vm' });
        v.juger('③a le corps ne porte pas de champ `prefixe` — un préfixe vide au coffre serait la panne muette',
            'prefixe' in (r.corps ?? {}), false);

        // ③b — L'IMMÉDIATETÉ. Vingt appels, et c'est le PIRE qui est jugé.
        const durees: number[] = [];
        for (let i = 0; i < 20; i += 1) {
            durees.push((await appeler(service.port, '/session', 'POST', jeton)).dureeMs);
        }
        const pire = Math.max(...durees);
        const median = [...durees].sort((a, b) => a - b)[10];
        v.dire(ligne('20 appels — médiane (ms)', Number(median.toFixed(2))));
        v.dire(ligne('20 appels — pire cas (ms)', Number(pire.toFixed(2))));
        v.dire(ligne('borne MESURÉE du critère (ms)', BORNE_REFUS_MS));
        v.jugerSous('③b le pire des 20 refus reste sous la borne', pire, BORNE_REFUS_MS);
    }

    if (critere === '4') {
        v.dire(entete("CRITÈRE ④ — une VM dont l’agent n’a pas été vu est ANNONCÉE injoignable, et l’API AVOUE ne pas savoir la redémarrer", moteur, commit));
        const dave = await creerCompte(base, 'dave@essai.local');
        const vm = await enroler(base, 'w-dave');
        v.juger('mise en place : la VM est à dave', await orch.attribuer(vm.vmId, dave), { ok: true });
        const jeton = jetonDe(dave);
        v.dire(ligne('SEUIL_INJOIGNABLE_MS', SEUIL_INJOIGNABLE_MS));

        // ④c, premier côté de la borne : l'agent vient d'être vu.
        await poserVuA(base, vm.vmId, Date.now());
        const frais = await appeler(service.port, '/session', 'POST', jeton);
        v.dire(ligne('vu_a = maintenant — code HTTP', frais.code));
        v.dire(ligne('vu_a = maintenant — corps', frais.corps));
        v.juger('④c avant la transition : 200', frais.code, 200);
        v.juger('④c avant la transition : `prete`', frais.corps?.etat, 'prete');
        v.juger('④c et le préfixe est délivré', frais.corps?.prefixe, vm.prefixe);

        // ④a / ④b : le même agent, muet depuis une milliseconde de trop.
        await poserVuA(base, vm.vmId, Date.now() - SEUIL_INJOIGNABLE_MS - 1);
        const muet = await appeler(service.port, '/session', 'POST', jeton);
        v.dire(ligne('vu_a = maintenant − SEUIL − 1 — code HTTP', muet.code));
        v.dire(ligne('vu_a = maintenant − SEUIL − 1 — corps', muet.corps));
        v.juger('④a l’état annoncé est `injoignable`', muet.corps?.etat, 'injoignable');
        v.juger('④a et le code est 503 — un état du monde, pas une erreur de requête', muet.code, 503);
        v.juger('④a le préfixe est rendu QUAND MÊME — il est connu et juste', muet.corps?.prefixe, vm.prefixe);
        // ④b — L'AVEU. `it()` distinct de ④a : le plan l'exige nommément,
        // sans quoi le premier verdict s'arrêterait avant celui-ci.
        v.juger('④b le champ `redemarrage` EXISTE', muet.corps?.redemarrage !== undefined, true);
        v.juger('④b et il AVOUE, avec son motif et son backend', muet.corps?.redemarrage,
            { possible: false, motif: 'non-supporte', backend: BACKEND_STATIQUE });

        // ④c — LA TRANSITION VUE, et la borne assiégée EXACTEMENT. `Date.now`
        // n'est pas injectable à travers le service ; elle l'est à
        // l'orchestrateur, sur la MÊME base réelle et la MÊME ligne.
        // Une époque RÉALISTE, jamais un `1_000` de commodité : c'est la leçon
        // la plus chère de P1.
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
        // Et le cas qu'aucune horloge ne rattrape : jamais vue du tout.
        await poserVuA(base, vm.vmId, null);
        v.juger('④c une VM JAMAIS vue est `injoignable`, pas « peut-être »',
            await orch.etat(vm.vmId), 'injoignable');
    }
} finally {
    await service.arreter();
}

process.exit(v.conclure());
