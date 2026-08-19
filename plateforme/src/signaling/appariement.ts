// La table des sessions du relais de signaling : qui occupe quel rôle, et
// quelle offre attend son destinataire.
//
// Ce module est PUR — générique en `S`, aucun socket, aucun `ws`, aucune
// horloge —, et c'est ce qui le rend testable sans ouvrir la moindre
// connexion. Le relais l'instancie en `Appariement<WebSocket>`.
//
// Extrait de l'ex-`server.ts` — aujourd'hui `relais.ts` — par le sous-bloc P1,
// AVANT que P2 (garde d'authentification), P3 (liaison agent ↔ VM) et P4
// (appartenance de session) n'y ajoutent quoi que ce soit. Ce dépôt a établi
// en D9 que l'extraction faite AVANT l'addition rend sa marge, et que celle
// faite après se paie d'une compression que `CLAUDE.md` interdit nommément.
//
// ✅ P2 A EU LIEU (19 août 2026), ET LA MARGE A SERVI — mais PAS ici. Ce
// module n'a pas gagné une ligne : la garde vit dans `identite/garde.ts`, la
// propriété de session dans `signaling/propriete.ts`, et c'est `relais.ts`
// seul qui a grossi de les appeler. **L'extraction a donc rendu sa marge au
// fichier qui en avait besoin, ce qui est exactement ce qu'elle promettait.**
// ✅ P3 A EU LIEU (19 août 2026), ET LA PROPRIÉTÉ TIENT UNE SECONDE FOIS : ce
// module n'a toujours pas gagné une ligne. Le préfixe de session vit dans
// `agents/prefixe.ts`, l'identité de l'agent dans `agents/enrolement.ts` et
// `identite/garde.ts`, le canal dans `agents/canal.ts` — et `Appariement` ne
// sait rien de tout cela : il apparie des NOMS, et un nom préfixé reste un
// nom. **C'est ce qui rend le préfixe si bon marché ici** ; deux VMs qui
// ouvraient toutes deux `bureau` cessent de se rencontrer dans la même entrée
// sans qu'une ligne de ce fichier ait bougé.
//
// P4 reste à venir, et la phrase ci-dessus vaut toujours pour lui.


export type Role = 'agent' | 'client';

interface Session<S> {
    agent?: S;
    client?: S;
    /// Dernière offre reçue du client, retenue tant qu'aucun agent n'est là
    /// pour la prendre.
    ///
    /// Le sous-bloc D1 renverse l'ordre d'arrivée : la page navigateur s'ouvre
    /// et envoie son offre AVANT que le superviseur n'ait lancé l'agent de
    /// cette fenêtre — c'est le viewport de cette page qui décide de la taille
    /// de la sortie virtuelle, donc rien ne peut être lancé plus tôt. Sans
    /// cette mémorisation, l'offre serait perdue en silence et la session ne
    /// s'établirait jamais.
    offreEnAttente?: string;
}

// Garde de type : nécessaire pour que TypeScript affine `message.role` (typé
// `any`) en `Role` et autorise l'indexation de `Session` sous `strict`.
export function isRole(value: unknown): value is Role {
    return value === 'agent' || value === 'client';
}

export class Appariement<S> {
    private readonly sessions = new Map<string, Session<S>>();

    /// `undefined` = accepté ; sinon le motif de refus, mot pour mot celui que
    /// l'ex-`server.ts:119-125` produisait. Un pair le lit
    /// (`agent/src/signaling.rs:139`, `client/src/webrtc.ts:130-131`) : le
    /// changer serait un changement de protocole.
    ///
    /// ❌ CES DEUX CITATIONS ÉTAIENT FAUSSES TOUTES LES DEUX, et l'une d'elles
    /// A ÉTÉ RENDUE FAUSSE PAR P3 LUI-MÊME (revue transverse, 19 août 2026).
    /// Elles portaient `signaling.rs:130` et `webrtc.ts:109` : la seconde avait
    /// dérivé sous P2 (la l. 109 est **vide** ; la lecture réelle vit aux
    /// l. 130-131), la première était **juste jusqu'au commit `5fbc89b` de
    /// cette branche**, qui a ajouté le jeton à la poignée de main et poussé la
    /// ligne de 130 à 139. **C'est le naufrage du « 487 » commis à
    /// l'intérieur de la branche qui le dénonce.**
    ///
    /// ⚠️ Le plan de P3 (E11) déclarait `signaling.rs:130` « relu et juste » —
    /// et il l'était **au moment où le plan a été écrit**. Relire une citation
    /// avant d'écrire ne suffit donc pas : il faut la relire **après avoir
    /// exécuté ce qui la déplace**.
    declarer(session: string, role: Role, socket: S): string | undefined {
        const entree = this.sessions.get(session) ?? {};
        if (entree[role]) {
            return `un ${role} est déjà connecté à la session ${session}`;
        }
        entree[role] = socket;
        this.sessions.set(session, entree);
        return undefined;
    }

    /// Le socket d'EN FACE : `pair(s, 'client')` rend l'agent, et l'inverse.
    pair(session: string, role: Role): S | undefined {
        const entree = this.sessions.get(session);
        if (!entree) return undefined;
        return role === 'client' ? entree.agent : entree.client;
    }

    /// La dernière écrase les précédentes — une offre périmée ne sert à rien,
    /// et en garder plusieurs n'aurait pas de destinataire distinct.
    retenirOffre(session: string, sdp: string): void {
        const entree = this.sessions.get(session) ?? {};
        entree.offreEnAttente = sdp;
        this.sessions.set(session, entree);
    }

    /// Rend l'offre en attente et l'oublie : elle ne doit être remise qu'une
    /// fois.
    prendreOffre(session: string): string | undefined {
        const entree = this.sessions.get(session);
        if (!entree) return undefined;
        const offre = entree.offreEnAttente;
        entree.offreEnAttente = undefined;
        return offre;
    }

    /// Retire un rôle, et dit si la session est devenue vide — auquel cas elle
    /// est oubliée, comme le faisait `server.ts:190-192`.
    retirer(session: string, role: Role): { vide: boolean } {
        const entree = this.sessions.get(session);
        if (!entree) return { vide: true };
        delete entree[role];
        const vide = !entree.agent && !entree.client;
        if (vide) this.sessions.delete(session);
        return { vide };
    }
}
