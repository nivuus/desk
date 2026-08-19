// Le registre d'appartenance de session : qui a le droit de rejoindre quel nom.
//
// Ce module est PUR — une `Map`, aucun socket, aucune horloge, aucune base —,
// sur le patron exact d'`appariement.ts`, et pour la même raison : c'est ce qui
// le rend testable sans ouvrir la moindre connexion.
//
// 🔴 POURQUOI LA DÉCISION EST EN MÉMOIRE ET NON EN BASE. Le gestionnaire
// `message` de `ws` est SYNCHRONE (`relais.ts`), et `trace.ts` explique en
// toutes lettres pourquoi une promesse rejetée y abat tout le process Node —
// c'est la raison pour laquelle P1 écrit sa trace sans l'attendre. Interroger
// la base pour décider d'accepter un pair remettrait exactement ce que P1 a
// délibérément évité.
//
// ⚠️ LE COÛT, nommé et NON corrigé : ce registre NE SURVIT PAS à un
// redémarrage du service. Après un redémarrage, un nom de session libéré peut
// être revendiqué par un autre utilisateur. Ce n'est pas rattrapable ici : la
// spec §3.1 dit qu'un WebSocket vit dans un processus et un seul, et son §6
// dit que les sessions média SURVIVENT au redémarrage — donc l'état de routage
// est perdu alors que le média continue. La vraie réponse est le PRÉFIXE
// OPAQUE de P3 (spec §3.4), qui rend un nom de session non devinable.
//
// L'ENREGISTREMENT durable, lui, existe bien : `session.utilisateur_id` est
// renseignée à l'appariement par la trace (tâche 13). C'est ce qui rend le mot
// « enregistrée » du critère ③ littéralement vrai, et c'est ce dont P4 aura
// besoin. La DÉCISION et l'ENREGISTREMENT sont deux étages distincts.

export class ProprieteDeSession {
    private readonly proprietaires = new Map<string, string>();

    /// L'identifiant du propriétaire, ou `undefined` si la session est libre.
    proprietaire(session: string): string | undefined {
        return this.proprietaires.get(session);
    }

    /// Revendique, ou reconduit. Le même utilisateur peut revendiquer deux
    /// fois sans erreur : une reconnexion casserait sinon sa propre session.
    revendiquer(session: string, utilisateurId: string): void {
        this.proprietaires.set(session, utilisateurId);
    }

    /// Rend la session à qui la demandera ensuite. Ne jamais libérer perdrait
    /// un nom de session à vie.
    liberer(session: string): void {
        this.proprietaires.delete(session);
    }
}
