// La FORME de la balise que le greffon Vite injecte pour l'amorce anti-FOUC —
// extraite de `client/vite.config.ts` pour une seule raison : RESTER
// TYPECHECKÉE.
//
// 🔴 `vite.config.ts` N'EST JAMAIS TYPECHECKÉ (voir son en-tête) : il est hors
// des deux motifs de `client/tsconfig.json:12` (`src/**/*.ts`,
// `../proto/ts/**/*.ts`). Un test qui l'importerait DIRECTEMENT traînerait ses
// erreurs de type dans `npx tsc --noEmit` — qui DOIT rester une étape
// DISTINCTE de `npx vitest run`, `CLAUDE.md` le dit noir sur blanc : « VITEST
// TRANSPILE SANS VÉRIFIER LES TYPES ». Ce fichier-ci vit sous `src/`, DONC il
// est typechecké, et `vite.config.ts` l'IMPORTE — le sens inverse ne pose
// aucun problème, Rollup ne typechecke jamais son propre fichier de config.
//
// ⚠️ CE FICHIER N'EST DÉLIBÉRÉMENT PAS CELUI QUI LIT LE CONTENU DE L'AMORCE.
// `vite.config.ts` doit lire `amorce-theme.js` par `node:fs` — un import
// `?raw` NE RÉSOUT PAS quand Vite bundle SA PROPRE configuration (mesuré :
// `npm run build` échoue avec « No matching export … for import "default" »,
// esbuild traitant `?raw` comme un chemin de fichier littéral hors du
// pipeline de greffons que ce mode de chargement n'active pas). Et
// `client/` n'a pas `@types/node` (`src/presse-papier.test.ts` porte le même
// constat), donc un `readFileSync` ICI casserait `npx tsc --noEmit`. D'où la
// scission : `vite.config.ts` garde SA lecture `node:fs` (non typechecké,
// comme avant ce lot), et ce module-ci ne porte que ce qui n'a besoin
// d'AUCUNE lecture disque — le NOM du fichier émis et la FORME de la balise
// qui le référence. Le contenu lui-même, `amorce-theme.csp.test.ts` le lit
// par `?raw` (qui, LUI, résout très bien sous Vitest).
//
// Voir `client/vite.config.ts` (le grand commentaire au-dessus de
// `NOM_FICHIER_AMORCE`) pour le RAISONNEMENT COMPLET du lot `csp-amorce`
// (29 août 2026) : pourquoi l'amorce est un fichier externe `'self'` et non
// un hash `sha256-…` dans la CSP.

/// Le nom sous lequel l'amorce est émise dans `dist/` — un nom STABLE, hors
/// de `assets/` (le seul répertoire que Vite empreinte), donc jamais un an
/// d'`immutable` sur un contenu qui change sans que le nom ne bouge.
export const NOM_FICHIER_AMORCE = 'amorce-theme.js';

/// La balise que le greffon Vite injecte dans le `<head>` de CHAQUE page.
///
/// 🔴 `src`, JAMAIS `children` — un script en LIGNE, ET C'EST TOUT LE DÉFAUT
/// mesuré le 29 août 2026 (`https://app.allanic.me`, Chrome) : la CSP de la
/// plateforme (`plateforme/src/http/page/entetes-page.ts::CSP`) porte
/// `script-src 'self'` sans `'unsafe-inline'` ni hash, et bloquait ce script
/// tant qu'il partait en ligne.
///
/// 🔴 NI `async`, NI `defer`, NI `type: 'module'` — les trois DIFFÉRERAIENT
/// l'exécution après l'analyse de `<body>`, donc après la première peinture,
/// et casseraient l'anti-FOUC que cette amorce existe pour éviter. Un
/// `<script src>` CLASSIQUE bloque l'analyse du document jusqu'à son
/// exécution, exactement comme le script en ligne qu'il remplace — à un
/// aller-retour réseau près, sur la MÊME origine.
export function baliseAmorce() {
    return {
        tag: 'script' as const,
        attrs: { src: `/${NOM_FICHIER_AMORCE}` },
        injectTo: 'head' as const,
    };
}
