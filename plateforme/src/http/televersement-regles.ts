// Les RÈGLES PURES de la surface HTTP du téléversement : reconnaître un chemin,
// dire quelle méthode chacun attend, juger la forme d'une empreinte.
//
// 🔴 EXTRAIT AVANT L'ADDITION, ET C'EST LA RÈGLE DU DÉPÔT, PAS UN GOÛT.
// `routes-televersement.ts` était à 497 lignes pour un plafond de 500 : la
// moindre ligne de commentaire d'une revue transverse l'aurait fait FRANCHIR,
// et le dépôt a payé DEUX FOIS en D9 pour l'avoir rattrapé par une COMPRESSION
// qu'il interdit nommément. L'extraction se joue donc AVANT, jamais après —
// c'est la forme forte, celle de D9 tâche 6 et des trois tâches de D10.
// Précédents de forme : `http/routes-harnais.ts`, `apps/magasin-tranches.ts`.
//
// 🔴 CE MODULE EST PUR : aucune base, aucun socket, aucune horloge, aucun
// `node:` — la propriété qui le rend éprouvable sans monter un serveur.

export type Cible =
    | { quoi: 'creer' }
    | { quoi: 'etat'; id: string }
    | { quoi: 'tranche'; id: string; rang: string }
    | { quoi: 'sceller'; id: string };

/// Reconnaît les QUATRE chemins, et RIEN d'autre.
///
/// 🔴 DÉCOUPÉ PAR SEGMENTS, JAMAIS PAR `startsWith`, ET CE N'EST PAS DU STYLE :
/// G1 a MESURÉ qu'un `startsWith('/application')` laissait DIX-SEPT tests VERTS
/// — la route mangeait toute la famille et rendait SON PROPRE 404 typé,
/// indiscernable du générique tant qu'on ne lisait que le statut. Le contrôle
/// qui vaut compare le CORPS. Chaque motif est ancré des DEUX bouts : nombre de
/// segments EXACT, segments constants comparés par égalité.
///
/// ⚠️ `URL.pathname` NE DÉCODE PAS LE POURCENT, et c'est voulu :
/// `/televersement/..%2F..%2Fetc/tranche/0` arrive en UN seul segment, que
/// `identifiantValide` refusera — décoder d'abord ferait apparaître des
/// séparateurs que le découpage prendrait pour des segments légitimes.
export function reconnaitre(chemin: string): Cible | undefined {
    const s = chemin.split('/');
    if (s[1] !== 'televersement') return undefined;
    // ['', 'televersement'] — exactement deux.
    if (s.length === 2) return { quoi: 'creer' };
    // Trois : `/televersement/` en a trois aussi, mais son identifiant est
    // vide — c'est une barre oblique de trop, pas un chemin.
    if (s.length === 3) return s[2] === '' ? undefined : { quoi: 'etat', id: s[2] };
    if (s.length === 4) {
        return s[2] === '' || s[3] !== 'sceller' ? undefined : { quoi: 'sceller', id: s[2] };
    }
    if (s.length === 5) {
        if (s[2] === '' || s[3] !== 'tranche' || s[4] === '') return undefined;
        return { quoi: 'tranche', id: s[2], rang: s[4] };
    }
    return undefined;
}

/// ⚠️ Une table plutôt que quatre `if` : la correspondance est exhaustive PAR
/// LE TYPAGE, si bien qu'une cinquième cible ne compilerait pas sans sa méthode.
export const METHODE: Readonly<Record<Cible['quoi'], string>> = Object.freeze({
    creer: 'POST',
    etat: 'GET',
    tranche: 'PUT',
    sceller: 'POST',
});

/// ⚠️ MINUSCULES SEULEMENT, comme `apps/icones.ts::empreinteValide` : deux
/// graphies de la même empreinte se compareraient FAUSSES au scellement, et le
/// déposant redéposerait sans fin un fichier juste.
export function empreinteValide(s: string): boolean {
    return /^[0-9a-f]{64}$/.test(s);
}
