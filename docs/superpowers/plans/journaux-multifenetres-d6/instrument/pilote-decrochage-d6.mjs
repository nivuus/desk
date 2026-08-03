#!/usr/bin/env node
// Sous-bloc D6, tâche 1bis — OÙ LA CHARGE AGRÉGÉE DÉCROCHE, et si un barreau
// plus bas la sauve.
//
// Dérivé de `pilote-lien-d6.mjs` (tâche 1), lui-même dérivé de
// `pilote-recette-d5.mjs`. Ce qui change : une montée en N (rangs
// configurables), un palier par rang dont on prend les compteurs en DELTA, et
// la lecture des cadences côté agent restreinte à la fenêtre temporelle du
// palier.
//
// LE RACCOURCI QUI REND CE MONTAGE FIDÈLE : `BITRATE` est hérité tel quel par
// chaque enfant (`superviseur/lanceur.rs`). Poser `BITRATE = B/N` simule donc
// EXACTEMENT ce que D6 donnerait avec des parts égales. Ce qui n'y est pas :
// la majoration de la fenêtre au premier plan (`FACTEUR_FOCUS`).
//
// Contraintes de protocole héritées, chacune ayant coûté une exécution :
//   1. la source BOUGE (page canvas animée, un `--user-data-dir` par fenêtre) ;
//   2. le navigateur est lancé AVANT le superviseur ;
//   3. `--disable-popup-blocking` et les trois drapeaux anti-gel ;
//   4. AUCUNE capture d'écran CDP pendant la séquence ;
//   5. toute évaluation CDP sur une page portant un flux WebRTC est BORNÉE ;
//   6. l'instance de navigateur pilotée est vérifiée par PID écoutant ;
//   7. survie de la VM contrôlée après chaque rang ;
//   8. `agent.log` est copié APRÈS la fin réelle.
//
// L'INSTRUMENT LE PLUS LOURD, hérité de D5 et OBLIGATOIRE ICI : **un Chrome
// sans interface rapporte `document.hidden = true` pour toute fenêtre
// d'arrière-plan**. Sans l'override, le vivier de D5 endormirait la plupart des
// fenêtres et l'on mesurerait le sommeil, pas la charge. La visibilité annoncée
// au produit est donc celle du SCÉNARIO — huit fenêtres visibles —, imposée
// page par page (`Page.addScriptToEvaluateOnNewDocument` ne court pas toujours
// sur une page ouverte par `window.open`).

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
const AIDE = process.env.AIDE ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d6/instrument');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const COPIE_LOG = process.env.COPIE_LOG ?? `/tmp/agent-decrochage-${ETIQUETTE}.log`;
const SORTIE_JSON = process.env.SORTIE_JSON
    ?? join(RACINE, `docs/superpowers/plans/journaux-multifenetres-d6/decrochage-${ETIQUETTE}.json`);
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
const BITRATE = process.env.BITRATE ?? '10000000';
const RANGS = (process.env.RANGS ?? '1,2,4,8').split(',').map(Number);
const PALIER_S = Number(process.env.PALIER_S ?? 35);

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

/// Les lignes `cadence du capteur` de l'agent, restreintes à la fenêtre
/// temporelle du palier — sans quoi on mélangerait les rangs. Une ligne toutes
/// les 10 s par fenêtre.
async function cadencesAgent(debutIso, finIso) {
    const plat = await journalPlat();
    const out = { capteur: {}, enfant: {}, endormies: [] };
    for (const l of plat.split('\n')) {
        const t = l.match(/(\d{4}-\d\d-\d\dT[\d:.]+Z)/)?.[1];
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

/// Les changements de taille d'encodage : c'est la PREUVE que le barreau a
/// bougé. Un `BITRATE` plus bas à surface constante ne répondrait pas à la
/// question du relevé B.
async function barreaux() {
    const plat = await journalPlat();
    const ok = [], ko = [];
    for (const l of plat.split('\n')) {
        const t = l.match(/(\d{4}-\d\d-\d\dT[\d:.]+Z)/)?.[1] ?? '?';
        const L = l.match(/largeur=(\d+)/)?.[1], H = l.match(/hauteur=(\d+)/)?.[1];
        if (l.includes("taille d'encodage changée")) ok.push({ t, taille: `${L}x${H}` });
        else if (l.includes("changement de taille d'encodage refusé")) ko.push({ t, taille: `${L}x${H}` });
    }
    return { changees: ok, refusees: ko };
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

  // La visibilité vient du SCÉNARIO, pas du navigateur — voir l'en-tête.
  window.__cachee = false;
  Object.defineProperty(document, 'hidden', {
    configurable: true, get: () => window.__cachee });
  Object.defineProperty(document, 'visibilityState', {
    configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
})();
`;

const pages = new Map();
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));
const nomDe = (url) => url.replace(/^.*\?session=/, 'w:');

const STATS = `(async () => {
  const pc = window.__pc;
  if (!pc) return { pc: null };
  const r = await pc.getStats(); const t = [...r.values()];
  const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
  const paire = t.find(x => x.type === 'candidate-pair' && (x.selected || x.nominated));
  const lien = window.__liens.length ? window.__liens[window.__liens.length - 1] : null;
  return {
    etat: pc.iceConnectionState, horloge: Date.now(),
    images_decodees: v?.framesDecoded ?? null,
    images_recues: v?.framesReceived ?? null,
    images_perdues: v?.framesDropped ?? null,
    gels: v?.freezeCount ?? null,
    octets_recus: v?.bytesReceived ?? null,
    paquets_recus: v?.packetsReceived ?? null,
    paquets_perdus: v?.packetsLost ?? null,
    nack: v?.nackCount ?? null, pli: v?.pliCount ?? null,
    temps_decodage_total: v?.totalDecodeTime ?? null,
    l: v?.frameWidth ?? null, h: v?.frameHeight ?? null,
    rtt: paire?.currentRoundTripTime ?? null,
    lien_bitrate: lien?.bitrate ?? null,
    lien_taille: lien ? (lien.w + 'x' + lien.h) : null,
    lien_qualite: lien?.quality ?? null,
    cachee: document.hidden,
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

/// Voir l'en-tête : sans cela le vivier de D5 endormirait les fenêtres
/// d'arrière-plan et l'on mesurerait le sommeil au lieu de la charge.
async function imposerVisibles(cdp, etiquette) {
    const etats = [];
    for (const [sid, p] of appPages()) {
        const r = await cdp.evalBorne(sid, `(() => {
            if (!Object.getOwnPropertyDescriptor(document, 'hidden')) {
                Object.defineProperty(document, 'hidden', {
                    configurable: true, get: () => window.__cachee });
                Object.defineProperty(document, 'visibilityState', {
                    configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
            }
            window.__cachee = false;
            document.dispatchEvent(new Event('visibilitychange'));
            return document.hidden;
        })()`, 6000, false);
        etats.push([nomDe(p.url), r]);
    }
    log(`VISIBILITÉ IMPOSÉE (${etiquette}) document.hidden=` + JSON.stringify(Object.fromEntries(etats)));
}

/// Charge de l'HÔTE. **`tdarr-ffmpeg` contaminait déjà le témoin de la tâche 1**
/// : on le relève NOMMÉMENT à chaque rang, pour pouvoir dire s'il a bougé.
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

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9994);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d6b-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns', '--disable-popup-blocking',
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', 'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} BITRATE=${BITRATE} RANGS=${RANGS} PALIER_S=${PALIER_S} chrome pid=${chrome.pid}`);

    const releve = { etiquette: ETIQUETTE, bitrate_par_fenetre: Number(BITRATE), palier_s: PALIER_S, rangs: [] };
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
            `cd ${RACINE} && SUPERVISEUR=1 BITRATE=${BITRATE} SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 ` +
            `RUST_LOG=info scripts/run-agent.sh`], { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        await dodo(6000);

        const ouvrirFenetre = async (n) => {
            const avant = appPages().length;
            log(`  · ouverture fenêtre ${n}`);
            vmIt(`ouvrird6b-${n}`, [
                `$a = @(`,
                `  "--app=file:///C:/dev/anim-d4.html?n=${n}",`,
                `  "--user-data-dir=C:\\dev\\chrome-d6b-${n}",`,
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

        let ouvertes = 0;
        for (const N of RANGS) {
            log(`>>> RANG N=${N} (BITRATE=${BITRATE} par fenêtre, ${Number(BITRATE) * N / 1e6} Mb/s cumulés visés)`);
            for (let n = ouvertes + 1; n <= N; n += 1) {
                const ok = await ouvrirFenetre(n);
                if (!ok) break;
                ouvertes = n;
            }
            await dodo(6000);
            await imposerVisibles(cdp, `rang ${N}`);
            await dodo(4000);

            const debutIso = new Date().toISOString();
            const a = await statsToutes(cdp, `rang ${N} — début du palier`);
            const cpu0 = cpuHote(`rang ${N}, début du palier`);
            await dodo(PALIER_S * 1000);
            const b = await statsToutes(cdp, `rang ${N} — fin du palier`);
            const cpu1 = cpuHote(`rang ${N}, fin du palier`);
            const finIso = new Date().toISOString();

            // Deltas par fenêtre. RELEVÉS ; les taux qui s'en déduisent sont CALCULÉS.
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
                    paquets_perdus: pl, paquets_recus: pr,
                    pc_paquets_perdus: (pr + pl) > 0 ? Number((100 * pl / (pr + pl)).toFixed(4)) : null,
                    temps_decodage: Number(dec_t.toFixed(3)),
                    pc_decodage: Number((100 * dec_t / dt).toFixed(1)),
                    gels: (ap.gels ?? 0) - (av.gels ?? 0),
                    pli: (ap.pli ?? 0) - (av.pli ?? 0),
                    taille: `${ap.l}x${ap.h}`,
                    rtt: ap.rtt, lien_bitrate: ap.lien_bitrate, lien_taille: ap.lien_taille,
                    lien_qualite: ap.lien_qualite, cachee: ap.cachee,
                };
            }
            const vals = Object.values(parFenetre).filter(Boolean);
            const somme = (f) => Number(vals.reduce((s, x) => s + (f(x) ?? 0), 0).toFixed(3));
            const px = vals.reduce((s, x) => {
                const [L, H] = String(x.taille).split('x').map(Number);
                return s + (Number.isFinite(L) && Number.isFinite(H) ? L * H * x.i_par_s : 0);
            }, 0);
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
                pc_paquets_perdus: Number((100 * somme((x) => x.paquets_perdus)
                    / (somme((x) => x.paquets_recus) + somme((x) => x.paquets_perdus))).toFixed(4)),
                pc_decodage_cumule: somme((x) => x.pc_decodage),
                mpx_par_s_decodes: Number((px / 1e6).toFixed(2)),
                gels: somme((x) => x.gels),
            };
            const cad = await cadencesAgent(debutIso, finIso);
            const barr = await barreaux();
            log(`RANG ${N} — PAR FENÊTRE ` + JSON.stringify(parFenetre, null, 1));
            log(`RANG ${N} — CUMULÉ ` + JSON.stringify(cumule));
            log(`RANG ${N} — AGENT ` + JSON.stringify(cad));
            log(`RANG ${N} — BARREAUX changés=${barr.changees.length} refusés=${barr.refusees.length} `
                + JSON.stringify(barr.changees.slice(-4)));
            const survie = vmVivante(`après rang ${N}`);
            releve.rangs.push({
                N, ouvertes, debutIso, finIso, parFenetre, cumule, agent: cad,
                barreaux: barr, cpu: [cpu0, cpu1], survie,
            });
            if (ouvertes < N) { log(`>>> ARRÊT : ${ouvertes} fenêtres ouvertes pour un rang ${N}`); break; }
        }

        releve.marqueurs = await marqueurs('FIN');
        await writeFile(SORTIE_JSON, JSON.stringify(releve, null, 1));
        log('relevé écrit dans ' + SORTIE_JSON);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
        await dodo(6000);
        copierLog();
        log('journal copié dans ' + COPIE_LOG + ' (après la fermeture du navigateur)');
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
