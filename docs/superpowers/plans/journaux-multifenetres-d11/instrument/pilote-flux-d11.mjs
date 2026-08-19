#!/usr/bin/env node
// Sous-bloc D11 — pilote de la recette ④ : LA SÉPARATION DES FLUX (leg 8 de D10).
//
// Le critère : à N fenêtres, chaque page décode le flux de SA fenêtre et
// d'aucune autre.
//
// POURQUOI D10 N'A PAS PU LE PROUVER. Son contrôle échantillonnait un
// sous-échantillon 8×8 de l'élément `<video>` VIVANT, page par page, par des
// allers-retours CDP indépendants (étalement mesuré : 187 et 245 ms) sur une
// source dont le fond DÉRIVE à chaque trame. Deux pages décodant le MÊME flux
// rendaient donc des empreintes différentes elles aussi : le contrôle NE
// POUVAIT PAS signaler une collision, et il ne gardait son pouvoir que sur les
// deux pages FIGÉES, où il ne trouvait aucune collision.
//
// CE QUI CHANGE ICI. `anim-d11.html` porte un marqueur d'identité INVARIANT
// DANS LE TEMPS (voir `EXPR_MARQUEUR` dans `commun-d11.mjs`), et le prédicat
// n'échantillonne QUE lui. Deux pages sur le même flux rendent le même
// marqueur PAR CONSTRUCTION, quel que soit l'instant d'échantillonnage.
//
// ⚠️ LE PRÉDICAT A ÉTÉ VU ROUGE, et sur la mire de D10 elle-même :
// `MIRE=…/anim-d4.html node controle-marqueur-d11.mjs` rend
// « CONTROLE REFUSE : 3 echec(s) » — le marqueur y dérive (9-9-7 → 7-9-7) et
// n=3 rend la MÊME valeur que n=4. Transcription versée
// (`controle-marqueur-rouge-sur-mire-d10.log`). C'est la démonstration par la
// mesure de ce que D10 n'établissait que par l'argument.
//
// Les invariants de montage sont dans `commun-d11.mjs` — les relire. En
// particulier : UN `--user-data-dir` PAR FENÊTRE ici, et AUCUNE capture
// d'écran CDP pendant la mesure.
//
// Usage : node pilote-flux-d11.mjs --url=<page-shell> --tours=3

import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, EXPR_MARQUEUR, lancerChrome } from './commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const URL_SHELL = arg('url', 'http://127.0.0.1:5173/shell.html?session=demo');
const TOURS = Number(arg('tours', '3'));
const SORTIE = arg('sortie', 'flux-separation.json');
const log = (...a) => console.log(new Date().toISOString(), ...a);

const dir = await mkdtemp(join(tmpdir(), 'flux-d11-'));
const port = 9400;
const chrome = lancerChrome(port, dir);
const tours = [];
try {
    const cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
    const pages = new Map();
    cdp.on(async (m) => {
        if (m.method !== 'Target.attachedToTarget') return;
        const sid = m.params.sessionId;
        pages.set(sid, m.params.targetInfo.url);
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
    await cdp.send('Target.createTarget', { url: URL_SHELL });
    await dodo(20000);

    for (let tour = 0; tour < TOURS; tour += 1) {
        const debut = Date.now();
        const releve = [];
        // ⚠️ L'étalement est MESURÉ et rapporté, jamais supposé négligeable :
        // c'est lui qui invalidait le contrôle de D10. Ici il n'a plus d'effet
        // — le marqueur est invariant dans le temps — mais le taire ferait
        // perdre au lecteur le moyen de le vérifier.
        for (const [sid, url] of pages) {
            if (/shell/.test(url ?? '')) continue;
            const r = await cdp.evalBorne(sid, EXPR_MARQUEUR, 8000, false);
            releve.push({ sessionId: sid, url, ...r });
        }
        const etalement_ms = Date.now() - debut;
        // Deux pages qui rendent le MÊME marqueur voient le MÊME flux.
        const parMarqueur = new Map();
        for (const r of releve) {
            if (!r || r.marqueur == null) continue;
            parMarqueur.set(r.marqueur, (parMarqueur.get(r.marqueur) ?? 0) + 1);
        }
        const collisions = [...parMarqueur.entries()].filter(([, n]) => n > 1);
        tours.push({ tour, etalement_ms, releve, collisions, pages_mesurees: releve.length });
        log(`tour ${tour} : ${releve.length} pages, etalement ${etalement_ms} ms, `
            + `collisions ${collisions.length ? JSON.stringify(collisions) : 'AUCUNE'}`);
        await dodo(5000);
    }
} finally {
    await writeFile(SORTIE, JSON.stringify({ tours }, null, 2));
    log('releves ecrits dans', SORTIE);
    chrome.kill('SIGKILL');
    await dodo(300);
    await rm(dir, { recursive: true, force: true });
}
