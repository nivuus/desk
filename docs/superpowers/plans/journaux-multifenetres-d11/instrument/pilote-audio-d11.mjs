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

import { spawnSync } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from './commun-d11.mjs';

function racineDepot() {
    const r = spawnSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' });
    if (r.status !== 0) {
        throw new Error(`hors du depot git : impossible de deriver RACINE (git rev-parse a echoue : ${(r.stderr ?? '').trim()})`);
    }
    return r.stdout.trim();
}

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const PROFIL = Number(arg('profil', '1'));
// `--url` accepte PLUSIEURS pages séparées par des virgules : les recettes
// ② et ③ mesurent deux sessions, et leurs relevés doivent tomber dans la MÊME
// fenêtre de temps.
const URLS = arg('url', 'http://127.0.0.1:5173/?session=demo').split(',').filter(Boolean);
// Les fréquences ASSIGNÉES aux fenêtres, dont le niveau est relevé en plus de
// la dominante — c'est ce qui rend « la voisine se tait » mesurable.
const CIBLES = arg('hz', '440,660').split(',').map(Number).filter((n) => Number.isFinite(n));
// `--shell` : la page-shell des recettes ② et ③, ouverte AVANT les pages
// d'application (elle seule reçoit les annonces `fenetre-ouverte`, et le
// signaling ne les mémorise pas — une annonce émise avant qu'elle ne soit
// connectée est perdue sans trace, piège de D1).
const URL_SHELL = arg('shell', '');
// `--superviseur=1` : le pilote lance lui-même l'agent en mode multi-fenêtres,
// APRÈS avoir ouvert la page-shell et AVANT d'ouvrir les fenêtres de la VM.
//
// ⚠️ CET ORDRE N'EST PAS NÉGOCIABLE, et il est celui de D10. Le signaling ne
// mémorise que les offres SDP : une annonce `fenetre-ouverte` émise avant que
// la page-shell ne soit connectée est PERDUE SANS TRACE (D1). Et depuis D3, une
// entrée en attente de viewport plus de `DELAI_ATTENTE_VIEWPORT_MAX` = 30 s est
// ABANDONNÉE, sans jamais être reproposée.
//
// ⚠️ Le pilote fait la séquence VM LUI-MÊME plutôt que de la laisser à
// l'opérateur : une commande backgroundée par le harnais ne survit pas à la fin
// du tour de l'agent qui l'a lancée, et D10 y a perdu deux exécutions.
const SUPERVISEUR = arg('superviseur', '') === '1';
// `--fenetres=440:C:\dev\p,660:C:\dev\p` — une par fenêtre à ouvrir sur la
// VM, `<hz>:<dossier de profil chrome>`. MÊME dossier = un seul `chrome.exe`,
// donc un seul groupe de PID : c'est CE QUI FAIT le critère des recettes ② et
// ③. Deux `notepad.exe` seraient deux PID distincts et le contrôle ne pourrait
// alors PAS échouer (piège de D8).
const FENETRES = arg('fenetres', '').split(',').filter(Boolean)
    // Découpe sur le PREMIER `:` seulement : un dossier de profil Windows en
    // contient un (`C:\dev\…`), et un `split(':')` nu rendait `profil = 'C'`
    // — la fenêtre ne s'ouvrait pas, et RIEN ne le disait sinon l'absence
    // d'annonce `fenetre-ouverte`. Trouvé à la première exécution de la
    // recette ②.
    .map((x) => { const i = x.indexOf(':'); return { hz: x.slice(0, i), profil: x.slice(i + 1) }; });
const AIDE_VM = new URL('./recette-audio-d11.sh', import.meta.url).pathname;
const POINTS = (() => {
    const v = arg('points', '');
    return v ? v.split(',').map(Number).filter((n) => Number.isFinite(n)) : null;
})();
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
//
// ⚠️ QUATRE DÉFAUTS D'INSTRUMENT trouvés PAR L'EXÉCUTION du bras ROUGE de la
// recette ① (19 août 2026, tâche 9), et corrigés ici. Aucun n'était visible à
// la relecture ; les quatre l'ont été à la première mesure réelle :
//
//   1. `ctx.resume()` N'ÉTAIT PAS APPELÉ et `ctx.state` n'était pas vérifié.
//      Un `AudioContext` naît `suspended` sans activation utilisateur : un
//      contexte suspendu rend `-Infinity` sur tous les bins, EXACTEMENT comme
//      un silence réel. Le contrôle ne pouvait donc pas distinguer « la
//      session ne produit aucun son » de « mon instrument n'écoute pas ». La
//      doctrine était pourtant écrite depuis D7 et appliquée par le pilote de
//      D10 (`pilote-critere2-d10.mjs`) ; elle avait été perdue à la réécriture.
//   2. AUCUN REPLI sur `<video>.srcObject` : `window.__pc` dépend d'une
//      AMORCE qui court contre le démarrage du document, et cinq cibles sur
//      huit ont rendu « aucune RTCPeerConnection » au bras rouge.
//   3. `-Infinity` se sérialise en `null` dans le JSON du relevé : le niveau
//      DISPARAISSAIT du journal versé. Une SENTINELLE numérique le conserve.
//   4. Les niveaux AUX FRÉQUENCES ASSIGNÉES n'étaient pas relevés — or c'est
//      la seule façon de juger la voisine des recettes ② et ③ : « aucune
//      dominante au-dessus du plancher » se lit sur ces niveaux-là.
const SENTINELLE_DB = -1000;
const exprSpectre = (ciblesHz) => `(async () => {
  const CIBLES = ${JSON.stringify(ciblesHz)};
  const SENTINELLE = ${SENTINELLE_DB};
  const pc = window.__pc;
  let piste = pc ? pc.getReceivers().map(r => r.track).find(t => t && t.kind === 'audio') : null;
  let voie = piste ? 'pc' : null;
  if (!piste) {
    const v = document.querySelector('video');
    piste = v && v.srcObject && v.srcObject.getAudioTracks ? v.srcObject.getAudioTracks()[0] : null;
    if (piste) voie = 'video.srcObject';
  }
  if (!piste) return { erreur: pc ? 'aucune piste audio' : 'aucune PeerConnection exposee', voie: null };
  const ctx = new AudioContext();
  try {
    await ctx.resume();
    if (ctx.state !== 'running') return { erreur: 'AudioContext non demarre etat=' + ctx.state, voie };
    const an = ctx.createAnalyser();
    an.fftSize = 8192;
    ctx.createMediaStreamSource(new MediaStream([piste])).connect(an);
    await new Promise(res => setTimeout(res, 1500));
    const d = new Float32Array(an.frequencyBinCount);
    an.getFloatFrequencyData(d);
    const db = (x) => (Number.isFinite(x) ? Number(x.toFixed(1)) : SENTINELLE);
    const binDe = (f) => Math.max(0, Math.min(d.length - 1, Math.round(f * an.fftSize / ctx.sampleRate)));
    let iMax = 0;
    for (let i = 1; i < d.length; i += 1) if (d[i] > d[iMax]) iMax = i;
    const finis = [...d].filter(Number.isFinite);
    const parBin = ctx.sampleRate / an.fftSize;
    let stats = null;
    if (pc) {
      try {
        const rap = await pc.getStats();
        const e = [...rap.values()].find(x => x.type === 'inbound-rtp' && x.kind === 'audio');
        if (e) stats = { bytes_recus: e.bytesReceived ?? null, paquets_recus: e.packetsReceived ?? null };
      } catch (_) { }
    }
    return {
      voie,
      etat_ctx: ctx.state,
      dominante_hz: Math.round(iMax * parBin),
      niveau_db: db(d[iMax]),
      plancher_db: finis.length ? Number((finis.reduce((a, b) => a + b, 0) / finis.length).toFixed(1)) : SENTINELLE,
      niveaux: CIBLES.map(f => ({ f, db: db(d[binDe(f)]) })),
      resolution_bin_hz: Number(parBin.toFixed(2)),
      sample_rate: ctx.sampleRate,
      piste_muted: piste.muted,
      piste_ready_state: piste.readyState,
      stats_audio: stats,
    };
  } finally {
    await ctx.close();
  }
})()`;

// Hameçon sur le WebSocket de la page-shell. Il RELÈVE la correspondance
// session ↔ titre de fenêtre au lieu de la SUPPOSER d'un ordre d'ouverture :
// `document.title` de `ton.html` porte la fréquence assignée, et le
// superviseur relaie ce texte comme `titre` dans `fenetre-ouverte`. C'est le
// produit lui-même qui dit quelle session montre quelle tonalité.
//
// ⚠️ Résoudre par rang de nom (`noms[0]`, `noms[1]`) N'EST PAS FIABLE, même
// sur une VM nettoyée : c'est ce qui a fait mesurer le critère ⑤ de D8 sur une
// fenêtre dont on ignorait ce qu'elle jouait.
const AMORCE_SHELL = `
  (() => {
    if (window.__hameconShell) return;
    window.__hameconShell = true;
    window.__fenetres = [];
    const N = window.WebSocket;
    window.WebSocket = function (...a) {
      const w = new N(...a);
      w.addEventListener('message', (e) => {
        try {
          const m = JSON.parse(e.data);
          if (m && m.type === 'fenetre-ouverte') {
            window.__fenetres.push({ t: Date.now(), session: m.session, titre: m.titre });
          }
        } catch (_) { }
      });
      return w;
    };
    window.WebSocket.prototype = N.prototype;
    for (const k of ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED']) window.WebSocket[k] = N[k];
  })();
`;

const dir = await mkdtemp(join(tmpdir(), `audio-d11-p${PROFIL}-`));
// ⚠️ Profils ② et ③ : un SEUL `--user-data-dir`, donc un seul `chrome.exe`.
const port = 9300 + PROFIL;
const chrome = lancerChrome(port, dir, ['--autoplay-policy=no-user-gesture-required']);
const releves = [];
const assignations = [];
try {
    const cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
    const pages = new Map();
    cdp.on(async (m) => {
        if (m.method === 'Target.targetInfoChanged') {
            const p = pages.get(m.params.targetInfo.targetId);
            if (p) p.url = m.params.targetInfo.url;
            return;
        }
        if (m.method !== 'Target.attachedToTarget') return;
        const sid = m.params.sessionId;
        // ⚠️ `type === 'page'` — DÉFAUT N°5 trouvé par l'exécution du bras
        // ROUGE : sans ce filtre, le pilote évalue aussi sur les cibles
        // `worker` / `service_worker`, qui n'ont pas de `window` et rendent
        // `ReferenceError: window is not defined`. Trois relevés sur huit
        // étaient ce bruit-là, indiscernables au premier coup d'œil d'un
        // échec de mesure sur une vraie page.
        if (m.params.targetInfo.type !== 'page') {
            await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
            return;
        }
        pages.set(m.params.targetInfo.targetId, { sid, url: m.params.targetInfo.url });
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
        await cdp.send('Page.enable', {}, sid).catch(() => { });
        const amorce = /shell/.test(m.params.targetInfo.url ?? '') ? AMORCE_SHELL : AMORCE;
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: amorce }, sid).catch(() => { });
        // ⚠️ `addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page déjà
        // ouverte par `window.open` (piège de D5) : on pose l'amorce aussi
        // explicitement, page par page.
        await cdp.send('Runtime.evaluate', { expression: amorce }, sid).catch(() => { });
        await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
        log('+ page attachée', sid.slice(0, 8), m.params.targetInfo.url);
    });
    // `waitForDebuggerOnStart` + `setDiscoverTargets` : la recette de D10,
    // éprouvée sur les pages ouvertes par `window.open` depuis la page-shell.
    // Sans elles, la première exécution de la recette ② n'a attaché AUCUNE page
    // d'application.
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await cdp.send('Target.setDiscoverTargets', { discover: true });
    // La page-shell d'abord, et son amorce posée AVANT que le superviseur
    // n'annonce quoi que ce soit.
    let sidShell = null;
    if (URL_SHELL) {
        await cdp.send('Target.createTarget', { url: URL_SHELL });
        await dodo(3000);
        sidShell = [...pages.values()].find((p) => /shell/.test(p.url ?? ''))?.sid ?? null;
        log('page-shell attachée :', sidShell ? sidShell.slice(0, 8) : 'ABSENTE');
    }
    // ⚠️ LES FENÊTRES D'ABORD, LE SUPERVISEUR ENSUITE — et la page-shell avant
    // les deux. Trouvé à la première exécution de la recette ② : `vm-it.sh`
    // passe par une tâche planifiée dont la console PowerShell est ÉLIGIBLE à
    // la capture, et le superviseur l'a détectée et lui a donné une session
    // (`fenetre-ouverte w-1 powershell.EXE`) au lieu des fenêtres visées.
    // Ouvrir les fenêtres AVANT laisse ces consoles mourir, et le superviseur
    // les trouve alors par `enumerer_existantes` — le chemin que D1 a éprouvé.
    // La page-shell restant connectée d'about en bout, aucune annonce n'est
    // perdue et le garde-fou des 30 s de D3 ne mord pas.
    for (const [i, f] of FENETRES.entries()) {
        log(`>>> ouverture de la fenêtre VM ${i + 1} : ${f.hz} Hz, profil ${f.profil}`);
        const r = spawnSync('bash', [AIDE_VM, 'ouvrir', String(i + 1), f.hz, f.profil],
            { encoding: 'utf8', env: process.env });
        if (r.status !== 0) log('!! ouverture échouée', (r.stderr ?? '').slice(0, 300));
        await dodo(8000);
    }
    if (SUPERVISEUR) {
        log('>>> lancement du superviseur');
        const r = spawnSync('bash', ['-c',
            `cd ${process.env.RACINE ?? racineDepot()} && SUPERVISEUR=1 scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (r.stdout ?? '').trim().split('\n').pop());
        if ((r.stderr ?? '').trim()) log('run-agent.sh STDERR :', r.stderr.trim().slice(0, 300));
        await dodo(8000);
    }
    for (const u of URLS) {
        await cdp.send('Target.createTarget', { url: u });
        await dodo(1500);
    }

    // Points de contrôle spectraux. ⚠️ DIMENSIONNEMENT contre la constante
    // RELEVÉE : `REPORT_INTERVAL = 30 s`, et le fil de capture est RECRÉÉ à la
    // reconstruction — son compteur de période repart donc de zéro. Le verdict
    // exige au moins DEUX lignes `compteurs audio` POSTÉRIEURES à la
    // reconstruction ; à 60 s de session on n'en aurait qu'une, et une seule
    // ne distingue pas un son qui tient d'un son qui reprend puis retombe.
    const points = POINTS ?? [30, Math.max(45, DUREE - 15)];
    let ecoule = 0;
    for (const t of points) {
        await dodo(Math.max(0, (t - ecoule)) * 1000);
        ecoule = t;
        // Les pages sont mesurées EN PARALLÈLE : les recettes ② et ③ exigent
        // que la porteuse et la voisine soient relevées DANS LA MÊME fenêtre
        // de temps, et leurs horodatages sont versés pour qu'on le vérifie.
        const entrees = [...pages.values()].filter((p) => /\?session=/.test(p.url ?? ''));
        const lots = await Promise.all(entrees.map(async (p) => {
            const t0 = new Date().toISOString();
            const s = await cdp.evalBorne(p.sid, exprSpectre(CIBLES), 12000, true);
            return { t_s: t, debut: t0, fin: new Date().toISOString(), url: p.url, spectre: s };
        }));
        for (const r of lots) { releves.push(r); log(`t+${t}s`, r.url, JSON.stringify(r.spectre)); }
        if (sidShell) {
            const f = await cdp.evalBorne(sidShell, 'JSON.stringify(window.__fenetres || [])', 8000, false);
            // Seconde voie, CORROBORANTE et non substituable : la liste du
            // DOM de la page-shell porte les TITRES mais pas les sessions. Elle
            // dit que les deux fenêtres ont bien été annoncées ; elle NE dit
            // PAS laquelle est quelle session — résoudre par rang de nom n'est
            // pas fiable (piège de D8), et l'assignation ci-dessus reste la
            // seule source de la correspondance.
            const listeShell = await cdp.evalBorne(sidShell,
                "document.querySelector('#fenetres') && document.querySelector('#fenetres').textContent", 8000, false);
            const statut = await cdp.evalBorne(sidShell,
                "document.querySelector('#statut') && document.querySelector('#statut').textContent", 8000, false);
            assignations.push({ t_s: t, fenetres: f, liste_shell: listeShell, statut_shell: statut });
            log(`t+${t}s ASSIGNATIONS`, f, '| liste :', JSON.stringify(listeShell), '| statut :', JSON.stringify(statut));
        }
    }
    await dodo(Math.max(0, DUREE - ecoule) * 1000);
} finally {
    await writeFile(SORTIE, JSON.stringify(
        { profil: PROFIL, duree_s: DUREE, url_shell: URL_SHELL, urls: URLS, cibles_hz: CIBLES, assignations, releves },
        null, 2));
    log('releves ecrits dans', SORTIE);
    chrome.kill('SIGKILL');
    await dodo(300);
    await rm(dir, { recursive: true, force: true });
}
// ⚠️ L'ORDRE EST ÉTABLI, PAS SUPPOSÉ : relever l'horodatage de
// `capture audio reconstruite` dans `agent.log` et vérifier que CHAQUE mesure
// spectrale lui est POSTÉRIEURE. Sans cela, on mesurerait la capture d'origine
// et on conclurait que le remède marche alors qu'il n'aurait rien fait.
