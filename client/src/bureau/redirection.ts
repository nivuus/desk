// LA REDIRECTION DE L'ANCIENNE PAGE-SHELL — une règle, pure et testée.
//
// 🔴 POURQUOI `shell.html` SURVIT COMME REDIRECTION PLUTÔT QUE D'ÊTRE
// SUPPRIMÉ (décision du propriétaire, 31 août 2026).
//
// ⚠️ CORRIGÉ LE 31 AOÛT 2026 (fix round 1 de cette même tâche) : cette phrase
// nommait `start_url` — vrai au moment où elle a été écrite, FAUX depuis que
// cette tâche a fait migrer `start_url` vers la racine (`hub/manifeste.ts`,
// commit `a100e46`). C'est désormais **`id`**, PAS `start_url`, qui porte
// `shell.html?app=<id>` (`hub/manifeste.ts:265`) — `id` est l'IDENTITÉ figée
// de l'application installée, jamais mise à jour (voir son commentaire).
//
// Les manifestes des PWA **DÉJÀ installées** portent donc encore, dans leur
// `id`, `shell.html?app=<id>` — et comme le manifeste est publié en `blob:`
// (`hub/manifeste.ts`, legs G5 déclaré), une telle PWA **ne relira jamais son
// manifeste** : c'est son `start_url` (figé, lui aussi, au moment de
// l'installation) qui décide où elle s'ouvre, et pour une installation faite
// AVANT ce lot, il valait encore `shell.html?app=<id>`. Elle ouvrira donc
// cette adresse pour toujours, et c'est pour CES installations-là — les
// installations déjà faites, pas les futures — que la redirection existe.
// Une installation faite à partir de maintenant porte le `start_url` migré
// vers la racine et n'atteindra plus jamais `shell.html` par ce chemin.
// Supprimer le fichier casserait donc définitivement les premières. Ce n'est
// pas une transition : c'est le chemin définitif de ces installations-là.

/// L'adresse vers laquelle rediriger, chaîne de requête CONSERVÉE.
///
/// ⚠️ `?app=` EST LE POINT : le perdre casserait les PWA aussi sûrement que
/// supprimer le fichier.
export function cibleDeRedirection(recherche: string): string {
    return `/${recherche}`;
}
