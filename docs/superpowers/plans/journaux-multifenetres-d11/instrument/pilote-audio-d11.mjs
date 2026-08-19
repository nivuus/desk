#!/usr/bin/env node
// Sous-bloc D11 — pilote des recettes ①, ② et ③ (le son).
//
//   ① mono-fenêtre : une capture audio tuée par injection est reconstruite ET
//     redevient AUDIBLE (leg 4 de D10).
//   ② deux fenêtres d'un MÊME groupe de PID : la porteuse reconstruite parle,
//     la voisine se tait, dans la MÊME mesure. C'est la discrimination que D10
//     n'a pas pu faire — à une seule fenêtre, `audio_porteuse` vaut toujours
//     `true`, et le correctif livré est indiscernable du `set_actif(true)`
//     inconditionnel que le code déclare PIRE.
//   ③ le repli sur la promotion d'une voisine, rendu atteignable par
//     `AUDIO_FAUTE_RECONSTRUCTION` (leg 5 de D10).
//
// Les invariants de montage sont dans `commun-d11.mjs` — les relire.
//
// ⚠️ LE VERDICT SE JUGE À LA FRÉQUENCE DOMINANTE, JAMAIS AU COMPTE D'OCTETS.
// D7 a relevé `bytesReceived` en croissance sur un spectre à −1000 dB, et le
// seuil de D10 comparait au PLANCHER DE BRUIT (−158 dB) : il ne pouvait
// quasiment pas échouer. Ici la dominante reçue est comparée à la fréquence
// ASSIGNÉE à la fenêtre, et l'écart au plancher est rapporté pour lecture.
//
// ⚠️ `--user-data-dir` PARTAGÉ pour ② et ③ (un seul `chrome.exe`, donc un seul
// groupe de PID) ; UN PAR FENÊTRE pour ①. Deux `notepad.exe` seraient deux PID
// distincts et le contrôle ② ne pourrait alors PAS échouer (piège de D8).
//
// Usage :
//   node pilote-audio-d11.mjs --profil=1 --url=<page-shell> --duree=120
//   node pilote-audio-d11.mjs --profil=2 --url=<page-shell> --duree=120
//   node pilote-audio-d11.mjs --profil=3 --url=<page-shell> --duree=120

import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from './commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const PROFIL = Number(arg('profil', '1'));
const URL_SHELL = arg('url', 'http://127.0.0.1:5173/shell.html?session=demo');
const DUREE = Number(arg('duree', '120'));
const SORTIE = arg('sortie', `audio-profil-${PROFIL}.json`);
const log = (...a) => console.log(new Date().toISOString(), ...a);

// L'amorce capture la `RTCPeerConnection` de la page — même mécanique que D10.
const AMORCE = `
  window.__pc = null;
  const N = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;
`;

// Spectre de la piste audio REÇUE. Rend la dominante, son niveau, et le
// plancher — les trois, pour que le lecteur puisse juger l'écart lui-même
// plutôt que de faire confiance à un seuil.
const EXPR_SPECTRE = `(async () => {
  const pc = window.__pc;
  if (!pc) return { erreur: 'aucune RTCPeerConnection' };
  const r = pc.getReceivers().find(x => x.track && x.track.kind === 'audio');
  if (!r) return { erreur: 'aucun receveur audio' };
  const ctx = new AudioContext();
  const src = ctx.createMediaStreamSource(new MediaStream([r.track]));
  const an = ctx.createAnalyser();
  an.fftSize = 8192;
  src.connect(an);
  await new Promise(res => setTimeout(res, 700));
  const d = new Float32Array(an.frequencyBinCount);
  an.getFloatFrequencyData(d);
  let iMax = 0;
  for (let i = 1; i < d.length; i += 1) if (d[i] > d[iMax]) iMax = i;
  const parBin = ctx.sampleRate / an.fftSize;
  let somme = 0, n = 0;
  for (let i = 0; i < d.length; i += 1) if (Number.isFinite(d[i])) { somme += d[i]; n += 1; }
  const res = {
    dominante_hz: Math.round(iMax * parBin),
    niveau_db: Number(d[iMax].toFixed(1)),
    plancher_db: n ? Number((somme / n).toFixed(1)) : null,
    resolution_bin_hz: Number(parBin.toFixed(2)),
    sample_rate: ctx.sampleRate,
  };
  await ctx.close();
  return res;
})()`;

const dir = await mkdtemp(join(tmpdir(), `audio-d11-p${PROFIL}-`));
// ⚠️ Profils ② et ③ : un SEUL `--user-data-dir`, donc un seul `chrome.exe`.
const port = 9300 + PROFIL;
const chrome = lancerChrome(port, dir, ['--autoplay-policy=no-user-gesture-required']);
const releves = [];
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
        // ⚠️ `addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page déjà
        // ouverte par `window.open` (piège de D5) : on pose l'amorce aussi
        // explicitement, page par page.
        await cdp.send('Runtime.evaluate', { expression: AMORCE }, sid).catch(() => { });
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
    await cdp.send('Target.createTarget', { url: URL_SHELL });

    // Points de contrôle spectraux. ⚠️ DIMENSIONNEMENT contre la constante
    // RELEVÉE : `REPORT_INTERVAL = 30 s`, et le fil de capture est RECRÉÉ à la
    // reconstruction — son compteur de période repart donc de zéro. Le verdict
    // exige au moins DEUX lignes `compteurs audio` POSTÉRIEURES à la
    // reconstruction ; à 60 s de session on n'en aurait qu'une, et une seule
    // ne distingue pas un son qui tient d'un son qui reprend puis retombe.
    const points = [30, Math.max(45, DUREE - 15)];
    let ecoule = 0;
    for (const t of points) {
        await dodo(Math.max(0, (t - ecoule)) * 1000);
        ecoule = t;
        for (const [sid, url] of pages) {
            if (!/shell/.test(url ?? '')) {
                const s = await cdp.evalBorne(sid, EXPR_SPECTRE, 9000, true);
                releves.push({ t_s: t, sessionId: sid, url, spectre: s });
                log(`t+${t}s`, sid.slice(0, 8), JSON.stringify(s));
            }
        }
    }
    await dodo(Math.max(0, DUREE - ecoule) * 1000);
} finally {
    await writeFile(SORTIE, JSON.stringify({ profil: PROFIL, duree_s: DUREE, releves }, null, 2));
    log('releves ecrits dans', SORTIE);
    chrome.kill('SIGKILL');
    await dodo(300);
    await rm(dir, { recursive: true, force: true });
}
// ⚠️ L'ORDRE EST ÉTABLI, PAS SUPPOSÉ : relever l'horodatage de
// `capture audio reconstruite` dans `agent.log` et vérifier que CHAQUE mesure
// spectrale lui est POSTÉRIEURE. Sans cela, on mesurerait la capture d'origine
// et on conclurait que le remède marche alors qu'il n'aurait rien fait.
