/**
 * Arrondit un viewport à des dimensions paires, plancher à 2.
 *
 * C'est cette taille qui décide de la résolution de la sortie virtuelle créée
 * côté agent. Or l'agent aligne ses régions de capture sur des valeurs paires
 * — l'encodeur NV12 l'exige — et apparie la sortie créée à la taille demandée.
 * Une hauteur impaire rendait donc l'appariement impossible, et la fenêtre ne
 * s'ouvrait jamais (recette D1 §3.1, 1280×713 mesuré sur un pop-up Chrome).
 *
 * Arrondi vers le BAS : agrandir demanderait une sortie plus grande que la
 * zone où l'image sera affichée, donc une image rognée.
 */
export function viewportPair(
    largeur: number,
    hauteur: number,
): { largeur: number; hauteur: number } {
    const pair = (valeur: number) => Math.max(2, Math.round(valeur) & ~1);
    return { largeur: pair(largeur), hauteur: pair(hauteur) };
}
