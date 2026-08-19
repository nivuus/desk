// Le dépôt `utilisateur` : créer un compte, le lire par courriel, remplacer
// son empreinte.
//
// 🔴 L'HORLOGE EST UN PARAMÈTRE, jamais lue ici — même règle que
// `depot/session.ts` et `signaling/ice.ts`, et c'est ce qui rend
// `utilisateur.test.ts` capable d'asserter une époque EXACTE.
//
// 🔴 AUCUNE VALEUR LITTÉRALE dans le SQL, pas même une constante : tout passe
// en paramètre, sans quoi `rendreMarqueurs` lèverait côté Postgres
// (`base/pilote.ts`).
//
// ⚠️ Ce module ne sait RIEN du hachage : il reçoit et rend une empreinte
// opaque. C'est ce qui permettra de changer d'algorithme sans le rouvrir —
// le format porte le sien (`identite/mot-de-passe.ts`).

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

export interface LigneUtilisateur {
    id: string;
    email: string;
    empreinte_mdp: string;
    cree_a: number;
}

/// Crée un compte et rend son identifiant.
///
/// Un courriel déjà pris fait LEVER, par l'index UNIQUE du socle — jamais un
/// retour silencieux : ici l'appelant est l'administrateur, et lui cacher
/// l'échec créerait deux comptes dans sa tête pour un seul en base.
export async function creerUtilisateur(
    p: Pilote,
    email: string,
    empreinteMdp: string,
    maintenant: number,
): Promise<string> {
    const id = randomUUID();
    await p.executer(
        'INSERT INTO utilisateur(id, email, empreinte_mdp, cree_a) VALUES(?, ?, ?, ?)',
        [id, email, empreinteMdp, maintenant],
    );
    return id;
}

/// Rend la ligne, ou `undefined`. JAMAIS une exception sur un courriel
/// inconnu : l'appelant HTTP doit répondre 401 comme pour un mot de passe
/// faux, et un 500 sur ce chemin serait un oracle d'énumération de comptes.
export async function lireParEmail(
    p: Pilote,
    email: string,
): Promise<LigneUtilisateur | undefined> {
    const lignes = await p.interroger<LigneUtilisateur>(
        'SELECT id, email, empreinte_mdp, cree_a FROM utilisateur WHERE email = ?',
        [email],
    );
    return lignes[0];
}

/// Remplace l'empreinte, et rien d'autre — la clause ne touche ni le courriel
/// ni l'instant de création. C'est le chemin du re-hachage à la connexion
/// suivante, celui qui rend inutile toute migration de données le jour où les
/// paramètres de `scrypt` changeront.
export async function remplacerEmpreinte(
    p: Pilote,
    id: string,
    empreinte: string,
): Promise<void> {
    await p.executer('UPDATE utilisateur SET empreinte_mdp = ? WHERE id = ?', [empreinte, id]);
}
