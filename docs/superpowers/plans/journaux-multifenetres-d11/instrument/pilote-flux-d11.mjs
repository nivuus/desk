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
// ⚠️ ET IL EST REJOUÉ SUR LA VM, EN PREMIER (tâche 12, step 1) : deux fenêtres
// portant le MÊME `n` doivent faire signaler une collision. Ce rouge-là est
// déterministe et ne dépend pas du produit — si le prédicat ne signale rien,
// il est vacueux et la recette est ANNULÉE.
//
// ⚠️ CE QUE CE CRITÈRE N'ÉTABLIT PAS, écrit d'avance : il établit que deux
// pages ne décodent pas le MÊME flux. Il n'établit PAS que chaque page décode
// le flux de la fenêtre Windows qu'elle prétend montrer — cela demanderait de
// corréler le marqueur au HWND, ce qui n'est pas au programme.
//
// ⚠️ AJOUT DE LA TÂCHE 12, déclaré : ce pilote ne portait, à sa création
// (tâche 8), QUE la plomberie CDP — il ouvrait la page-shell et attendait que
// des fenêtres apparaissent d'elles-mêmes. Aucune n'apparaît si personne ne
// lance le superviseur ni n'ouvre les applications sur la VM. Le montage vient
// de `montage-d11.mjs`, qui porte celui, éprouvé à dix fenêtres, de la
// recette ① de D10.
//
// Les invariants de montage sont dans `commun-d11.mjs` — les relire. En
// particulier : UN `--user-data-dir` PAR FENÊTRE ici, et AUCUNE capture
// d'écran CDP pendant la mesure.
//
// Usage :
//   ETIQUETTE=flux-rouge MIRES=3,3   node pilote-flux-d11.mjs
//   ETIQUETTE=flux-1     MIRES=1,2,3,4,5,6,7,8,9,10 node pilote-flux-d11.mjs

import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, EXPR_MARQUEUR, lancerChrome } from './commun-d11.mjs';
import {
    D11, HOTE, copierLog, journal as log, loadavg, lancerSuperviseur, marqueurs,
    ouvrirFenetre, preparerVm, tuerAgent, virshEtat, winrm,
} from './montage-d11.mjs';

const ETIQUETTE = process.env.ETIQUETTE ?? 'flux-sans-etiquette';
// `MIRES` : le paramètre `n` de CHAQUE fenêtre, dans l'ordre d'ouverture.
// `3,3` = le ROUGE (deux mires identiques) ; `1..10` = le VERT.
const MIRES = (process.env.MIRES ?? '1,2,3').split(',').map((x) => x.trim()).filter(Boolean);
const TOURS = Number(process.env.TOURS ?? 3);
const DELAI_ENTRE_FENETRES_MS = Number(process.env.DELAI_ENTRE_FENETRES_MS ?? 8000);
const ATTENTE_REGLAGE_S = Number(process.env.ATTENTE_REGLAGE_S ?? 45);
const BITRATE = process.env.BITRATE ?? '8000000';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const SORTIE = process.env.SORTIE ?? `${D11}/${ETIQUETTE}.json`;

const port = Number(process.env.PORT_CDP ?? 9400);
const dir = await mkdtemp(join(tmpdir(), 'flux-d11-'));
const chrome = lancerChrome(port, dir, [
    '--remote-allow-origins=*', '--disable-dev-shm-usage', '--disable-gpu',
    '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
    '--disable-features=WebRtcHideLocalIpsWithMdns', 'about:blank',
]);
const releve = { etiquette: ETIQUETTE, mires: MIRES, tours: [], virsh_debut: virshEtat(), loadavg_debut: loadavg() };
try {
    const cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
    const pages = new Map();
    cdp.on(async (m) => {
        if (m.method === 'Target.targetInfoChanged') {
            for (const [, p] of pages) if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
            return;
        }
        if (m.method === 'Target.detachedFromTarget') { pages.delete(m.params.sessionId); return; }
        if (m.method !== 'Target.attachedToTarget') return;
        const { sessionId: sid, targetInfo: ti } = m.params;
        // ⚠️ `type === 'page'` : sans ce filtre le pilote évalue aussi sur les
        // cibles `worker`, qui n'ont pas de `window` et rendent
        // `ReferenceError` — bruit indiscernable d'un échec de mesure (D11,
        // défaut n°5 trouvé par le bras rouge de la recette ①).
        if (ti.type !== 'page') { await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { }); return; }
        pages.set(sid, { targetId: ti.targetId, url: ti.url });
        await cdp.send('Page.enable', {}, sid).catch(() => { });
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
        if (!ti.url.includes('shell.html') && ti.url !== 'about:blank') {
            await cdp.send('Emulation.setDeviceMetricsOverride',
                { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false }, sid).catch(() => { });
        }
        await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
        log('+ page attachée', sid.slice(0, 8), ti.url);
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await cdp.send('Target.setDiscoverTargets', { discover: true });

    log(`>>> ÉTAPE 0 : préparation VM (registre NON touché) — ${MIRES.length} fenêtre(s), mires ${MIRES.join(',')}`);
    releve.preparation = preparerVm(ETIQUETTE);

    await cdp.send('Target.createTarget', { url: URL_SHELL });
    await dodo(4000);
    const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
    if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
    log('statut shell :', await cdp.evalBorne(sidShell, "document.querySelector('#statut').textContent", 8000, false));

    log('>>> lancement du superviseur');
    lancerSuperviseur({ BITRATE });
    await dodo(8000);
    marqueurs('superviseur démarré, 0 fenêtre');

    for (const [i, n] of MIRES.entries()) {
        log(`>>> OUVERTURE fenêtre ${i + 1}/${MIRES.length} (mire n=${n})`);
        ouvrirFenetre(ETIQUETTE, i + 1, n);
        await dodo(DELAI_ENTRE_FENETRES_MS);
    }
    log(`>>> RÉGLAGE (${ATTENTE_REGLAGE_S} s)`);
    await dodo(ATTENTE_REGLAGE_S * 1000);
    releve.marqueurs_journal = marqueurs('avant relevé des marqueurs de flux');

    for (let tour = 0; tour < TOURS; tour += 1) {
        const debut = Date.now();
        const lot = [];
        // ⚠️ L'étalement est MESURÉ et rapporté, jamais supposé négligeable :
        // c'est lui qui invalidait le contrôle de D10. Ici il n'a plus
        // d'effet — le marqueur est invariant dans le temps — mais le taire
        // ferait perdre au lecteur le moyen de le vérifier.
        for (const [sid, p] of pages) {
            if (/shell/.test(p.url ?? '') || p.url === 'about:blank') continue;
            const r = await cdp.evalBorne(sid, EXPR_MARQUEUR, 8000, false);
            lot.push({ sessionId: sid.slice(0, 8), url: p.url, ...r });
        }
        const etalement_ms = Date.now() - debut;
        const parMarqueur = new Map();
        for (const r of lot) {
            if (!r || r.marqueur == null) continue;
            parMarqueur.set(r.marqueur, [...(parMarqueur.get(r.marqueur) ?? []), r.sessionId]);
        }
        const collisions = [...parMarqueur.entries()].filter(([, s]) => s.length > 1);
        releve.tours.push({ tour, etalement_ms, releve: lot, collisions, pages_mesurees: lot.length });
        log(`tour ${tour} : ${lot.length} pages, etalement ${etalement_ms} ms, `
            + `marqueurs ${JSON.stringify(lot.map((r) => r.marqueur))}, `
            + `collisions ${collisions.length ? JSON.stringify(collisions) : 'AUCUNE'}`);
        await dodo(5000);
    }
} finally {
    releve.loadavg_fin = loadavg();
    releve.virsh_fin = virshEtat();
    await writeFile(SORTIE, JSON.stringify(releve, null, 1));
    log('relevé écrit dans', SORTIE);
    chrome.kill('SIGKILL');
    await dodo(500);
    await rm(dir, { recursive: true, force: true }).catch(() => { });
    // ⚠️ Le journal se copie APRÈS la fin réelle de l'exécution : les enfants
    // meurent quand le navigateur se ferme, donc APRÈS le pilote (D4).
    await dodo(8000);
    log('arrêt de l’agent :', tuerAgent().replace(/\s+/g, ' ').trim().slice(-60));
    copierLog(ETIQUETTE);
    log('fenêtres VM restantes :',
        winrm('$c=@(Get-Process chrome -ErrorAction SilentlyContinue).Count; Write-Output "chrome=$c"')
            .replace(/\s+/g, ' ').trim().slice(-30));
}
