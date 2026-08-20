// La ligne de journal des évènements NEUFS de P5. Fonction PURE : elle REND la
// ligne, elle ne l'ÉCRIT pas.
//
// 🔴 POURQUOI LA MIGRATION GÉNÉRALE N'A PAS LIEU, et voici le compte qui l'a
// tranchée plutôt qu'une opinion. Relevé le 20 août 2026 :
//   - `console.<methode>` hors tests et hors harnais rend DIX-NEUF
//     occurrences dans ONZE fichiers, dont une est une CHAÎNE DE CODE dans
//     `base/pilote-sqlite.ts:29` et non un appel — donc DIX-HUIT sites réels.
//     Les plus chargés : `agents/canal.ts` (5), `signaling/trace.ts` (3),
//     `signaling/relais.ts` (2), `http/routes-applications.ts` (2) ;
//   - `grep -rln 'spyOn(console' --include='*.test.ts'` rend HUIT fichiers de
//     test qui capturent la console ET ASSERTENT SUR LE CONTENU du message :
//     `agents/canal.test.ts`, `agents/canal-apps.test.ts`,
//     `agents/registre.test.ts`, `http/routes-applications.test.ts`,
//     `http/routes-auth.test.ts`, `orchestration/inventaire-statique.test.ts`,
//     `signaling/garde-fil.test.ts`, `signaling/trace.test.ts` ;
//
//   ⚠️ CES NOMBRES SONT CEUX DU 20 AOÛT 2026, RELEVÉS PAR LA COMMANDE, ET ILS
//   NE SONT PAS CEUX DU PLAN DE P5 — qui annonce 15 occurrences, 9 fichiers et
//   5 fichiers de test. L'écart n'est pas une erreur du plan : le sous-bloc G1
//   a été exécuté ENTIÈREMENT entre sa rédaction et celle-ci, et il a ajouté
//   `agents/registre.ts`, `http/routes-applications.ts` et leurs tests. Le
//   coût de la migration a donc AUGMENTÉ de trois fichiers de test depuis que
//   la décision de ne pas la faire a été prise — ce qui la renforce plutôt
//   qu'il ne l'affaiblit. Les recompter avant de s'y fier : ils dériveront
//   encore.
//   - `index.ts` porte un COUPLAGE NOMMÉ : sa ligne d'annonce doit contenir
//     `le port <n>`, sans quoi `signaling/resilience.test.ts` expire au bout
//     de 10 s SANS QUE RIEN NE DÉSIGNE LA CAUSE.
// Dix-huit sites, huit fichiers de test qui assertent sur des messages, un
// couplage nommé, et AUCUN critère pour juger le résultat : c'est exactement
// le churn non mesuré que ce dépôt punit, au dernier sous-bloc d'une branche.
//
// CE QUE CE MODULE SERT DONC : les évènements NEUFS de P5 — `frein`, `sante`,
// `enrolement freine` — et EUX SEULS. Les quatorze sites existants gardent
// leur forme libre.
//
// ⚠️ LE COÛT DE LA MIGRATION EST CHIFFRÉ CI-DESSUS POUR QUE LE CHANTIER QUI LA
// FERA N'AIT PAS À LE RECOMPTER. Ce qu'elle demande, dans l'ordre : reprendre
// les huit fichiers de test qui assertent sur des messages (ce sont eux le
// coût, pas les dix-huit `console.`), puis le couplage d'`index.ts`, qui est
// le seul dont la rupture est SILENCIEUSE.
//
// ⚠️ ET IL N'ÉCRIT PAS. Rendre la ligne plutôt que l'écrire est ce qui le rend
// testable sans `spyOn(console)` — donc ce qui évite d'ajouter un SIXIÈME
// fichier à la liste ci-dessus.

/// Vrai si la valeur doit être citée. Une espace ou un `=` non cités rendent
/// la ligne ambiguë : on ne saurait pas où finit la valeur et où commence le
/// champ suivant.
function doitEtreCitee(valeur: string): boolean {
    return valeur === '' || /[\s="]/.test(valeur);
}

/// Rend `evenement k=v k=v`.
///
/// 🔴 AUCUNE VALEUR N'EST TRONQUÉE, et ce n'est pas un détail : une adresse
/// tronquée est AMBIGUË — `203.0.113.7` et `203.0.113.70` se liraient pareil
/// —, et l'exploitant ne pourrait plus reconnaître l'adresse de son proxy.
/// Or c'est le SEUL remède au mode de défaillance nommé dans
/// `http/adresse-source.ts` : un proxy dont la confiance n'a pas été déclarée
/// fait dégénérer le frein par adresse en frein GLOBAL, et la seule chose qui
/// le rende visible est cette ligne.
///
/// ⚠️ L'ORDRE DES CHAMPS SUIT CELUI DE L'OBJET, jamais un tri : deux lignes du
/// même évènement doivent se comparer à l'œil.
export function ligne(evenement: string, champs: Record<string, string | number>): string {
    const morceaux = [evenement];
    for (const [cle, brut] of Object.entries(champs)) {
        const valeur = String(brut);
        morceaux.push(
            `${cle}=${doitEtreCitee(valeur) ? `"${valeur.replace(/"/g, '\\"')}"` : valeur}`,
        );
    }
    return morceaux.join(' ');
}
