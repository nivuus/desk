// Le dépôt `televersement` : la ligne d'un fichier déposé par un utilisateur.
//
// 🔴 L'HORLOGE EST UN PARAMÈTRE, jamais lue ici — règle de `depot/agent.ts`,
// `depot/session.ts`, `depot/application.ts` et de tout ce dépôt.
//
// 🔴 AUCUNE VALEUR LITTÉRALE dans le SQL : tout passe en paramètre, `null`
// compris, sans quoi `rendreMarqueurs` lèverait côté Postgres.
// ⚠️ Cette moitié du lint ne mord QUE sur le chemin Postgres — le lint statique
// de `base/sous-ensemble.test.ts` ne balaie que les `.sql`. Une requête fautive
// écrite ici serait verte sous `test:sqlite` seul.
//
// 🔴 CE MODULE NE DÉCIDE RIEN, ET SURTOUT PAS DU DÉCOUPAGE. La règle des
// tranches vit dans `proto/ts/tranches.ts`, PURE et PARTAGÉE avec le
// navigateur : deux arithmétiques indépendantes divergeraient un jour, et le
// symptôme serait un scellement qui refuse sans qu'on sache lequel des deux
// bouts a tort. Ici on écrit ce qu'on nous donne.
//
// 🔴 LES TRANCHES NE SONT PAS DANS CETTE TABLE, et c'est la décision D7 :
// la reprise est un LISTAGE de répertoire, jamais une comptabilité qui
// pourrait diverger du disque. Une colonne `tranches_presentes` serait
// exactement cette seconde source de vérité.

import { randomUUID } from 'node:crypto';
import type { Pilote } from '../base/pilote';

export interface LigneTeleversement {
    id: string;
    utilisateur_id: string;
    /// Le nom tel que le NAVIGATEUR l'annonce. Il n'est assaini qu'au moment de
    /// devenir un chemin, côté agent — jamais ici, où il n'est qu'une donnée.
    nom: string;
    /// ✅ `number` EST VRAI SUR LES DEUX MOTEURS, et ce ne l'a pas toujours
    /// été : `pg` rend les `BIGINT` en chaîne, et cette déclaration aurait été
    /// FAUSSE en production sans le `setTypeParser` de
    /// `base/pilote-postgres.ts` (recette de P3). `interroger<T>` faisant un
    /// `as T[]`, aucun typage ne l'attraperait.
    taille: number;
    /// L'empreinte du fichier ENTIER — une seule valeur, comparable partout, y
    /// compris par un humain avec un `sha256sum`.
    sha256: string;
    /// Figée à la création : le découpage ne doit pas changer sous les tranches
    /// déjà déposées.
    taille_tranche: number;
    cree_a: number;
    /// `null` = pas encore scellé. ⚠️ Ce n'est PAS `0`, qui se lirait comme une
    /// époque de 1970 — même raisonnement que `application.disparue_a`.
    scelle_a: number | null;
}

export async function creer(
    p: Pilote,
    entree: {
        utilisateurId: string;
        nom: string;
        taille: number;
        sha256: string;
        tailleTranche: number;
    },
    maintenant: number,
): Promise<LigneTeleversement> {
    const ligne: LigneTeleversement = {
        id: randomUUID(),
        utilisateur_id: entree.utilisateurId,
        nom: entree.nom,
        taille: entree.taille,
        sha256: entree.sha256,
        taille_tranche: entree.tailleTranche,
        cree_a: maintenant,
        scelle_a: null,
    };
    await p.executer(
        'INSERT INTO televersement(id,utilisateur_id,nom,taille,sha256,taille_tranche,cree_a,scelle_a)'
            + ' VALUES(?,?,?,?,?,?,?,?)',
        [
            ligne.id,
            ligne.utilisateur_id,
            ligne.nom,
            ligne.taille,
            ligne.sha256,
            ligne.taille_tranche,
            ligne.cree_a,
            ligne.scelle_a,
        ],
    );
    return ligne;
}

export async function lireParId(
    p: Pilote,
    id: string,
): Promise<LigneTeleversement | undefined> {
    const lignes = await p.interroger<LigneTeleversement>(
        'SELECT id, utilisateur_id, nom, taille, sha256, taille_tranche, cree_a, scelle_a'
            + ' FROM televersement WHERE id = ?',
        [id],
    );
    return lignes[0];
}

/// Marque le scellement. **L'empreinte a DÉJÀ été recalculée** par l'appelant :
/// ce module n'en juge pas.
export async function sceller(p: Pilote, id: string, maintenant: number): Promise<void> {
    await p.executer('UPDATE televersement SET scelle_a = ? WHERE id = ?', [maintenant, id]);
}

/// Combien de téléversements NON SCELLÉS un utilisateur a en cours.
///
/// 🔴 C'EST UN QUOTA, PAS UN FREIN, et la distinction est écrite dans la
/// décision D8 du plan : le frein (`securite/frein.ts`) existe pour les portes
/// PRÉ-AUTHENTIFIÉES, où un pair anonyme devine un secret. Ces routes-ci
/// exigent toutes un jeton valide ; ce qui les protège est une borne sur ce
/// qu'un utilisateur AUTHENTIFIÉ peut faire travailler le disque.
export async function compterEnCours(p: Pilote, utilisateurId: string): Promise<number> {
    const lignes = await p.interroger<{ n: number }>(
        'SELECT COUNT(*) AS n FROM televersement WHERE utilisateur_id = ? AND scelle_a IS NULL',
        [utilisateurId],
    );
    return Number(lignes[0]?.n ?? 0);
}

/// Les téléversements plus vieux que `avant`, pour le balayage d'âge.
///
/// ⚠️ IL REND AUSSI LES SCELLÉS : un téléversement scellé dont l'installation
/// a réussi n'a plus de raison d'occuper le disque. C'est l'appelant qui décide
/// lesquels supprimer — la clé étrangère refusera ceux qu'une installation
/// référence encore, et **c'est voulu** : l'historique d'une installation doit
/// rester lisible.
export async function lirePlusVieuxQue(
    p: Pilote,
    avant: number,
): Promise<LigneTeleversement[]> {
    return p.interroger<LigneTeleversement>(
        'SELECT id, utilisateur_id, nom, taille, sha256, taille_tranche, cree_a, scelle_a'
            + ' FROM televersement WHERE cree_a < ?',
        [avant],
    );
}

export async function supprimer(p: Pilote, id: string): Promise<void> {
    await p.executer('DELETE FROM televersement WHERE id = ?', [id]);
}
