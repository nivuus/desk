#!/usr/bin/env node
// Sous-bloc D9, tâche 14 — pilote de la recette ① : la chaîne `Resize` (rejeu
// + unité HiDPI) et la non-régression de la détection plein écran.
//
// Dérivé de `docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs`
// (Cdp, vmIt, purgerFenetresVMRescapees, ouvrirFenetre, togglerStyleFenetre,
// resoudreIdentite, journalPlat/sessionDeLigne/pleinEcranAgent, attendreLeFait —
// repris quasi tels quels, cette machinerie a déjà été éprouvée par D8). RETIRÉ :
// tout ce qui concernait le changement de mode de sortie (retiré au commit
// 4e03e34, tâche 3 de ce sous-bloc) et le critère audio ⑤ (D8), hors périmètre
// de la tâche 14. AJOUTÉ : l'assignation d'un `deviceScaleFactor` distinct par
// fenêtre, l'interception de `window.opener.postMessage` (l'annonce de
// viewport) et de `console.warn` (la trace du rejeu), et le forçage d'un
// changement de viewport EN COURS DE SESSION.
//
// ============================================================================
// CE QUE CE PILOTE MESURE, ET COMMENT
// ============================================================================
//
// (a) « un Resize émis alors que le canal n'était pas ouvert est rejoué à son
//     ouverture » — observé SANS le forcer : le `<video>` est en
//     `width:100vw;height:100vh` (`client/src/style.css`), donc le
//     `ResizeObserver` se déclenche dès la mise en page de la page, avant même
//     que la connexion WebRTC ait eu la moindre chance d'ouvrir le canal de
//     contrôle (négociation SDP par le signaling, ICE, DTLS, SCTP). La
//     déférence est donc un événement NATUREL de toute ouverture de fenêtre, pas
//     un cas qu'il faut fabriquer. `console.warn` est intercepté pour capturer
//     `"Resize différé : canal de contrôle non ouvert"` (client/src/main.ts),
//     et `agent.log` est grepé pour la ligne `contrôle reçu` correspondante,
//     arrivée plus tard : si les deux existent, avec la même taille, c'est le
//     rejeu. Si le canal était déjà ouvert au premier tir (réseau local très
//     rapide), la déférence ne se produit simplement pas cette fois-ci — ce
//     pilote le CONSTATE, il ne le suppose pas.
// (b) « chaque contrôle reçu Resize porte son champ session » — lu directement
//     dans `agent.log`, sur toutes les fenêtres.
// (c) « à deviceScaleFactor=2, l'annonce de viewport et le Resize sont dans la
//     même unité » — la fenêtre CIBLE (la première ouverte) reçoit
//     `deviceScaleFactor: 2`, les VOISINES restent à 1. L'annonce de viewport
//     est interceptée côté page (override de `window.opener.postMessage`) ;
//     le Resize est lu dans `agent.log`. Un changement de viewport EN COURS DE
//     SESSION (`Emulation.setDeviceMetricsOverride` sur la cible, après
//     connexion) fournit une seconde paire de valeurs, sur le mécanisme
//     "steady state" plutôt que sur le tout premier tir.
// (d) « la détection du plein écran annonce toujours à la bonne fenêtre et à
//     elle seule » — non-régression de D8 : bascule de style Windows
//     (`togglerStyleFenetre`, retrait RÉEL de `WS_CAPTION`/`WS_THICKFRAME`) sur
//     chacune des trois fenêtres, tour à tour, en vérifiant qu'exactement UNE
//     session distincte annonce `actif=true` dans la fenêtre temporelle de
//     chaque bascule.
//
// Ce pilote NE MESURE PAS le changement de mode de sortie (retiré, tâche 3) ni
// l'audio (hors périmètre de la tâche 14, voir la tâche 15).
//
// ============================================================================
// GARDE-FOUS REPRIS DE D8 (non renumérotés, mêmes raisons)
// ============================================================================
//   - `--user-data-dir` PAR FENÊTRE (`ouvrirFenetre`).
//   - Aucune capture d'écran CDP pendant une mesure ; toute évaluation CDP sur
//     une page WebRTC active est BORNÉE (`Cdp.evalBorne`).
//   - `Get-Process agent` vérifié après CHAQUE tentative, y compris échouée
//     (fait par l'appelant shell, pas ce fichier — voir le rapport de tâche).
//   - Survie de la VM contrôlée après chaque rang (`vmVivante`).
//   - `agent.log` copié APRÈS la fin réelle de l'exécution, pas à la fin du
//     pilote (leçon D4).
//   - Le navigateur PILOTE tourne sur l'HÔTE, jamais sur la VM.
//
// ============================================================================
// PARAMÈTRES (permettent de rejouer ce même pilote pour le contrôle ROUGE)
// ============================================================================
//   PORT_SHELL   — port du serveur vite servant shell.html (5173 = HEAD/vert,
//                  5174 = commit 4e03e34/rouge, voir le rapport de tâche).
//   N_FENETRES   — nombre de fenêtres (3 en vert, pour éprouver (d) ; 1 en
//                  rouge, où seule la chaîne Resize/HiDPI est mesurée).
//   PHASE_EXCLUSIVITE — '1' (défaut) pour jouer la phase (d) ; '0' pour la
//                  sauter (inutile à N_FENETRES=1).

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
// Réutilisé en lecture seule depuis son propre répertoire (D7) : un ASSET
// HTML animé (Desktop Duplication n'émet qu'au changement du bureau), requis
// pour que la fenêtre Windows source produise des trames.
const TON_HTML_D7 = join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d7/instrument/ton.html');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const PORT_SHELL = process.env.PORT_SHELL ?? '5173';
const URL_SHELL = `http://${HOTE}:${PORT_SHELL}/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const COPIE_LOG = process.env.COPIE_LOG ?? `/tmp/agent-critere-1-${ETIQUETTE}.log`;
const SORTIE_JSON = process.env.SORTIE_JSON
    ?? join(RACINE, `docs/superpowers/plans/journaux-multifenetres-d9/critere-1-${ETIQUETTE}.json`);
const VIEWPORT_BASE = process.env.VIEWPORT_BASE ?? '1280x720';
const VIEWPORT_MID_SESSION = process.env.VIEWPORT_MID_SESSION ?? '1000x600';
const BITRATE = process.env.BITRATE ?? '8000000';
const N_FENETRES = Number(process.env.N_FENETRES ?? 3);
const PHASE_EXCLUSIVITE = process.env.PHASE_EXCLUSIVITE ?? '1';
const DPR_CIBLE = Number(process.env.DPR_CIBLE ?? 2);
// Sur ce réseau local, le canal de contrôle s'est ouvert AVANT le premier tir
// du `ResizeObserver` aux deux exécutions jouées sans cette option : la
// déférence (leg 10/tâche 5) ne s'est donc jamais produite naturellement. Ceci
// force la course, sans rien changer côté produit : `Network.
// emulateNetworkConditions` (latence ajoutée) retarde tout ce qui passe par
// la pile réseau de la page — CHARGEMENT COMPRIS — alors que le premier tir
// du `ResizeObserver` ne dépend QUE d'une mise en page CSS (`#remote` en
// 100vw/100vh), synchrone et sans réseau. Le canal de contrôle, lui, exige EN
// PLUS l'ouverture d'un WebSocket vers le signaling puis toute la
// négociation SDP/ICE/DTLS/SCTP — plusieurs allers-retours de plus, sur la
// MÊME latence. L'écart entre les deux se creuse artificiellement au lieu de
// se refermer.
const FORCER_DEFERT = process.env.FORCER_DEFERT === '1';
const LATENCE_FORCEE_MS = Number(process.env.LATENCE_FORCEE_MS ?? 3000);
// Sur le binaire ROUGE (avant la tâche 5), l'annonce initiale de viewport
// n'applique PAS le dpr (`viewportPair(window.innerWidth, window.innerHeight)`
// nu, c'est précisément le défaut à révéler) : diviser les pixels CSS par le
// dpr y ferait annoncer VIEWPORT_BASE/dpr (ex. 640×360), une taille SANS
// historique sur le slot déjà aligné à VIEWPORT_BASE par les essais
// précédents — collision de registre garantie. Sur ROUGE, les pixels CSS
// envoyés à `setDeviceMetricsOverride` restent VIEWPORT_BASE tel quel : le
// vieux code annonce alors VIEWPORT_BASE nu (sans dpr, c'est le défaut), la
// création réussit (elle correspond à l'historique du slot), et le PLUS TARD
// `Resize` (déjà multiplié par le dpr côté client, ce bout-là n'a pas changé)
// porte VIEWPORT_BASE×dpr — les deux unités enfin observables côte à côte.
const DIVISER_CSS_PAR_DPR = process.env.DIVISER_CSS_PAR_DPR !== '0';

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(7);
    console.log(`[${dt}s ${new Date().toISOString()}] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));
const maintenantIso = () => new Date().toISOString();

// ---------------------------------------------------------------- VM (session interactive)
function vmIt(nom, ps) {
    const userName = process.env.WINDOWS_ADMIN_USERNAME ?? 'Administrateur';
    const password = process.env.WINDOWS_ADMIN_PASSWORD ?? '';
    spawnSync('bash', ['-c', `cat > /media/vm/dev/it-${nom}.ps1`], { input: ps, encoding: 'utf8' });
    const commande = [
        `schtasks /delete /tn it-${nom} /f 2>$null;`,
        `schtasks /create /tn it-${nom} /f /it /ru '${userName}' /rp '${password}'`,
        `/sc once /st 00:00 /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\it-${nom}.ps1';`,
        `schtasks /run /tn it-${nom}`,
    ].join(' ');
    const r = spawnSync('node', [join(RACINE, 'scripts/winrm.js'), commande], { encoding: 'utf8', env: process.env });
    if (r.status !== 0) log(`!! vm-it ${nom} a échoué`, (r.stderr ?? '').slice(0, 400));
    return r;
}
function vmVivante(etiquette) {
    const virsh = spawnSync('virsh', ['domstate', 'Windows'], { encoding: 'utf8' }).stdout?.trim() ?? '?';
    const acces = spawnSync('bash', ['-c', 'ls /media/vm/dev/ton.html >/dev/null 2>&1 && echo OUI || echo NON'],
        { encoding: 'utf8' }).stdout?.trim() ?? '?';
    log(`SURVIE VM (${etiquette}) virsh="${virsh}" acces_partage=${acces}`);
    return { virsh, acces };
}
function copierLog() {
    spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
}

// Repris de D8 (revue tâche 11, Critique 2, remède B2) : tuer tout chrome.exe
// et vérifier qu'aucun agent ne survit AVANT de lancer le superviseur, pour ne
// pas fausser l'attribution cible/voisine avec une fenêtre ou une session
// rescapée d'une tentative précédente.
async function purgerFenetresVMRescapees() {
    const script = [
        'Get-Process chrome -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue',
        'Start-Sleep -Seconds 2',
        '$n = (Get-Process chrome -ErrorAction SilentlyContinue | Measure-Object).Count',
        '$agents = (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count',
        '@{ chrome_restants = $n; agents_restants = $agents } | ConvertTo-Json -Compress | Out-File -Encoding ascii C:\\dev\\purge-rescapees.json',
    ].join('\n');
    vmIt('purge-rescapees', script);
    await dodo(5000);
    let resultat = null;
    try {
        resultat = JSON.parse(await readFile('/media/vm/dev/purge-rescapees.json', 'utf8'));
    } catch { }
    log('PURGE FENÊTRES/AGENTS RESCAPÉS (avant lancement du superviseur) ' + JSON.stringify(resultat));
    if (!resultat || resultat.chrome_restants !== 0 || resultat.agents_restants !== 0) {
        throw new Error(
            `état VM non propre avant lancement : ${JSON.stringify(resultat)} — ` +
            'une fenêtre ou un agent rescapé fausserait l\'attribution cible/voisine');
    }
}
async function journalPlat() {
    copierLog();
    let texte = '';
    try { texte = await readFile(COPIE_LOG, 'utf8'); } catch { }
    return texte.replace(/\x1b\[[0-9;]*m/g, '');
}
const horodate = (l) => l.match(/(\d{4}-\d\d-\d\dT[\d:.]+Z)/)?.[1];
const sessionDeLigne = (l) => l.match(/fenetre\{session=([^}]+)\}/)?.[1]
    ?? l.match(/\bsession=(\S+)/)?.[1] ?? null;
const champ = (l, nom) => l.match(new RegExp(`\\b${nom}=(\\S+)`))?.[1] ?? null;

// ---------------------------------------------------------------- parseurs de journal
async function pleinEcranAgent(debutIso, finIso) {
    const plat = await journalPlat();
    const out = [];
    for (const l of plat.split('\n')) {
        if (!l.includes('plein ecran de la fenetre Windows')) continue;
        const t = horodate(l);
        if (!t || (debutIso && t < debutIso) || (finIso && t > finIso)) continue;
        out.push({ t, session: sessionDeLigne(l), actif: champ(l, 'actif') === 'true' });
    }
    return out;
}

/// NEUF (tâche 14). Les lignes `contrôle reçu … Resize { … }`
/// (`agent/src/demarrage.rs`), bornées à une fenêtre temporelle. `session` est
/// LU sur la ligne (posé par la tâche 5) — c'est exactement le point (b) du
/// brief : si une ligne Resize n'en portait pas, `session` vaudrait `null` ici,
/// et le rapport le dirait.
///
/// SIXIÈME BUG TROUVÉ EN COURS D'EXÉCUTION : une PREMIÈRE version de ce
/// parseur cherchait la sous-chaîne `message=Resize` — supposition erronée
/// sur le format `tracing`. Le champ posé par `?message` dans
/// `tracing::info!(session = %session_id, ?message, "contrôle reçu")` est
/// nommé `message`, et `tracing` traite spécialement tout champ de ce nom :
/// il est imprimé SANS clé, directement après le message littéral de
/// l'événement — d'où la ligne RÉELLE observée dans `agent.log` :
/// `contrôle reçu session=w-3 Resize { version: 3, width: 1280, height: 720 }`,
/// sans aucun `message=`. Cette erreur a fait manquer TOUTES les lignes
/// Resize d'une première exécution complète (`critere-1-1.log`, avant
/// correction) alors qu'elles étaient bien présentes dans `agent.log` — versé
/// tel quel dans le rapport de tâche, avec cette correction.
async function resizeAgent(debutIso, finIso) {
    const plat = await journalPlat();
    const out = [];
    for (const l of plat.split('\n')) {
        if (!l.includes('contrôle reçu') || !l.includes('Resize {')) continue;
        const t = horodate(l);
        if (!t || (debutIso && t < debutIso) || (finIso && t > finIso)) continue;
        const width = l.match(/width:\s*(\d+)/)?.[1];
        const height = l.match(/height:\s*(\d+)/)?.[1];
        out.push({
            t, session: champ(l, 'session'),
            largeur: width ? Number(width) : null, hauteur: height ? Number(height) : null,
            ligne: l.trim(),
        });
    }
    return out;
}

// ---------------------------------------------------------------- CDP (repris de D8/D6)
class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.handlers = [];
        this.ready = new Promise((res) => this.ws.addEventListener('open', () => res(), { once: true }));
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.pending.has(m.id)) {
                const { resolve, reject } = this.pending.get(m.id);
                this.pending.delete(m.id);
                if (m.error) reject(new Error(JSON.stringify(m.error)));
                else resolve(m.result);
            } else if (m.method) { for (const h of this.handlers) h(m); }
        });
    }
    on(h) { this.handlers.push(h); }
    async send(method, params = {}, sessionId) {
        await this.ready;
        const id = this.nextId++;
        const msg = { id, method, params };
        if (sessionId) msg.sessionId = sessionId;
        return new Promise((resolve, reject) => {
            this.pending.set(id, { resolve, reject });
            this.ws.send(JSON.stringify(msg));
        });
    }
    async eval(sessionId, expression, awaitPromise = false) {
        const r = await this.send('Runtime.evaluate',
            { expression, awaitPromise, returnByValue: true }, sessionId);
        if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 600));
        return r.result.value;
    }
    async evalBorne(sessionId, expression, ms = 8000, awaitPromise = true) {
        return Promise.race([
            this.eval(sessionId, expression, awaitPromise),
            new Promise((r) => setTimeout(() => r({ __timeout: ms }), ms)),
        ]).catch((e) => ({ __erreur: String(e).slice(0, 200) }));
    }
}
async function attendreDevtools(port) {
    for (let i = 0; i < 80; i += 1) {
        try {
            const r = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (r.ok) return await r.json();
        } catch { }
        await dodo(250);
    }
    throw new Error('devtools timeout');
}
async function attendreLeFait(etiquette, test, maxS) {
    const debut = Date.now();
    while ((Date.now() - debut) / 1000 < maxS) {
        const r = await test();
        if (r) {
            log(`FAIT ATTEINT (${etiquette}) après ${((Date.now() - debut) / 1000).toFixed(1)} s`);
            return r;
        }
        await dodo(2000);
    }
    log(`!! FAIT NON ATTEINT (${etiquette}) après ${maxS} s`);
    return null;
}

// L'AMORCE — interception de trois choses, posée à chaque page AVANT tout
// script du produit (Page.addScriptToEvaluateOnNewDocument + Runtime.evaluate
// immédiat, garde-fou 2 de D8 : ne court pas sur une page ouverte par
// window.open sans être reposée explicitement) :
//   1. `window.opener.postMessage({type:'viewport',...})` — l'annonce initiale
//      du viewport (client/src/main.ts). Interceptée en substituant la
//      fonction elle-même sur `window.opener` (même origine, donc accessible),
//      AVANT que `main.ts` ne s'exécute.
//   2. `console.warn` — la trace du rejeu (leg 10/tâche 5) :
//      "Resize différé : canal de contrôle non ouvert".
//   3. `RTCDataChannel` de label `control` — les messages `fullscreen` reçus,
//      pour la non-régression du critère ① de D8 (point d du brief). Repris
//      tel quel de `pilote-recette-d8.mjs`.
const AMORCE = `
(() => {
  if (window.__amorceD9C1) return;
  window.__amorceD9C1 = true;
  window.__pc = null;
  window.__pleinEcran = [];
  window.__viewportMsg = null;
  window.__consoleWarn = [];

  if (window.opener) {
    try {
      const origPost = window.opener.postMessage.bind(window.opener);
      window.opener.postMessage = function (msg, origin) {
        try { if (msg && msg.type === 'viewport') window.__viewportMsg = Object.assign({}, msg, { t: Date.now() }); } catch {}
        return origPost(msg, origin);
      };
    } catch {}
  }

  try {
    const origWarn = console.warn.bind(console);
    console.warn = function (...a) {
      try { window.__consoleWarn.push({ t: Date.now(), msg: a.map(String).join(' ') }); } catch {}
      return origWarn(...a);
    };
  } catch {}

  const N = window.RTCPeerConnection;
  const creer = N.prototype.createDataChannel;
  N.prototype.createDataChannel = function (label, ...r) {
    const c = creer.call(this, label, ...r);
    if (label === 'control') {
      c.addEventListener('message', (e) => {
        try {
          const m = JSON.parse(e.data);
          if (m && m.type === 'fullscreen') {
            window.__pleinEcran.push({ t: Date.now(), active: m.active });
            if (window.__pleinEcran.length > 50) window.__pleinEcran.shift();
          }
        } catch { }
      });
    }
    return c;
  };
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;

  window.__cachee = false;
  window.__focalisee = true;
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => window.__cachee });
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
  document.hasFocus = () => window.__focalisee;
})();
`;

const pages = new Map();
// Le sid CDP de la DERNIÈRE page "app" attachée, posé par le handler
// `Target.attachedToTarget` de `main`. Sert à `ouvrirFenetre` pour rendre le
// sid de la page qu'il vient d'ouvrir, SANS avoir à le retrouver après coup
// par une correspondance sur `p.url` — voir la note « troisième bug trouvé »
// plus bas : cette correspondance s'est révélée non fiable.
let dernierSidAppPage = null;
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));
const nomDe = (url) => url.replace(/^.*\?session=/, 'w:');
const rang = (n) => Number(String(n).match(/(\d+)\s*$/)?.[1] ?? 0);
const nomsTries = () => appPages().map(([, p]) => nomDe(p.url)).sort((a, b) => rang(a) - rang(b));
const sidDe = (nom) => appPages().find(([, p]) => nomDe(p.url) === nom)?.[0];
// TROIS bugs trouvés en cours d'exécution de la tâche 14 (première à
// troisième tentative), documentés ici parce qu'ils dictent la conception du
// reste de ce fichier :
//   1. une page ouverte par `window.open()` attache avec une URL VIDE — voir
//      la note à l'endroit où `main` décide `estAppPage` ;
//   2. le produit (mécanisme non élucidé côté superviseur, corrélé aux
//      bascules de style Windows pendant la phase d'exclusivité, hors
//      périmètre d'un pilote de MESURE) ouvre des pages EXCÉDENTAIRES pour
//      une fenêtre déjà ouverte ;
//   3. résoudre "la page vivante pour une session" en interrogeant
//      `location.search` sur chaque page "app" attachée (une première version
//      de ce fichier le faisait ici même) a rendu un résultat FAUX : une page
//      dont `location.search` disait `session=w-5` portait un
//      `window.__viewportMsg` capturé sous `session=w-8` — signe qu'une MÊME
//      cible CDP peut être réutilisée pour des sessions SUCCESSIVES sans que
//      rien ne le signale. Aucune correspondance a posteriori sur une page
//      DÉJÀ attachée n'est donc fiable.
//
// Remède retenu pour les trois : ne plus jamais chercher une page après coup.
// `dernierSidAppPage` (posé par le handler d'attachement de `main`, juste
// après avoir traité une page) et le retour de `ouvrirFenetre` donnent le sid
// exact de la page qui vient tout juste de s'attacher, sans recherche ni
// filtrage — voir `phaseResizeEtDpr` et l'ouverture d'une fenêtre DÉDIÉE au
// test Resize/HiDPI, après la phase d'exclusivité, dans `main`.

const STATS = `(async () => {
  const pc = window.__pc;
  if (!pc) return { pc: null };
  const r = await pc.getStats(); const t = [...r.values()];
  const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
  return {
    etat: pc.iceConnectionState, horloge: Date.now(),
    images_decodees: v?.framesDecoded ?? null,
    l: v?.frameWidth ?? null, h: v?.frameHeight ?? null,
    dpr: window.devicePixelRatio,
    inner: window.innerWidth + 'x' + window.innerHeight,
    video_client: (document.querySelector('#remote')?.clientWidth ?? null) + 'x' + (document.querySelector('#remote')?.clientHeight ?? null),
    viewport_msg: window.__viewportMsg,
    console_warn: window.__consoleWarn.slice(),
  };
})()`;

// ---------------------------------------------------------------- Windows : style réel (repris de D8)
function psStyleFenetre(marqueur, action) {
    return [
        'Add-Type @"',
        'using System;',
        'using System.Runtime.InteropServices;',
        'public class D9C1Fenetre {',
        '  [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr GetWindowLongPtrW(IntPtr hWnd, int nIndex);',
        '  [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SetWindowLongPtrW(IntPtr hWnd, int nIndex, IntPtr dwNewLong);',
        '  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);',
        '}',
        '"@',
        '$GWL_STYLE = -16',
        '$WS_CAPTION = 0x00C00000',
        '$WS_THICKFRAME = 0x00040000',
        '$SWP_FLAGS = 0x0037',
        `$procs = Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" | Where-Object { $_.CommandLine -like '*${marqueur}*' }`,
        '$hwnd = [IntPtr]::Zero',
        'foreach ($p in $procs) {',
        '  $proc = Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue',
        '  if ($proc -and $proc.MainWindowHandle -ne [IntPtr]::Zero) { $hwnd = $proc.MainWindowHandle; break }',
        '}',
        'if ($hwnd -eq [IntPtr]::Zero) {',
        `  '{"erreur":"fenetre introuvable pour ${marqueur}"}' | Out-File -Encoding ascii C:\\dev\\${marqueur}-style.json`,
        '  exit',
        '}',
        '$avant = [D9C1Fenetre]::GetWindowLongPtrW($hwnd, $GWL_STYLE).ToInt64()',
        action === 'lire'
            ? '$nouveau = $avant'
            : action === 'sansBordure'
                ? '$nouveau = $avant -band (-bnot ($WS_CAPTION -bor $WS_THICKFRAME))'
                : '$nouveau = $avant -bor ($WS_CAPTION -bor $WS_THICKFRAME)',
        action === 'lire' ? '' : '[void][D9C1Fenetre]::SetWindowLongPtrW($hwnd, $GWL_STYLE, [IntPtr]$nouveau)',
        action === 'lire' ? '' : '[void][D9C1Fenetre]::SetWindowPos($hwnd, [IntPtr]::Zero, 0, 0, 0, 0, $SWP_FLAGS)',
        '$apres = [D9C1Fenetre]::GetWindowLongPtrW($hwnd, $GWL_STYLE).ToInt64()',
        '$sansBordureAvant = (($avant -band ($WS_CAPTION -bor $WS_THICKFRAME)) -eq 0)',
        '$sansBordureApres = (($apres -band ($WS_CAPTION -bor $WS_THICKFRAME)) -eq 0)',
        '$resultat = @{ hwnd = $hwnd.ToString("x"); avant = $avant; apres = $apres; sans_bordure_avant = $sansBordureAvant; sans_bordure_apres = $sansBordureApres }',
        `$resultat | ConvertTo-Json -Compress | Out-File -Encoding ascii C:\\dev\\${marqueur}-style.json`,
    ].filter((l) => l !== '').join('\n');
}
async function togglerStyleFenetre(marqueur, action) {
    vmIt(`style-${marqueur}`, psStyleFenetre(marqueur, action));
    await dodo(4000);
    let brut = '';
    try { brut = await readFile(`/media/vm/dev/${marqueur}-style.json`, 'utf8'); } catch { }
    let resultat = null;
    try { resultat = JSON.parse(brut); } catch { }
    log(`STYLE FENÊTRE (${marqueur}, action=${action}) ` + (brut || '(fichier absent)'));
    return resultat;
}

// ---------------------------------------------------------------- ouverture des fenêtres VM
function marqueurFenetre(n) { return `chrome-d9c1-${ETIQUETTE}-${n}`; }
function hzDe(n) { return 300 + 110 * n; }
function assurerTonHtml(hz) {
    spawnSync('bash', ['-c', `cp ${TON_HTML_D7} /media/vm/dev/ton-${hz}.html`]);
}
async function ouvrirFenetre(n) {
    const marqueur = marqueurFenetre(n);
    const hz = hzDe(n);
    assurerTonHtml(hz);
    const avant = appPages().length;
    log(`  · ouverture fenêtre ${n} (hz=${hz}, marqueur=${marqueur})`);
    vmIt(`ouvrird9c1-${n}`, [
        '$a = @(',
        `  "--app=file:///C:/dev/ton-${hz}.html?hz=${hz}",`,
        `  "--user-data-dir=C:\\dev\\${marqueur}",`,
        "  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',",
        `  '--window-size=1280,720','--window-position=${30 + n * 12},${30 + n * 12}',`,
        "  '--autoplay-policy=no-user-gesture-required',",
        "  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',",
        "  '--disable-renderer-backgrounding')",
        "Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a",
        'Start-Sleep -Seconds 2',
    ].join('\n'));
    for (let i = 0; i < 30; i += 1) {
        await dodo(2000);
        // `sid` = `dernierSidAppPage`, posé par le handler d'attachement au
        // moment même où il traite la page neuve : c'est la source de vérité
        // la plus directe, jamais une correspondance a posteriori sur `p.url`
        // (voir la note du troisième bug, plus haut — une telle
        // correspondance s'est révélée non fiable pour distinguer des pages
        // ouvertes coup sur coup).
        if (appPages().length > avant) return { ok: true, hz, marqueur, sid: dernierSidAppPage };
    }
    log(`  !! fenêtre ${n} : aucune page de plus après 60 s`);
    return { ok: false, hz, marqueur, sid: null };
}

/// NEUF. Bascule le style Windows de `marqueur`, attend une annonce
/// `actif=true` CÔTÉ AGENT, vérifie qu'UNE SEULE session distincte l'a
/// annoncée dans cette fenêtre temporelle (« et elle seule », point d),
/// restaure, et retourne la session résolue. Contrairement à
/// `resoudreIdentite` de D8, l'exclusivité est vérifiée ICI plutôt que reportée
/// à une phase séparée — elle porte sur `agent.log` directement (jamais sur le
/// tampon navigateur `window.__pleinEcran`, qui n'est ni vidé ni borné dans le
/// temps, défaut connu et non corrigé de D8).
async function identifierEtVerifierExclusivite(marqueur, etiquette) {
    log(`>>> IDENTITÉ + EXCLUSIVITÉ (${etiquette}) marqueur=${marqueur}`);
    const debut = maintenantIso();
    await togglerStyleFenetre(marqueur, 'sansBordure');
    const actifs = await attendreLeFait(`plein écran actif=true (${etiquette})`, async () => {
        const evts = await pleinEcranAgent(debut, maintenantIso());
        const uniques = [...new Set(evts.filter((e) => e.actif === true).map((e) => e.session))];
        return uniques.length > 0 ? uniques : null;
    }, 25);
    const finBascule = maintenantIso();
    await togglerStyleFenetre(marqueur, 'restaurer');
    const restaure = await attendreLeFait(`plein écran actif=false, restauration (${etiquette})`, async () => {
        const evts = await pleinEcranAgent(finBascule, maintenantIso());
        return (actifs && evts.find((e) => e.session === actifs[0] && e.actif === false)) ?? null;
    }, 20);
    if (!actifs) {
        throw new Error(`aucune annonce plein écran pour marqueur=${marqueur} — identité non résolue`);
    }
    const exclusif = actifs.length === 1;
    log(`  → ${marqueur} : sessions_actives=${JSON.stringify(actifs)} exclusif=${exclusif} restaure=${!!restaure}`);
    return { marqueur, session: actifs[0], sessions_actives: actifs, exclusif, restaure: !!restaure, debut, fin: finBascule };
}

function cpuHote(etiquette) {
    const charge = spawnSync('bash', ['-c', 'cat /proc/loadavg'], { encoding: 'utf8' }).stdout?.trim() ?? '';
    log(`CPU HÔTE (${etiquette}) loadavg=${charge}`);
    return { loadavg: charge };
}

// ---------------------------------------------------------------- phase Resize/HiDPI
/// Le cœur de la tâche. Reçoit le sid CDP CONNU (rendu par `ouvrirFenetre` —
/// jamais retrouvé après coup par une correspondance sur `p.url`, voir le
/// troisième bug documenté plus haut) d'une fenêtre DÉDIÉE, ouverte
/// spécialement pour cette mesure APRÈS la phase d'exclusivité, pour ne
/// souffrir d'aucune interférence avec les doublons que celle-ci engendre
/// (second bug documenté plus haut). Lit l'annonce de viewport et les Resize
/// émis, force un changement de viewport EN COURS DE SESSION, et relit ce
/// que ça produit. Ne SUPPOSE aucune issue — verse tout ce qui est observé,
/// y compris l'absence d'un rejeu naturel.
async function phaseResizeEtDpr(cdp, sidInitial, dprAttendu, nomFenetre, controleDpr) {
    log(`>>> PHASE RESIZE/HiDPI — fenêtre=${nomFenetre} sid_initial=${sidInitial?.slice(0, 8)} dpr_attendu=${dprAttendu}`);
    const debut = maintenantIso();

    // La session AGENT de cette page, lue sur la page elle-même (vérité
    // vivante, jamais une métadonnée CDP). Utile pour le relevé et pour une
    // première lecture, mais PAS pour filtrer le Resize qui suivra la
    // réouverture forcée ci-dessous : une relance de la même fenêtre Windows
    // par le superviseur peut recevoir un NOUVEAU numéro de session (D2/D3),
    // donc la session à surveiller sera relue sur la page FRAÎCHE.
    const sessionAvant = await cdp.evalBorne(sidInitial,
        `new URLSearchParams(location.search).get('session')`, 4000, false);
    const etatAvantForcage = await cdp.evalBorne(sidInitial, STATS, 8000, true);
    log(`  état AVANT forçage (${nomFenetre}) session=${sessionAvant} sid=${sidInitial?.slice(0, 8)} `
        + JSON.stringify(etatAvantForcage));

    // FORÇAGE DÉLIBÉRÉ, pas subi. On arme `controleDpr.valeur` au dpr voulu,
    // on FERME la page vivante (`Target.closeTarget`), et on attend que le
    // produit la ROUVRE : la fenêtre Windows source existe toujours côté VM,
    // la perte de connexion fait mourir l'enfant, et le superviseur relance —
    // même mécanisme que celui déjà éprouvé par D2/D3 (« fermer puis rouvrir
    // une fenêtre réussit, la place libérée suffit »), employé ici comme
    // OUTIL DE MESURE. Le prochain attachement de page "app" (`dernierSidAppPage`,
    // posé par le handler d'attachement de `main`) est pris pour la
    // réouverture — fiable ICI parce qu'aucune autre fenêtre ne s'ouvre plus
    // à ce stade de l'exécution (la phase d'exclusivité est terminée).
    controleDpr.valeur = dprAttendu;
    const sidAvantFermeture = dernierSidAppPage;
    const targetId = pages.get(sidInitial)?.targetId;
    if (targetId) {
        await cdp.send('Target.closeTarget', { targetId }).catch(() => { });
        log(`  page fermée délibérément (${nomFenetre}) pour forcer une réouverture à dpr=${dprAttendu}`);
    } else {
        log(`  !! targetId introuvable pour ${nomFenetre} (sid=${sidInitial?.slice(0, 8)}) — fermeture impossible`);
    }
    const debutReouverture = maintenantIso();
    const sidFrais = await attendreLeFait(`réouverture pour dpr=${dprAttendu} (${nomFenetre})`, async () => {
        return (dernierSidAppPage && dernierSidAppPage !== sidAvantFermeture) ? dernierSidAppPage : null;
    }, 60);
    controleDpr.valeur = 1;
    const reouvertureReussie = !!sidFrais;
    const sid = sidFrais ?? sidInitial;
    log(`  réouverture (${nomFenetre}) reussie=${reouvertureReussie} sid=${sid?.slice(0, 8) ?? '(aucun)'} `
        + `depuis=${debutReouverture}`);

    // Session de la page FRAÎCHE — peut différer de `sessionAvant` (voir la
    // remarque ci-dessus).
    const session = reouvertureReussie
        ? await cdp.evalBorne(sid, `new URLSearchParams(location.search).get('session')`, 4000, false)
        : sessionAvant;
    log(`  session après réouverture (${nomFenetre}) = ${session} (avant : ${sessionAvant})`);

    // État initial : annonce de viewport et avertissements console déjà
    // capturés à l'exécution de l'AMORCE + main.ts (avant même que ce pilote
    // n'interroge quoi que ce soit) — sur la page FRAÎCHE si la réouverture a
    // réussi, sinon sur la page précédente (versé tel quel, non masqué).
    const etatInitial = await cdp.evalBorne(sid, STATS, 8000, true);
    log(`  état initial (${nomFenetre}) sid=${sid?.slice(0, 8)} ` + JSON.stringify(etatInitial));

    // Premier Resize observé côté agent, sur CETTE session, DEPUIS LA
    // RÉOUVERTURE forcée ci-dessus (jamais depuis le tout début : un Resize
    // antérieur appartiendrait à la page précédente, pas à celle dont le dpr
    // est connu avec certitude).
    const premierResize = await attendreLeFait(`premier Resize agent (${nomFenetre})`, async () => {
        const r = await resizeAgent(debutReouverture, maintenantIso());
        return r.find((e) => e.session === session) ?? null;
    }, 30);

    // Changement de viewport EN COURS DE SESSION, sur la MÊME page fraîche.
    const [midL, midH] = VIEWPORT_MID_SESSION.split('x').map(Number);
    const debutMid = maintenantIso();
    await cdp.send('Emulation.setDeviceMetricsOverride',
        { width: midL, height: midH, deviceScaleFactor: dprAttendu, mobile: false }, sid).catch(() => { });
    log(`  viewport forcé en cours de session (${nomFenetre}) → ${midL}x${midH} @dpr${dprAttendu}`);
    await dodo(1500); // le debounce du ResizeObserver est de 200 ms (client/src/main.ts)
    const etatApresMid = await cdp.evalBorne(sid, STATS, 8000, true);
    const resizeMidSession = await attendreLeFait(`Resize mi-session agent (${nomFenetre})`, async () => {
        const r = await resizeAgent(debutMid, maintenantIso());
        return r.find((e) => e.session === session) ?? null;
    }, 20);

    // Restauration.
    const [baseL, baseH] = VIEWPORT_BASE.split('x').map(Number);
    await cdp.send('Emulation.setDeviceMetricsOverride',
        { width: baseL, height: baseH, deviceScaleFactor: dprAttendu, mobile: false }, sid).catch(() => { });
    await dodo(1500);

    const fin = maintenantIso();
    const tousLesResize = await resizeAgent(debut, fin);
    const resizeDeCetteSession = tousLesResize.filter((e) => e.session === session);

    const consoleWarnDeference = (etatInitial?.console_warn ?? [])
        .filter((w) => w.msg.includes('Resize différé'));
    const rejeuObserve = consoleWarnDeference.length > 0 && !!premierResize
        && premierResize.t >= (etatInitial.horloge ? new Date(etatInitial.horloge).toISOString() : '');

    // (c) même unité : comparaison de l'annonce de viewport (postMessage) et
    // du Resize (contrôle) — les deux doivent porter la MÊME grandeur si le
    // client applique le dpr aux deux endroits. `viewport_msg` peut être
    // absent si la page a été ouverte SANS `window.opener` (rechargement
    // direct) — ce n'est PAS le cas ici (toujours ouverte par shell.html).
    const viewportMsg = etatInitial?.viewport_msg ?? null;
    const memeUnite = !!(viewportMsg && premierResize
        && Math.abs(viewportMsg.largeur - premierResize.largeur) <= 2
        && Math.abs(viewportMsg.hauteur - premierResize.hauteur) <= 2);

    const releve = {
        fenetre: nomFenetre, session_avant: sessionAvant, session, dpr_attendu: dprAttendu, debut, fin,
        forcage: {
            sid_initial: sidInitial, etat_avant_forcage: etatAvantForcage,
            debut_reouverture: debutReouverture, reouverture_reussie: reouvertureReussie, sid_frais: sidFrais,
        },
        etat_initial: etatInitial,
        viewport_msg: viewportMsg,
        console_warn: etatInitial?.console_warn ?? [],
        premier_resize_agent: premierResize,
        resize_mi_session: { demande: `${midL}x${midH}`, dpr: dprAttendu, evenement: resizeMidSession, inner_apres: etatApresMid },
        tous_les_resize_toutes_sessions_pendant_la_phase: tousLesResize,
        resize_de_cette_session: resizeDeCetteSession,
        deference_console_observee: consoleWarnDeference.length > 0,
        rejeu_observe_naturellement: rejeuObserve,
        meme_unite_annonce_et_resize: memeUnite,
        toutes_les_lignes_portent_leur_session: resizeDeCetteSession.every((e) => !!e.session),
    };
    log(`RESIZE/HiDPI (${nomFenetre}) — RELEVÉ ` + JSON.stringify(releve, null, 1));
    return releve;
}


// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9995);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d9c1-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns', '--disable-popup-blocking',
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', 'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} PORT_SHELL=${PORT_SHELL} N_FENETRES=${N_FENETRES} DPR_CIBLE=${DPR_CIBLE} `
        + `PHASE_EXCLUSIVITE=${PHASE_EXCLUSIVITE} chrome pid=${chrome.pid}`);

    const releve = { etiquette: ETIQUETTE, port_shell: PORT_SHELL, n_fenetres: N_FENETRES, phases: {} };
    // dpr appliqué à TOUTE prochaine page "app" qui s'attache — piloté
    // explicitement par le code plus bas (JAMAIS déduit d'un ordre
    // d'attachement, voir le second bug documenté ci-dessous).
    const controleDpr = { valeur: 1 };
    const sidParOrdre = [];
    // BUG TROUVÉ EN COURS D'EXÉCUTION (première tentative de la tâche 14) : une
    // page ouverte par `window.open()` (donc TOUTE page "app") attache avec une
    // URL VIDE — la navigation n'a pas encore démarré au moment de
    // `Target.attachedToTarget` (`waitForDebuggerOnStart` pause AVANT la
    // navigation, précisément pour que nos overrides s'appliquent avant tout
    // script du produit). Décider "est-ce une page app ?" sur `targetInfo.url`
    // à cet instant est donc TOUJOURS faux pour une page ouverte par
    // `window.open` (contrairement à la page-shell elle-même, créée par
    // `Target.createTarget` avec son URL connue D'AVANCE — cette asymétrie est
    // le piège). Conséquence observée : AUCUN override de deviceMetrics
    // n'était appliqué aux pages app, qui atterrissaient donc sur la taille
    // "naturelle" d'un popup headless (1280×632, ~88 px de moins que demandé —
    // même chiffre que le "88 px" documenté ailleurs pour d'autres navigateurs),
    // que le pilote SudoVDA refusait de faire correspondre à la sortie qu'il
    // avait réellement créée (720 de haut, la persistance déjà documentée pour
    // ce sous-bloc) — le superviseur relançait alors un enfant en boucle.
    // Remède : ne JAMAIS décider sur l'URL pour les pages app. La page-shell
    // est identifiée par son URL (fiable, connue d'avance) ; l'UNIQUE page
    // "about:blank" initiale (celle du lancement de Chrome, PAS une future
    // page app) est celle dont c'est le tout premier attachement de type page
    // ; tout le reste, dans l'ordre d'arrivée, est une page app.
    //
    // SECOND BUG TROUVÉ EN COURS D'EXÉCUTION (deuxième et troisième tentative
    // de la tâche 14) : `shell.ts::fenetreOuverte` n'est PAS idempotente (elle
    // ouvre une page à CHAQUE appel, sans vérifier `connues.has(session)`), et
    // le superviseur réémet en pratique `fenetre-ouverte` pour une fenêtre
    // DÉJÀ ouverte — observé corrélé à chaque bascule de style Windows
    // (`togglerStyleFenetre`), mécanisme non élucidé côté agent et hors
    // périmètre de cette tâche de mesure. Conséquence : "la première page app
    // jamais attachée" n'est PAS un identifiant fiable de "la fenêtre 1" —
    // une ou plusieurs pages EXCÉDENTAIRES pour la MÊME fenêtre peuvent
    // apparaître à tout moment, y compris pendant la phase d'exclusivité.
    // Assigner `dpr=DPR_CIBLE` par ordre d'attachement a fait lire, une fois,
    // un état où la page réellement vivante pour la session cible portait
    // `dpr=1` (elle avait attaché EN SECOND, hors de tout ordre prévisible).
    //
    // Remède retenu : ne plus JAMAIS déduire le dpr d'un ordre d'attachement.
    // `controleDpr.valeur` est piloté EXPLICITEMENT par le code de `main`
    // (armé à `DPR_CIBLE` juste avant `ouvrirFenetre(1)`, désarmé juste après)
    // pour capturer l'ouverture initiale, ET REPRIS explicitement dans
    // `phaseResizeEtDpr`, qui ferme la page vivante puis attend sa
    // RÉOUVERTURE — provoquée délibérément, jamais subie — pour garantir que
    // la MESURE, elle, porte sur une page dont le dpr est connu avec
    // certitude au moment de son tout premier script.
    let indexAttachePage = 0;
    try {
        const version = await attendreDevtools(port);
        const qui = spawnSync('bash', ['-c',
            `ss -ltnp 2>/dev/null | grep ':${port} ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1`],
            { encoding: 'utf8' }).stdout.trim();
        log(`identité du navigateur : pid écoutant=${qui} pid lancé=${chrome.pid} version=${version.Browser}`);
        if (String(qui) !== String(chrome.pid)) throw new Error(`port ${port} tenu par ${qui}, pas ${chrome.pid}`);
        releve.navigateur = version.Browser;

        const cdp = new Cdp(version.webSocketDebuggerUrl);
        cdp.on(async (m) => {
            if (m.method === 'Target.attachedToTarget') {
                const { sessionId, targetInfo } = m.params;
                if (targetInfo.type !== 'page') {
                    await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
                    return;
                }
                pages.set(sessionId, { targetId: targetInfo.targetId, url: targetInfo.url });
                indexAttachePage += 1;
                const estInitiale = indexAttachePage === 1;
                const estShell = targetInfo.url.includes('shell.html');
                const estAppPage = !estInitiale && !estShell;
                log(`+ page attachée  session=${sessionId.slice(0, 8)} url=${targetInfo.url} `
                    + `index=${indexAttachePage} initiale=${estInitiale} shell=${estShell} app=${estAppPage}`);
                await cdp.send('Page.enable', {}, sessionId).catch(() => { });
                await cdp.send('Runtime.enable', {}, sessionId).catch(() => { });
                await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: AMORCE }, sessionId).catch(() => { });
                await cdp.send('Runtime.evaluate', { expression: AMORCE, returnByValue: true }, sessionId).catch(() => { });
                if (estAppPage) {
                    sidParOrdre.push(sessionId);
                    dernierSidAppPage = sessionId;
                    const dpr = controleDpr.valeur;
                    const [L, H] = VIEWPORT_BASE.split('x').map(Number);
                    // QUATRIÈME BUG TROUVÉ EN COURS D'EXÉCUTION : demander
                    // 2560×1440 (1280×720 en pixels CSS à dpr=2) fait échouer
                    // la création de la sortie virtuelle — persistance déjà
                    // documentée (CLAUDE.md, D8/D9-2bis) : le pilote SudoVDA
                    // snappe sur une taille laissée par un essai ANTÉRIEUR
                    // (3840×2160, observé), pas celle demandée, et l'agent
                    // rejette la sortie « introuvable dans la topologie ».
                    // AUCUN enfant ne se lance alors, donc AUCUNE session ne
                    // s'établit — le test entier serait mort avant même
                    // d'avoir commencé. Remède : les pixels CSS envoyés à
                    // `setDeviceMetricsOverride` sont divisés par `dpr`, pour
                    // que L'ÉQUIVALENT EN PIXELS PÉRIPHÉRIQUES (ce que
                    // `main.ts` annonce et ce que la sortie virtuelle demande)
                    // reste TOUJOURS `VIEWPORT_BASE`, quel que soit le dpr —
                    // exactement la taille que les fenêtres à dpr=1 viennent
                    // de faire réussir dans CETTE MÊME exécution. Le test du
                    // dpr porte sur les UNITÉS (même grandeur, deux dpr
                    // différents), pas sur la résolution physique obtenue.
                    const [Lcss, Hcss] = DIVISER_CSS_PAR_DPR
                        ? [Math.round(L / dpr), Math.round(H / dpr)]
                        : [L, H];
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: Lcss, height: Hcss, deviceScaleFactor: dpr, mobile: false }, sessionId).catch(() => { });
                    log(`  → page app #${sidParOrdre.length} (session=${sessionId.slice(0, 8)}) : `
                        + `dpr=${dpr} css=${Lcss}x${Hcss} (équivalent périphérique ${L}x${H})`);
                    if (FORCER_DEFERT) {
                        await cdp.send('Network.enable', {}, sessionId).catch(() => { });
                        await cdp.send('Network.emulateNetworkConditions', {
                            offline: false, latency: LATENCE_FORCEE_MS,
                            downloadThroughput: -1, uploadThroughput: -1,
                        }, sessionId).catch(() => { });
                        log(`  → FORCER_DEFERT : latence de ${LATENCE_FORCEE_MS} ms posée (session=${sessionId.slice(0, 8)})`);
                        setTimeout(() => {
                            cdp.send('Network.emulateNetworkConditions', {
                                offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1,
                            }, sessionId).catch(() => { });
                            log(`  → FORCER_DEFERT : latence retirée (session=${sessionId.slice(0, 8)})`);
                        }, LATENCE_FORCEE_MS + 4000);
                    }
                }
                await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
            } else if (m.method === 'Target.detachedFromTarget') {
                pages.delete(m.params.sessionId);
            } else if (m.method === 'Target.targetInfoChanged') {
                for (const [, p] of pages) {
                    if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
                }
            }
        });
        await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
        await cdp.send('Target.setDiscoverTargets', { discover: true });

        log('>>> ÉTAPE 0 : préparation — VM sans fenêtre éligible (vérifié, pas supposé)');
        releve.cpu_repos = cpuHote('au repos, avant toute session');
        await purgerFenetresVMRescapees();
        releve.purge_rescapees = true;

        await cdp.send('Target.createTarget', { url: URL_SHELL });
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut').textContent`));

        log('>>> lancement du superviseur');
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 BITRATE=${BITRATE} ` +
            `SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        await dodo(6000);

        // CINQUIÈME BUG TROUVÉ EN COURS D'EXÉCUTION : ouvrir une fenêtre
        // DÉDIÉE au test dpr (une 4ᵉ fenêtre, après les trois de la phase
        // d'exclusivité) l'a fait atterrir sur un slot de sortie virtuelle
        // (`\\.\DISPLAY8`) dont le registre restait bloqué à une taille
        // laissée par un essai ANTÉRIEUR (3840×2160, hors de ce fichier) —
        // AUCUNE taille demandée, pas même 1280×720 (déjà éprouvée par les
        // fenêtres 1 à 3 sur LEURS PROPRES slots), ne pouvait y réussir :
        // seul un changement de mode explicite (`CDS_UPDATEREGISTRY`,
        // absent du produit depuis la tâche 3) peut modifier un registre déjà
        // écrit, une simple création ne fait jamais que s'y voir imposer sa
        // valeur. Remède retenu : NE PLUS DEMANDER de slot neuf pour le test
        // dpr. La fenêtre 1 sert AUX DEUX : son slot est éprouvé à 1280×720
        // dès son ouverture normale (dpr=1), et la RÉTENTION de sortie d'une
        // relance à l'autre (D3 : « la sortie virtuelle est retenue d'une
        // relance à l'autre ») garantit que la fermeture forcée de
        // `phaseResizeEtDpr` (qui ne fait AUCUNE différence entre relance
        // "spontanée" et relance provoquée) réutilise CE MÊME slot, déjà
        // aligné — jamais un slot neuf. Le test dpr est donc joué
        // IMMÉDIATEMENT après l'ouverture de la fenêtre 1, tant que
        // `dernierSidAppPage` la désigne SANS AMBIGUÏTÉ (avant que les
        // fenêtres 2 et 3 ne s'ouvrent et n'y ajoutent leurs propres pages).
        controleDpr.valeur = 1;
        const r1 = await ouvrirFenetre(1);
        if (!r1.ok || !r1.sid) throw new Error('fenêtre 1 non ouverte');
        await dodo(2000);

        releve.phases.resize_dpr_cible = await phaseResizeEtDpr(cdp, r1.sid, DPR_CIBLE, 'cible', controleDpr);
        const sessionCible = releve.phases.resize_dpr_cible.session;
        log(`SESSION CIBLE (fenêtre 1) = ${sessionCible}`);
        releve.session_cible = sessionCible;

        // Le reste des fenêtres (2..N_FENETRES) s'ouvre à dpr=1, ensuite.
        controleDpr.valeur = 1;
        const fenetres = [{ ok: true, marqueur: marqueurFenetre(1), sid: r1.sid }];
        for (let n = 2; n <= N_FENETRES; n += 1) {
            const r = await ouvrirFenetre(n);
            fenetres.push(r);
            if (!r.ok) break;
        }
        log(`  ${fenetres.filter((f) => f.ok).length}/${N_FENETRES} fenêtres ouvertes`);
        await dodo(6000);
        releve.fenetres_ouvertes = fenetres;

        // ---- (d) : non-régression de la détection plein écran, exclusivité ----
        // Jouée pour LES TROIS fenêtres, y compris la fenêtre 1 — sa page a
        // changé (fermeture + réouverture ci-dessus), mais sa fenêtre
        // Windows, elle, n'a jamais bougé : c'est bien elle que la bascule de
        // style vise.
        const idsExclusivite = [];
        if (PHASE_EXCLUSIVITE === '1') {
            for (let n = 1; n <= fenetres.filter((f) => f.ok).length; n += 1) {
                const marqueur = marqueurFenetre(n);
                const id = await identifierEtVerifierExclusivite(marqueur, `fenêtre ${n}`);
                idsExclusivite.push(id);
            }
        }
        releve.exclusivite = idsExclusivite;

        // Toutes les lignes Resize toutes fenêtres confondues, pour (b).
        const tousResizeGlobal = await resizeAgent(null, maintenantIso());
        releve.tous_les_resize_toutes_sessions = tousResizeGlobal;
        releve.toutes_les_lignes_resize_portent_leur_session = tousResizeGlobal.every((e) => !!e.session);

        releve.marqueurs_finaux = {
            lignes_resize: tousResizeGlobal.length,
            sessions_distinctes_resize: [...new Set(tousResizeGlobal.map((e) => e.session))],
        };
        releve.survie_finale = vmVivante('fin de mesure');
        await writeFile(SORTIE_JSON, JSON.stringify(releve, null, 1));
        log('relevé écrit dans ' + SORTIE_JSON);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
        await dodo(8000);
        copierLog();
        log('journal copié dans ' + COPIE_LOG + ' (après la fermeture du navigateur)');
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
