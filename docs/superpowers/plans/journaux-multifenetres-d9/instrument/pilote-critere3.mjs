#!/usr/bin/env node
// Sous-bloc D9, tâche 16 — pilote de la recette ③ : le focus à palier LONG
// (rejeu du critère ④ de D6, palier 25 s → 60 s), et l'A/B différentiel sur
// `set_desired_bitrate` (`PART_SONDAGE`, tâche 12), jamais joué par D6.
//
// Dérivé de `docs/superpowers/plans/journaux-multifenetres-d6/instrument/pilote-recette-d6.mjs`
// (structure des phases, `imposerScenario` blur-puis-focus, `attendreBarreauxStables`,
// `partsAgent`, `barreaux`, `cadencesAgent`, `deltas`, l'AMORCE d'interception
// RTCPeerConnection — repris quasi tels quels, ce montage a déjà été éprouvé)
// et de `pilote-critere1.mjs` (tâche 14, D9) pour la mécanique VM (`vmIt` par
// tâche planifiée directe via `winrm.js`, `purgerFenetresVMRescapees`,
// `sessionDeLigne`/`champ` — le correctif D8 du piège span/`w-2}:`).
//
// RETIRÉ par rapport à D6 : la PHASE 3 (montée à dix fenêtres, sommeil) — hors
// périmètre de cette tâche, déjà mesurée par D5/D6.
//
// AJOUTÉ : un MODE binaire.
//   MODE=focus (défaut) — construit 8 fenêtres éveillées, impose le focus sur
//     la première, ATTEND que l'échelle soit posée (le FAIT, jamais une
//     durée), puis rejoue exactement la PHASE 2 de D6 (deux déplacements de
//     focus) à `PALIER_FOCUS_S` (60 s par défaut, contre 25 s en D6).
//   MODE=ab — construit les mêmes 8 fenêtres éveillées, attend l'échelle
//     posée, puis mesure le trafic vidéo cumulé (delta `bytesReceived`, côté
//     RÉCEPTEUR — voir la note méthodologique ci-dessous) sur `PALIER_AB_S`
//     secondes. `PART_SONDAGE` est un paramètre D'ENVIRONNEMENT, jamais posé
//     ici : le lancer avec `PART_SONDAGE=0 node pilote-critere3.mjs` suffit —
//     `spawnSync(..., { env: process.env })` le relaie tel quel à
//     `run-agent.sh`, qui le relaie à son tour à l'agent (tâche 12).
//
// ============================================================================
// NOTE MÉTHODOLOGIQUE — bytesSent vs bytesReceived
// ============================================================================
// Le brief demande le `bytesSent` cumulé. Aucune stat `outbound-rtp` n'existe
// côté navigateur pour une piste RECVONLY (le sens est agent → navigateur) :
// la seule mesure accessible par CDP est `inbound-rtp.bytesReceived`, côté
// RÉCEPTEUR. C'est le proxy déjà employé par tout ce chantier (D4 à D6) pour
// approximer ce que l'agent envoie, et D6 a établi que le lien local ne perd
// AUCUN paquet à cette échelle (`packetsLost = 0` aux onze exécutions
// pertinentes) : sur ce montage, `bytesReceived` cumulé EST `bytesSent`
// cumulé, à l'overhead de retransmission près (nul si `packetsLost = 0`).
// Ce pilote relève aussi `paquets_perdus` à chaque mesure pour vérifier cette
// condition plutôt que la supposer.
//
// ============================================================================
// PIÈGES DE MÉTHODE, TOUS PAYÉS PAR D6 (repris du brief de tâche 16)
// ============================================================================
//   1. Énoncer la règle de sélection AVANT de compter : « 4 sur 4 » a caché 14
//      en D6, dont l'exécution écartée était précisément celle qui échoue.
//      Règle ICI, énoncée D'AVANCE : la population est l'ensemble des
//      déplacements de focus joués par CE pilote, dans TOUTES les exécutions
//      versées sous `critere-3-focus-*.log` — aucune exclusion.
//   2. Un changement de barreau produit DEUX lignes au même horodatage (côté
//      capteur ET côté enfant) : tout compteur BRUT de `barreaux()` vaut donc
//      le double d'un compte d'ÉVÉNEMENTS. Le calcul de promotion ci-dessous
//      ne compte PAS ces lignes : il compare des TAILLES relevées via
//      `getStats()`, jamais un nombre de lignes de journal.
//   3. Faire passer la cible par `blur` PUIS `focus` (repris tel quel de
//      `imposerScenario`, voir son commentaire).
//   4. Attendre le FAIT (`attendreBarreauxStables`), jamais une durée.
//   5. Un contrôle doit pouvoir ÉCHOUER pour valoir quelque chose — le
//      contrôle de budget (`budgetAnnonce`) n'est appelé qu'APRÈS la première
//      fenêtre, jamais avant (le `OnceLock` de `capteur/sommeil/parts.rs`).
//
// ============================================================================
// GARDE-FOUS REPRIS DE D8/D9 (mêmes raisons)
// ============================================================================
//   - `--user-data-dir` PAR FENÊTRE.
//   - Aucune capture d'écran CDP pendant une mesure ; toute évaluation CDP sur
//     une page WebRTC active est BORNÉE (`Cdp.evalBorne`).
//   - `Get-Process agent`/`chrome` vérifiés et PURGÉS avant lancement
//     (`purgerFenetresVMRescapees`), pour ne pas hériter d'une session
//     rescapée d'une tentative précédente (échouée ou non).
//   - Survie de la VM contrôlée après chaque phase (`vmVivante`).
//   - `agent.log` copié APRÈS la fin réelle de l'exécution (leçon D4).
//   - Le navigateur PILOTE tourne sur l'HÔTE, jamais sur la VM.
//   - La source BOUGE (`anim-d4.html`, un `--user-data-dir` par fenêtre).

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

function racineDepot() {
    const r = spawnSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' });
    if (r.status !== 0) {
        throw new Error(`hors du depot git : impossible de deriver RACINE (git rev-parse a echoue : ${(r.stderr ?? '').trim()})`);
    }
    return r.stdout.trim();
}
const RACINE = process.env.RACINE ?? racineDepot();
const ANIM_HTML = join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d9/instrument/anim-d4.html');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const PORT_SHELL = process.env.PORT_SHELL ?? '5173';
const URL_SHELL = `http://${HOTE}:${PORT_SHELL}/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const COPIE_LOG = process.env.COPIE_LOG ?? `/tmp/agent-critere-3-${ETIQUETTE}.log`;
const SORTIE_JSON = process.env.SORTIE_JSON
    ?? join(RACINE, `docs/superpowers/plans/journaux-multifenetres-d9/critere-3-${ETIQUETTE}.json`);
const MODE = process.env.MODE ?? 'focus'; // 'focus' | 'ab'
const BUDGET_BPS = process.env.BUDGET_BPS ?? '12000000';
const BITRATE = process.env.BITRATE ?? '12000000';
const N_EVEIL = Number(process.env.N_EVEIL ?? 8); // vivier::PLAFOND_EVEIL = 8
const PALIER_FOCUS_S = Number(process.env.PALIER_FOCUS_S ?? 60); // DELAI_REMONTEE=20s, D6 mesurait 25s
const PALIER_AB_S = Number(process.env.PALIER_AB_S ?? 30);
const CALME_S = Number(process.env.CALME_S ?? 15);
const STABILISATION_MAX_S = Number(process.env.STABILISATION_MAX_S ?? 90);
// Relayé par héritage d'environnement à `run-agent.sh` — jamais posé ici.
const PART_SONDAGE_VU = process.env.PART_SONDAGE ?? '(absent → armé)';

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(7);
    console.log(`[${dt}s ${new Date().toISOString()}] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- VM
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
    const acces = spawnSync('bash', ['-c', 'ls /media/vm/dev/anim-d4.html >/dev/null 2>&1 && echo OUI || echo NON'],
        { encoding: 'utf8' }).stdout?.trim() ?? '?';
    log(`SURVIE VM (${etiquette}) virsh="${virsh}" acces_partage=${acces}`);
    return { virsh, acces };
}
function copierLog() {
    spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
}
// Repris de D9-T14 (revue T11, remède B2) : purger avant TOUTE tentative, y
// compris après une tentative précédente échouée — sinon une fenêtre ou un
// agent rescapé fausse le compte de fenêtres éveillées.
async function purgerFenetresVMRescapees() {
    const script = [
        'Get-Process chrome, agent -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue',
        'Start-Sleep -Seconds 2',
        '$n = (Get-Process chrome -ErrorAction SilentlyContinue | Measure-Object).Count',
        '$agents = (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count',
        '@{ chrome_restants = $n; agents_restants = $agents } | ConvertTo-Json -Compress | Out-File -Encoding ascii C:\\dev\\purge-rescapees-c3.json',
    ].join('\n');
    vmIt('purge-rescapees-c3', script);
    await dodo(5000);
    let resultat = null;
    try { resultat = JSON.parse(await readFile('/media/vm/dev/purge-rescapees-c3.json', 'utf8')); } catch { }
    log('PURGE FENÊTRES/AGENTS RESCAPÉS (avant lancement du superviseur) ' + JSON.stringify(resultat));
    if (!resultat || resultat.chrome_restants !== 0 || resultat.agents_restants !== 0) {
        throw new Error(`état VM non propre avant lancement : ${JSON.stringify(resultat)}`);
    }
}
async function journalPlat() {
    copierLog();
    let texte = '';
    try { texte = await readFile(COPIE_LOG, 'utf8'); } catch { }
    return texte.replace(/\x1b\[[0-9;]*m/g, '');
}
const horodate = (l) => l.match(/(\d{4}-\d\d-\d\dT[\d:.]+Z)/)?.[1];
// Correctif D8 (défaut span/`w-2}:`) : le motif span `fenetre{session=…}` est
// tenté EN PREMIER, avec une classe qui s'arrête à `}` — sinon `\S+` capture
// l'accolade et les deux-points sur les lignes du fil de fenêtre.
const sessionDeLigne = (l) => l.match(/fenetre\{session=([^}]+)\}/)?.[1] ?? l.match(/\bsession=(\S+)/)?.[1] ?? null;
const champ = (l, nom) => l.match(new RegExp(`\\b${nom}=(\\S+)`))?.[1] ?? null;

/// LE CONTRÔLE QUE `BUDGET_BPS` EST ARRIVÉ. `budget_bps()`
/// (`capteur/sommeil/parts.rs`) est un `OnceLock` : n'a de sens qu'APRÈS la
/// première fenêtre (piège D6, payé trois fois avant elle).
async function budgetAnnonce() {
    const plat = await journalPlat();
    const vues = [...plat.matchAll(/budget de debit de la session budget_bps=(\d+)/g)].map((m) => Number(m[1]));
    log(`CONTRÔLE BUDGET : valeurs=${JSON.stringify(vues)} attendu=${BUDGET_BPS}`);
    return vues;
}

/// Les parts accordées, PAR SESSION — la dernière vue pour chaque session
/// avant `finIso`.
async function partsAgent(finIso) {
    const plat = await journalPlat();
    const dernieres = {}, toutes = [];
    for (const l of plat.split('\n')) {
        if (!l.includes('part de budget appliquee')) continue;
        const t = horodate(l);
        if (!t || (finIso && t > finIso)) continue;
        const s = champ(l, 'session');
        const p = Number(champ(l, 'part_bps'));
        if (!s || !Number.isFinite(p)) continue;
        dernieres[s] = { bps: p, t };
        toutes.push({ s, bps: p, t });
    }
    const somme = Object.values(dernieres).reduce((a, x) => a + x.bps, 0);
    return { dernieres, somme, nombre_de_lignes: toutes.length };
}

/// Les changements de taille d'encodage — PREUVE qu'un barreau a bougé.
/// ⚠️ Chaque changement produit DEUX lignes (capteur + enfant, même
/// horodatage) : ce compte brut n'est PAS un compte d'événements.
async function barreaux() {
    const plat = await journalPlat();
    const ok = [], ko = [];
    for (const l of plat.split('\n')) {
        const t = horodate(l) ?? '?';
        const L = l.match(/largeur=(\d+)/)?.[1], H = l.match(/hauteur=(\d+)/)?.[1];
        if (l.includes("taille d'encodage changée")) ok.push({ t, taille: `${L}x${H}` });
        else if (l.includes("changement de taille d'encodage refusé")) ko.push({ t, taille: `${L}x${H}` });
    }
    return { changees: ok, refusees: ko };
}

/// Attend que l'échelle se soit POSÉE — le FAIT, jamais une durée (leçon D3,
/// repayée par D6/D8). Tant qu'un changement de barreau est apparu depuis
/// moins de `calmeS` secondes, on continue d'attendre, borné par `maxS`.
async function attendreBarreauxStables(calmeS, maxS) {
    const debut = Date.now();
    let dernierCompte = -1, dernierChangement = Date.now();
    while ((Date.now() - debut) / 1000 < maxS) {
        const b = await barreaux();
        if (b.changees.length !== dernierCompte) {
            dernierCompte = b.changees.length;
            dernierChangement = Date.now();
        } else if ((Date.now() - dernierChangement) / 1000 >= calmeS) {
            const attendu = Number(((Date.now() - debut) / 1000).toFixed(1));
            log(`ÉCHELLE POSÉE après ${attendu} s (${dernierCompte} changements cumulés, ${calmeS} s sans changement)`);
            return { stable: true, attente_s: attendu, changements: dernierCompte };
        }
        await dodo(3000);
    }
    const attendu = Number(((Date.now() - debut) / 1000).toFixed(1));
    log(`!! ÉCHELLE NON POSÉE après ${attendu} s (${dernierCompte} changements cumulés)`);
    return { stable: false, attente_s: attendu, changements: dernierCompte };
}

async function marqueurs(etiquette) {
    const plat = await journalPlat();
    const compte = (motif) => (plat.match(new RegExp(motif, 'g')) ?? []).length;
    const m = {
        lignes: plat.split('\n').length,
        enfant_lance: compte('enfant lancé'),
        attachee_capteur: compte('fenêtre attachée au capteur'),
        cloture: compte('clôture de session amorcée'),
        endormie: compte('fenêtre endormie'),
        part_appliquee: compte('part de budget appliquee'),
        taille_changee: compte("taille d'encodage changée"),
        taille_refusee: compte("changement de taille d'encodage refusé"),
        erreurs: compte('ERROR'),
    };
    log(`MARQUEURS (${etiquette}) ` + JSON.stringify(m));
    return m;
}

// ---------------------------------------------------------------- CDP
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
        const r = await this.send('Runtime.evaluate', { expression, awaitPromise, returnByValue: true }, sessionId);
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

// L'amorce — visibilité ET focus pilotés par le SCÉNARIO, jamais le
// navigateur (repris tel quel de D6, éprouvé).
const AMORCE = `
(() => {
  if (window.__amorceD9C3) return;
  window.__amorceD9C3 = true;
  window.__pc = null;
  window.__liens = [];
  const N = window.RTCPeerConnection;
  const creer = N.prototype.createDataChannel;
  N.prototype.createDataChannel = function (label, ...r) {
    const c = creer.call(this, label, ...r);
    if (label === 'control') {
      c.addEventListener('message', (e) => {
        try {
          const m = JSON.parse(e.data);
          if (m && m.type === 'link') {
            window.__liens.push({ t: Date.now(), bitrate: m.bitrate, w: m.width, h: m.height,
                                  quality: m.quality, adaptation: m.adaptation });
            if (window.__liens.length > 400) window.__liens.shift();
          }
        } catch { }
      });
    }
    return c;
  };
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;

  window.__cachee = false;
  window.__focalisee = false;
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => window.__cachee });
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
  document.hasFocus = () => window.__focalisee;
})();
`;

const pages = new Map();
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));
const nomDe = (url) => url.replace(/^.*\?session=/, 'w:');
const rang = (n) => Number(String(n).match(/(\d+)\s*$/)?.[1] ?? 0);
const nomsTries = () => appPages().map(([, p]) => nomDe(p.url)).sort((a, b) => rang(a) - rang(b));

const STATS = `(async () => {
  const pc = window.__pc;
  if (!pc) return { pc: null };
  const r = await pc.getStats(); const t = [...r.values()];
  const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
  const a = t.find(x => x.type === 'inbound-rtp' && x.kind === 'audio');
  const paire = t.find(x => x.type === 'candidate-pair' && (x.selected || x.nominated));
  const lien = window.__liens.length ? window.__liens[window.__liens.length - 1] : null;
  return {
    etat: pc.iceConnectionState, horloge: Date.now(),
    images_decodees: v?.framesDecoded ?? null,
    images_recues: v?.framesReceived ?? null,
    octets_recus: v?.bytesReceived ?? null,
    octets_audio: a?.bytesReceived ?? null,
    paquets_recus: v?.packetsReceived ?? null,
    paquets_perdus: v?.packetsLost ?? null,
    l: v?.frameWidth ?? null, h: v?.frameHeight ?? null,
    rtt: paire?.currentRoundTripTime ?? null,
    lien_bitrate: lien?.bitrate ?? null, lien_taille: lien ? (lien.w + 'x' + lien.h) : null,
    cachee: document.hidden, focalisee: document.hasFocus(),
  };
})()`;

async function statsToutes(cdp, etiquette) {
    const entrees = appPages();
    const resultats = await Promise.all(entrees.map(async ([sid, p]) =>
        [nomDe(p.url), await cdp.evalBorne(sid, STATS)]));
    const out = Object.fromEntries(resultats);
    log(`STATS (${etiquette}) ` + JSON.stringify(out));
    return out;
}

/// Impose le scénario page par page : toutes visibles, UNE SEULE focalisée.
/// La cible passe ELLE AUSSI par `blur` avant `focus` : sans cela, une page
/// tout juste ouverte peut déjà s'annoncer focalisée avant que l'amorce
/// n'ait posé l'override, et la déduplication de `client/src/visibilite.ts`
/// avale alors le `focus` qui suit — plus AUCUNE fenêtre focalisée. Piège
/// documenté par D6, repris ici tel quel.
async function imposerScenario(cdp, cible, etiquette) {
    const etats = [];
    const ordre = appPages();
    for (const passe of ['blur', 'focus']) {
        for (const [sid, p] of ordre) {
            const veutFocus = passe === 'focus' && nomDe(p.url) === cible;
            if (passe === 'focus' && !veutFocus) continue;
            await cdp.evalBorne(sid, `(() => {
                if (!Object.getOwnPropertyDescriptor(document, 'hidden')) {
                    Object.defineProperty(document, 'hidden', { configurable: true, get: () => window.__cachee });
                    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
                }
                document.hasFocus = () => window.__focalisee;
                window.__cachee = false;
                window.__focalisee = ${veutFocus};
                document.dispatchEvent(new Event('visibilitychange'));
                window.dispatchEvent(new Event(${veutFocus ? "'focus'" : "'blur'"}));
                return document.hidden + '/' + document.hasFocus();
            })()`, 6000, false);
        }
    }
    for (const [sid, p] of ordre) {
        const r = await cdp.evalBorne(sid, `document.hidden + '/' + document.hasFocus()`, 6000, false);
        etats.push([nomDe(p.url), r]);
    }
    const focalisees = etats.filter(([, v]) => String(v).endsWith('/true')).map(([k]) => k);
    log(`SCÉNARIO IMPOSÉ (${etiquette}) cible=${cible} hidden/hasFocus=` + JSON.stringify(Object.fromEntries(etats)));
    log(`  → focalisées effectives : ${JSON.stringify(focalisees)} (une seule attendue)`);
    return { etats: Object.fromEntries(etats), focalisees };
}

/// Deltas par fenêtre entre deux relevés. RELEVÉS ; les taux sont CALCULÉS.
/// `octets_video_cumules` : la somme des DELTAS de `bytesReceived` — le
/// proxy de `bytesSent`, voir la note méthodologique en tête de fichier.
function deltas(a, b) {
    const parFenetre = {};
    for (const k of Object.keys(b)) {
        const av = a[k], ap = b[k];
        if (!av || !ap || typeof av.images_decodees !== 'number' || typeof ap.images_decodees !== 'number') {
            parFenetre[k] = null; continue;
        }
        const dt = (ap.horloge - av.horloge) / 1000;
        const octets = ap.octets_recus - av.octets_recus;
        const pl = (ap.paquets_perdus ?? 0) - (av.paquets_perdus ?? 0);
        const pr = (ap.paquets_recus ?? 0) - (av.paquets_recus ?? 0);
        parFenetre[k] = {
            secondes: Number(dt.toFixed(3)),
            octets_video: octets,
            mbps: Number((octets * 8 / dt / 1e6).toFixed(3)),
            octets_audio: (ap.octets_audio ?? 0) - (av.octets_audio ?? 0),
            paquets_perdus: pl, paquets_recus: pr,
            pc_paquets_perdus: (pr + pl) > 0 ? Number((100 * pl / (pr + pl)).toFixed(4)) : null,
            taille: `${ap.l}x${ap.h}`, rtt: ap.rtt,
            lien_bitrate: ap.lien_bitrate, lien_taille: ap.lien_taille,
            cachee: ap.cachee, focalisee: ap.focalisee,
        };
    }
    const vals = Object.values(parFenetre).filter(Boolean);
    const somme = (f) => vals.reduce((s, x) => s + (f(x) ?? 0), 0);
    const cumule = {
        fenetres_mesurees: vals.length,
        octets_video_cumules: somme((x) => x.octets_video),
        mbps_cumule: Number(somme((x) => x.mbps).toFixed(3)),
        paquets_perdus: somme((x) => x.paquets_perdus),
        paquets_recus: somme((x) => x.paquets_recus),
        pc_paquets_perdus_global: (somme((x) => x.paquets_recus) + somme((x) => x.paquets_perdus)) > 0
            ? Number((100 * somme((x) => x.paquets_perdus)
                / (somme((x) => x.paquets_recus) + somme((x) => x.paquets_perdus))).toFixed(4)) : null,
    };
    return { parFenetre, cumule };
}

// ---------------------------------------------------------------- ouverture des fenêtres
async function ouvrirFenetre(n) {
    const avant = appPages().length;
    log(`  · ouverture fenêtre ${n}`);
    vmIt(`ouvrird9c3-${ETIQUETTE}-${n}`, [
        `$a = @(`,
        `  "--app=file:///C:/dev/anim-d4.html?n=${n}",`,
        `  "--user-data-dir=C:\\dev\\chrome-d9c3-${ETIQUETTE}-${n}",`,
        `  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',`,
        `  '--window-size=1280,720','--window-position=${30 + n * 12},${30 + n * 12}',`,
        `  '--autoplay-policy=no-user-gesture-required',`,
        `  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',`,
        `  '--disable-renderer-backgrounding')`,
        `Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a`,
        `Start-Sleep -Seconds 2`,
    ].join('\n'));
    for (let i = 0; i < 30; i += 1) {
        await dodo(2000);
        if (appPages().length > avant) return true;
    }
    log(`  !! fenêtre ${n} : aucune page de plus après 60 s`);
    return false;
}

function cpuHote(etiquette) {
    const charge = spawnSync('bash', ['-c', 'cat /proc/loadavg'], { encoding: 'utf8' }).stdout?.trim() ?? '';
    log(`CPU HÔTE (${etiquette}) loadavg=${charge}`);
    return { loadavg: charge };
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9997);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d9c3-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns', '--disable-popup-blocking',
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', 'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} MODE=${MODE} BUDGET_BPS=${BUDGET_BPS} BITRATE=${BITRATE} N_EVEIL=${N_EVEIL} `
        + `PALIER_FOCUS_S=${PALIER_FOCUS_S} PALIER_AB_S=${PALIER_AB_S} PART_SONDAGE=${PART_SONDAGE_VU} chrome pid=${chrome.pid}`);

    const releve = {
        etiquette: ETIQUETTE, mode: MODE, budget_bps: Number(BUDGET_BPS), bitrate_par_enfant: Number(BITRATE),
        n_eveil: N_EVEIL, part_sondage: PART_SONDAGE_VU, phases: {},
    };
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
                log(`+ page attachée  session=${sessionId.slice(0, 8)} url=${targetInfo.url}`);
                await cdp.send('Page.enable', {}, sessionId).catch(() => { });
                await cdp.send('Runtime.enable', {}, sessionId).catch(() => { });
                await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: AMORCE }, sessionId).catch(() => { });
                await cdp.send('Runtime.evaluate', { expression: AMORCE, returnByValue: true }, sessionId).catch(() => { });
                await cdp.send('Emulation.setDeviceMetricsOverride',
                    { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false }, sessionId).catch(() => { });
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

        log('>>> ÉTAPE 0 : purge, VM sans fenêtre ni agent rescapé');
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
            `cd ${RACINE} && SUPERVISEUR=1 BITRATE=${BITRATE} BUDGET_BPS=${BUDGET_BPS} ` +
            `SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        await dodo(6000);

        // ---------------------------------------------------- CONSTRUCTION : N_EVEIL fenêtres
        log(`>>> CONSTRUCTION — montée à ${N_EVEIL} fenêtres (toutes doivent rester ÉVEILLÉES)`);
        let ouvertes = 0;
        for (let n = 1; n <= N_EVEIL; n += 1) {
            if (!(await ouvrirFenetre(n))) break;
            ouvertes = n;
            // Réimposé à CHAQUE ouverture : sans cela la fenêtre neuve serait
            // la seule que le navigateur juge focalisée (piège D5/D6).
            await imposerScenario(cdp, nomsTries()[0], `après ouverture ${n}`);
        }
        log(`  ${ouvertes} fenêtres ouvertes`);
        if (ouvertes !== N_EVEIL) log(`  !! ATTENTION : ${ouvertes}/${N_EVEIL} seulement — mesure sur un effectif réduit`);
        await dodo(6000);
        const cible1 = nomsTries()[0];
        const scen1 = await imposerScenario(cdp, cible1, 'construction — focus initial');
        await dodo(8000);
        const pose1 = await attendreBarreauxStables(CALME_S, STABILISATION_MAX_S);
        releve.budget_annonce = await budgetAnnonce();
        releve.construction = { ouvertes, cible1, scenario: scen1, pose: pose1, survie: vmVivante('après construction') };

        if (MODE === 'ab') {
            // ---------------------------------------------------- A/B : mesure du trafic cumulé
            log(`>>> A/B — mesure du trafic vidéo cumulé sur ${PALIER_AB_S} s (PART_SONDAGE=${PART_SONDAGE_VU})`);
            const debut = new Date().toISOString();
            const a = await statsToutes(cdp, 'A/B — début du palier');
            await dodo(PALIER_AB_S * 1000);
            const b = await statsToutes(cdp, 'A/B — fin du palier');
            const fin = new Date().toISOString();
            const d = deltas(a, b);
            const cad = null;
            log('A/B — PAR FENÊTRE ' + JSON.stringify(d.parFenetre, null, 1));
            log('A/B — CUMULÉ ' + JSON.stringify(d.cumule));
            releve.phases.ab = { debut, fin, ...d, survie: vmVivante('après A/B') };
        } else {
            // ---------------------------------------------------- FOCUS : rejeu du critère ④ à palier long
            log(`>>> FOCUS — deux déplacements à PALIER_FOCUS_S=${PALIER_FOCUS_S} s (rejeu du critère ④ de D6)`);
            releve.phases.focus = [];
            const noms = nomsTries();
            for (const cible of [noms[1], noms[2]].filter(Boolean)) {
                const scen = await imposerScenario(cdp, cible, `focus sur ${cible}`);
                await dodo(PALIER_FOCUS_S * 1000);
                const fin = new Date().toISOString();
                const s = await statsToutes(cdp, `focus sur ${cible}`);
                const parts = await partsAgent(fin);
                const barr = await barreaux();
                // Promotion : la focalisée est-elle strictement au-dessus,
                // en AIRE, des autres fenêtres éveillées à ce même relevé ?
                // Comparaison de TAILLES relevées, JAMAIS un compte de lignes
                // de journal (piège n°2 ci-dessus).
                const aire = (v) => (Number(v?.l) || 0) * (Number(v?.h) || 0);
                const entreesFocalisee = Object.entries(s).filter(([, v]) => v?.focalisee === true);
                const entreesAutres = Object.entries(s).filter(([, v]) => v?.focalisee === false);
                const aireFocalisee = entreesFocalisee.length ? Math.max(...entreesFocalisee.map(([, v]) => aire(v))) : null;
                const aireMaxAutres = entreesAutres.length ? Math.max(...entreesAutres.map(([, v]) => aire(v))) : null;
                const promotion = (aireFocalisee !== null && aireMaxAutres !== null)
                    ? aireFocalisee > aireMaxAutres : null;
                const tailles = Object.fromEntries(Object.entries(s).map(([k, v]) =>
                    [k, { taille: `${v?.l}x${v?.h}`, aire: aire(v), lien_taille: v?.lien_taille,
                          lien_bitrate: v?.lien_bitrate, focalisee: v?.focalisee }]));
                log(`FOCUS ${cible} — TAILLES ` + JSON.stringify(tailles));
                log(`FOCUS ${cible} — PARTS somme=${parts.somme} ` + JSON.stringify(parts.dernieres));
                log(`FOCUS ${cible} — PROMOTION aire_focalisee=${aireFocalisee} aire_max_autres=${aireMaxAutres} promotion=${promotion}`);
                releve.phases.focus.push({
                    cible, scenario: scen, fin, tailles, parts,
                    aire_focalisee: aireFocalisee, aire_max_autres: aireMaxAutres, promotion,
                    barreaux_cumules_a_date: barr.changees.length,
                });
            }
            log('>>> FOCUS — survie VM après la phase');
            releve.survie_apres_focus = vmVivante('après phase focus');
        }

        releve.marqueurs = await marqueurs('FIN');
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
