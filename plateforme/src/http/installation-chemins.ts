// La RECONNAISSANCE DES TROIS CHEMINS de `routes-installation.ts`, extraite le
// 22 août 2026. PURE : ni `http`, ni base, ni horloge — c'est ce qui la rend
// éprouvable sans monter de serveur.
//
// 🔴 POURQUOI CETTE EXTRACTION EXISTE, ET LE DIRE VAUT MIEUX QUE DE LA LAISSER
// PARAÎTRE GRATUITE : `routes-installation.ts` était EXACTEMENT à 500 lignes
// sur 500, et la vague de correction de la revue finale devait y ajouter huit
// lignes de commentaire — la phrase « le 404 générique répond alors seul », que
// le servant de page a rendue fausse sans condition. `CLAUDE.md` interdit de
// comprimer pour regagner la marge (« extraire, jamais comprimer »), et il
// interdit tout autant de laisser un fichier franchir le plafond. L'extraction
// est donc la seule issue, et elle porte sur ce que le fichier avait de plus
// autonome : trois déclarations sans aucune dépendance.
//
// ⚠️ UNE EXTRACTION N'EST JAMAIS RIGOUREUSEMENT VERBATIM, et `CLAUDE.md` le
// dit : ici, les deux fonctions passent de `function` privée à `export`, et
// c'est le SEUL changement — aucun corps n'est touché, aucun commentaire n'est
// réécrit.

/// Le chemin de l'ORDRE d'installation, comparé EXACTEMENT.
export const CHEMIN_ORDRE = '/installation';

/// Reconnaît `/installation/:id`, et RIEN d'autre. 🔴 DÉCOUPÉ PAR SEGMENTS,
/// JAMAIS PAR `startsWith` : G1 a MESURÉ qu'un `startsWith('/application')`
/// laissait DIX-SEPT tests verts — la route mangeait la famille et rendait SON
/// PROPRE 404 typé, indiscernable du générique. Ancré des DEUX bouts.
export function installationDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'installation', '<id>'] — exactement trois.
    if (segments.length !== 3) return undefined;
    if (segments[1] !== 'installation') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}

/// Reconnaît `/televersement/:id/contenu`, et RIEN d'autre — même règle. ⚠️ LE
/// MOTIF S'ARRÊTE À `contenu` : c'est ce qui laisse la place aux autres routes
/// de la famille `/televersement/…`, qu'un `startsWith` mangerait toutes.
export function contenuDe(chemin: string): string | undefined {
    const segments = chemin.split('/');
    // ['', 'televersement', '<id>', 'contenu'] — exactement quatre.
    if (segments.length !== 4) return undefined;
    if (segments[1] !== 'televersement' || segments[3] !== 'contenu') return undefined;
    return segments[2] === '' ? undefined : segments[2];
}
