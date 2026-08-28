#!/usr/bin/env node
// Sous-bloc D6, tâche 10 — RECETTE du partage de capacité entre N fenêtres.
//
// Dérivé de `pilote-decrochage-d6.mjs` (tâche 1bis), lui-même dérivé de
// `pilote-lien-d6.mjs` (tâche 1) et de `pilote-recette-d5.mjs`.
//
// CE QUI CHANGE PAR RAPPORT À LA TÂCHE 1bis, et pourquoi :
//
//   1. La tâche 1bis SIMULAIT le budget en posant `BITRATE = B/N` sur chaque
//      enfant. Ici le produit fait le travail : `BUDGET_BPS` va au capteur,
//      qui calcule et pousse les parts. On mesure donc le mécanisme, plus son
//      succédané.
//   2. Le FOCUS est imposé page par page, et pas seulement la visibilité.
//      `client/src/main.ts` lit `document.hasFocus()` ; un Chrome sans
//      interface ne le rend vrai que pour au plus une page, et pas
//      nécessairement celle qu'on veut. La règle de part DÉPEND du focus
//      (`FACTEUR_FOCUS`) : sans cet override le critère ④ ne mesurerait rien.
//      Une seule page focalisée à la fois, et on le VÉRIFIE avant de relever.
//   3. Les parts accordées sont relevées NOMMÉMENT dans `agent.log`
//      (`part de budget appliquee session=… part_bps=…`), pour que leur somme
//      soit un relevé et non une reconstruction.
//   4. Trois phases enchaînées dans UNE session d'agent : le palier à huit
//      fenêtres (critères ①②③), deux déplacements de focus (critère ④), puis
//      la montée à dix fenêtres qui en endort deux (critère ⑤).
//
// Contraintes de protocole héritées, chacune ayant coûté une exécution :
//   1. la source BOUGE (page canvas animée, un `--user-data-dir` par fenêtre) ;
//   2. le navigateur est lancé AVANT le superviseur ;
//   3. `--disable-popup-blocking` et les trois drapeaux anti-gel ;
//   4. AUCUNE capture d'écran CDP pendant la séquence ;
//   5. toute évaluation CDP sur une page portant un flux WebRTC est BORNÉE ;
//   6. l'instance de navigateur pilotée est vérifiée par PID écoutant ;
//   7. survie de la VM contrôlée après chaque phase ;
//   8. `agent.log` est copié APRÈS la fin réelle.
//   9. le bandeau d'une page d'application est `#status` ; `#statut` est celui
//      de la page-shell.

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
const AIDE = process.env.AIDE ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d6/instrument');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const COPIE_LOG = process.env.COPIE_LOG ?? `/tmp/agent-recette-${ETIQUETTE}.log`;
const SORTIE_JSON = process.env.SORTIE_JSON
    ?? join(RACINE, `docs/superpowers/plans/journaux-multifenetres-d6/recette-${ETIQUETTE}.json`);
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
// `BUDGET_BPS` est LA variable calibrée par cette recette. `BITRATE` reste le
// plafond par enfant hérité d'avant D6 : on le laisse à sa valeur de produit,
// puisque c'est désormais la part qui borne.
const BUDGET_BPS = process.env.BUDGET_BPS ?? '12000000';
const BITRATE = process.env.BITRATE ?? '12000000';
const N_EVEIL = Number(process.env.N_EVEIL ?? 8);
const N_TOTAL = Number(process.env.N_TOTAL ?? 10);
const PALIER_S = Number(process.env.PALIER_S ?? 40);
const PALIER_FOCUS_S = Number(process.env.PALIER_FOCUS_S ?? 25);
const PALIER_SOMMEIL_S = Number(process.env.PALIER_SOMMEIL_S ?? 30);
// Le palier ne commence qu'une fois l'echelle POSEE : `CALME_S` secondes sans
// aucun changement de barreau, dans la limite de `STABILISATION_MAX_S`.
const CALME_S = Number(process.env.CALME_S ?? 20);
const STABILISATION_MAX_S = Number(process.env.STABILISATION_MAX_S ?? 150);

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(7);
    console.log(`[${dt}s ${new Date().toISOString()}] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- VM
function vmIt(nom, ps) {
    const r = spawnSync('bash', [join(AIDE, 'vm-it.sh'), nom, ps], { encoding: 'utf8', env: process.env });
    if (r.status !== 0) log(`!! vm-it ${nom} a échoué`, (r.stderr ?? '').slice(0, 400));
    return r;
}
function vmItFichier(nom, fichierPs) {
    const ps = spawnSync('cat', [join(AIDE, fichierPs)], { encoding: 'utf8' }).stdout ?? '';
    return vmIt(nom, ps);
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
async function journalPlat() {
    copierLog();
    let texte = '';
    try { texte = await readFile(COPIE_LOG, 'utf8'); } catch { }
    return texte.replace(/\x1b\[[0-9;]*m/g, '');
}
const horodate = (l) => l.match(/(\d{4}-\d\d-\d\dT[\d:.]+Z)/)?.[1];

/// LE CONTRÔLE QUE LA VARIABLE EST ARRIVÉE. Ce dépôt a payé trois fois le
/// piège de la variable non transmise (`SUPERVISEUR` en D1,
/// `MULTIFENETRE_REPRISE` en D2, `BUDGET_BPS` aurait été le quatrième). On
/// compare la VALEUR, pas la seule présence de la ligne : son absence dirait
/// que la variable n'est pas arrivée, pas que le budget est absent.
///
/// ⚠️ **À N'APPELER QU'APRÈS LA PREMIÈRE FENÊTRE.** `budget_bps()`
/// (`capteur/sommeil/parts.rs`) est un `OnceLock` : la trace ne part qu'au
/// PREMIER calcul de parts, donc à la première inscription de fenêtre. Appelé
/// juste après le lancement du superviseur, ce contrôle rend `[]` — et ce
/// vide ne veut rien dire. Les cinq exécutions de la recette ont toutes
/// journalisé `[]` à cet endroit, alors que la valeur était bel et bien
/// arrivée (vérifiée après coup dans les cinq `agent.log`).
async function budgetAnnonce() {
    const plat = await journalPlat();
    const vues = [...plat.matchAll(/budget de debit de la session budget_bps=(\d+)/g)].map((m) => Number(m[1]));
    log(`CONTRÔLE BUDGET : lignes="budget de debit de la session" valeurs=${JSON.stringify(vues)} attendu=${BUDGET_BPS}`);
    return vues;
}

/// Les parts accordées, PAR SESSION (le champ `session` a été ajouté à la
/// trace pour cette recette : sans lui la somme ne serait pas attribuable).
/// On rend la DERNIÈRE part vue pour chaque session avant `finIso`.
async function partsAgent(finIso) {
    const plat = await journalPlat();
    const dernieres = {}, toutes = [];
    for (const l of plat.split('\n')) {
        if (!l.includes('part de budget appliquee')) continue;
        const t = horodate(l);
        if (!t || (finIso && t > finIso)) continue;
        const s = l.match(/session=(\S+)/)?.[1];
        const p = Number(l.match(/part_bps=(\d+)/)?.[1]);
        if (!s || !Number.isFinite(p)) continue;
        dernieres[s] = { bps: p, t };
        toutes.push({ s, bps: p, t });
    }
    const somme = Object.values(dernieres).reduce((a, x) => a + x.bps, 0);
    return { dernieres, somme, nombre_de_lignes: toutes.length };
}

/// Les lignes `cadence du capteur` de l'agent, restreintes à une fenêtre
/// temporelle. Une ligne toutes les 10 s par fenêtre.
async function cadencesAgent(debutIso, finIso) {
    const plat = await journalPlat();
    const out = { capteur: {}, enfant: {}, endormies: [] };
    for (const l of plat.split('\n')) {
        const t = horodate(l);
        if (!t || t < debutIso || t > finIso) continue;
        const s = l.match(/session=(\S+)/)?.[1];
        const c = l.match(/cadence="([\d.]+)"/)?.[1];
        if (!s || !c) continue;
        if (l.includes('cadence du capteur')) {
            (out.capteur[s] ??= []).push(Number(c));
            if (l.includes('endormie=true')) out.endormies.push(`${s}@${t}`);
        } else if (l.includes('cadence de la piste vidéo')) {
            (out.enfant[s] ??= []).push(Number(c));
        }
    }
    const moy = (o) => Object.fromEntries(Object.entries(o).map(([k, v]) =>
        [k, Number((v.reduce((a, b) => a + b, 0) / v.length).toFixed(2))]));
    return {
        capteur: moy(out.capteur), enfant: moy(out.enfant),
        capteur_cumule: Number(Object.values(moy(out.capteur)).reduce((a, b) => a + b, 0).toFixed(2)),
        endormies: out.endormies,
    };
}

/// Les changements de taille d'encodage : la PREUVE que le barreau a bougé.
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

/// Attend que l'échelle se soit POSÉE — le fait, jamais une durée (leçon de
/// D3). Tant qu'une `taille d'encodage changée` est apparue depuis moins de
/// `calmeS` secondes, on continue d'attendre, dans la limite de `maxS`.
///
/// **Pourquoi cette attente existe.** La première exécution
/// (`critere1-budget12.log`) a mesuré son palier PENDANT la descente : le
/// dernier changement de barreau tombe 4 s avant la fin du palier, et le taux
/// d'images jetées y mêle donc le régime établi et le transitoire de descente
/// depuis 1280×720. La part arrive après l'attache de la fenêtre — ce que le
/// montage de la tâche 1bis n'avait pas, `BITRATE` y étant posé au lancement
/// de l'enfant.
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
            log(`ÉCHELLE POSÉE après ${attendu} s (${dernierCompte} changements cumulés, `
                + `${calmeS} s sans changement)`);
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
        reveillee: compte('fenêtre réveillée'),
        part_appliquee: compte('part de budget appliquee'),
        taille_changee: compte("taille d'encodage changée"),
        taille_refusee: compte("changement de taille d'encodage refusé"),
        refus_debit: compte("l'encodeur refuse le réglage du débit à chaud"),
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

// L'amorce pose les deux overrides — visibilité ET focus — plus la capture des
// messages `link`. `Page.addScriptToEvaluateOnNewDocument` ne court PAS sur une
// page ouverte par `window.open` (éprouvé en D5) : elle est REPOSÉE
// explicitement page par page, ce que fait `imposerScenario`.
const AMORCE = `
(() => {
  if (window.__amorceD6) return;
  window.__amorceD6 = true;
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

  // Visibilité ET focus viennent du SCÉNARIO, pas du navigateur.
  window.__cachee = false;
  window.__focalisee = false;
  Object.defineProperty(document, 'hidden', {
    configurable: true, get: () => window.__cachee });
  Object.defineProperty(document, 'visibilityState', {
    configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
  document.hasFocus = () => window.__focalisee;
})();
`;

const pages = new Map();
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));
const nomDe = (url) => url.replace(/^.*\?session=/, 'w:');
// Tri NUMÉRIQUE sur le suffixe : `w-10` précède `w-2` en ordre lexical, et le
// choix de la fenêtre focalisée deviendrait alors imprévisible d'une phase à
// l'autre. Un Bloc-notes fait avancer le compteur de deux (leçon de D4) : les
// numéros ne sont ni contigus ni garantis, seul leur ORDRE l'est.
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
    images_perdues: v?.framesDropped ?? null,
    gels: v?.freezeCount ?? null,
    octets_recus: v?.bytesReceived ?? null,
    octets_audio: a?.bytesReceived ?? null,
    paquets_recus: v?.packetsReceived ?? null,
    paquets_perdus: v?.packetsLost ?? null,
    nack: v?.nackCount ?? null, pli: v?.pliCount ?? null,
    temps_decodage_total: v?.totalDecodeTime ?? null,
    l: v?.frameWidth ?? null, h: v?.frameHeight ?? null,
    rtt: paire?.currentRoundTripTime ?? null,
    lien_bitrate: lien?.bitrate ?? null,
    lien_taille: lien ? (lien.w + 'x' + lien.h) : null,
    lien_qualite: lien?.quality ?? null,
    cachee: document.hidden, focalisee: document.hasFocus(),
    bandeau: document.querySelector('#status')?.textContent ?? null,
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
///
/// Sans cela, deux mesures seraient fausses à la fois : le vivier de D5
/// endormirait les fenêtres d'arrière-plan (on mesurerait le sommeil au lieu
/// de la charge — piège hérité de D5), et le répartiteur de D6 majorerait une
/// fenêtre arbitraire, voire aucune (le critère ④ ne mesurerait rien).
///
/// Le `dernier` de `client/src/visibilite.ts` déduplique l'annonce : on émet
/// donc `blur` sur TOUTES les pages — la cible comprise — avant `focus` sur la
/// cible seule.
///
/// **Pourquoi la cible passe elle aussi par `blur`, alors que c'est
/// contre-intuitif.** Relevé à la première exécution (`critere1-budget12.log`,
/// phase 3) : une page tout juste ouverte annonce `focalisee=true` si
/// `main.ts` s'attache avant que l'amorce n'ait posé l'override — la course
/// que `Page.addScriptToEvaluateOnNewDocument` perd sur une page issue de
/// `window.open`, déjà éprouvée en D5. Le capteur retient alors CETTE page
/// comme focalisée ; le `blur` qu'on lui envoie ensuite vide le champ, et la
/// cible, dont l'état n'a pas changé, ne réémet RIEN à cause de la
/// déduplication. Résultat mesuré : plus aucune fenêtre focalisée, et une
/// majoration de focus absente du calcul (parts toutes égales à `reste/8` au
/// lieu de `reste/9`). Faire passer la cible par `blur` puis `focus` garantit
/// un changement d'état, donc une annonce.
async function imposerScenario(cdp, cible, etiquette) {
    const etats = [];
    const ordre = appPages();
    for (const passe of ['blur', 'focus']) {
        for (const [sid, p] of ordre) {
            const veutFocus = passe === 'focus' && nomDe(p.url) === cible;
            if (passe === 'focus' && !veutFocus) continue;
            await cdp.evalBorne(sid, `(() => {
                if (!Object.getOwnPropertyDescriptor(document, 'hidden')) {
                    Object.defineProperty(document, 'hidden', {
                        configurable: true, get: () => window.__cachee });
                    Object.defineProperty(document, 'visibilityState', {
                        configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
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

function cpuHote(etiquette) {
    const cpu = spawnSync('bash', ['-c',
        `top -b -n 3 -d 2 | grep -E '^%Cpu' | sed 's/^/    /'`], { encoding: 'utf8' }).stdout ?? '';
    const tdarr = spawnSync('bash', ['-c',
        `ps -o pcpu= -C tdarr-ffmpeg --no-headers 2>/dev/null | paste -sd, - ; echo -n ''`],
        { encoding: 'utf8' }).stdout?.trim() ?? '';
    const charge = spawnSync('bash', ['-c', 'cat /proc/loadavg'], { encoding: 'utf8' }).stdout?.trim() ?? '';
    const r = { cpu: cpu.trim(), tdarr_pcpu: tdarr || 'absent', loadavg: charge };
    log(`CPU HÔTE (${etiquette})\n${cpu.trimEnd()}\n    tdarr-ffmpeg %CPU=${r.tdarr_pcpu}  loadavg=${charge}`);
    return r;
}

/// Deltas par fenêtre entre deux relevés. RELEVÉS ; les taux sont CALCULÉS.
function deltas(a, b) {
    const parFenetre = {};
    for (const k of Object.keys(b)) {
        const av = a[k], ap = b[k];
        if (!av || !ap || typeof av.images_decodees !== 'number' || typeof ap.images_decodees !== 'number') {
            parFenetre[k] = null; continue;
        }
        const dt = (ap.horloge - av.horloge) / 1000;
        const rec = ap.images_recues - av.images_recues;
        const dec = ap.images_decodees - av.images_decodees;
        const jet = (ap.images_perdues ?? 0) - (av.images_perdues ?? 0);
        const pl = (ap.paquets_perdus ?? 0) - (av.paquets_perdus ?? 0);
        const pr = (ap.paquets_recus ?? 0) - (av.paquets_recus ?? 0);
        const dec_t = ap.temps_decodage_total - av.temps_decodage_total;
        parFenetre[k] = {
            secondes: Number(dt.toFixed(3)),
            images_recues: rec, images_decodees: dec, images_jetees: jet,
            i_par_s: Number((dec / dt).toFixed(2)),
            pc_jetees: rec > 0 ? Number((100 * jet / rec).toFixed(2)) : null,
            mbps: Number(((ap.octets_recus - av.octets_recus) * 8 / dt / 1e6).toFixed(3)),
            kbps_audio: Number((((ap.octets_audio ?? 0) - (av.octets_audio ?? 0)) * 8 / dt / 1e3).toFixed(1)),
            paquets_perdus: pl, paquets_recus: pr,
            pc_paquets_perdus: (pr + pl) > 0 ? Number((100 * pl / (pr + pl)).toFixed(4)) : null,
            temps_decodage: Number(dec_t.toFixed(3)),
            pc_decodage: Number((100 * dec_t / dt).toFixed(1)),
            gels: (ap.gels ?? 0) - (av.gels ?? 0),
            pli: (ap.pli ?? 0) - (av.pli ?? 0),
            taille: `${ap.l}x${ap.h}`,
            rtt: ap.rtt, lien_bitrate: ap.lien_bitrate, lien_taille: ap.lien_taille,
            lien_qualite: ap.lien_qualite, cachee: ap.cachee, focalisee: ap.focalisee,
            bandeau: ap.bandeau,
        };
    }
    const vals = Object.values(parFenetre).filter(Boolean);
    const somme = (f) => Number(vals.reduce((s, x) => s + (f(x) ?? 0), 0).toFixed(3));
    const px = vals.reduce((s, x) => {
        const [L, H] = String(x.taille).split('x').map(Number);
        return s + (Number.isFinite(L) && Number.isFinite(H) ? L * H * x.i_par_s : 0);
    }, 0);
    const rtts = vals.map((x) => x.rtt).filter((x) => typeof x === 'number').sort((a, b) => a - b);
    const cumule = {
        fenetres_mesurees: vals.length,
        mbps_cumule: somme((x) => x.mbps),
        i_par_s_cumule: somme((x) => x.i_par_s),
        images_recues: somme((x) => x.images_recues),
        images_jetees: somme((x) => x.images_jetees),
        pc_jetees: somme((x) => x.images_recues) > 0
            ? Number((100 * somme((x) => x.images_jetees) / somme((x) => x.images_recues)).toFixed(2)) : null,
        paquets_perdus: somme((x) => x.paquets_perdus),
        paquets_recus: somme((x) => x.paquets_recus),
        pc_decodage_cumule: somme((x) => x.pc_decodage),
        mpx_par_s_decodes: Number((px / 1e6).toFixed(2)),
        gels: somme((x) => x.gels),
        rtt_median: rtts.length ? rtts[Math.floor(rtts.length / 2)] : null,
    };
    return { parFenetre, cumule };
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9993);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d6r-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns', '--disable-popup-blocking',
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', 'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} BUDGET_BPS=${BUDGET_BPS} BITRATE=${BITRATE} `
        + `N_EVEIL=${N_EVEIL} N_TOTAL=${N_TOTAL} PALIER_S=${PALIER_S} chrome pid=${chrome.pid}`);

    const releve = {
        etiquette: ETIQUETTE, budget_bps: Number(BUDGET_BPS), bitrate_par_enfant: Number(BITRATE),
        n_eveil: N_EVEIL, n_total: N_TOTAL, palier_s: PALIER_S, phases: {},
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
                if (VIEWPORT_FORCE) {
                    const [L, H] = VIEWPORT_FORCE.split('x').map(Number);
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: L, height: H, deviceScaleFactor: 1, mobile: false }, sessionId).catch(() => { });
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

        log('>>> ÉTAPE 0 : la VM part SANS fenêtre éligible');
        vmItFichier('preparerd6', 'preparer-d6.ps1');
        await dodo(8000);
        releve.cpu_repos = cpuHote('au repos, avant toute session');

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

        const ouvrirFenetre = async (n) => {
            const avant = appPages().length;
            log(`  · ouverture fenêtre ${n}`);
            vmIt(`ouvrird6r-${n}`, [
                `$a = @(`,
                `  "--app=file:///C:/dev/anim-d4.html?n=${n}",`,
                `  "--user-data-dir=C:\\dev\\chrome-d6r-${n}",`,
                `  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',`,
                `  '--window-size=1280,720','--window-position=${30 + n * 12},${30 + n * 12}',`,
                `  '--disable-features=CalculateNativeWinOcclusion',`,
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
        };

        // ---------------------------------------------------- PHASE 1
        log(`>>> PHASE 1 — montée à ${N_EVEIL} fenêtres, palier de ${PALIER_S} s`);
        let ouvertes = 0;
        for (let n = 1; n <= N_EVEIL; n += 1) {
            if (!(await ouvrirFenetre(n))) break;
            ouvertes = n;
            // Le scénario est réimposé À CHAQUE ouverture : sans cela, la
            // fenêtre neuve serait la seule que le navigateur juge focalisée,
            // et les précédentes s'endormiraient (piège de D5).
            await imposerScenario(cdp, nomsTries()[0], `après ouverture ${n}`);
        }
        log(`  ${ouvertes} fenêtres ouvertes`);
        await dodo(6000);
        const cible1 = nomsTries()[0];
        const scen1 = await imposerScenario(cdp, cible1, 'phase 1');
        await dodo(8000);
        const pose1 = await attendreBarreauxStables(CALME_S, STABILISATION_MAX_S);
        // Après la première fenêtre, donc après l'initialisation du `OnceLock`.
        releve.budget_annonce = await budgetAnnonce();

        const debut1 = new Date().toISOString();
        const a1 = await statsToutes(cdp, 'phase 1 — début du palier');
        const cpu10 = cpuHote('phase 1, début du palier');
        await dodo(PALIER_S * 1000);
        const b1 = await statsToutes(cdp, 'phase 1 — fin du palier');
        const cpu11 = cpuHote('phase 1, fin du palier');
        const fin1 = new Date().toISOString();
        const d1 = deltas(a1, b1);
        const parts1 = await partsAgent(fin1);
        const cad1 = await cadencesAgent(debut1, fin1);
        const barr1 = await barreaux();
        log('PHASE 1 — PAR FENÊTRE ' + JSON.stringify(d1.parFenetre, null, 1));
        log('PHASE 1 — CUMULÉ ' + JSON.stringify(d1.cumule));
        log('PHASE 1 — AGENT ' + JSON.stringify(cad1));
        log(`PHASE 1 — PARTS somme=${parts1.somme} budget=${BUDGET_BPS} `
            + `respecte=${parts1.somme <= Number(BUDGET_BPS)} ` + JSON.stringify(parts1.dernieres));
        log(`PHASE 1 — BARREAUX changés=${barr1.changees.length} refusés=${barr1.refusees.length} `
            + JSON.stringify(barr1.changees.slice(-10)));
        releve.phases.palier = {
            cible_focus: cible1, scenario: scen1, ouvertes, debut1, fin1, pose: pose1,
            ...d1, agent: cad1, parts: parts1, barreaux: barr1, cpu: [cpu10, cpu11],
            survie: vmVivante('après phase 1'),
        };

        // ---------------------------------------------------- PHASE 2
        log('>>> PHASE 2 — déplacement du focus (critère ④)');
        releve.phases.focus = [];
        const noms = nomsTries();
        for (const cible of [noms[1], noms[2]].filter(Boolean)) {
            const scen = await imposerScenario(cdp, cible, `focus sur ${cible}`);
            await dodo(PALIER_FOCUS_S * 1000);
            const fin = new Date().toISOString();
            const s = await statsToutes(cdp, `focus sur ${cible}`);
            const parts = await partsAgent(fin);
            const tailles = Object.fromEntries(Object.entries(s).map(([k, v]) =>
                [k, { taille: `${v?.l}x${v?.h}`, lien_taille: v?.lien_taille,
                      lien_bitrate: v?.lien_bitrate, focalisee: v?.focalisee }]));
            log(`FOCUS ${cible} — TAILLES ` + JSON.stringify(tailles));
            log(`FOCUS ${cible} — PARTS somme=${parts.somme} ` + JSON.stringify(parts.dernieres));
            releve.phases.focus.push({ cible, scenario: scen, fin, tailles, parts, stats: s });
        }
        vmVivante('après phase 2');

        // ---------------------------------------------------- PHASE 3
        log(`>>> PHASE 3 — montée à ${N_TOTAL} fenêtres : ${N_TOTAL - N_EVEIL} endormies (critère ⑤)`);
        for (let n = ouvertes + 1; n <= N_TOTAL; n += 1) {
            if (!(await ouvrirFenetre(n))) break;
            ouvertes = n;
        }
        log(`  ${ouvertes} fenêtres ouvertes au total`);
        await dodo(6000);
        // Toutes visibles, focus toujours sur la même : c'est la RÉCENCE qui
        // décide alors du sommeil (vivier LRU de D5), donc les deux plus
        // anciennes s'endorment.
        const cible3 = noms[2] ?? cible1;
        const scen3 = await imposerScenario(cdp, cible3, 'phase 3');
        await dodo(8000);
        const pose3 = await attendreBarreauxStables(CALME_S, STABILISATION_MAX_S);
        const debut3 = new Date().toISOString();
        const a3 = await statsToutes(cdp, 'phase 3 — début du palier');
        await dodo(PALIER_SOMMEIL_S * 1000);
        const b3 = await statsToutes(cdp, 'phase 3 — fin du palier');
        const fin3 = new Date().toISOString();
        const d3 = deltas(a3, b3);
        const parts3 = await partsAgent(fin3);
        const cad3 = await cadencesAgent(debut3, fin3);
        log('PHASE 3 — PAR FENÊTRE ' + JSON.stringify(d3.parFenetre, null, 1));
        log('PHASE 3 — CUMULÉ ' + JSON.stringify(d3.cumule));
        log('PHASE 3 — AGENT ' + JSON.stringify(cad3));
        log(`PHASE 3 — PARTS somme=${parts3.somme} budget=${BUDGET_BPS} ` + JSON.stringify(parts3.dernieres));
        releve.phases.sommeil = {
            cible_focus: cible3, scenario: scen3, ouvertes, debut3, fin3, pose: pose3,
            ...d3, agent: cad3, parts: parts3, cpu: cpuHote('phase 3'),
            survie: vmVivante('après phase 3'),
        };

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
