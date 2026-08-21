// Les branches MONTANTES du sous-projet ④ : ce qu'un agent enrôlé rapporte de
// son catalogue, de ses lancements et de ses installations.
//
// 🔴 EXTRAIT AVANT L'ADDITION, JAMAIS APRÈS. `canal.ts` était à 462 lignes,
// marge 38 ; le sous-bloc G3 y ajoute deux branches et la réémission à
// l'enrôlement. Le plan le relevait à 423 le 20 août : c'est G2 qui a consommé
// la différence. La doctrine de `CLAUDE.md` est de rendre la marge par une
// extraction jouée d'avance, jamais par une compression — et ce dépôt écrit
// quatre fois que la marge regagnée par une extraction se reperd si on la
// traite comme acquise.
//
// 🔴 LA FRONTIÈRE EST CELLE QUE TROIS AUTRES FICHIERS EMPLOIENT DÉJÀ : cycle de
// vie d'un côté (enrôlement, battement, refus, frein), gestion d'apps de
// l'autre. C'est la coupure de `proto/src/plateforme/apps.rs`, de
// `proto/src/plateforme/tests_apps.rs` et de `proto/ts/plateforme-apps.test.ts`.
// Une seule coupure pour quatre fichiers, c'est ce qui la rend mémorisable.
//
// 🔴 ET C'EST UNE GARDE DE TYPE, PAS UNE CHUTE. `traiter` est appelée derrière
// le prédicat `estMontantDeQuatre`, qui NOMME les quatre types. `canal.ts`
// laissait auparavant `enroler` être le RESTE d'un `if/else` — et l'élargissement
// de l'union par G3 a fait de `progression` et `termine` deux membres de ce
// reste, donc deux messages que la destructuration `const { vm, secret }`
// aurait lus comme un enrôlement. **C'est `tsc` qui l'a dit, et il a eu de la
// chance : la même fragilité sur une valeur plutôt qu'un type serait passée en
// silence.** Le prédicat referme la classe.

import type { WebSocket } from 'ws';
import {
    encodeIconesManquantes,
    encodeInstaller,
    type CatalogueMessage,
    type LanceeMessage,
    type ProgressionMessage,
    type TermineMessage,
    type VersLaPlateforme,
} from '../../../proto/ts/plateforme';
import type { Magasin } from '../apps/icones';
import type { Pilote } from '../base/pilote';
import { fusionner } from '../apps/catalogue';
import { appliquer, lireConnues } from '../depot/application';
import { avancer, lireEnAttentePourVm, terminer } from '../depot/installation';
import { lireParId as lireTeleversement } from '../depot/televersement';
import type { RegistreAgents } from './registre';

/// Les quatre types montants qui appartiennent à ④.
///
/// 🔴 DÉRIVÉE DE L'UNION PAR LE TYPE DE RETOUR DU PRÉDICAT : une variante
/// ajoutée à `VersLaPlateforme` sans sa clé ici ne serait pas refusée par
/// `tsc`, mais elle tomberait dans le `enroler` de `canal.ts`, où la
/// destructuration la refuserait BRUYAMMENT. C'est le comportement voulu — un
/// message de ④ qu'on oublie de router doit se voir, pas se perdre.
export type MontantDeQuatre =
    | CatalogueMessage
    | LanceeMessage
    | ProgressionMessage
    | TermineMessage;

const TYPES_DE_QUATRE: readonly MontantDeQuatre['type'][] = [
    'catalogue',
    'lancee',
    'progression',
    'termine',
];

export function estMontantDeQuatre(message: VersLaPlateforme): message is MontantDeQuatre {
    return (TYPES_DE_QUATRE as readonly string[]).includes(message.type);
}

export interface DependancesMontantes {
    base: Pilote;
    registre: RegistreAgents;
    magasin: Magasin | undefined;
    socket: WebSocket;
    vmId: string;
    maintenant: () => number;
    envoyer: (brut: string) => void;
}

/// L'inventaire des icônes que la plateforme n'a pas, réclamé APRÈS l'écriture
/// du catalogue.
///
/// 🔴 L'INVENTAIRE INTERROGE LE DISQUE, PAS UNE TABLE. Une table de
/// comptabilité divergerait du magasin le jour où un fichier serait perdu — et
/// c'est PRÉCISÉMENT le jour où l'on a besoin de le savoir.
function reclamerLesIcones(
    deps: DependancesMontantes,
    message: CatalogueMessage,
): void {
    if (deps.magasin === undefined) return;
    const annoncees = message.applications
        .map((a) => a.icone)
        .filter((e): e is string => e !== null);
    if (annoncees.length === 0) return;
    const manque = deps.magasin.manquantes(annoncees);
    if (manque.length === 0) return;
    deps.envoyer(encodeIconesManquantes(manque));
}

/// Traite un montant de ④. L'appelant a DÉJÀ vérifié l'enrôlement.
///
/// ⚠️ RIEN N'EST ATTENDU ICI, ET C'EST LA RÈGLE DU FICHIER : un `await` sur le
/// chemin d'un message ferait qu'une base momentanément indisponible
/// ABATTRAIT LA CONNEXION d'un agent qui va très bien, et une promesse rejetée
/// sans `catch` abattrait tout le process Node. C'est la règle que `canal.ts`
/// s'impose déjà pour `marquerVu`.
export function traiter(deps: DependancesMontantes, message: MontantDeQuatre): void {
    const instant = deps.maintenant();

    if (message.type === 'lancee') {
        // ⚠️ SYNCHRONE, et rien à écrire : `resoudre` ne touche qu'une `Map` en
        // mémoire, et IGNORE une demande inconnue plutôt que de lever.
        deps.registre.resoudre(message.demande, message.issue);
        return;
    }

    if (message.type === 'progression') {
        // ⚠️ LE COÛT EST NOMMÉ : une progression perdue ne se voit qu'au
        // journal. Elle se rattrape toute seule — la suivante arrive une
        // seconde plus tard, et le `termine` porte l'état final.
        void avancer(
            deps.base,
            message.installation,
            {
                phase: message.phase,
                octetsFaits: message.octets_faits,
                octetsTotal: message.octets_total,
                ecouleMs: message.ecoule_ms,
            },
            instant,
        ).catch((cause) => {
            console.error(
                `progression non écrite pour l'installation ${message.installation} : ${String(cause)}`,
            );
        });
        return;
    }

    if (message.type === 'termine') {
        // 🔴 CELUI-CI, PERDU, NE SE RATTRAPE PAS TOUT SEUL — et il faut le dire
        // plutôt que de le laisser croire. L'agent n'émet `termine` qu'une
        // fois ; si l'écriture échoue, la ligne reste `en_cours` pour toujours
        // et la réémission ne la reprend pas non plus, puisqu'elle ne vise que
        // les `en_attente`. Le journal est alors la seule trace, et c'est
        // pourquoi il porte l'identifiant.
        //
        // ⚠️ Ce qui protège l'utilisateur d'une installation rejouée n'est PAS
        // cette écriture mais le marqueur sur le disque de la VM : la seconde
        // ceinture existe pour le cas où la première a perdu sa base.
        void terminer(
            deps.base,
            message.installation,
            {
                issue: message.issue,
                motif: message.motif,
                codeSortie: message.code_sortie,
                journal: message.journal,
                journalTronque: message.journal_tronque,
            },
            instant,
        ).catch((cause) => {
            console.error(
                `issue non écrite pour l'installation ${message.installation} : ${String(cause)}`,
            );
        });
        return;
    }

    const identifiant = deps.vmId;
    void lireConnues(deps.base, identifiant)
        .then((connues) => appliquer(deps.base, identifiant, fusionner(connues, message), instant))
        .then(() => reclamerLesIcones(deps, message))
        .catch((cause) => {
            console.error(`catalogue non écrit pour la VM ${identifiant} : ${String(cause)}`);
        });
}

export interface DependancesReemission {
    base: Pilote;
    socket: WebSocket;
    vmId: string;
    envoyer: (brut: string) => void;
}

/// Repousse les ordres d'installation qu'une VM n'a pas encore acquittés.
///
/// 🔴 C'EST LE FILET DU `push` SANS GARANTIE DE LIVRAISON, et il a un jumeau
/// que la recette de G1 a vu fonctionner sur le chemin réel : le
/// `complet = true` que l'agent renvoie à chaque réenrôlement. Sans lui, un
/// ordre émis pendant une coupure serait perdu SANS TERME.
///
/// ⚠️ **ELLE NE VISE QUE LES `en_attente`.** Dès qu'un agent a rapporté une
/// progression, la ligne passe `en_cours` et cesse d'être réémise : c'est la
/// PREMIÈRE des deux ceintures contre une double exécution. La seconde est le
/// marqueur sur le disque de la VM, et elle protège du cas où la première a
/// perdu sa base.
///
/// ⚠️ **L'URL EST DÉRIVÉE, PAS CONFIGURÉE.** Elle est relative — `/televersement/
/// :id/contenu` — et l'agent la résout contre l'adresse de son propre canal,
/// comme il dérive déjà celle du téléversement d'icônes. Deux variables pour la
/// même adresse divergeraient le jour où l'une des deux serait changée.
export async function reemettreLesInstallations(
    deps: DependancesReemission,
): Promise<void> {
    const enAttente = await lireEnAttentePourVm(deps.base, deps.vmId);
    if (enAttente.length === 0) return;
    for (const ligne of enAttente) {
        const tel = await lireTeleversement(deps.base, ligne.televersement_id);
        if (tel === undefined) {
            // ⚠️ INATTEIGNABLE PAR LA CLÉ ÉTRANGÈRE, et gardé quand même : la
            // supposition « la clé étrangère l'empêche » est vraie de la base,
            // pas de ce code. La sauter en le DISANT vaut mieux qu'émettre un
            // ordre dont l'URL ne mènerait nulle part.
            console.error(
                `installation ${ligne.id} sans téléversement ${ligne.televersement_id} : ordre non réémis`,
            );
            continue;
        }
        deps.envoyer(
            encodeInstaller(
                ligne.id,
                `/televersement/${tel.id}/contenu`,
                tel.nom,
                tel.taille,
                tel.sha256,
            ),
        );
    }
}
