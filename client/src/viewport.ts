/**
 * Rounds a viewport to even dimensions, floor at 2.
 *
 * It is this size that decides the resolution of the virtual output created
 * on the agent side. Yet the agent aligns its capture regions on even values
 * — the NV12 encoder requires it — and pairs the created output with the requested size.
 * An odd height therefore made pairing impossible, and the window never
 * opened (acceptance run D1 §3.1, 1280×713 measured on a Chrome pop-up).
 *
 * Rounded DOWN: enlarging would request an output larger than the
 * area where the image will be displayed, hence a cropped image.
 */
export function viewportPair(
    largeur: number,
    hauteur: number,
): { largeur: number; hauteur: number } {
    const pair = (valeur: number) => Math.max(2, Math.round(valeur) & ~1);
    return { largeur: pair(largeur), hauteur: pair(hauteur) };
}
