// La vérification d'un secret d'enrôlement d'agent.
//
// 🔴 LE REFUS N'ÉNUMÈRE RIEN. « VM inconnue » et « secret faux » rendent le
// MÊME objet, mot pour mot. Les distinguer donnerait à quiconque ouvre le
// canal `/agent` un oracle : il apprendrait par tâtonnement quelles VMs sont
// enrôlées, en n'ayant qu'à comparer deux réponses. C'est ce que la spec
// §4 P3 ② nomme explicitement comme la rouge de son critère.
//
// ⚠️ CE QUE LE DEMANDEUR N'APPREND PAS, L'EXPLOITANT DOIT POUVOIR LE LIRE. Le
// refus est donc JOURNALISÉ avec le nom de VM demandé, par un `journaliser`
// passé en paramètre — même partage que `identite/garde.ts`, qui porte
// `message` (sur le fil) et `journal` (chez nous) pour la même raison. Le
// journal ne recopie JAMAIS le secret : il vient d'être refusé, le réécrire
// ailleurs n'aurait aucun sens.
//
// ⚠️ L'EMPREINTE RÉEMPLOIE `identite/mot-de-passe.ts`, celle des comptes
// humains, format `scrypt$N$r$p$sel$empreinte`. Deux dérivations dans le même
// service divergeraient le jour où l'une serait durcie — et `verifier` gère
// déjà l'égalisation des longueurs, dont l'absence FAIT LEVER
// `timingSafeEqual` (mesuré en P2).

import type { Pilote } from '../base/pilote';
import { verifier } from '../identite/mot-de-passe';
import { lireParVm } from '../depot/agent';

export type VerdictEnrolement =
    | { ok: true; vmId: string; prefixe: string }
    | { ok: false; motif: 'enrolement' };

/// L'UNIQUE refus. Un seul objet, construit une seule fois, pour qu'aucune
/// branche ne puisse en fabriquer une variante par inadvertance — c'est plus
/// sûr que de se fier à deux littéraux restés identiques par discipline.
const REFUS: VerdictEnrolement = { ok: false, motif: 'enrolement' };

/// Vérifie qu'une VM présente le secret de son enrôlement.
///
/// 🔴 ELLE NE RATTRAPE PAS TOUTES LES EXCEPTIONS, et c'est délibéré :
/// `mot-de-passe.ts::verifier` LÈVE sur un algorithme inconnu, parce qu'un
/// refus muet y serait indiscernable d'un secret faux et que personne ne
/// saurait diagnostiquer une base écrite par une version future du service.
/// Cette exception traverse donc jusqu'au canal, qui la traduit en refus
/// `enrolement` en la journalisant AVEC sa cause. Une empreinte simplement
/// MALFORMÉE, elle, rend `false` sans lever — les deux cas sont distincts.
export async function verifierEnrolement(
    p: Pilote,
    vmId: string,
    secret: string,
    journaliser: (ligne: string) => void,
): Promise<VerdictEnrolement> {
    const ligne = await lireParVm(p, vmId);
    if (ligne === undefined) {
        journaliser(`enrôlement refusé pour la VM ${vmId}`);
        return REFUS;
    }

    if (!(await verifier(secret, ligne.empreinte_secret))) {
        // ⚠️ EXACTEMENT LE MÊME TEXTE que ci-dessus, à dessein : deux libellés
        // différents dans un journal finissent par se retrouver dans une
        // réponse, et l'oracle renaîtrait par la porte de derrière.
        journaliser(`enrôlement refusé pour la VM ${vmId}`);
        return REFUS;
    }

    return { ok: true, vmId, prefixe: ligne.prefixe_session };
}
