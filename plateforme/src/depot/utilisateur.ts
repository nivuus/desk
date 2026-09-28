// The `user` repository: create an account, read it by email, replace
// its hash.
//
// 🔴 THE CLOCK IS A PARAMETER, never read here — same rule as
// `depot/session.ts` and `signaling/ice.ts`, and it is what makes
// `user.test.ts` able to assert an EXACT epoch.
//
// 🔴 NO LITERAL VALUE in the SQL, not even a constant: everything goes in
// as a parameter, otherwise `rendreMarqueurs` would throw on the Postgres side
// (`base/pilote.ts`).
//
// ⚠️ This module knows NOTHING about hashing: it receives and returns an opaque
// hash. That is what will allow changing the algorithm without reopening it —
// the format carries its own (`identite/mot-de-passe.ts`).

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

export interface UserRow {
    id: string;
    email: string;
    empreinte_mdp: string;
    cree_a: number;
}

/// Creates an account and returns its identifier.
///
/// An email already taken makes it THROW, through the UNIQUE index of the base schema — never a
/// silent return: here the caller is the administrator, and hiding
/// the failure from them would create two accounts in their head for a single one in the database.
export async function createUser(
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

/// Returns the row, or `undefined`. NEVER an exception on an unknown
/// email: the HTTP caller must answer 401 as for a wrong
/// password, and a 500 on this path would be an account enumeration oracle.
export async function lireParEmail(
    p: Pilote,
    email: string,
): Promise<UserRow | undefined> {
    const lignes = await p.interroger<UserRow>(
        'SELECT id, email, empreinte_mdp, cree_a FROM utilisateur WHERE email = ?',
        [email],
    );
    return lignes[0];
}

/// Replaces the hash, and nothing else — the clause touches neither the email
/// nor the creation instant. It is the path of rehashing at the next
/// sign-in, the one that makes any data migration useless the day the
/// parameters of `scrypt` change.
export async function remplacerEmpreinte(
    p: Pilote,
    id: string,
    empreinte: string,
): Promise<void> {
    await p.executer('UPDATE utilisateur SET empreinte_mdp = ? WHERE id = ?', [empreinte, id]);
}
