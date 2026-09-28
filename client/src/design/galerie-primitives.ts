/**
 * THE ENTRY POINT OF `client/primitives.html` — the primitives gallery.
 *
 * ⚠️ THIS MODULE IS NOT PART OF THE PRODUCT, and it has no test: same status as
 * `galerie.ts` and `selecteur-theme.ts`, for the same reason.
 *
 * 🔴 IT READS NO TOKEN, and that is what distinguishes it from `galerie.ts`:
 * this page does not show VALUES, it shows COMPONENTS. There
 * is therefore no call to `getComputedStyle`, and nothing to redraw when the theme
 * changes — the `apres` callback is deliberately empty, the primitives following
 * the theme through their `var(--…)`s alone.
 */
import { installThemeSelectorInDOM } from './selecteur-theme';

const hote = document.getElementById('themes');
if (!hote) throw new Error('the primitives gallery expects a #themes element');

installThemeSelectorInDOM(hote, () => {});
