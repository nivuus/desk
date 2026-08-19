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
// ⚠️ CE QUE P2 NE FERME PAS, et il faut le lire ici plutôt que le découvrir :
// un pair qui se déclare `{"role":"agent", session:"n-importe-quoi"}` est
// ACCEPTÉ SANS JETON, et reçoit donc des identifiants TURN valables 86 400 s
// (`signaling/ice.ts`). L'agent Rust n'a aucune identité avant P3
// (`agent/src/signaling.rs`), et lui en exiger une casserait le chantier D en
// cours. P2 ne ferme donc que la MOITIÉ `client` du trou, et c'est écrit dans
// le libellé même de son critère ①.

import { verifierJeton } from './jeton';
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
            // Voir l'avertissement de tête : la fenêtre `agent` est déclarée,
            // pas oubliée.
            if (role === 'agent') return { ok: true };

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
            // Un `agent` n'a pas d'identité avant P3 : il ne revendique rien,
            // et sa session reste revendicable par le client qui la rejoindra.
            if (utilisateurId === undefined) return;
            proprietes.revendiquer(session, utilisateurId);
        },

        liberer(session): void {
            proprietes.liberer(session);
        },
    };
}
