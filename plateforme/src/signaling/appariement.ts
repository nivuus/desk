// La table des sessions du relais de signaling : qui occupe quel rôle, et
// quelle offre attend son destinataire.
//
// Ce module est PUR — générique en `S`, aucun socket, aucun `ws`, aucune
// horloge —, et c'est ce qui le rend testable sans ouvrir la moindre
// connexion. Le relais l'instancie en `Appariement<WebSocket>`.
//
// Extrait de l'ex-`server.ts` — aujourd'hui `relais.ts` — par le sous-bloc P1,
// AVANT que P2 (garde
// d'authentification), P3 (liaison agent ↔ VM) et P4 (appartenance de session)
// n'y ajoutent quoi que ce soit. Ce dépôt a établi en D9 que l'extraction
// faite AVANT l'addition rend sa marge, et que celle faite après se paie d'une
// compression que `CLAUDE.md` interdit nommément.

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
    /// (`agent/src/signaling.rs:130`, `client/src/webrtc.ts:109`) : le changer
    /// serait un changement de protocole.
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
