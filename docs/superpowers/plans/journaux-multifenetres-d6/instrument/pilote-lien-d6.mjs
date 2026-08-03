#!/usr/bin/env node
// Sous-bloc D6, tâche 1 — LA PORTE : mesurer ce que le chemin
// VM → pont → navigateur hôte porte réellement, avec UNE seule fenêtre et un
// plafond de débit très haut (`BITRATE=100000000`).
//
// Dérivé de `journaux-multifenetres-d5/instrument/pilote-recette-d5.mjs` :
// même squelette CDP + vm-it + marqueurs. Ce qui change : une seule fenêtre,
// aucune manipulation de visibilité ni de focus (rien à évincer), et un
// palier long dont on échantillonne les compteurs.
//
// Contraintes de protocole héritées, chacune ayant coûté une exécution :
//   1. la source BOUGE (page canvas animée, `--user-data-dir` propre) ;
//   2. le navigateur est lancé AVANT le superviseur (DELAI_ATTENTE_VIEWPORT_MAX) ;
//   3. `--disable-popup-blocking` et les trois drapeaux anti-gel ;
//   4. AUCUNE capture d'écran CDP pendant la séquence ;
//   5. toute évaluation CDP sur une page portant un flux WebRTC est BORNÉE ;
//   6. l'instance de navigateur pilotée est vérifiée par PID écoutant ;
//   7. survie de la VM contrôlée après la séquence ;
//   8. `agent.log` est copié APRÈS la fin réelle, pas à la fin du pilote.
//
// L'INSTRUMENT À DÉCLARER : l'amorce enveloppe `RTCPeerConnection` (pour
// atteindre `getStats`) et `createDataChannel` (pour retenir les messages
// `link` que l'agent pousse sur le canal `control`). Elle n'altère aucun
// comportement : elle observe.

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
const AIDE = process.env.AIDE ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d6/instrument');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const COPIE_LOG = process.env.COPIE_LOG ?? '/tmp/agent-encours-d6.log';
const SORTIE_JSON = process.env.SORTIE_JSON
    ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d6/mesure-lien-navigateur.json');
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
const BITRATE = process.env.BITRATE ?? '100000000';
const PALIER_S = Number(process.env.PALIER_S ?? 90);
const PAS_S = Number(process.env.PAS_S ?? 5);

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

/// La VM se met en veille prolongée toute seule (comportement documenté, cause
/// inconnue). Éprouvé par un ACCÈS réel : la présence de l'entrée de montage
/// CIFS ne prouve rien.
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
        capteur_lance: compte('capteur lancé'),
        enfant_lance: compte('enfant lancé'),
        attachee_capteur: compte('fenêtre attachée au capteur'),
        cloture: compte('clôture de session amorcée'),
        observation: compte('observation réseau'),
        taille_changee: compte("taille d'encodage changée"),
        taille_refusee: compte("changement de taille d'encodage refusé"),
        refus_debit: compte("l'encodeur refuse le réglage du débit à chaud"),
        absence_bwe: compte('aucune estimation de bande passante reçue'),
        erreurs: compte('ERROR'),
        avertissements: compte('WARN'),
    };
    log(`MARQUEURS (${etiquette}) ` + JSON.stringify(m));
    return m;
}

/// Les lignes `observation réseau` (niveau `debug`, module
/// `agent::transport::evenements`) portent l'estimation BWE, le RTT et la
/// perte vus par l'AGENT. C'est la grandeur ① du brief.
async function observations() {
    const plat = await journalPlat();
    return plat.split('\n')
        .filter((l) => l.includes('observation réseau'))
        .map((l) => ({
            t: l.match(/(\d{4}-\d\d-\d\dT[\d:.]+Z)/)?.[1] ?? '?',
            estimation: l.match(/estimation=Some\((\d+)\)/)?.[1] ?? null,
            rtt: l.match(/rtt=Some\(([^)]+)\)/)?.[1] ?? null,
            perte: l.match(/perte=Some\(([^)]+)\)/)?.[1] ?? null,
        }));
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
            } else if (m.method) {
                for (const h of this.handlers) h(m);
            }
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
    /// Une évaluation CDP sur une page portant un flux WebRTC actif peut ne
    /// JAMAIS rendre. Toute lecture passe par ici.
    async evalBorne(sessionId, expression, ms = 6000, awaitPromise = true) {
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
  // Garde d'idempotence : l'amorce est posée DEUX fois (script de nouveau
  // document + évaluation directe, voir la ceinture côté pilote). Sans elle,
  // le second passage envelopperait un enveloppé et chaque message \`link\`
  // serait compté deux fois.
  if (window.__amorceD6) return;
  window.__amorceD6 = true;
  window.__pc = null;
  window.__liens = [];
  const N = window.RTCPeerConnection;
  const creer = N.prototype.createDataChannel;
  // On retient les messages du canal 'control' : c'est par lui que l'agent
  // annonce \`link\` (débit décidé, taille, qualité, adaptation). Observation
  // pure — le message est relayé intact au code de la page.
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
})();
`;

const pages = new Map(); // sessionId -> {targetId, url}
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));
const nomDe = (url) => url.replace(/^.*\?session=/, 'w:');

/// Tout ce que `getStats()` porte d'utile à cette mesure. `framesDropped` et
/// `packetsLost` sont le TÉMOIN qui départage « le lien a saturé » de
/// « le décodeur ou la machine hôte a saturé ».
const STATS = `(async () => {
  const pc = window.__pc;
  if (!pc) return { pc: null };
  const r = await pc.getStats(); const t = [...r.values()];
  const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
  const paire = t.find(x => x.type === 'candidate-pair' && (x.selected || x.nominated));
  const loc = paire ? t.find(x => x.id === paire.localCandidateId) : null;
  const dis = paire ? t.find(x => x.id === paire.remoteCandidateId) : null;
  const lien = window.__liens.length ? window.__liens[window.__liens.length - 1] : null;
  return {
    etat: pc.iceConnectionState,
    horloge: Date.now(),
    // --- inbound-rtp video
    images_decodees: v?.framesDecoded ?? null,
    images_recues: v?.framesReceived ?? null,
    images_perdues: v?.framesDropped ?? null,
    gels: v?.freezeCount ?? null,
    gel_total_s: v?.totalFreezesDuration ?? null,
    octets_recus: v?.bytesReceived ?? null,
    paquets_recus: v?.packetsReceived ?? null,
    paquets_perdus: v?.packetsLost ?? null,
    nack: v?.nackCount ?? null,
    pli: v?.pliCount ?? null,
    jitter: v?.jitter ?? null,
    temps_decodage_total: v?.totalDecodeTime ?? null,
    l: v?.frameWidth ?? null, h: v?.frameHeight ?? null,
    // --- paire de candidats
    rtt: paire?.currentRoundTripTime ?? null,
    debit_entrant_dispo: paire?.availableIncomingBitrate ?? null,
    octets_paire: paire?.bytesReceived ?? null,
    type_local: loc?.candidateType ?? null,
    type_distant: dis?.candidateType ?? null,
    protocole: loc?.protocol ?? null,
    // --- ce que l'agent ANNONCE (canal control)
    lien_bitrate: lien?.bitrate ?? null,
    lien_taille: lien ? (lien.w + 'x' + lien.h) : null,
    lien_qualite: lien?.quality ?? null,
    lien_adaptation: lien?.adaptation ?? null,
    liens_recus: window.__liens.length,
  };
})()`;

async function lire(cdp, etiquette) {
    const e = appPages()[0];
    if (!e) { log(`STATS (${etiquette}) aucune page d'application`); return null; }
    const s = await cdp.evalBorne(e[0], STATS);
    log(`STATS (${etiquette}) ` + JSON.stringify(s));
    return s;
}

/// Charge CPU de l'HÔTE — l'autre cause possible de l'effondrement de D4.
function cpuHote(etiquette) {
    const top = spawnSync('bash', ['-c',
        `top -b -n 3 -d 2 | grep -E '^%Cpu|^top -' | head -8`], { encoding: 'utf8' }).stdout ?? '';
    const qemu = spawnSync('bash', ['-c',
        `ps -o pid,pcpu,comm -C qemu-system-x86_64 --no-headers 2>/dev/null; ` +
        `ps -o pid,pcpu,comm --sort=-pcpu -e --no-headers | head -6`], { encoding: 'utf8' }).stdout ?? '';
    const charge = spawnSync('bash', ['-c', 'cat /proc/loadavg; nproc'], { encoding: 'utf8' }).stdout ?? '';
    log(`CPU HÔTE (${etiquette})\n${top.trimEnd()}\n--- processus les plus chargés ---\n${qemu.trimEnd()}\n--- loadavg / nproc ---\n${charge.trimEnd()}`);
    return { top: top.trim(), processus: qemu.trim(), charge: charge.trim() };
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9993);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d6-'));
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
    log(`BITRATE=${BITRATE} PALIER_S=${PALIER_S} chrome pid=${chrome.pid} port=${port}`);

    const releve = { bitrate_plafond: BITRATE, palier_s: PALIER_S, echantillons: [], cpu: [] };
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
                // CEINTURE : `addScriptToEvaluateOnNewDocument` ne court pas
                // toujours sur une page ouverte par `window.open` (piège
                // relevé en D5). La page est ici arrêtée par
                // `waitForDebuggerOnStart`, donc une évaluation directe passe
                // AVANT tout script de la page. L'amorce est idempotente :
                // si les deux chemins courent, le second réenveloppe des
                // fonctions déjà enveloppées, sans changer d'observable.
                await cdp.send('Runtime.evaluate',
                    { expression: AMORCE, returnByValue: true }, sessionId).catch(() => { });
                if (VIEWPORT_FORCE) {
                    const [L, H] = VIEWPORT_FORCE.split('x').map(Number);
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: L, height: H, deviceScaleFactor: 1, mobile: false }, sessionId)
                        .catch((e) => log('  !! imposition de viewport impossible', String(e).slice(0, 150)));
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
        cpuHote('au repos, avant toute session');

        await cdp.send('Target.createTarget', { url: URL_SHELL });
        log('shell demandée', URL_SHELL);
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut').textContent`));

        log('>>> lancement du superviseur');
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 BITRATE=${BITRATE} SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 ` +
            `RUST_LOG='info,agent::transport::evenements=debug' scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        if ((sup.stderr ?? '').trim()) log('run-agent.sh STDERR :', sup.stderr.trim().replace(/\n/g, ' / '));
        await dodo(6000);

        log('>>> OUVERTURE de LA fenêtre (une seule)');
        vmIt('ouvrird6-1', [
            `$a = @(`,
            `  "--app=file:///C:/dev/anim-d4.html?n=1",`,
            `  "--user-data-dir=C:\\dev\\chrome-d6-1",`,
            `  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',`,
            `  '--window-size=1280,720','--window-position=50,50',`,
            `  '--disable-features=CalculateNativeWinOcclusion',`,
            `  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',`,
            `  '--disable-renderer-backgrounding')`,
            `Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a`,
            `Start-Sleep -Seconds 2`,
        ].join('\n'));

        // Attendre le FAIT (une page d'application ouverte), jamais une durée.
        let ouverte = false;
        for (let i = 0; i < 40; i += 1) {
            await dodo(2000);
            if (appPages().length > 0) { ouverte = true; log(`page d'application ouverte après ${(i + 1) * 2}s`); break; }
        }
        if (!ouverte) throw new Error("aucune page d'application ne s'est ouverte");

        // Attendre le FAIT : des images qui arrivent réellement.
        let amorcee = false;
        for (let i = 0; i < 40; i += 1) {
            const s = await lire(cdp, `amorçage ${i}`);
            if (s && typeof s.images_decodees === 'number' && s.images_decodees > 30) { amorcee = true; break; }
            await dodo(2000);
        }
        if (!amorcee) log('!! le flux ne semble pas amorcé — la suite est relevée quand même');
        releve.amorcee = amorcee;

        // --------------------------------------------------- LE PALIER
        log(`>>> PALIER de ${PALIER_S} s, échantillon toutes les ${PAS_S} s`);
        const debut = Date.now();
        releve.cpu.push({ etiquette: 'début du palier', ...cpuHote('début du palier') });
        while ((Date.now() - debut) / 1000 < PALIER_S) {
            const s = await lire(cdp, `palier +${Math.round((Date.now() - debut) / 1000)}s`);
            if (s) releve.echantillons.push(s);
            await dodo(PAS_S * 1000);
        }
        releve.cpu.push({ etiquette: 'fin du palier', ...cpuHote('fin du palier') });

        releve.observations_agent = await observations();
        log(`OBSERVATIONS AGENT : ${releve.observations_agent.length} ligne(s) « observation réseau »`);
        log('  dix dernières : ' + JSON.stringify(releve.observations_agent.slice(-10)));

        releve.marqueurs = await marqueurs('FIN');
        releve.survie = vmVivante('fin de mesure');

        // Les cadences et débits qui se déduisent des échantillons. CALCULÉS.
        const e = releve.echantillons.filter((x) => x && typeof x.images_decodees === 'number');
        if (e.length >= 2) {
            const a = e[0], b = e[e.length - 1];
            const dt = (b.horloge - a.horloge) / 1000;
            releve.bilan_calcule = {
                secondes: Number(dt.toFixed(3)),
                images_decodees: b.images_decodees - a.images_decodees,
                i_par_s: Number(((b.images_decodees - a.images_decodees) / dt).toFixed(2)),
                images_perdues: (b.images_perdues ?? 0) - (a.images_perdues ?? 0),
                paquets_perdus: (b.paquets_perdus ?? 0) - (a.paquets_perdus ?? 0),
                paquets_recus: (b.paquets_recus ?? 0) - (a.paquets_recus ?? 0),
                gels: (b.gels ?? 0) - (a.gels ?? 0),
                mbps_recu: Number((((b.octets_recus - a.octets_recus) * 8) / dt / 1e6).toFixed(3)),
                taille: `${b.l}x${b.h}`,
                lien_bitrate_final: b.lien_bitrate,
                rtt_final: b.rtt,
                chemin: `${b.type_local} ↔ ${b.type_distant} (${b.protocole})`,
            };
            log('BILAN CALCULÉ ' + JSON.stringify(releve.bilan_calcule, null, 1));
        }

        await writeFile(SORTIE_JSON, JSON.stringify(releve, null, 1));
        log('relevé navigateur écrit dans ' + SORTIE_JSON);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
        // `agent.log` est copié APRÈS la fin réelle : les enfants meurent
        // quand le navigateur se ferme, et leurs lignes de libération
        // partiraient sinon avec le journal suivant (piège de D4).
        await dodo(6000);
        copierLog();
        log('journal copié dans ' + COPIE_LOG + ' (après la fermeture du navigateur)');
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
