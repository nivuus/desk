// Le nettoyage de fond des DEUX magasins sur disque (icônes, tranches).
//
// 🔴 SANS CE FICHIER, `Magasin.evincer` ET `MagasinTranches.evincer` SONT UN
// MÉCANISME ÉCRIT, TESTÉ, DOCUMENTÉ — ET APPELÉ PAR PERSONNE. C'est la panne
// la plus discrète possible (round de correction 1, 25 août 2026, sur ce
// même chantier) : le legs disait « les magasins ne sont JAMAIS nettoyés »,
// et une éviction que rien n'invoque ne le referme pas — elle en crée un
// second, plus difficile à voir que le premier, parce qu'il PARAÎT fermé.
//
// 🔴 LA CADENCE SUIT UN PRÉCÉDENT DU DÉPÔT, ELLE NE L'INVENTE PAS. Deux
// mécanismes de fond existent déjà dans ce service, et AUCUN DES DEUX N'A DE
// MINUTEUR : `routes-sante.ts::CacheSante` recalcule au plus tard à la
// PROCHAINE LECTURE (`PERIODE_SANTE_MS`), et `securite/frein.ts::Frein`
// évince ses entrées au moment d'un ÉCHEC suivant (`faireDeLaPlace`) — les
// deux sont PARESSEUX, déclenchés par un usage. Un magasin sur disque n'a pas
// cet usage : la route qui LIT une icône (`routes-icone.ts`) est le chemin
// CHAUD d'un navigateur, et y greffer un balayage de répertoire romprait la
// même règle que « ne jamais tracer par paquet dans la boucle de transport »
// — un coût de maintenance payé par chaque requête utile. Le seul précédent
// de TÂCHE DE FOND PÉRIODIQUE (un minuteur dédié) de tout ce dépôt vit côté
// AGENT : la réconciliation périodique du catalogue d'applications
// (sous-bloc G4, `APPS_SURVEILLANCE`, un `Mode` qui porte sa propre période).
// C'est cette FORME-là qu'on reprend ici — un minuteur dédié, une période NON
// CALIBRÉE, arrêtable explicitement à la fermeture du service — jamais son
// chiffre, propre à un autre processus et à une autre cadence de travail.
//
// ⚠️ NON CALIBRÉE, comme toute constante de ce dépôt.
export const PERIODE_NETTOYAGE_MS = 6 * 60 * 60_000; // 6 heures.

import type { Pilote } from '../base/pilote';
import type { Magasin } from './icones';
import { AGE_EVICTION_TRANCHES_MS, type MagasinTranches } from './magasin-tranches';
import {
    lirePlusVieuxQue as televersementsPlusVieuxQue,
    supprimer as supprimerTeleversement,
} from '../depot/televersement';

/// L'ensemble des empreintes d'icônes RÉFÉRENCÉES par une entrée VIVANTE du
/// catalogue.
///
/// 🔴 UNE ENTRÉE MASQUÉE RESTE VIVANTE, ET C'EST DÉLIBÉRÉ : `masquee_a` est un
/// geste d'AFFICHAGE (`depot/application.ts`), jamais une suppression — seule
/// `disparue_a` retire une ligne au sens de ce plancher. Filtrer aussi sur
/// `masquee_a IS NULL`, comme le fait `lireParVm` pour l'AFFICHAGE, évincerait
/// l'icône d'une application démasquable, et la démasquer la retrouverait
/// cassée — c'est exactement la corruption que le plancher existe pour
/// empêcher.
export async function referencesIcones(p: Pilote): Promise<Set<string>> {
    const lignes = await p.interroger<{ icone: string }>(
        'SELECT DISTINCT icone FROM application WHERE disparue_a IS NULL AND icone IS NOT NULL',
        [],
    );
    return new Set(lignes.map((l) => l.icone));
}

/// Referme d'abord les LIGNES de téléversement devenues trop vieilles, PUIS
/// évince sur le DISQUE ce qui n'a plus de ligne.
///
/// 🔴 LA LIGNE D'ABORD, LE DISQUE ENSUITE — ET C'EST L'ORDRE QUI REND LE
/// PLANCHER RÉEL. `depot/televersement.ts::lirePlusVieuxQue`/`supprimer`
/// existaient déjà, ENTIÈREMENT TESTÉS (`depot/installation.test.ts`,
/// « le balayage d'âge… », « REFUSE de supprimer un téléversement qu'une
/// installation référence »), et n'étaient appelés par AUCUN code de
/// production : un TROISIÈME mécanisme mort, cette fois-ci le commentaire de
/// leur propre test le dit d'avance — « les TRANCHES du disque, elles, sont
/// balayées PAR AILLEURS ». Ce fichier est cet « ailleurs ». La suppression
/// d'une ligne est REFUSÉE PAR LA CLÉ ÉTRANGÈRE tant qu'une `installation` la
/// référence encore (« l'historique d'une installation doit rester
/// lisible ») : ce refus EST le plancher, à l'étage de la BASE, avant même
/// celui du disque. Évincer le disque AVANT cette purge laisserait une
/// fenêtre où une ligne tout juste libérée n'a pas encore eu la chance de
/// protéger son répertoire ; l'ordre inverse est sans risque symétrique — une
/// ligne encore là protège son répertoire jusqu'au TOUR SUIVANT au pire.
async function nettoyerTranches(
    p: Pilote,
    tranches: MagasinTranches,
    maintenant: number,
): Promise<void> {
    const seuil = maintenant - AGE_EVICTION_TRANCHES_MS;
    for (const ligne of await televersementsPlusVieuxQue(p, seuil)) {
        try {
            await supprimerTeleversement(p, ligne.id);
        } catch {
            // 🔴 REFUS ATTENDU ET SILENCIEUX, PAS UNE PANNE : c'est la clé
            // étrangère d'`installation` qui protège une ligne encore
            // référencée. Un tour qui s'arrêterait là abandonnerait les
            // lignes suivantes de la même boucle sans raison — chacune est
            // indépendante, et le tour SUIVANT retente celles qui restent.
        }
    }
    tranches.evincer({ maintenant, referencees: await referencesTranches(p) });
}

/// L'ensemble des identifiants de téléversement RÉFÉRENCÉS — au sens le plus
/// large : la LIGNE existe encore dans `televersement`. Exportée séparément,
/// comme `referencesIcones`, pour que le garde « l'ensemble n'est jamais vide
/// par accident » (`nettoyage.test.ts`) puisse la mesurer sans passer par un
/// tour complet.
export async function referencesTranches(p: Pilote): Promise<Set<string>> {
    const lignes = await p.interroger<{ id: string }>('SELECT id FROM televersement', []);
    return new Set(lignes.map((l) => l.id));
}

/// UN tour de nettoyage, sur les DEUX magasins. Exportée séparément du
/// minuteur pour rester testable SANS `setInterval` — voir
/// `nettoyage.test.ts`.
export async function unTour(deps: {
    base: Pilote;
    magasin: Magasin;
    tranches: MagasinTranches;
    maintenant: number;
}): Promise<void> {
    const refs = await referencesIcones(deps.base);
    deps.magasin.evincer({ maintenant: deps.maintenant, referencees: refs });
    await nettoyerTranches(deps.base, deps.tranches, deps.maintenant);
}

/// Démarre le nettoyage de fond : un tour IMMÉDIAT et ATTENDU — un service
/// qui redémarre après des semaines d'arrêt ne doit pas attendre une pleine
/// période avant son premier balayage, et c'est aussi ce qui rend la panne du
/// round 1 REPRODUCTIBLE PAR UN TEST DÉTERMINISTE : `http/serveur.test.ts`
/// démarre un service réel et observe une icône orpheline disparaître SANS
/// appeler `evincer` à la main, précisément parce que ce premier tour est
/// attendu avant que `demarrerServeur` ne rende la main. Puis un tour toutes
/// les `periodeMs`, en tâche de fond — CEUX-LÀ ne sont jamais attendus, comme
/// le reste de ce service ne lit jamais un minuteur en bloquant une requête.
///
/// ⚠️ AUCUNE ERREUR NE REMONTE au-delà d'un tour : une base injoignable ou un
/// disque plein ne doivent ni interrompre le démarrage ni empêcher le tour
/// SUIVANT — même philosophie que le `.catch` de chaque requête HTTP dans
/// `serveur.ts`. Elle est journalisée, jamais avalée en silence.
export async function demarrerNettoyage(
    deps: { base: Pilote; magasin: Magasin; tranches: MagasinTranches; maintenant: () => number },
    periodeMs: number = PERIODE_NETTOYAGE_MS,
): Promise<{ arreter(): void }> {
    const tour = async (): Promise<void> => {
        try {
            await unTour({ ...deps, maintenant: deps.maintenant() });
        } catch (cause) {
            console.error(`nettoyage des magasins en echec : ${String(cause)}`);
        }
    };
    await tour();
    const minuteur = setInterval(() => {
        void tour();
    }, periodeMs);
    // 🔴 `unref()` : ce minuteur ne doit jamais être, À LUI SEUL, la raison
    // pour laquelle le processus reste vivant — `close()` (voir `serveur.ts`)
    // reste le chemin NORMAL d'arrêt, `unref()` n'est que le filet.
    minuteur.unref();
    return {
        arreter(): void {
            clearInterval(minuteur);
        },
    };
}
