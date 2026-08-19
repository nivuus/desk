#!/usr/bin/env node
// Sous-bloc D11 — pilote de la recette ⑤ : LE COÛT DE LA DUPLICATION D'UNE
// SORTIE SURDIMENSIONNÉE (leg 7 de D10).
//
// Depuis D10, le superviseur TOLÈRE une sortie née trop grande (le registre
// laissé sale) et RECADRE le rectangle utile dans la duplication. C'est ce qui
// a fait passer le produit de 3 à 10 fenêtres. Le prix de cette voie —
// dupliquer du 3840×2160 pour n'en recadrer que 1280×720 — n'est mesuré PAR
// RIEN, et D10 le déclare tel quel.
//
// CE QUE CE PILOTE MESURE, ET CE QU'IL NE MESURE PAS. Il relève, côté
// NAVIGATEUR, la cadence décodée et les images jetées par fenêtre ; le coût
// côté AGENT (temps de duplication, de recadrage, de copie GPU) se lit dans
// `agent.log`, pas ici. ⚠️ Les deux bras à opposer sont « sortie née à la
// taille demandée » et « sortie née surdimensionnée » — la variable est l'état
// du REGISTRE, que l'on pose avec la sonde `MULTIFENETRE_MODE_SORTIE=<LxH>`,
// DANS UN LANCEMENT À ELLE SEULE (l'aiguillage de `diagnostics/multifenetre.rs`
// retourne après la première sonde reconnue).
//
// ⚠️ AUCUN TAUX ne sera revendiqué : deux exécutions par bras au mieux, et
// chaque énoncé porte son nombre d'exécutions.
//
// Les invariants de montage sont dans `commun-d11.mjs` — les relire.
//
// Usage : node pilote-cout-d11.mjs --url=<page-shell> --bras=surdimensionnee --palier=60

import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from './commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const URL_SHELL = arg('url', 'http://127.0.0.1:5173/shell.html?session=demo');
const BRAS = arg('bras', 'surdimensionnee');
const PALIER = Number(arg('palier', '60'));
const SORTIE = arg('sortie', `cout-${BRAS}.json`);
const log = (...a) => console.log(new Date().toISOString(), ...a);

const AMORCE = `
  window.__pc = null;
  const N = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;
`;

// `framesDecoded` et `framesDropped` côté receveur, plus la taille réellement
// reçue : c'est le barreau de l'échelle qui dit ce que la fenêtre a coûté.
const EXPR_STATS = `(async () => {
  const pc = window.__pc;
  if (!pc) return { erreur: 'aucune RTCPeerConnection' };
  const t = [...(await pc.getStats()).values()];
  const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
  if (!v) return { erreur: 'aucun inbound-rtp video' };
  return {
    framesDecoded: v.framesDecoded, framesDropped: v.framesDropped,
    framesReceived: v.framesReceived, packetsLost: v.packetsLost,
    bytesReceived: v.bytesReceived,
    frameWidth: v.frameWidth, frameHeight: v.frameHeight,
    totalDecodeTime: v.totalDecodeTime, totalInterFrameDelay: v.totalInterFrameDelay,
    horodatage_ms: Date.now(),
  };
})()`;

const dir = await mkdtemp(join(tmpdir(), `cout-d11-${BRAS}-`));
const port = 9450;
const chrome = lancerChrome(port, dir);
const echantillons = [];
try {
    const cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
    const pages = new Map();
    cdp.on(async (m) => {
        if (m.method !== 'Target.attachedToTarget') return;
        const sid = m.params.sessionId;
        pages.set(sid, m.params.targetInfo.url);
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
        await cdp.send('Page.enable', {}, sid).catch(() => { });
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: AMORCE }, sid).catch(() => { });
        await cdp.send('Runtime.evaluate', { expression: AMORCE }, sid).catch(() => { });
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
    await cdp.send('Target.createTarget', { url: URL_SHELL });
    await dodo(20000);

    // Deux bornes encadrant le palier : la différence donne la cadence, et
    // `framesDropped` sur la même fenêtre donne le taux d'images jetées.
    for (const borne of ['debut', 'fin']) {
        if (borne === 'fin') await dodo(PALIER * 1000);
        for (const [sid, url] of pages) {
            if (/shell/.test(url ?? '')) continue;
            const s = await cdp.evalBorne(sid, EXPR_STATS, 9000, true);
            echantillons.push({ borne, sessionId: sid, url, ...s });
        }
        log(`borne ${borne} : ${echantillons.filter(e => e.borne === borne).length} pages`);
    }
} finally {
    await writeFile(SORTIE, JSON.stringify({ bras: BRAS, palier_s: PALIER, echantillons }, null, 2));
    log('releves ecrits dans', SORTIE);
    chrome.kill('SIGKILL');
    await dodo(300);
    await rm(dir, { recursive: true, force: true });
}
