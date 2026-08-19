// La règle de la poignée de main : qui passe, qui est refusé, avec quel motif.
//
// 🔴 CE MODULE EST PUR ET SYNCHRONE, et ce n'est pas un détail de confort. Le
// gestionnaire `message` de `ws` est synchrone (`signaling/relais.ts`), et
// `signaling/trace.ts` explique pourquoi une promesse rejetée y abat tout le
// process Node. Une signature qui rendrait une promesse INVITERAIT un appelant
// à l'attendre — exactement ce que P1 a interdit pour la trace. La garde ne
// touche donc ni la base, ni une horloge réelle : les deux lui sont injectées.
//
// 🔴 `verifier` ET `revendiquer` SONT DEUX APPELS DISTINCTS, et la distinction
// n'est pas cosmétique : la revendication ne doit avoir lieu qu'APRÈS que
// `Appariement::declarer` a accepté. Sinon un pair refusé pour cause de rôle
// déjà occupé laisserait derrière lui une appartenance FANTÔME, et le pair
// légitime se verrait refuser sa propre session. Les deux appels sont dans le
// même bloc synchrone du relais : il n'y a pas de course entre eux, et c'est
// ce qui autorise à les séparer.
//
// ✅ CE QUE P2 NE FERMAIT PAS EST FERMÉ (sous-bloc P3). P2 acceptait ici tout
// pair déclarant `{"role":"agent"}` SANS jeton, qui recevait donc des
// identifiants TURN valables 86 400 s (`signaling/ice.ts`) sans présenter la
// moindre identité — la moitié `agent` du trou, que le libellé du critère ① de
// P2 déclarait explicitement. Le rôle `agent` exige désormais son jeton,
// exactement comme le rôle `client`, et deux choses de plus :
//
//   1. le jeton doit être DE TYPE `agent` (`identite/jeton.ts`, claim `sty`) —
//      sans quoi un jeton humain volé ouvrirait un rôle `agent` ; et
//      réciproquement un jeton d'agent ne peut PAS ouvrir un rôle `client`,
//      ce qui contournerait l'appartenance de session posée par P2 ;
//   2. le SUJET du jeton doit PRÉFIXER le nom de session demandé — sans quoi
//      un agent enrôlé occuperait la session de toute autre VM, et
//      l'enrôlement n'authentifierait que l'existence d'une VM, jamais
//      LAQUELLE.
//
// 🔴 LA GARDE NE LIT PAS `agent_enrole`, ET C'EST STRUCTUREL, pas une
// économie. Elle est PURE ET SYNCHRONE (voir l'avertissement ci-dessus), et
// une lecture de base y demanderait un `await` sur le chemin de la poignée de
// main. L'identité a été établie AILLEURS — sur le canal `/agent`, qui est
// asynchrone sans gêner personne et qui délivre le jeton ; la garde ne fait
// que la relire dans ce jeton.

import { verifierJeton, type TypeSujet } from './jeton';
import { SEPARATEUR } from '../agents/prefixe';
import type { ProprieteDeSession } from '../signaling/propriete';
import type { Role } from '../signaling/appariement';

export type MotifRefus = 'jeton-absent' | 'jeton-invalide' | 'jeton-expire' | 'session-refusee';

/// ⚠️ Le refus porte DEUX textes, et c'est délibéré : `message` part SUR LE
/// FIL, `journal` reste chez nous.
///
/// La spec exige un « refus typé, journalisé, avec l'identifiant demandé »
/// (critère ③) — mais dire au demandeur que la session appartient à un autre,
/// ou même qu'elle existe, serait un ORACLE : il apprendrait par tâtonnement
/// quels noms de session sont pris. Le champ `journal` porte donc le nom de
/// session et l'identifiant du demandeur ; `message` ne porte ni l'un ni
/// l'autre. C'est ce qui sépare un diagnostic d'un oracle.
///
/// ⚠️ `journal` est une EXTENSION de l'interface fixée par le plan (§
/// « Interfaces partagées »), assumée ici : l'alternative aurait été de
/// journaliser DANS la garde, ce qui lui donnerait un effet de bord d'entrée /
/// sortie et contredirait le mot « pure » de sa propre spécification. Le
/// relais écrit la ligne (`signaling/relais.ts`).
export type Verdict =
    | { ok: true; utilisateurId?: string }
    | { ok: false; motif: MotifRefus; message: string; journal: string };

export interface Garde {
    /// SANS EFFET DE BORD : elle décide, elle n'inscrit rien.
    verifier(poignee: { role: Role; session: string; jeton?: unknown }): Verdict;
    /// Appelée APRÈS que `Appariement::declarer` a accepté, et seulement alors.
    revendiquer(session: string, utilisateurId: string | undefined): void;
    liberer(session: string): void;
}

/// Le seul texte qu'un pair refusé pour cause d'appartenance reçoit. Il ne dit
/// ni à qui la session appartient, ni si elle existe.
const MESSAGE_SESSION_REFUSEE = 'accès refusé à la session demandée';

export function garde(
    secret: string,
    maintenant: () => number,
    proprietes: ProprieteDeSession,
): Garde {
    return {
        verifier({ role, session, jeton }): Verdict {
            if (jeton === undefined || jeton === null || jeton === '') {
                return {
                    ok: false,
                    motif: 'jeton-absent',
                    message: 'authentification requise',
                    journal: `poignée de main sans jeton sur la session ${session}`,
                };
            }

            const verdict = verifierJeton(jeton, secret, maintenant());
            if (!verdict.ok) {
                const expire = verdict.motif === 'expire';
                return {
                    ok: false,
                    motif: expire ? 'jeton-expire' : 'jeton-invalide',
                    // Le pair a besoin de savoir s'il doit RAFRAÎCHIR ou se
                    // reconnecter : la distinction expiré / invalide n'est pas
                    // un oracle, elle porte sur SON propre jeton.
                    message: expire ? 'jeton expiré' : 'jeton invalide',
                    journal: `jeton refusé (${verdict.motif}) sur la session ${session}`,
                };
            }

            // 🔴 LE TYPE ATTENDU DÉPEND DU RÔLE, ET LES DEUX SENS SONT
            // GARDÉS. Ne garder qu'un sens laisserait l'autre confusion
            // ouverte, et chacune est grave à sa façon — voir l'en-tête.
            const attendu: TypeSujet = role === 'agent' ? 'agent' : 'utilisateur';
            if (verdict.type !== attendu) {
                return {
                    ok: false,
                    motif: 'session-refusee',
                    message: MESSAGE_SESSION_REFUSEE,
                    journal:
                        `session ${session} refusée à ${verdict.sujet} : ` +
                        `jeton de type ${verdict.type} présenté pour le rôle ${role}`,
                };
            }

            if (role === 'agent') {
                // Le sujet d'un jeton d'agent EST le préfixe de sa VM. La
                // comparaison porte le SÉPARATEUR, et ce n'est pas cosmétique :
                // un `startsWith(sujet)` nu ferait qu'un agent de préfixe `AB`
                // occupe les sessions de la VM `ABC`, dont le préfixe le
                // prolonge — une collision qui ne se produirait qu'entre deux
                // VMs précises, donc jamais en essai et toujours en production.
                if (!session.startsWith(verdict.sujet + SEPARATEUR)) {
                    return {
                        ok: false,
                        motif: 'session-refusee',
                        message: MESSAGE_SESSION_REFUSEE,
                        journal:
                            `session ${session} refusée à l'agent ${verdict.sujet} : ` +
                            `elle ne porte pas son préfixe`,
                    };
                }
                // ⚠️ L'AGENT NE REVENDIQUE TOUJOURS RIEN, et `verifier` ne rend
                // donc PAS d'`utilisateurId` ici. Sa session doit rester
                // revendicable par le client humain qui la rejoindra — c'est
                // ce que `revendiquer` documente juste en dessous, et le rendre
                // ferait de l'agent le propriétaire de sa propre session, donc
                // interdirait à quiconque de s'y connecter.
                return { ok: true };
            }

            const proprietaire = proprietes.proprietaire(session);
            if (proprietaire !== undefined && proprietaire !== verdict.sujet) {
                return {
                    ok: false,
                    motif: 'session-refusee',
                    message: MESSAGE_SESSION_REFUSEE,
                    journal: `session ${session} refusée à ${verdict.sujet} : elle appartient à un autre utilisateur`,
                };
            }

            return { ok: true, utilisateurId: verdict.sujet };
        },

        revendiquer(session, utilisateurId): void {
            // Un `agent` a désormais une identité (P3), mais il ne revendique
            // TOUJOURS rien : sa session doit rester revendicable par le client
            // humain qui la rejoindra. `verifier` ne rend aucun `utilisateurId`
            // pour le rôle `agent`, et c'est ce qui fait passer ce chemin-ci.
            if (utilisateurId === undefined) return;
            proprietes.revendiquer(session, utilisateurId);
        },

        liberer(session): void {
            proprietes.liberer(session);
        },
    };
}
