/// <reference types="vite/client" />
import { describe, expect, it } from 'vitest';
import { PREFIXE_FAUTE } from './noms';

/* The bench's fault injection must not SHIP. It is read from the address bar
   (`?faute-fichiers=1`), and a leftover bench link or a shared one would arm it
   in the real bridge: every folder whose name starts with the fault prefix
   would then fail or hang. The gate is the build itself — `import.meta.env.DEV`
   is replaced by `false` in a production build and the injection is
   tree-shaken out — so the check reads what `npm run build` produced.

   ⚠️ `client/dist` is a BUILD ARTEFACT: run `npm run build` in `client/` before
   this test. Zero bundle found is a FAILURE, never a green check. */

const BUNDLES = import.meta.glob<string>('../../dist/assets/*.js', {
    query: '?raw',
    import: 'default',
    eager: true,
});

function bundles(): [string, string][] {
    const all = Object.entries(BUNDLES);
    if (all.length === 0) {
        throw new Error(
            "client/dist/assets holds no bundle: run 'npm run build' in client/ before " +
                'this test — a missing build is not a green check',
        );
    }
    return all;
}

describe('the production build carries no fault injection', () => {
    it('the bundles DO carry the files bridge (control: the search below can find something)', () => {
        // Without this control, an absence below would also be what a build
        // that lost the whole files bridge returns.
        const withBridge = bundles().filter(([, text]) =>
            text.includes('cannot be told apart from'),
        );
        expect(withBridge.length).toBeGreaterThan(0);
    });

    it('🔴 no bundle carries the fault prefix, its silence or its address-bar switch', () => {
        const culprits = bundles()
            .filter(
                ([, text]) =>
                    text.includes(PREFIXE_FAUTE) ||
                    text.includes(`requests an unknown fault « `) ||
                    text.includes('faute-fichiers'),
            )
            .map(([name]) => name);
        expect(culprits).toEqual([]);
    });
});
