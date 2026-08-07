#!/usr/bin/env node
// Sous-bloc D10, tâche 14 — recette ② : la reconstruction de la capture audio.
//
// Dérivé de `journaux-multifenetres-d7/instrument/pilote-recette-d7.mjs`
// (le mécanisme d'isolation par fréquence pure, AnalyserNode, AMORCE,
// expressionReleveFrequence — repris quasi verbatim, ce n'est pas réinventé)
// et de `journaux-multifenetres-d10/instrument/pilote-critere1-d10.mjs`
// (le lancement du superviseur, la copie/aplatissement du journal, les
// marqueurs). Ce fichier-ci ajoute :
//   - le mode MEME_PROCESSUS (un seul `--user-data-dir` partagé, DEUX
//     fenêtres, donc UN SEUL groupe de PID côté agent — condition du critère) ;
//   - `AUDIO_FAUTE_LECTURE`, transmis à `scripts/run-agent.sh`, qui l'a reçu
//     à la tâche 13 ;
//   - une phase optionnelle de critère ④ : après la mesure du critère ③, tue
//     le PID chrome partagé (le « reconstructeur voué à l'échec » du brief),
//     et relève ce que ça produit CÔTÉ JOURNAL — les deux fenêtres partagent
//     le même PID, donc les tuer ensemble tue aussi la « voisine » : ce que
//     cela signifie pour l'observation de la promotion N'EST PAS présupposé,
//     c'est relevé.
//
// Contraintes de protocole héritées (toutes payées par un sous-bloc antérieur,
// voir CLAUDE.md) :
//   1. `--user-data-dir` PARTAGÉ entre les deux fenêtres (mode MEME_PROCESSUS
//      de D7) — c'est CE QUI FAIT le groupe de PID unique ;
//   2. `AudioContext` démarre `suspended` sans activation utilisateur : ce
//      pilote appelle `ctx.resume()` et VÉRIFIE `ctx.state === 'running'` ;
//   3. toute évaluation CDP sur une page WebRTC active est BORNÉE ;
//   4. AUCUNE capture d'écran CDP pendant une mesure ;
//   5. Chrome porte les trois drapeaux anti-gel ;
//   6. `--disable-popup-blocking`, sans quoi la page-shell voit ses fenêtres
//      refusées en silence ;
//   7. `compteurs audio` est PÉRIODIQUE (30 s) : `A = 0` veut dire « mesure
//      non prise », jamais « rien ne va mal » — tenir la session au-delà de
//      `REPORT_INTERVAL`.
//   8. Ne jamais juger au compte d'octets (`bytesReceived`) : la preuve est
//      la fréquence DOMINANTE reçue, via `AnalyserNode`.

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
const AIDE = process.env.AIDE ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d10/instrument');
const AIDE_D7 = join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d7/instrument');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const PORT_SHELL = process.env.PORT_SHELL ?? '5173';
const URL_SHELL = `http://${HOTE}:${PORT_SHELL}/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const SORTIE_DIR = process.env.SORTIE_DIR ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d10');
const COPIE_LOG = process.env.COPIE_LOG ?? join(SORTIE_DIR, `agent-${ETIQUETTE}.log`);
const COPIE_LOG_PLAT = process.env.COPIE_LOG_PLAT ?? join(SORTIE_DIR, `agent-${ETIQUETTE}-plat.log`);
const SORTIE_JSON = process.env.SORTIE_JSON ?? join(SORTIE_DIR, `critere-2-${ETIQUETTE}.json`);
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
const AUDIO_FAUTE_LECTURE = process.env.AUDIO_FAUTE_LECTURE ?? '15';
const HZ_A = Number(process.env.HZ_A ?? 440); // porteuse (premiere arrivee)
const HZ_B = Number(process.env.HZ_B ?? 880); // voisine
const ATTENTE_S = Number(process.env.ATTENTE_S ?? 40); // > REPORT_INTERVAL (30s)
const CRITERE4 = process.env.CRITERE4 === '1';
const ATTENTE_CRITERE4_S = Number(process.env.ATTENTE_CRITERE4_S ?? 15);
// Step 5 — le cinquième déclencheur, la tentative honnête : tuer un
// processus RENDERER de l'arbre (sans fenêtre principale), qui laisse la
// fenêtre vivante. Issue inconnue par construction — voir le brief.
const DECLENCHEUR5 = process.env.DECLENCHEUR5 === '1';
const ATTENTE_DECLENCHEUR5_S = Number(process.env.ATTENTE_DECLENCHEUR5_S ?? 20);
const PREPARER = process.env.PREPARER !== '0';
const BITRATE = process.env.BITRATE ?? '8000000';

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(7);
    console.log(`[${dt}s ${new Date().toISOString()}] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- VM
function vmIt(nom, ps) {
    const r = spawnSync('bash', [join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d10/instrument/vm-it.sh'), nom, ps],
        { encoding: 'utf8', env: process.env });
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
    const acces = spawnSync('bash', ['-c', 'ls /media/vm/dev/ton.html >/dev/null 2>&1 && echo OUI || echo NON'],
        { encoding: 'utf8' }).stdout?.trim() ?? '?';
    log(`SURVIE VM (${etiquette}) virsh="${virsh}" acces_partage=${acces}`);
    return { virsh, acces };
}
function copierLog() {
    spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
}
function retirerAnsi(texte) {
    return texte.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '');
}
async function journalPlat() {
    copierLog();
    let texte = '';
    try { texte = await readFile(COPIE_LOG, 'utf8'); } catch { }
    const plat = retirerAnsi(texte);
    await writeFile(COPIE_LOG_PLAT, plat).catch(() => { });
    return plat;
}
async function marqueurs(etiquette) {
    const plat = await journalPlat();
    const compte = (motif) => (plat.match(new RegExp(motif, 'g')) ?? []).length;
    const m = {
        lignes: plat.split('\n').length,
        enfant_lance: compte('enfant lancé'),
        attachee_capteur: compte('fenêtre attachée au capteur'),
        cloture: compte('clôture de session amorcée'),
        audio_active: compte('audio activé'),
        injection_armee: compte('injection de fautes de lecture audio ARMEE'),
        lecture_echouee: compte('lecture audio échouée, nouvelle tentative'),
        capture_arretee_definitivement: compte('capture arrêtée définitivement'),
        capture_reconstruite: compte('capture audio reconstruite'),
        reconstruction_refusee: compte('reconstruction de la capture audio refusée'),
        compteurs_audio: compte('compteurs audio'),
        compteurs_audio_actif_true: compte('compteurs audio[^\\n]*actif=true'),
        ordre_audio_actif_true: compte('ordre audio applique[^\\n]*actif=true'),
        ordre_audio_actif_false: compte('ordre audio applique[^\\n]*actif=false'),
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

// AMORCE : expose window.__pc, comme D7. Pas besoin des overrides de
// visibilité/focus ici : deux fenêtres, l'ordre d'ouverture tranche déjà
// (« aucune focalisée : la première arrivée porte le son », voir
// capteur/sommeil/porteurs.rs).
const AMORCE = `
(() => {
  if (window.__amorceD10c2) return;
  window.__amorceD10c2 = true;
  window.__pc = null;
  const N = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;
})();
`;

// Reprise quasi verbatim de `expressionReleveFrequence` de D7 (voir ce
// fichier pour les FINDING numérotés qui la justifient) : repli sur
// <video>.srcObject (ne dépend d'aucune course avec l'AMORCE),
// ctx.resume()+state==='running' vérifié, -Infinity clampé sur un SENTINEL
// numérique, statsAudio en discriminant annexe seulement.
const SENTINEL_DB = -1000;
function expressionReleveFrequence(hzsAssignes) {
    return `(async () => {
  const CIBLES = ${JSON.stringify(hzsAssignes)};
  const SENTINEL = ${SENTINEL_DB};
  const pc = window.__pc;
  let piste = pc ? pc.getReceivers().map(r => r.track).find(t => t && t.kind === 'audio') : null;
  if (!piste) {
    const v = document.querySelector('video');
    piste = v?.srcObject?.getAudioTracks?.()[0] ?? null;
  }
  if (!piste) return { erreur: pc ? 'aucune piste audio' : 'aucune PeerConnection exposee' };
  const ctx = new AudioContext();
  try {
    await ctx.resume();
    if (ctx.state !== 'running') {
      return { erreur: 'AudioContext non demarre etat=' + ctx.state };
    }
    const analyseur = ctx.createAnalyser();
    analyseur.fftSize = 8192;
    ctx.createMediaStreamSource(new MediaStream([piste])).connect(analyseur);
    await new Promise(r => setTimeout(r, 1500));
    const bins = new Float32Array(analyseur.frequencyBinCount);
    analyseur.getFloatFrequencyData(bins);
    const db = (x) => (Number.isFinite(x) ? Math.round(x) : SENTINEL);
    const binDe = (f) => Math.max(0, Math.min(bins.length - 1,
      Math.round(f * analyseur.fftSize / ctx.sampleRate)));
    let meilleur = 0;
    for (let i = 1; i < bins.length; i++) if (bins[i] > bins[meilleur]) meilleur = i;
    const finis = [...bins].filter(Number.isFinite);
    const plancher_db = finis.length ? Math.round(finis.reduce((a, b) => a + b, 0) / finis.length) : SENTINEL;
    let statsAudio = null;
    if (pc) {
      try {
        const rapport = await pc.getStats();
        const entree = [...rapport.values()].find((x) => x.type === 'inbound-rtp' && x.kind === 'audio');
        if (entree) statsAudio = { bytes_recus: entree.bytesReceived ?? null, paquets_recus: entree.packetsReceived ?? null };
      } catch { }
    }
    return {
      hz: Math.round(meilleur * ctx.sampleRate / analyseur.fftSize),
      db: db(bins[meilleur]),
      plancher_db,
      sample_rate: ctx.sampleRate,
      niveaux: CIBLES.map((f) => ({ f, db: db(bins[binDe(f)]) })),
      piste_muted: piste.muted,
      piste_ready_state: piste.readyState,
      stats_audio: statsAudio,
    };
  } finally {
    await ctx.close();
  }
})()`;
}

const pages = new Map();
const appPages = () => [...pages].filter(([, p]) => p.url.includes('?session=') && !p.url.includes('shell.html'));
const nomDe = (url) => url.replace(/^.*\?session=/, 'w:');

async function frequencesToutes(cdp, etiquette, hzsAssignes) {
    const entrees = appPages();
    const expression = expressionReleveFrequence(hzsAssignes);
    const resultats = await Promise.all(entrees.map(async ([sid, p]) =>
        [nomDe(p.url), await cdp.evalBorne(sid, expression)]));
    const out = Object.fromEntries(resultats);
    log(`FRÉQUENCES (${etiquette}) ` + JSON.stringify(out));
    return out;
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9995);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d10c2-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns', '--disable-popup-blocking',
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', 'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} AUDIO_FAUTE_LECTURE=${AUDIO_FAUTE_LECTURE} HZ_A=${HZ_A} HZ_B=${HZ_B} `
        + `ATTENTE_S=${ATTENTE_S} CRITERE4=${CRITERE4} chrome pid=${chrome.pid}`);

    const releve = { etiquette: ETIQUETTE, audio_faute_lecture: AUDIO_FAUTE_LECTURE, hz_a: HZ_A, hz_b: HZ_B };
    try {
        const version = await attendreDevtools(port);
        const qui = spawnSync('bash', ['-c', `ss -ltnp 2>/dev/null | grep ':${port} ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1`],
            { encoding: 'utf8' }).stdout.trim();
        log(`identité du navigateur pilote : pid écoutant=${qui} pid lancé=${chrome.pid} version=${version.Browser}`);
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
                    await cdp.send('Emulation.setDeviceMetricsOverride', { width: L, height: H, deviceScaleFactor: 1, mobile: false }, sessionId).catch(() => { });
                }
                await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
            } else if (m.method === 'Target.detachedFromTarget') {
                const p = pages.get(m.params.sessionId);
                if (p) log(`- page détachée   session=${m.params.sessionId.slice(0, 8)} url=${p.url}`);
                pages.delete(m.params.sessionId);
            } else if (m.method === 'Target.targetInfoChanged') {
                for (const [, p] of pages) if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
            }
        });
        await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
        await cdp.send('Target.setDiscoverTargets', { discover: true });

        // ton.html + copies par fréquence, comme D7.
        spawnSync('bash', ['-c', `cp ${join(AIDE_D7, 'ton.html')} /media/vm/dev/ton.html`]);
        spawnSync('bash', ['-c', `cp ${join(AIDE_D7, 'ton.html')} /media/vm/dev/ton-${HZ_A}.html`]);
        spawnSync('bash', ['-c', `cp ${join(AIDE_D7, 'ton.html')} /media/vm/dev/ton-${HZ_B}.html`]);
        log('ton.html + copies par fréquence poussés vers /media/vm/dev/');

        if (PREPARER) {
            log('>>> préparation VM : purge des processus résiduels (registre NON touché)');
            vmIt('preparerd10c2', [
                `Get-Process notepad, mspaint, wordpad, chrome, agent -ErrorAction SilentlyContinue | Stop-Process -Force`,
                `Start-Sleep -Seconds 3`,
                `Get-ChildItem 'C:\\dev' -Directory -Filter 'chrome-d10c2-*' -ErrorAction SilentlyContinue |`,
                `  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue`,
            ].join('\n'));
            await dodo(4000);
        }
        releve.survie_avant = vmVivante('avant lancement');

        // Page-shell AVANT le superviseur.
        await cdp.send('Target.createTarget', { url: URL_SHELL });
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut')?.textContent`));

        log(`>>> lancement du superviseur (SUPERVISEUR=1 AUDIO_FAUTE_LECTURE=${AUDIO_FAUTE_LECTURE})`);
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 BITRATE=${BITRATE} AUDIO_FAUTE_LECTURE=${AUDIO_FAUTE_LECTURE} `
            + `SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        if ((sup.stderr ?? '').trim()) log('run-agent.sh STDERR :', sup.stderr.trim().replace(/\n/g, ' / '));
        await dodo(6000);
        await marqueurs('superviseur démarré, 0 fenêtre');

        // ---- Deux fenêtres, UN SEUL --user-data-dir partagé (mode
        // MEME_PROCESSUS de D7) : c'est ce qui fait le groupe de PID unique.
        const dossierPartage = 'C:\\dev\\chrome-d10c2-groupe';
        const ouvrirFenetreTon = async (n, hz, gain) => {
            const avant = appPages().length;
            const fichier = `ton-${hz}.html`;
            log(`  · ouverture fenêtre ${n} (${fichier}?hz=${hz}&gain=${gain}, profil PARTAGÉ)`);
            vmIt(`ouvrird10c2-${ETIQUETTE}-${n}`, [
                `$a = @(`,
                `  "--app=file:///C:/dev/${fichier}?hz=${hz}&gain=${gain}",`,
                `  "--user-data-dir=${dossierPartage}",`,
                `  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',`,
                `  '--window-size=1280,720','--window-position=${30 + n * 40},${30 + n * 40}',`,
                `  '--disable-features=CalculateNativeWinOcclusion',`,
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
        };

        log(`>>> OUVERTURE — fenêtre A (porteuse attendue, ${HZ_A} Hz) puis fenêtre B (voisine, ${HZ_B} Hz)`);
        const ouvA = await ouvrirFenetreTon(1, HZ_A, 0.25);
        const ouvB = await ouvrirFenetreTon(2, HZ_B, 0.25);
        releve.ouvertures = { A: ouvA, B: ouvB };

        // Step 1 : relever les PID chrome VM et vérifier le groupe unique.
        await dodo(3000);
        const pidsBrut = winrm('Get-Process chrome | Select-Object Id,MainWindowTitle | Format-List');
        log('PIDs chrome (VM) :\n' + pidsBrut.trim());
        releve.pids_chrome_vm = pidsBrut.trim();
        const idsUniques = [...new Set([...pidsBrut.matchAll(/^Id\s*:\s*(\d+)/gm)].map((m) => m[1]))];
        releve.pids_chrome_uniques = idsUniques;
        log(`PID(s) chrome distincts relevés : ${JSON.stringify(idsUniques)} `
            + `(1 attendu pour un groupe unique, en excluant le pilote local qui tourne sur l'HÔTE, pas la VM)`);

        // ---- Step 2 : stabilisation > REPORT_INTERVAL (30 s), puis relevé. ----
        log(`>>> STABILISATION (${ATTENTE_S} s) avant le relevé décisif`);
        await dodo(ATTENTE_S * 1000);
        const survie1 = vmVivante('après stabilisation');
        releve.survie_apres_stabilisation = survie1;
        const m1 = await marqueurs('critère ③ — relevé décisif');
        releve.marqueurs_critere3 = m1;

        const freqs = await frequencesToutes(cdp, 'critère ③', [HZ_A, HZ_B]);
        releve.frequences_critere3 = freqs;
        const pageA = nomDe(appPages()[0]?.[1]?.url ?? '');
        log(`Récapitulatif critère ③ : capture_reconstruite=${m1.capture_reconstruite} `
            + `compteurs_audio(A)=${m1.compteurs_audio} compteurs_audio_actif_true(B)=${m1.compteurs_audio_actif_true}`);

        // ---- Step 4 (optionnel) : critère ④, reconstructeur voué à l'échec. ----
        if (CRITERE4) {
            log('>>> CRITÈRE ④ — tuer le(s) PID chrome partagé(s) après injection déjà armée');
            for (const pid of idsUniques) {
                const r = winrm(`Stop-Process -Id ${pid} -Force -ErrorAction SilentlyContinue; 'tue pid=${pid}'`);
                log(`  Stop-Process pid=${pid} : ${r.trim()}`);
            }
            releve.critere4_pids_tues = idsUniques;
            log(`>>> ATTENTE (${ATTENTE_CRITERE4_S} s) avant le relevé du critère ④`);
            await dodo(ATTENTE_CRITERE4_S * 1000);
            const survie2 = vmVivante('après critère ④');
            releve.survie_apres_critere4 = survie2;
            const m2 = await marqueurs('critère ④ — relevé décisif');
            releve.marqueurs_critere4 = m2;
            const plat2 = await journalPlat();
            const audioMortLignes = (plat2.match(/audio_mort\(|AudioMort|inapte/gi) ?? []).length;
            const promotionLignes = plat2.match(/ordre audio applique session=\S+ actif=true[^\n]*/g) ?? [];
            releve.critere4_diagnostic = {
                audio_mort_occurrences_brutes: audioMortLignes,
                lignes_ordre_actif_true: promotionLignes,
            };
            log(`CRITÈRE ④ — DIAGNOSTIC lignes_actif_true=${JSON.stringify(promotionLignes)}`);
        } else {
            log('>>> CRITÈRE ④ — SAUTÉ (CRITERE4=0)');
        }

        // ---- Step 5 (optionnel) : le cinquième déclencheur, la tentative
        // honnête. Tue UN processus chrome SANS fenêtre principale (un
        // renderer), qui laisse les fenêtres vivantes — contrairement au
        // critère ④, qui tue le PID propriétaire des fenêtres lui-même.
        // Issue INCONNUE : elle peut ne rien produire. Relevé quel qu'il soit. ----
        if (DECLENCHEUR5) {
            log('>>> STEP 5 — tuer un processus chrome renderer (sans MainWindowTitle), fenêtre laissée vivante');
            const avant5 = await marqueurs('avant déclencheur 5');
            const r5 = winrm(`Get-Process chrome | Where-Object { $_.MainWindowTitle -eq "" } | Select-Object -First 1 | Stop-Process -Force -PassThru | Select-Object Id`);
            log('résultat du Stop-Process (renderer) : ' + r5.trim());
            releve.declencheur5_stop_process = r5.trim();
            log(`>>> ATTENTE (${ATTENTE_DECLENCHEUR5_S} s) avant le relevé du déclencheur 5`);
            await dodo(ATTENTE_DECLENCHEUR5_S * 1000);
            const survie5 = vmVivante('après déclencheur 5');
            releve.survie_apres_declencheur5 = survie5;
            const m5 = await marqueurs('déclencheur 5 — relevé décisif');
            releve.marqueurs_declencheur5 = m5;
            releve.declencheur5_delta_lecture_echouee = m5.lecture_echouee - avant5.lecture_echouee;
            log(`STEP 5 — DELTA 'lecture audio échouée' = ${releve.declencheur5_delta_lecture_echouee} `
                + `(0 est un relevé valide, pas un échec de tâche)`);
        } else {
            log('>>> STEP 5 — SAUTÉ (DECLENCHEUR5=0)');
        }

        releve.survie_finale = vmVivante('fin de mesure');
        await writeFile(SORTIE_JSON, JSON.stringify(releve, null, 1));
        log('relevé écrit dans ' + SORTIE_JSON);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
        await dodo(8000);
        await journalPlat();
        log('journal copié + aplati dans ' + COPIE_LOG + ' / ' + COPIE_LOG_PLAT + ' (après fermeture réelle du navigateur)');
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
