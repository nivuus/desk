#!/usr/bin/env node
// Sous-bloc D10 (tâche 15) — recette ③ : l'A/B différentiel sur
// `set_desired_bitrate` (leg n°4 de D6, joué une première fois par D9 tâche 16
// sans rien établir : +23,2 % entre bras contre +83,1 % de variance
// intra-bras, le bruit dépassant le signal).
//
// CE QUE CE SCRIPT CHANGE PAR RAPPORT À D9 : le plan d'expérience, pas le
// montage. D9 comparait des MOYENNES de bras sur des exécutions non appariées.
// Ici, chaque appel construit UNE exécution (ARMÉ ou DÉSARMÉ) ; l'appariement
// — jouer ARMÉ puis DÉSARMÉ dos à dos, comparer PAR PAIRE — est fait par le
// script appelant (bash), pas ici : `PART_SONDAGE` est lu UNE SEULE FOIS par
// processus enfant via un `OnceLock` (`transport/part.rs`), donc chaque bras
// exige un cycle complet purge→superviseur→enfants neuf. Ce fichier EST donc
// une seule exécution du protocole D9 MODE=ab, réutilisée telle quelle huit
// fois avec des étiquettes différentes.
//
// Dérivé de :
//  - `journaux-multifenetres-d9/instrument/pilote-critere3.mjs` (MODE=ab :
//    `statsToutes`/`deltas` sur `getStats()`, note méthodologique
//    bytesReceived vs bytesSent, `imposerScenario`, `attendreBarreauxStables`)
//  - `journaux-multifenetres-d10/instrument/pilote-critere1-d10.mjs` et
//    `pilote-critere2-d10.mjs` (scaffolding D10 : `vm-it.sh`, purge qui tue
//    aussi `agent`, lancement du superviseur via `run-agent.sh`, jamais de
//    suivi page↔fenêtre par sid)
//
// ⚠️ NE TOUCHE JAMAIS AU REGISTRE D'AFFICHAGE — aucun `MULTIFENETRE_MODE_SORTIE`
// ici. Le registre de cette VM est déjà sale (voir tâche 10) ; ce sous-bloc
// vérifie seulement que la branche l'encaisse (elle le fait, task 10) et ne
// mesure rien qui en dépende directement ici.
//
// USAGE :
//   ETIQUETTE=arme-1   node pilote-ab-d10.mjs         # bras ARMÉ (défaut)
//   PART_SONDAGE=0 ETIQUETTE=desarme-1 node pilote-ab-d10.mjs   # bras DÉSARMÉ

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

function racineDepot() {
    // Chemin du DEPOT, derive de l'EMPLACEMENT du fichier (jamais du cwd) :
    // patron prescrit, deja employe par pilote-f4.mjs (fileURLToPath).
    // .../docs/superpowers/plans/journaux-*/instrument/<ce-fichier>.mjs
    // remonter 5 niveaux : instrument, journaux-*, plans, superpowers, docs.
    return join(dirname(fileURLToPath(import.meta.url)), '../../../../..');
}
const RACINE = process.env.RACINE ?? racineDepot();
const AIDE = process.env.AIDE ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d10/instrument');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const PORT_SHELL = process.env.PORT_SHELL ?? '5173';
const URL_SHELL = `http://${HOTE}:${PORT_SHELL}/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const SORTIE_DIR = process.env.SORTIE_DIR
    ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d10');
const COPIE_LOG = process.env.COPIE_LOG ?? join(SORTIE_DIR, `agent-ab-${ETIQUETTE}.log`);
const SORTIE_JSON = process.env.SORTIE_JSON ?? join(SORTIE_DIR, `ab-${ETIQUETTE}.json`);
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
const N_FENETRES = Number(process.env.N_FENETRES ?? 8); // vivier::PLAFOND_EVEIL = 8
const BITRATE = process.env.BITRATE ?? '12000000'; // défaut agent (demarrage/source.rs)
const DELAI_ENTRE_FENETRES_MS = Number(process.env.DELAI_ENTRE_FENETRES_MS ?? 8000);
const CALME_S = Number(process.env.CALME_S ?? 15);
const STABILISATION_MAX_S = Number(process.env.STABILISATION_MAX_S ?? 90);
const PALIER_AB_S = Number(process.env.PALIER_AB_S ?? 30);
// Relayé par héritage d'environnement jusqu'à `run-agent.sh` (spawnSync avec
// `env: process.env`) — JAMAIS posé ici. Convention : `PART_SONDAGE=0`
// désarme ; toute autre valeur ou l'absence de la variable arme.
const PART_SONDAGE_VU = process.env.PART_SONDAGE ?? '(absent -> arme)';

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
function winrm(commande) {
    const r = spawnSync('node', [join(RACINE, 'scripts/winrm.js'), commande],
        { encoding: 'utf8', env: process.env, maxBuffer: 16 * 1024 * 1024 });
    if (r.status !== 0) log('!! winrm a échoué', (r.stderr ?? '').slice(0, 400));
    return r.stdout ?? '';
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
async function marqueurs(etiquette) {
    const plat = await journalPlat();
    const compte = (motif) => (plat.match(new RegExp(motif, 'g')) ?? []).length;
    const m = {
        lignes: plat.split('\n').length,
        attachee_capteur: compte('fenêtre attachée au capteur'),
        endormie: compte('fenêtre endormie'),
        part_appliquee: compte('part de budget appliquee'),
        sondage_desarme: compte('objectif de sondage DESARME'),
        taille_changee: compte("taille d'encodage changée"),
        taille_refusee: compte("changement de taille d'encodage refusé"),
        erreurs: compte('ERROR'),
    };
    log(`MARQUEURS (${etiquette}) ` + JSON.stringify(m));
    return m;
}
/// Le contrôle exigé par le brief : la trace de désarmement doit être
/// présente si et seulement si le bras est désarmé.
async function controleSondage() {
    const plat = await journalPlat();
    const lignes = plat.split('\n').filter((l) => l.includes('objectif de sondage DESARME'));
    return { present: lignes.length > 0, occurrences: lignes.length, extrait: lignes.slice(0, 2) };
}

/// Attend que l'échelle se soit POSÉE — le FAIT, jamais une durée (leçon D3,
/// repayée par D6/D8/D9). Comparaison de TAILLES, jamais un compte brut de
/// lignes (chaque changement produit deux lignes, capteur + enfant).
async function attendreBarreauxStables(calmeS, maxS) {
    const debut = Date.now();
    let dernierCompte = -1, dernierChangement = Date.now();
    while ((Date.now() - debut) / 1000 < maxS) {
        const plat = await journalPlat();
        const compte = (plat.match(/taille d'encodage changée/g) ?? []).length;
        if (compte !== dernierCompte) {
            dernierCompte = compte;
            dernierChangement = Date.now();
        } else if ((Date.now() - dernierChangement) / 1000 >= calmeS) {
            const attendu = Number(((Date.now() - debut) / 1000).toFixed(1));
            log(`ÉCHELLE POSÉE après ${attendu} s (${dernierCompte} lignes cumulées, ${calmeS} s sans changement)`);
            return { stable: true, attente_s: attendu, lignes_changement: dernierCompte };
        }
        await dodo(3000);
    }
    const attendu = Number(((Date.now() - debut) / 1000).toFixed(1));
    log(`!! ÉCHELLE NON POSÉE après ${attendu} s (${dernierCompte} lignes cumulées)`);
    return { stable: false, attente_s: attendu, lignes_changement: dernierCompte };
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

// Visibilité ET focus pilotés par le SCÉNARIO, jamais le navigateur — repris
// tel quel de D6/D9 (piège : Chrome sans interface rapporte `hidden=true`
// pour toute page en arrière-plan, ce qui déclencherait le sommeil bien avant
// PLAFOND_EVEIL sans cet override).
const AMORCE = `
(() => {
  if (window.__amorceD10T15) return;
  window.__amorceD10T15 = true;
  window.__pc = null;
  const N = window.RTCPeerConnection;
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
  return {
    etat: pc.iceConnectionState, horloge: Date.now(),
    octets_recus: v?.bytesReceived ?? null,
    paquets_recus: v?.packetsReceived ?? null,
    paquets_perdus: v?.packetsLost ?? null,
    l: v?.frameWidth ?? null, h: v?.frameHeight ?? null,
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
    return { etats: Object.fromEntries(etats), focalisees };
}

/// Deltas par fenêtre entre deux relevés. RELEVÉS ; les taux sont CALCULÉS.
function deltas(a, b) {
    const parFenetre = {};
    for (const k of Object.keys(b)) {
        const av = a[k], ap = b[k];
        if (!av || !ap || typeof av.octets_recus !== 'number' || typeof ap.octets_recus !== 'number') {
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
            paquets_perdus: pl, paquets_recus: pr,
            taille: `${ap.l}x${ap.h}`, cachee: ap.cachee, focalisee: ap.focalisee,
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
    };
    return { parFenetre, cumule };
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9994);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d10t15-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns', '--disable-popup-blocking',
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', 'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} PART_SONDAGE=${PART_SONDAGE_VU} N_FENETRES=${N_FENETRES} BITRATE=${BITRATE} `
        + `PALIER_AB_S=${PALIER_AB_S} chrome pid=${chrome.pid} port=${port}`);

    const releve = {
        etiquette: ETIQUETTE, part_sondage_vu: PART_SONDAGE_VU, n_fenetres: N_FENETRES,
        bitrate: Number(BITRATE), palier_ab_s: PALIER_AB_S,
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
                if (!targetInfo.url.includes('shell.html') && targetInfo.url !== 'about:blank') {
                    const [L, H] = VIEWPORT_FORCE.split('x').map(Number);
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: L, height: H, deviceScaleFactor: 1, mobile: false }, sessionId).catch(() => { });
                }
                await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
            } else if (m.method === 'Target.detachedFromTarget') {
                pages.delete(m.params.sessionId);
            } else if (m.method === 'Target.targetInfoChanged') {
                for (const [, p] of pages) if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
            }
        });
        await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
        await cdp.send('Target.setDiscoverTargets', { discover: true });

        // ---- ÉTAPE 0 : purge (tue AUSSI `agent`, contrairement à D9). ----
        log('>>> ÉTAPE 0 : purge — VM sans fenêtre ni agent résiduel (registre NON touché)');
        vmIt(`purge-t15-${ETIQUETTE}`, [
            "Get-Process notepad, mspaint, wordpad, chrome, agent -ErrorAction SilentlyContinue | Stop-Process -Force",
            "Start-Sleep -Seconds 3",
            "Get-ChildItem 'C:\\dev' -Directory -Filter 'chrome-d10t15-*' -ErrorAction SilentlyContinue |",
            "  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue",
        ].join('\n'));
        await dodo(5000);
        releve.survie_avant_purge = vmVivante('avant lancement, après purge');

        // ---- Page-shell AVANT le superviseur. ----
        await cdp.send('Target.createTarget', { url: URL_SHELL });
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut')?.textContent`));

        // ---- Le superviseur. `PART_SONDAGE` voyage par `process.env`
        // (posé par le script bash appelant AVANT `node pilote-ab-d10.mjs`),
        // relayé tel quel par `spawnSync(..., { env: process.env })`. ----
        log(`>>> lancement du superviseur (SUPERVISEUR=1, PART_SONDAGE=${PART_SONDAGE_VU})`);
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 BITRATE=${BITRATE} `
            + `SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        if ((sup.stderr ?? '').trim()) log('run-agent.sh STDERR :', sup.stderr.trim().replace(/\n/g, ' / '));
        await dodo(6000);
        await marqueurs('superviseur démarré, 0 fenêtre');
        releve.survie_apres_lancement = vmVivante('après lancement superviseur');

        // ---- Construction : N_FENETRES fenêtres, une par une. ----
        log(`>>> CONSTRUCTION — montée à ${N_FENETRES} fenêtres`);
        let ouvertes = 0;
        for (let n = 1; n <= N_FENETRES; n += 1) {
            log(`  · ouverture fenêtre ${n}/${N_FENETRES}`);
            vmIt(`ouvrirt15-${ETIQUETTE}-${n}`, [
                `$a = @(`,
                `  "--app=file:///C:/dev/anim-d4.html?n=${n}",`,
                `  "--user-data-dir=C:\\dev\\chrome-d10t15-${ETIQUETTE}-${n}",`,
                `  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',`,
                `  '--window-size=1280,720','--window-position=${40 + n * 10},${40 + n * 10}',`,
                `  '--disable-features=CalculateNativeWinOcclusion',`,
                `  '--autoplay-policy=no-user-gesture-required',`,
                `  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',`,
                `  '--disable-renderer-backgrounding')`,
                `Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a`,
                `Start-Sleep -Seconds 2`,
            ].join('\n'));
            await dodo(DELAI_ENTRE_FENETRES_MS);
            ouvertes = n;
            const surv = vmVivante(`après ouverture fenêtre ${n}`);
            if (surv.virsh !== 'en cours d’exécution' && surv.virsh !== 'running') {
                log('!! VM non vivante, arrêt de la montée');
                releve.arret_premature = { rang: n, survie: surv };
                break;
            }
        }
        log(`  ${ouvertes} fenêtres ouvertes (côté VM)`);
        await dodo(6000);

        // ---- Un focus, pour un arbitrage réaliste (le repartiteur donne
        // priorité à la fenêtre focalisée). ----
        const cible1 = nomsTries()[0];
        let scen1 = null;
        if (cible1) scen1 = await imposerScenario(cdp, cible1, 'construction — focus initial');
        await dodo(8000);

        const pose = await attendreBarreauxStables(CALME_S, STABILISATION_MAX_S);
        const marqueursAvant = await marqueurs('AVANT LE PALIER A/B — relevé du nombre de fenêtres attachées');
        releve.construction = { ouvertes, cible1, scenario: scen1, pose, survie: vmVivante('après construction') };
        releve.fenetres_attachees = marqueursAvant.attachee_capteur;
        releve.controle_sondage_avant_palier = await controleSondage();

        // ---- LE PALIER A/B : trafic vidéo cumulé, mesuré côté RÉCEPTEUR
        // (bytesReceived — aucune stat outbound-rtp n'existe côté navigateur
        // pour une piste RECVONLY ; proxy déjà employé D4-D9, valide tant que
        // packetsLost reste nul sur ce lien local). ----
        log(`>>> A/B — mesure du trafic vidéo cumulé sur ${PALIER_AB_S} s (PART_SONDAGE=${PART_SONDAGE_VU})`);
        const debut = new Date().toISOString();
        const a = await statsToutes(cdp, 'A/B — début du palier');
        await dodo(PALIER_AB_S * 1000);
        const b = await statsToutes(cdp, 'A/B — fin du palier');
        const fin = new Date().toISOString();
        const d = deltas(a, b);
        log('A/B — PAR FENÊTRE ' + JSON.stringify(d.parFenetre, null, 1));
        log('A/B — CUMULÉ ' + JSON.stringify(d.cumule));
        releve.palier = { debut, fin, ...d, survie: vmVivante('après A/B') };

        const marqueursFin = await marqueurs('FIN');
        releve.marqueurs_fin = marqueursFin;
        releve.controle_sondage_fin = await controleSondage();
        log(`SYNTHÈSE (${ETIQUETTE}) fenetres_attachees=${releve.fenetres_attachees} `
            + `mbps_cumule=${d.cumule.mbps_cumule} paquets_perdus=${d.cumule.paquets_perdus} `
            + `sondage_desarme_occurrences=${releve.controle_sondage_fin.occurrences}`);

        await writeFile(SORTIE_JSON, JSON.stringify(releve, null, 1));
        log('relevé écrit dans ' + SORTIE_JSON);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
        await dodo(8000);
        copierLog();
        log('journal copié dans ' + COPIE_LOG + ' (après la fermeture du navigateur, fin réelle de l\'exécution)');
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
