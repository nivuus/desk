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

/// Même remède, même raison que `icones.ts::PAS_DE_REPRISE` — la boucle qui
/// tente de purger un ARRIÉRÉ de lignes trop vieilles ne doit pas non plus
/// geler le service à elle seule. ⚠️ NON CALIBRÉ, même raisonnement.
const PAS_DE_REPRISE = 50;

async function rendreLaMain(): Promise<void> {
    await new Promise<void>((resolve) => setImmediate(resolve));
}

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
///
/// 🔵 FENÊTRE RÉSIDUELLE DÉCLARÉE, PAS CORRIGÉE (round de correction 2) : les
/// références sont lues, PUIS une application adopte une icône vieille et
/// jusque-là orpheline, PUIS l'éviction tourne sur l'instantané lu avant
/// cette adoption — l'icône part alors qu'elle vient de redevenir vivante.
/// AUTO-RÉPARANT, PAS UNE PERTE : la réconciliation suivante
/// (`agents/canal-apps.ts::deps.magasin.manquantes`) redemande toute
/// empreinte absente, exactement comme un magasin perdu se reconstruit tout
/// seul (voir l'en-tête d'`icones.ts`, critère ⑦). L'icône est cassée pour
/// une fenêtre bornée par `PERIODE_NETTOYAGE_MS`, jamais indéfiniment.
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
///
/// 🔴 CRITIQUE FERMÉE (round de correction 2) — LES DEUX PLANCHERS DOIVENT
/// MESURER LE MÊME ÂGE. `lirePlusVieuxQue` filtre sur `cree_a` (la ligne),
/// `MagasinTranches.evincer` filtre sur `mtime` (le disque, la DERNIÈRE
/// ACTIVITÉ) — CE N'EST PAS LA MÊME CHOSE, et c'était DÉTERMINISTE, pas une
/// course : un téléversement créé il y a 31 jours dont une tranche vient
/// d'arriver À L'INSTANT rendait sa LIGNE supprimée (candidate par `cree_a`,
/// aucune installation ne la référençant) alors que son RÉPERTOIRE restait
/// intact (trop jeune par `mtime`) — la reprise meurt (`routes-televersement.ts`
/// rend `404 televersement-inconnu` sur des octets pourtant PRÉSENTS), et le
/// disque n'est même pas libéré. C'est la corruption que le plancher existe
/// pour empêcher, entrée par la porte de la BASE plutôt que par celle du
/// DISQUE. LE REMÈDE : on ne supprime la ligne QUE SI SON RÉPERTOIRE EST LUI
/// AUSSI ÉVINCIBLE (ou absent) — `derniereActivite` fait mesurer le MÊME âge
/// aux deux planchers, PAR CONSTRUCTION, sans toucher au schéma.
async function nettoyerTranches(
    p: Pilote,
    tranches: MagasinTranches,
    maintenant: number,
): Promise<void> {
    const seuil = maintenant - AGE_EVICTION_TRANCHES_MS;
    // 🔵 `lirePlusVieuxQue` charge la LIGNE ENTIÈRE (nom, taille, sha256…)
    // alors que seul `id` sert ici — DÉCLARÉ, PAS CORRIGÉ (round de
    // correction 2) : un pic mémoire est possible sur un arriéré, mais
    // ajouter une requête plus étroite dupliquerait une primitive DÉJÀ
    // testée (`depot/installation.test.ts`) pour gagner une économie qui ne
    // compte que si l'arriéré est énorme — une base injoignable pendant des
    // semaines, un cas qui a d'autres symptômes avant celui-ci.
    let i = 0;
    for (const ligne of await televersementsPlusVieuxQue(p, seuil)) {
        if (i > 0 && i % PAS_DE_REPRISE === 0) await rendreLaMain();
        i += 1;

        const activite = await tranches.derniereActivite(ligne.id);
        if (activite !== undefined && maintenant - activite < AGE_EVICTION_TRANCHES_MS) {
            // Le RÉPERTOIRE est encore actif : la ligne reste, quel que soit
            // l'âge de `cree_a`. C'est le remède à la critique ci-dessus.
            continue;
        }

        try {
            await supprimerTeleversement(p, ligne.id);
        } catch (cause) {
            if (!estRefusDeCleEtrangere(cause)) {
                // ⚠️ Important ① (round de correction 2) : SEUL le refus de
                // clé étrangère reste muet — il est ATTENDU, voir plus haut.
                // Un autre échec (base coupée, disque plein…) N'A RIEN
                // D'ATTENDU et doit se voir : mesuré par la revue, un
                // `catch {}` nu rendait ZÉRO ligne de journal sur une panne
                // partielle. L'argument d'amplification ne s'applique pas :
                // cette boucle est BORNÉE, et tourne au plus toutes les six
                // heures — rien à voir avec un paquet par trame.
                console.error(
                    `purge de la ligne de televersement ${ligne.id} en echec : ${String(cause)}`,
                );
            }
        }
    }
    await tranches.evincer({ maintenant, referencees: await referencesTranches(p) });
}

/// Discrimine un refus de CLÉ ÉTRANGÈRE — le SEUL échec attendu de
/// `supprimerTeleversement` — de tout le reste.
///
/// 🔴 UN CODE, PAS UN TEXTE : `errcode` (node:sqlite,
/// `SQLITE_CONSTRAINT_FOREIGNKEY = 787`, mesuré sur ce dépôt — voir
/// `nettoyage.test.ts`) et `code` (`pg`, `23503`, le code stable
/// `foreign_key_violation` de Postgres) sont des CODES STABLES, publiés par
/// chaque moteur — jamais le `message`, qui est de la prose et peut changer
/// d'une version à l'autre sans que rien ne le signale ici.
function estRefusDeCleEtrangere(cause: unknown): boolean {
    if (!(cause instanceof Error)) return false;
    const e = cause as Error & { errcode?: unknown; code?: unknown };
    return e.errcode === 787 || e.code === '23503';
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
///
/// 🔵 AUCUN GARDE DE RÉ-ENTRANCE — DÉCLARÉ, PAS AJOUTÉ (round de correction
/// 2). En PRODUCTION, `PERIODE_NETTOYAGE_MS` (6 h) est de plusieurs ordres de
/// grandeur plus grand que la durée d'un tour (une fraction de seconde, même
/// à l'échelle des bancs de la revue) : deux tours qui se chevauchent n'est
/// pas un cas qu'on attend de voir. Et si cela arrivait quand même — un
/// `periodeMs` de test minuscule, par exemple —, les deux moitiés du tour
/// sont IDEMPOTENTES : `evincer` sur un fichier déjà parti est un `rm force`
/// qui ne trouve rien, et `supprimer` sur une ligne déjà purgée touche zéro
/// ligne. Un garde ajouterait de la surface pour un risque qui n'en a pas
/// besoin. ⚠️ CETTE ANALYSE SUPPOSAIT LE TOUR SYNCHRONE ; il ne l'est plus
/// depuis le remède à l'Important ② ci-dessous (`evincer` rend la main entre
/// les entrées), ce qui allonge sa durée réelle et rapproche — sans l'ouvrir
/// — la fenêtre d'un chevauchement en régime de test à cadence minuscule.
/// L'idempotence ci-dessus tient toujours, et c'est elle qui reste le
/// répondant, pas l'absence de chevauchement.
export async function unTour(deps: {
    base: Pilote;
    magasin: Magasin;
    tranches: MagasinTranches;
    maintenant: number;
}): Promise<void> {
    const refs = await referencesIcones(deps.base);
    await deps.magasin.evincer({ maintenant: deps.maintenant, referencees: refs });
    await nettoyerTranches(deps.base, deps.tranches, deps.maintenant);
}

/// Démarre le nettoyage de fond : un tour IMMÉDIAT et ATTENDU, puis un tour
/// toutes les `periodeMs` en tâche de fond (CEUX-LÀ ne sont jamais attendus,
/// comme le reste de ce service ne lit jamais un minuteur en bloquant une
/// requête).
///
/// ⚠️ POURQUOI LE PREMIER TOUR EST ATTENDU, DIT COMME CE QUE C'EST (round de
/// correction 2, sur relève de la revue) : la raison DÉCISIVE est la
/// TESTABILITÉ, pas une propriété du produit — c'est ce qui rend la panne du
/// round 1 REPRODUCTIBLE PAR UN TEST DÉTERMINISTE (`http/serveur.test.ts`
/// démarre un service réel et observe une icône orpheline disparaître SANS
/// appeler `evincer` à la main, sans sondage ni délai arbitraire, précisément
/// parce que ce premier tour est attendu avant que `demarrerServeur` ne rende
/// la main). Qu'un service fraîchement redémarré n'attende pas une pleine
/// période avant son premier balayage est un bénéfice réel mais SECONDAIRE :
/// une requête DB de plus avant `http.listen` a un coût de latence de
/// démarrage que ce fichier n'a pas mesuré, et rien n'exclut qu'un jour ce
/// coût pèse plus que le bénéfice — ce serait alors un arbitrage à reprendre,
/// pas une régression de ce remède.
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
        /// 🔴 CE QU'`arreter()` FAIT, ET CE QU'IL NE FAIT PAS — CORRIGÉ (round
        /// de correction 2) : la phrase précédente laissait croire qu'appeler
        /// cette méthode empêchait un tour en cours de heurter une base
        /// bientôt fermée. **FAUX** : `clearInterval` empêche seulement la
        /// PROCHAINE PLANIFICATION — il n'interrompt PAS un tour déjà en vol.
        /// Un tour démarré juste avant cet appel continue, `await`e ses
        /// requêtes, et peut encore échouer (et se journaliser, voir plus
        /// haut) si la base ferme entre-temps. Ce que cette méthode GARANTIT
        /// réellement : après son retour, AUCUN NOUVEAU tour ne démarrera.
        /// `nettoyage.test.ts` en tient la preuve — un test ferme le service
        /// et vérifie qu'aucun tour supplémentaire ne s'exécute au-delà de
        /// celui déjà en vol au moment de l'appel.
        arreter(): void {
            clearInterval(minuteur);
        },
    };
}
