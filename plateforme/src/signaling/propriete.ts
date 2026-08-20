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
// ✅ CE PRÉFIXE EXISTE DEPUIS LE 19 AOÛT 2026 (sous-bloc P3), et l'énoncé
// ci-dessus cesse d'être un pronostic : `agents/prefixe.ts` tire 128 bits de
// `randomBytes`, et `agent_enrole.prefixe_session` les rend DURABLES —
// contrairement à ce registre-ci, le préfixe survit donc au redémarrage du
// service, ce qui est exactement ce qui manquait.
//
// ⚠️ CE QUE LE PRÉFIXE NE RÉPARE PAS, et qu'il faut dire pour ne pas lire la
// ligne ci-dessus comme une clôture : il est PAR VM, pas par session. Après un
// redémarrage, deux clients humains de la MÊME VM retrouvent le même préfixe,
// et le registre en mémoire qui décidait lequel possède `<préfixe>:w-1` est,
// lui, toujours perdu. Le préfixe ferme la devinabilité entre VMs ; il ne
// ferme pas la revendication d'une session au sein d'une VM. Le coût nommé
// ci-dessus reste donc OUVERT, réduit et non supprimé.
//
// L'ENREGISTREMENT durable, lui, existe bien : `session.utilisateur_id` est
// renseignée à l'appariement par la trace (tâche 13). C'est ce qui rend le mot
// « enregistrée » du critère ③ littéralement vrai, et c'est ce dont P4 aura
// besoin. La DÉCISION et l'ENREGISTREMENT sont deux étages distincts.
//
// ✅ P4 EN A EU BESOIN, ET IL LA LIT (20 août 2026) : `depot/session.ts::
// compterOuvertesDe` compte les lignes non closes d'un utilisateur, et
// `GET /vm` en rend le champ `sessions_ouvertes` par VM. Le futur « aura
// besoin » est donc du passé. ⚠️ MAIS CE LECTEUR NE DIT PAS CE QU'ON POURRAIT
// LUI FAIRE DIRE : il compte des LIGNES ouvertes, pas des sessions média
// vivantes — le média survit à un redémarrage du service alors que la ligne
// est close par le balayage, et une ligne ouverte peut correspondre à un pair
// parti sans que la déconnexion ait été vue. Le champ s'appelle
// `sessions_ouvertes` et non `sessions_actives` : le NOM porte la réserve.
//
// ⚠️ ET LE PARAGRAPHE PLUS HAUT — le préfixe PAR VM, ce registre EN MÉMOIRE —
// n'est PAS soldé par P4, qui n'a fait que brancher la SOURCE du préfixe
// (`client/src/connexion.ts` -> `poserPrefixe`). Sa PORTÉE n'a pas changé :
// deux clients humains de la même VM retrouvent toujours le même préfixe après
// un redémarrage du service, et `<préfixe>:w-1` redevient revendicable.

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
