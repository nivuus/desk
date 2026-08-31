// LA REDIRECTION DE L'ANCIENNE PAGE-SHELL — une règle, pure et testée.
//
// 🔴 POURQUOI `shell.html` SURVIT COMME REDIRECTION PLUTÔT QUE D'ÊTRE
// SUPPRIMÉ (décision du propriétaire, 31 août 2026). Les manifestes des PWA
// par application portent `start_url: shell.html?app=<id>`, et ils sont
// publiés en `blob:` (`hub/manifeste.ts`, legs G5 déclaré) : une PWA installée
// **ne relira jamais son manifeste**, donc elle ouvrira cette adresse pour
// toujours. Supprimer le fichier les casserait définitivement. Ce n'est pas
// une transition : c'est le chemin définitif de ces installations-là.

/// L'adresse vers laquelle rediriger, chaîne de requête CONSERVÉE.
///
/// ⚠️ `?app=` EST LE POINT : le perdre casserait les PWA aussi sûrement que
/// supprimer le fichier.
export function cibleDeRedirection(recherche: string): string {
    return `/${recherche}`;
}
