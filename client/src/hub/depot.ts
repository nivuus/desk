// LE POINT DE CONVERGENCE DES DEUX CHEMINS DE DÉPÔT D'UN INSTALLEUR.
//
// 🔴 C'EST CE MODULE QUI REND LE CRITÈRE ② DÉCIDABLE, et son existence est
// une décision, pas un rangement. L'amendement du 28/07/2026 au cadrage
// produit dit : « **tenter l'enregistrement, avec repli silencieux sur le
// glisser-déposer** », et **aucune fonctionnalité ne doit dépendre** de
// `file_handlers`. Deux chemins mènent donc un fichier ici :
//
//   ① le GLISSER-DÉPOSER (`drop`, `DataTransfer.files`) et le sélecteur de
//      fichiers — le chemin NOMINAL, qui doit fonctionner SEUL ;
//   ② la FILE DE LANCEMENT (`launchQueue.setConsumer`), qui n'existe que si
//      le navigateur a honoré `file_handlers` — donc seulement dans une PWA
//      installée, et pas du tout ailleurs.
//
// **Les deux appellent `deposer`, et rien d'autre.** C'est ce qui donne son
// sens à la ROUGE du critère ② : retirer ② doit laisser ① VERT. S'ils avaient
// chacun leur propre séquence, la rouge ne mesurerait que la moitié qu'elle
// retire, et un dépôt cassé au chemin ① passerait inaperçu.
//
// 🔴 AUCUN DOM ICI. `televerser` fait déjà tout le travail — empreindre,
// créer, déposer les tranches manquantes, sceller — et il est PUR, ses
// dépendances injectées. Ce module ne fait que **converger** et **traduire le
// résultat en une phrase**, ce qui est la seule chose que les deux chemins
// avaient en commun et qu'aucun des deux ne devait porter deux fois.

import { televerser, type DepsTeleversement, type Issue } from './televersement';

/// Ce que le hub montre après un dépôt : un ton et une phrase.
///
/// ⚠️ `Ton` EST RECOPIÉ PLUTÔT QU'IMPORTÉ, et c'est une dette CONNUE du dépôt,
/// pas une négligence : `Ton` et `CLASSE_DE_TON` sont déjà dupliqués entre
/// `shell.ts`, `connexion.ts` et `ecran-terminal.ts` — c'est le legs n°8 de ⑥,
/// dont le point de chute nommé est la couche `design/`. G5 ne l'unifie pas
/// (⑥ est clos, et ④ n'a pas juridiction sur ses modules) et **ne l'aggrave
/// pas non plus** : il réemploie le vocabulaire au lieu d'en inventer un
/// quatrième.
export type Ton = 'neutre' | 'succes' | 'danger';

export interface Resume {
    ton: Ton;
    texte: string;
    /// L'identifiant du téléversement, quand il en existe un — c'est ce qui
    /// permettrait de REPRENDRE. Présent même sur un refus, `televerser` le
    /// capturant plutôt que de le passer à chaque issue.
    id?: string;
}

/// Dépose un fichier, et rend ce qu'il faut en dire.
///
/// ⚠️ CETTE FONCTION NE LÈVE PAS SUR UN REFUS — elle en rend un `Resume`.
/// Une panne d'ENVIRONNEMENT (le `fetch` qui rejette hors interruption)
/// remonte, elle, telle quelle : c'est l'arbitrage de
/// `plateforme/src/orchestration/refus.ts`, que `televersement.ts` tient déjà,
/// et le déguiser ici ferait passer une panne pour une décision de protocole.
export async function deposer(fichier: File, deps: DepsTeleversement): Promise<Resume> {
    return resumer(fichier, await televerser(fichier, deps));
}

/// La traduction d'une issue en une phrase. SÉPARÉE de `deposer` pour être
/// éprouvable sans monter un `fetch` factice complet.
export function resumer(fichier: File, issue: Issue): Resume {
    if (issue.etat === 'scelle') {
        return {
            ton: 'succes',
            texte: `${fichier.name} a été téléversé et scellé (${issue.deposees.length} tranche(s) déposée(s)).`,
            id: issue.id,
        };
    }
    const r = issue.refus;
    // ⚠️ LE MOTIF DU SERVICE EST RENDU TEL QUEL, JAMAIS RÉÉCRIT. Le vocabulaire
    // des refus appartient à `plateforme/`, que `client/` ne peut pas importer ;
    // le traduire ici en ferait une copie qu'aucun type ne confronte à sa
    // source, silencieusement fausse au renommage — le défaut que
    // `connexion.ts` déclare sur `aucune-vm` et que P4 a légué sans le fermer.
    const texte =
        r.source === 'client'
            ? `${fichier.name} n'a pas été téléversé : ${r.motif} (${r.detail}).`
            : `${fichier.name} a été refusé par le service à l'étape « ${r.etape} » : ${r.motif}.`;
    return { ton: 'danger', texte, id: issue.id };
}
