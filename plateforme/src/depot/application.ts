// Le dépôt `application` : lire le catalogue d'une VM, et lui appliquer une
// fusion.
//
// 🔴 L'HORLOGE EST UN PARAMÈTRE, jamais lue ici — même règle que
// `depot/agent.ts`, `depot/session.ts` et `depot/utilisateur.ts`, et c'est ce
// qui rend `application.test.ts` capable d'asserter une époque EXACTE.
//
// 🔴 AUCUNE VALEUR LITTÉRALE dans le SQL : tout passe en paramètre, `null`
// compris, sans quoi `rendreMarqueurs` lèverait côté Postgres
// (`base/pilote.ts`). ⚠️ Cette moitié du lint ne mord QUE sur le chemin
// Postgres — le lint statique de `base/sous-ensemble.test.ts` ne balaie que les
// `.sql`. Une requête fautive écrite ici serait donc verte sous `test:sqlite`
// seul.
//
// 🔴 CE MODULE NE DÉCIDE RIEN. Ce qu'il faut insérer, mettre à jour, marquer
// disparu ou ressusciter est décidé par `apps/catalogue.ts`, qui est PUR. Ici
// on écrit, et rien d'autre : une règle qui vivrait dans cette couche ne serait
// éprouvable que par un test qui traverse un moteur SQL.
//
// 🔴 UNE LIGNE N'EST JAMAIS SUPPRIMÉE. `disparue_a` est posée et la ligne
// reste : une application installée côté navigateur porte l'identifiant de sa
// ligne, et un `DELETE` suivi d'une réinsertion à la réapparition lui en
// donnerait un autre. Il n'y a, dans tout ce module, aucun `DELETE`.

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';
import type { Connue, Fusion } from '../apps/catalogue';
import type { SourceMax } from '../../../proto/ts/plateforme';

export interface LigneApplication {
    id: string;
    vm_id: string;
    nom: string;
    /// Le chemin du `.lnk` LUI-MÊME, et c'est lui qu'on lance.
    chemin: string;
    /// La dernière réconciliation qui a vu cette application.
    ///
    /// ✅ `number` EST VRAI SUR LES DEUX MOTEURS, et ce ne l'a pas toujours
    /// été : `pg` rend les `BIGINT` en chaîne, et cette déclaration aurait été
    /// FAUSSE en production sans le `setTypeParser` de
    /// `base/pilote-postgres.ts` (recette de P3). `interroger<T>` faisant un
    /// `as T[]`, aucun typage ne l'attraperait — c'est `pilotes.test.ts` qui
    /// le tient, colonne par colonne, et `application.test.ts` au point
    /// d'usage.
    vue_a: number;
    /// L'empreinte du triplet `(cible, arguments, repertoire)` — l'identité au
    /// sens de l'agent, unique par VM (`application_cle`).
    cle: string;
    cible: string;
    /// BRUTS et sensibles à la casse. Vide = `''`, jamais NULL.
    arguments: string;
    repertoire: string;
    /// La PREMIÈRE vue. ⚠️ ÉCRITE ICI ET LUE PAR PERSONNE avant le sous-bloc
    /// G3 : elle existe parce qu'une colonne NOT NULL ne peut plus être
    /// ajoutée une fois la table peuplée (voir `0004-applications.sql`).
    apparue_a: number;
    /// `null` = vivante. ⚠️ Ce n'est PAS `0` : zéro se lirait comme une époque
    /// de 1970, et les deux états sont distincts — même raisonnement que
    /// `agent_enrole.vu_a`.
    disparue_a: number | null;
    /// Le geste explicite qui masque une entrée. ⚠️ AUCUN ÉCRIVAIN EN G1.
    masquee_a: number | null;
    /// L'empreinte SHA-256 du PNG, en hexadécimal minuscule. `null` =
    /// l'extraction a échoué, et ce n'est PAS une erreur.
    icone: string | null;
    /// 🔴 `null` = `SourceMax.NonMesuree`, JAMAIS `0` NI `256`. La colonne est
    /// `INTEGER` : elle ne peut pas porter le mot `non-mesuree`. Voir
    /// l'invariant à trois cas de `0005-icones.sql`, et la quatrième
    /// combinaison qui y est INTERDITE.
    source_max_px: number | null;
}

/// Reconstruit la `SourceMax` du fil depuis les deux colonnes.
///
/// 🔴 ÉCRITE UNE SEULE FOIS, ICI, ET C'EST DÉLIBÉRÉ : deux reconstructions
/// divergeraient le jour où l'une déciderait que `null` vaut `0`. C'est le
/// critère ④ jusqu'au bout de la chaîne — `NonMesuree` n'est JAMAIS rendue
/// comme un nombre.
export function sourceMaxDepuis(px: number | null): SourceMax {
    return px === null ? 'non-mesuree' : { pixels: px };
}

/// L'inverse : ce qu'on écrit en colonne pour une `SourceMax` du fil.
export function pxDepuisSourceMax(source: SourceMax): number | null {
    return source === 'non-mesuree' ? null : source.pixels;
}

/// Les colonnes sont ÉNUMÉRÉES, jamais `SELECT *` : une colonne ajoutée un
/// jour n'apparaîtrait pas toute seule dans un type qui ne la déclare pas.
const COLONNES =
    'id, vm_id, nom, chemin, vue_a, cle, cible, arguments, repertoire, apparue_a, disparue_a,'
    + ' masquee_a, icone, source_max_px';

/// Le catalogue AFFICHABLE d'une VM : ni les disparues, ni les masquées.
///
/// 🔴 CE N'EST PAS LE LECTEUR DE LA FUSION, et les deux ne peuvent pas être le
/// même — voir `lireConnues`.
export async function lireParVm(p: Pilote, vmId: string): Promise<LigneApplication[]> {
    return p.interroger<LigneApplication>(
        `SELECT ${COLONNES} FROM application`
            + ' WHERE vm_id = ? AND disparue_a IS NULL AND masquee_a IS NULL'
            + ' ORDER BY nom',
        [vmId],
    );
}

/// Rend la ligne, ou `undefined`. JAMAIS une exception sur un identifiant
/// inconnu : une exception qui remonterait en 500 serait à elle seule un
/// oracle. Précédents : `depot/agent.ts::lireParVm`, `depot/vm.ts::lireParId`.
///
/// ⚠️ ELLE NE FILTRE NI LES DISPARUES NI LES MASQUÉES, à dessein : son appelant
/// est la route de lancement, qui doit pouvoir distinguer « inconnue » de
/// « connue mais plus là », et son autre appelant est le test qui vérifie
/// qu'une disparition n'a pas effacé la ligne.
export async function lireParId(p: Pilote, id: string): Promise<LigneApplication | undefined> {
    const lignes = await p.interroger<LigneApplication>(
        `SELECT ${COLONNES} FROM application WHERE id = ?`,
        [id],
    );
    return lignes[0];
}

/// Ce que la fusion a besoin de savoir, et RIEN DE PLUS.
///
/// 🔴 IL REND AUSSI LES DISPARUES, et c'est ce qui le distingue de
/// `lireParVm`. Une fusion qui ne les verrait pas les RÉINSÉRERAIT à leur
/// retour, avec un identifiant neuf — c'est-à-dire exactement la perte
/// d'identifiant que ce module existe pour empêcher.
export async function lireConnues(p: Pilote, vmId: string): Promise<Connue[]> {
    return p.interroger<Connue>(
        'SELECT id, cle, disparue_a FROM application WHERE vm_id = ?',
        [vmId],
    );
}

/// Applique une fusion, EN ENTIER OU PAS DU TOUT.
///
/// 🔴 LA TRANSACTION N'EST PAS DÉCORATIVE. Un catalogue à moitié écrit est
/// indiscernable d'un catalogue correct au tour suivant : la réconciliation
/// suivante le prendrait pour l'état de la VM, et les lignes manquantes ne
/// reviendraient qu'au prochain envoi complet — ou jamais, si l'agent n'en
/// émet plus. C'est la classe de panne muette contre laquelle tout ce dépôt
/// est écrit, et c'est le même raisonnement que celui de `migrations.ts`.
///
/// 🔴 L'IDENTIFIANT EST GÉNÉRÉ ICI, jamais reçu de l'agent. La clé est
/// l'empreinte d'un triplet de chemins Windows : deux VMs portant la même
/// application produisent la MÊME clé, et un identifiant qui en viendrait
/// serait en collision d'une VM à l'autre.
export async function appliquer(
    p: Pilote,
    vmId: string,
    fusion: Fusion,
    maintenant: number,
): Promise<void> {
    await p.transaction(async (tx) => {
        for (const app of fusion.aInserer) {
            await tx.executer(
                // 🔴 QUATORZE MARQUEURS POUR QUATORZE COLONNES. Un `INSERT`
                // mal compté LÈVE sur les DEUX moteurs — c'est le garde le
                // moins cher du fichier, et il est gratuit.
                `INSERT INTO application(${COLONNES})`
                    + ' VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)',
                [
                    randomUUID(),
                    vmId,
                    app.nom,
                    app.chemin,
                    maintenant,
                    app.cle,
                    app.cible,
                    app.arguments,
                    app.repertoire,
                    // `apparue_a` et `vue_a` naissent égales : c'est la même
                    // vue. Elles divergent à la mise à jour suivante, et c'est
                    // tout le sens de la première.
                    maintenant,
                    null,
                    null,
                    app.icone,
                    pxDepuisSourceMax(app.source_max),
                ],
            );
        }

        for (const { id, app } of fusion.aMettreAJour) {
            // ⚠️ `apparue_a` N'EST PAS DANS CE `SET`. L'avancer en ferait un
            // doublon de `vue_a`, et le verdict d'installation qui la lira un
            // jour ne verrait plus jamais une apparition.
            //
            // ⚠️ `cle` non plus, et pour une raison différente : c'est par
            // elle qu'on a trouvé la ligne, et elle est l'identité. La
            // réécrire n'aurait aucun effet dans le meilleur des cas, et
            // violerait `application_cle` dans le pire.
            // ⚠️ `icone` ET `source_max_px` SONT DANS CE `SET`, contrairement
            // à `cle` et `apparue_a`. Une icône CHANGE quand l'application se
            // met à jour, et c'est le cas nominal, pas l'exception : les
            // omettre ferait qu'une icône neuve n'atteindrait jamais la base,
            // et le seul symptôme serait une image périmée que rien
            // n'expliquerait.
            await tx.executer(
                'UPDATE application SET nom = ?, chemin = ?, cible = ?, arguments = ?,'
                    + ' repertoire = ?, vue_a = ?, icone = ?, source_max_px = ? WHERE id = ?',
                [
                    app.nom,
                    app.chemin,
                    app.cible,
                    app.arguments,
                    app.repertoire,
                    maintenant,
                    app.icone,
                    pxDepuisSourceMax(app.source_max),
                    id,
                ],
            );
        }

        for (const id of fusion.aMarquerDisparues) {
            // ⚠️ AUCUN `DELETE`, ici ni ailleurs dans ce fichier.
            await tx.executer('UPDATE application SET disparue_a = ? WHERE id = ?', [maintenant, id]);
        }

        for (const id of fusion.aRessusciter) {
            // Le `null` passe en PARAMÈTRE comme toute valeur. Ce n'est pas
            // `rendreMarqueurs` qui l'exige — `NULL` est un mot-clé, pas une
            // chaîne littérale, et un `SET disparue_a = NULL` écrit en dur
            // passerait. C'est la règle générale du dépôt, dont
            // `depot/vm.ts::detacher` a posé l'usage : une seule forme
            // d'écriture, pour qu'aucun lecteur n'ait à se demander laquelle
            // des deux il a sous les yeux.
            await tx.executer('UPDATE application SET disparue_a = ? WHERE id = ?', [null, id]);
        }
    });
}
