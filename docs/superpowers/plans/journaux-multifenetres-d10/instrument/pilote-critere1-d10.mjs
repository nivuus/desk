#!/usr/bin/env node
// Sous-bloc D10 (tâche 10) — recette ① : dix fenêtres, une par une, sur un
// registre d'affichage laissé SALE par les sous-blocs précédents.
//
// VERSION 2. La version 1 de ce fichier suivait chaque fenêtre
// individuellement (un sid CDP par fenêtre, via `dernierSidAppPage`) sur le
// modèle de `journaux-multifenetres-d9/instrument/pilote-critere1.mjs`.
// Rejouée en conditions réelles sur le binaire de `main`, elle a révélé que
// CE terrain a un comportement que D9 n'avait pas : une fenêtre dont la
// sortie est rejetée fait boucler le superviseur (`main` seulement — c'est
// précisément le défaut que ce sous-bloc corrige), et CHAQUE tentative de
// cette boucle ouvre puis referme une page-shell avec un NOUVEAU numéro de
// session, en quelques centaines de millisecondes, SANS qu'aucune fenêtre
// Chrome supplémentaire n'ait été ouverte côté VM. `estApp` calculé UNE FOIS
// à l'attachement (`targetInfo.url`, vide pour une page ouverte par
// `window.open` avant sa navigation — piège documenté par D9 tâche 14) ne se
// remettait jamais à jour, et `dernierSidAppPage` restait donc `null` en
// pratique : suivre une fenêtre précise par son sid n'a plus de sens quand le
// nombre de sessions créées ne correspond pas au nombre de fenêtres VM
// ouvertes.
//
// Remède retenu, plus simple et robuste : ne plus jamais essayer d'apparier
// UNE fenêtre VM à UNE page CDP. `agent.log` (compté après coup) est la
// SEULE source de vérité pour le critère ① — c'est exactement ce que le
// brief demande (Step 4 : des `grep` sur le journal, rien côté CDP). Les
// fenêtres sont ouvertes avec un délai FIXE entre elles, sans attendre un
// événement CDP par fenêtre. Le CDP ne sert plus qu'à : (a) maintenir la
// page-shell vivante et poser `deviceMetricsOverride=1280x720` sur TOUTE
// page qui s'attache (générique, jamais liée à un rang précis), et (b), pour
// le critère ②, échantillonner l'empreinte visuelle de TOUTES les pages
// vivantes à la fin, sans avoir besoin de savoir laquelle correspond à quel
// rang d'ouverture.
//
// ⚠️ CE SCRIPT NE TOUCHE JAMAIS AU REGISTRE. Aucune variable
// MULTIFENETRE_MODE_SORTIE n'est jamais posée ici : le registre sale EST
// l'état mesuré, le nettoyer annulerait la mesure entière.

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
const AIDE = process.env.AIDE ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d10/instrument');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const PORT_SHELL = process.env.PORT_SHELL ?? '5173';
const URL_SHELL = `http://${HOTE}:${PORT_SHELL}/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const SORTIE_DIR = process.env.SORTIE_DIR
    ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d10');
const COPIE_LOG = process.env.COPIE_LOG ?? join(SORTIE_DIR, `agent-${ETIQUETTE}.log`);
const SORTIE_JSON = process.env.SORTIE_JSON ?? join(SORTIE_DIR, `critere-1-${ETIQUETTE}.json`);
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
const N_CIBLE = Number(process.env.N_CIBLE ?? 10);
const BITRATE = process.env.BITRATE ?? '8000000';
const DELAI_ENTRE_FENETRES_MS = Number(process.env.DELAI_ENTRE_FENETRES_MS ?? 8000);
const ATTENTE_REGLAGE_S = Number(process.env.ATTENTE_REGLAGE_S ?? 45);
const FAIRE_CRITERE_2 = process.env.FAIRE_CRITERE_2 !== '0';

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
        introuvable_topologie: compte('introuvable dans la topologie DXGI'),
        aucune_sortie_apparue: compte('aucune sortie apparue ne peut servir'),
        sortie_creee: compte('sortie virtuelle créée'),
        sortie_rendue: compte('sortie virtuelle rendue au pilote'),
        creation_refusee: compte('création de sortie refusée'),
        enfant_lance: compte('enfant lancé'),
        enfant_termine: compte('enfant terminé'),
        cloture: compte('clôture de session amorcée'),
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
  window.__pc = null;
  const N = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;
`;

const pages = new Map(); // sessionId -> {targetId, url}
// Générique, JAMAIS lié à un rang d'ouverture précis (voir l'en-tête) :
// n'importe quelle page dont l'URL COURANTE (mise à jour par
// Target.targetInfoChanged, pas figée à l'attachement) porte `?session=`.
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));

const STATS = `(async () => {
  const pc = window.__pc;
  if (!pc) return { pc: null };
  const r = await pc.getStats(); const t = [...r.values()];
  const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
  const video = document.querySelector('#remote');
  let empreinte = null, video_native = null;
  try {
    if (video && video.videoWidth > 0) {
      video_native = video.videoWidth + 'x' + video.videoHeight;
      const cv = document.createElement('canvas');
      cv.width = 8; cv.height = 8;
      const ctx = cv.getContext('2d');
      ctx.drawImage(video, 0, 0, 8, 8);
      const d = ctx.getImageData(0, 0, 8, 8).data;
      let s = '';
      for (let i = 0; i < d.length; i += 4) {
        s += d[i].toString(16).padStart(2, '0') + d[i + 1].toString(16).padStart(2, '0') + d[i + 2].toString(16).padStart(2, '0');
      }
      empreinte = s;
    }
  } catch (e) { empreinte = 'ERR:' + String(e).slice(0, 100); }
  return {
    etat: pc.iceConnectionState,
    images: v?.framesDecoded ?? null,
    l: v?.frameWidth ?? null, h: v?.frameHeight ?? null,
    octets: v?.bytesReceived ?? null,
    horloge: Date.now(),
    empreinte, video_native,
  };
})()`;

async function statsToutes(cdp, etiquette) {
    const entrees = appPages();
    const resultats = await Promise.all(entrees.map(async ([sid, p]) => {
        const s = await cdp.evalBorne(sid, STATS);
        return [p.url.replace(/^.*\?session=/, 'w:'), s];
    }));
    const out = Object.fromEntries(resultats);
    log(`STATS (${etiquette}, ${entrees.length} page(s)) ` + JSON.stringify(out));
    return out;
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9993);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d10-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new',
        `--remote-debugging-port=${port}`,
        '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`,
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--disable-gpu',
        '--window-size=1280,720',
        '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns',
        '--disable-popup-blocking',
        '--disable-background-timer-throttling',
        '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding',
        'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} N_CIBLE=${N_CIBLE} DELAI_ENTRE_FENETRES_MS=${DELAI_ENTRE_FENETRES_MS} `
        + `ATTENTE_REGLAGE_S=${ATTENTE_REGLAGE_S} FAIRE_CRITERE_2=${FAIRE_CRITERE_2} chrome pid=${chrome.pid} port=${port}`);

    const releve = { etiquette: ETIQUETTE, n_cible: N_CIBLE };

    try {
        const version = await attendreDevtools(port);
        const qui = spawnSync('bash', ['-c',
            `ss -ltnp 2>/dev/null | grep ':${port} ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1`],
            { encoding: 'utf8' }).stdout.trim();
        log(`identité du navigateur : pid écoutant=${qui} pid lancé=${chrome.pid} version=${version.Browser}`);
        if (String(qui) !== String(chrome.pid)) {
            throw new Error(`le port ${port} est tenu par le pid ${qui}, pas par notre Chrome (${chrome.pid})`);
        }
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
                // Posé INCONDITIONNELLEMENT sur toute page qui n'est ni la
                // shell ni la page initiale about:blank — générique, jamais
                // lié à un rang d'ouverture (voir l'en-tête du fichier).
                if (!targetInfo.url.includes('shell.html') && targetInfo.url !== 'about:blank') {
                    const [L, H] = VIEWPORT_FORCE.split('x').map(Number);
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: L, height: H, deviceScaleFactor: 1, mobile: false }, sessionId)
                        .catch((e) => log('  !! imposition de viewport impossible', String(e).slice(0, 150)));
                }
                await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
            } else if (m.method === 'Target.detachedFromTarget') {
                const p = pages.get(m.params.sessionId);
                if (p) log(`- page détachée   session=${m.params.sessionId.slice(0, 8)} url=${p.url}`);
                pages.delete(m.params.sessionId);
            } else if (m.method === 'Target.targetInfoChanged') {
                for (const [, p] of pages) {
                    if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
                }
            }
        });
        await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
        await cdp.send('Target.setDiscoverTargets', { discover: true });

        // ---- Étape 0 : préparation VM. NE TOUCHE JAMAIS AU REGISTRE. ----
        log('>>> ÉTAPE 0 : préparation — VM sans fenêtre éligible (registre NON touché)');
        vmItFichier('preparerd10', 'preparer-d10.ps1');
        await dodo(6000);
        const prepStatut = spawnSync('cat', ['/media/vm/dev/preparer-d10.txt'], { encoding: 'utf8' }).stdout ?? '';
        log('statut préparation : ' + prepStatut.trim());
        releve.preparation = prepStatut.trim();

        // ---- La page-shell, AVANT le superviseur. ----
        await cdp.send('Target.createTarget', { url: URL_SHELL });
        log('shell demandée', URL_SHELL);
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut').textContent`));

        // ---- Le superviseur, APRÈS la shell. ----
        log('>>> lancement du superviseur (SUPERVISEUR=1)');
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 BITRATE=${BITRATE} ` +
            `SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        if ((sup.stderr ?? '').trim()) log('run-agent.sh STDERR :', sup.stderr.trim().replace(/\n/g, ' / '));
        await dodo(6000);
        await marqueurs('superviseur démarré, 0 fenêtre');
        log('PIDs agent après démarrage : ' + winrm(
            'Get-Process agent -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id | Sort-Object').replace(/\s+/g, ' ').trim());
        vmVivante('après démarrage superviseur');

        // ---- Ouverture séquentielle des N_CIBLE fenêtres, DÉLAI FIXE. ----
        // Ne suit plus AUCUNE fenêtre individuellement (voir l'en-tête) : on
        // se contente d'espacer les ouvertures pour laisser au superviseur
        // le temps de réagir à chacune avant la suivante, puis on lit le
        // verdict entièrement dans `agent.log`.
        for (let n = 1; n <= N_CIBLE; n += 1) {
            log(`>>> OUVERTURE fenêtre ${n}/${N_CIBLE}`);
            vmIt(`ouvrird10-${n}`, [
                `$a = @(`,
                `  "--app=file:///C:/dev/anim-d4.html?n=${n}",`,
                `  "--user-data-dir=C:\\dev\\chrome-d10-${ETIQUETTE}-${n}",`,
                `  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',`,
                `  '--window-size=1280,720','--window-position=${40 + n * 10},${40 + n * 10}',`,
                `  '--disable-features=CalculateNativeWinOcclusion',`,
                `  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',`,
                `  '--disable-renderer-backgrounding')`,
                `Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a`,
                `Start-Sleep -Seconds 2`,
            ].join('\n'));
            await dodo(DELAI_ENTRE_FENETRES_MS);
            const m = await marqueurs(`après ouverture fenêtre ${n}`);
            const surv = vmVivante(`après ouverture fenêtre ${n}`);
            if (surv.virsh !== 'en cours d’exécution' && surv.virsh !== 'running') {
                log('!! VM non vivante, arrêt de la montée');
                releve.arret_premature = { rang: n, survie: surv };
                break;
            }
        }

        // ---- Réglage final : laisser le temps aux tentatives en cours de
        // se stabiliser (ou, sur le binaire cassé, de continuer à boucler —
        // ce qui EST le signal attendu) avant le relevé décisif. ----
        log(`>>> RÉGLAGE (${ATTENTE_REGLAGE_S} s) avant le relevé final`);
        await dodo(ATTENTE_REGLAGE_S * 1000);
        const finaux = await marqueurs('FIN DE RÉGLAGE — relevé décisif du critère ①');
        releve.marqueurs_finaux = finaux;
        log(`SYNTHÈSE : attachee_capteur=${finaux.attachee_capteur} introuvable_topologie=${finaux.introuvable_topologie} `
            + `aucune_sortie_apparue=${finaux.aucune_sortie_apparue}`);

        // ---- Critère ② : empreinte visuelle, sur TOUTES les pages vivantes
        // à cet instant — sans savoir à quel rang d'ouverture chacune
        // correspond, ce qui n'est pas nécessaire pour juger ce critère
        // (voir l'en-tête). ----
        if (FAIRE_CRITERE_2) {
            log(`>>> CRITÈRE ② — empreinte visuelle sur ${appPages().length} page(s) vivante(s)`);
            const relA = await statsToutes(cdp, 'critère ② — relevé A');
            await dodo(8000);
            const relB = await statsToutes(cdp, 'critère ② — relevé B');
            const empreintesB = Object.entries(relB).map(([k, v]) => [k, v?.empreinte ?? null]);
            const distinctesB = empreintesB.every(([, e]) => e)
                && new Set(empreintesB.map(([, e]) => e)).size === empreintesB.length;
            const vivantes = Object.keys(relB).filter((k) => relA[k]?.empreinte && relB[k]?.empreinte && relA[k].empreinte !== relB[k].empreinte);
            const imagesCroissent = Object.keys(relB).filter((k) =>
                relA[k]?.images != null && relB[k]?.images != null && relB[k].images > relA[k].images);
            releve.critere_2 = {
                relA, relB, empreintes_distinctes_B: distinctesB,
                pages_vivantes_empreinte_changee: vivantes,
                pages_images_croissent: imagesCroissent,
                n_pages_evaluees: Object.keys(relB).length,
            };
            log('CRITÈRE ② — SYNTHÈSE ' + JSON.stringify({
                empreintes_distinctes_B: distinctesB, vivantes: vivantes.length,
                images_croissent: imagesCroissent.length, evaluees: Object.keys(relB).length,
            }));
        } else {
            log('>>> CRITÈRE ② — SAUTÉ (FAIRE_CRITERE_2=0)');
        }

        releve.survie_finale = vmVivante('fin de mesure');
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
