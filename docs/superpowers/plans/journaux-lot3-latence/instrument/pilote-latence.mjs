#!/usr/bin/env node
// Lot 3, item 7 (3.1) — LA LATENCE CAPTURE → AFFICHAGE.
//
// 🔴 JAMAIS MESURÉE PAR AUCUN SOUS-BLOC DEPUIS D1. Ce pilote établit une
// session réelle (plateforme → shell → page de session), y injecte
// `sonde-latence.js` PAR LECTURE DE SON FICHIER D'ORIGINE, et rend la
// distribution de `presentationTime - captureTime`.
//
// ⚠️ CE QU'IL NE MESURE PAS, et le relevé le porte : la latence VERRE À
// VERRE. Le maillon d'affichage APRÈS `presentationTime` n'y est pas (le
// compositeur, le balayage de la dalle), et la boucle d'ENTRÉE pas du tout.
//
// ⚠️ Chrome porte les trois drapeaux d'anti-throttling exigés par toute
// mesure de plus de 5 minutes ; toute évaluation CDP est BORNÉE ; aucune
// capture d'écran CDP ne court pendant la mesure.
//
// Usage :
//   node pilote-latence.mjs --etiquette=1f-nominal --fenetres=1 --duree=60 \
//        --sortie=/chemin/distribution.json
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { attendreDevtools } from '../../../../../client/recette/devtools.mjs';

// 🔴 La racine se DÉRIVE, elle ne se porte pas : 48 scripts de ce dépôt ont
// porté `/home/mallanic/Projects/Guacamole` après son déplacement.
const ICI = dirname(fileURLToPath(import.meta.url));

const arg = (n, d) =>
    (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const PLATEFORME = process.env.PLATEFORME ?? 'http://192.168.3.1:3445';
const COURRIEL = process.env.COURRIEL ?? 'maxime.g.allanic@gmail.com';
const ETIQUETTE = arg('etiquette', 'nominal');
const FENETRES = Number(arg('fenetres', '1'));
const DUREE_S = Number(arg('duree', '60'));
const SORTIE = arg('sortie', `/var/tmp/latence-${ETIQUETTE}.json`);
const APP = process.env.APP ?? 'chrome|edge|bloc.?notes|notepad';
const PORT = 9341;

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString(), ...a);

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.n = 1;
        this.attente = new Map();
        this.pret = new Promise((r) => this.ws.addEventListener('open', () => r(), { once: true }));
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.attente.has(m.id)) {
                const { resolve, reject } = this.attente.get(m.id);
                this.attente.delete(m.id);
                m.error ? reject(new Error(JSON.stringify(m.error))) : resolve(m.result);
            }
        });
    }
    async envoyer(method, params = {}) {
        await this.pret;
        const id = this.n++;
        return new Promise((resolve, reject) => {
            this.attente.set(id, { resolve, reject });
            // ⚠️ BORNÉE : une évaluation CDP qui ne rend jamais fige le pilote
            // au milieu de la mesure, et le relevé serait perdu.
            const minuteur = setTimeout(() => {
                this.attente.delete(id);
                reject(new Error(`CDP ${method} sans réponse après 20 s`));
            }, 20000);
            const enveloppe = {
                resolve: (v) => { clearTimeout(minuteur); resolve(v); },
                reject: (e) => { clearTimeout(minuteur); reject(e); },
            };
            this.attente.set(id, enveloppe);
            this.ws.send(JSON.stringify({ id, method, params }));
        });
    }
    async evaluer(expr) {
        const r = await this.envoyer('Runtime.evaluate',
            { expression: expr, awaitPromise: true, returnByValue: true });
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
        return r.result.value;
    }
}

const cibles = async () => await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();

/// L'AMORCE POSÉE AVANT LE PREMIER SCRIPT DE CHAQUE CIBLE NEUVE.
///
/// 🔴 `Page.addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page
/// ouverte par `window.open` (piège transverse du dépôt), et une page de
/// session RECHARGÉE cesse de répondre — le rôle `client` est EXCLUSIF par
/// session, la page rechargée demande un rôle que la précédente n'a pas
/// rendu (mesuré le 5 septembre 2026 : `Runtime.evaluate` sans réponse
/// après 20 s). Le remède est celui du lot 32C : `Target.setAutoAttach`
/// avec `waitForDebuggerOnStart` AU NIVEAU NAVIGATEUR — chaque cible neuve
/// pause avant son premier script, on y pose l'amorce, puis
/// `Runtime.runIfWaitingForDebugger` la relâche. On ne ferme RIEN.
class Navigateur {
    constructor(wsUrl, amorce) {
        this.ws = new WebSocket(wsUrl);
        this.n = 1;
        this.attente = new Map();
        this.amorce = amorce;
        this.posees = [];
        this.pret = new Promise((r) => this.ws.addEventListener('open', () => r(), { once: true }));
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.attente.has(m.id)) {
                const { resolve, reject } = this.attente.get(m.id);
                this.attente.delete(m.id);
                m.error ? reject(new Error(JSON.stringify(m.error))) : resolve(m.result);
                return;
            }
            if (m.method === 'Target.attachedToTarget') this.equiper(m.params).catch(() => {});
        });
    }
    async envoyer(method, params = {}, sessionId) {
        await this.pret;
        const id = this.n++;
        return new Promise((resolve, reject) => {
            const minuteur = setTimeout(() => {
                this.attente.delete(id);
                reject(new Error(`CDP ${method} sans réponse après 20 s`));
            }, 20000);
            this.attente.set(id, {
                resolve: (v) => { clearTimeout(minuteur); resolve(v); },
                reject: (e) => { clearTimeout(minuteur); reject(e); },
            });
            this.ws.send(JSON.stringify(sessionId ? { id, method, params, sessionId } : { id, method, params }));
        });
    }
    async equiper({ sessionId, targetInfo }) {
        try {
            if (targetInfo.type === 'page') {
                await this.envoyer('Page.enable', {}, sessionId);
                await this.envoyer('Page.addScriptToEvaluateOnNewDocument',
                    { source: this.amorce }, sessionId);
                this.posees.push(targetInfo.targetId);
            }
        } finally {
            // ⚠️ TOUJOURS relâcher, même si la pose a échoué : une cible
            // laissée en pause ne charge jamais, et le symptôme se lit comme
            // « la session ne s'ouvre pas ».
            await this.envoyer('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => {});
        }
    }
}

async function jeton() {
    const r = await fetch(`${PLATEFORME}/auth/moi`, { headers: { 'X-Pomerium-Claim-Email': COURRIEL } });
    const c = await r.json();
    if (!r.ok) throw new Error(`/auth/moi a refusé : ${r.status}`);
    return c.acces ?? c.jeton;
}
async function laVm(j) {
    const v = await (await fetch(`${PLATEFORME}/vm`, { headers: { Authorization: `Bearer ${j}` } })).json();
    return (v.vms ?? v)[0];
}
async function prefixe(j, vm) {
    const r = await fetch(`${PLATEFORME}/session`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${j}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ vm: vm.id }),
    });
    const c = await r.json();
    return c.prefixe ?? c.session?.split(':')[0];
}
/// 🔴 DESK N'ADOPTE QUE LES FENÊTRES DES APPLICATIONS QU'IL A LANCÉES (règle
/// d'appartenance, lot 32I). Le pilote doit donc en ouvrir lui-même, APRÈS
/// que la shell est connectée — une fenêtre ouverte plus de 30 s avant la
/// connexion du navigateur est perdue et jamais reproposée (legs du lot 10D).
async function lancerUneApplication(j, vmId, motif) {
    const r = await fetch(`${PLATEFORME}/applications?vm=${encodeURIComponent(vmId)}`, {
        headers: { Authorization: `Bearer ${j}` },
    });
    const c = await r.json();
    const liste = c.applications ?? c;
    if (!Array.isArray(liste)) throw new Error(`/applications a rendu ${JSON.stringify(c).slice(0, 200)}`);
    const choisie = liste.find((a) => new RegExp(motif, 'i').test(a.nom ?? a.cle ?? ''));
    if (!choisie) {
        throw new Error(`aucune application ne correspond à /${motif}/ parmi ${liste.length} : `
            + liste.slice(0, 30).map((a) => a.nom).join(' | '));
    }
    const l = await fetch(`${PLATEFORME}/application/${choisie.id}/lancer`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${j}`, 'Content-Type': 'application/json' },
    });
    return { nom: choisie.nom ?? choisie.cle, id: choisie.id, statut: l.status };
}

/// La distribution. ⚠️ Le rang du percentile est celui de la liste TRIÉE, et
/// une liste vide ne rend pas 0 — elle rend `null` : un zéro serait un chiffre
/// là où il n'y a pas de mesure.
function distribution(valeurs) {
    if (valeurs.length === 0) {
        return { n: 0, mediane_ms: null, p90_ms: null, p99_ms: null, min_ms: null, max_ms: null };
    }
    const t = [...valeurs].sort((a, b) => a - b);
    const q = (p) => t[Math.min(t.length - 1, Math.floor(p * (t.length - 1)))];
    return {
        n: t.length,
        mediane_ms: q(0.5), p90_ms: q(0.9), p99_ms: q(0.99),
        min_ms: t[0], max_ms: t[t.length - 1],
    };
}

const releve = {
    etiquette: ETIQUETTE, regime_fenetres: FENETRES, duree_demandee_s: DUREE_S,
    instant: new Date().toISOString(), bras: {},
};

const SONDE = await readFile(join(ICI, 'sonde-latence.js'), 'utf8');
const profil = await mkdtemp(join(tmpdir(), 'lot3-latence-'));
const chrome = spawn('google-chrome', [
    '--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profil}`,
    '--no-sandbox', '--no-first-run', '--disable-popup-blocking',
    // ⚠️ Les TROIS drapeaux exigés par toute mesure de plus de 5 minutes.
    '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
    '--disable-renderer-backgrounding',
    '--autoplay-policy=no-user-gesture-required',
    'about:blank',
], { stdio: 'ignore' });

const AMORCE = `(() => {
  const O = window.RTCPeerConnection;
  window.__pcLot3 = [];
  window.RTCPeerConnection = function (...a) {
    const p = new O(...a); window.__pcLot3.push(p); return p;
  };
  window.RTCPeerConnection.prototype = O.prototype;
})()`;
let navigateur = null;

try {
    await attendreDevtools(PORT);
    {
        const v = await (await fetch(`http://127.0.0.1:${PORT}/json/version`)).json();
        navigateur = new Navigateur(v.webSocketDebuggerUrl, AMORCE);
        await navigateur.envoyer('Target.setAutoAttach',
            { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
        log('amorce armée au niveau navigateur (Target.setAutoAttach)');
    }

    // ── BRAS TÉMOIN NÉGATIF ────────────────────────────────────────────────
    // 🔴 LA SONDE DOIT POUVOIR NE RIEN RENDRE. Injectée sur une page SANS
    // flux vidéo, elle rend un tableau vide. Sans ce bras, un tableau non
    // vide plus bas ne prouverait pas qu'il vient du flux : il pourrait
    // venir d'une sonde qui fabrique.
    const vide = new Cdp((await cibles()).find((c) => c.type === 'page').webSocketDebuggerUrl);
    await vide.envoyer('Runtime.enable');
    const attache_vide = await vide.evaluer(SONDE);
    await dormir(3000);
    releve.bras.temoin_negatif = {
        page: 'about:blank (aucun <video>)',
        attache: attache_vide,
        echantillons: await vide.evaluer('window.__latenceLot3()'),
        compteurs: await vide.evaluer('window.__latenceLot3Compteurs()'),
    };
    log('témoin négatif : ' + JSON.stringify(releve.bras.temoin_negatif));

    // ── LA SESSION ─────────────────────────────────────────────────────────
    const j = await jeton();
    const vm = await laVm(j);
    releve.vm = vm?.id;
    const p = await prefixe(j, vm);
    releve.prefixe = p;

    const page = (await cibles()).find((c) => c.type === 'page');
    const shell = new Cdp(page.webSocketDebuggerUrl);
    await shell.envoyer('Page.enable');
    await shell.envoyer('Runtime.enable');
    await shell.envoyer('Page.navigate', { url: `${PLATEFORME}/` });
    await dormir(2500);
    await shell.evaluer(
        `localStorage.setItem('guac.jeton.acces', ${JSON.stringify(j)});`
        + `localStorage.setItem('guac.prefixe', ${JSON.stringify(p)});'ok'`);
    await shell.envoyer('Page.navigate', { url: `${PLATEFORME}/shell.html` });
    await dormir(4000);

    releve.lancements = [];
    for (let k = 0; k < FENETRES; k += 1) {
        releve.lancements.push(await lancerUneApplication(j, vm.id, APP));
        log(`application ${k + 1}/${FENETRES} lancée : ${JSON.stringify(releve.lancements[k])}`);
        await dormir(3000);
    }

    let sessions = [];
    releve.cibles_vues = [];
    for (let i = 0; i < 90 && sessions.length < FENETRES; i += 1) {
        await dormir(1000);
        const vues = await cibles();
        // 🔴 ON JOURNALISE CE QU'ON VOIT : un échec muet est indiscernable
        // d'un produit en panne.
        const empreinte = vues.map((c) => `${c.type}:${c.url}`).sort().join(' ~ ');
        if (releve.cibles_vues[releve.cibles_vues.length - 1] !== empreinte) {
            releve.cibles_vues.push(empreinte);
            log(`cibles (t=${i}s) : ${empreinte}`);
        }
        const connues = new Set(sessions.map((s) => s.url));
        for (const c of vues) {
            if (c.type === 'page' && /session=/.test(c.url) && !connues.has(c.url)) {
                const cdp = new Cdp(c.webSocketDebuggerUrl);
                await cdp.envoyer('Runtime.enable');
                sessions.push({ url: c.url, cdp });
                log(`page de session ${sessions.length} : ${c.url}`);
            }
        }
    }
    if (sessions.length === 0) {
        releve.shell = await shell.evaluer(`(() => ({
            texte: document.body.innerText.slice(0, 1200),
            boutons: [...document.querySelectorAll('button,a,[role=button]')].map((e) => e.textContent.trim()).slice(0, 30),
        }))()`);
        throw new Error('aucune page de session : voir releve.shell');
    }
    releve.sessions_obtenues = sessions.length;

    // ── LA MESURE ──────────────────────────────────────────────────────────
    // La sonde s'attache APRÈS que le <video> existe : `querySelectorAll` ne
    // voit que ce qui est là. On attend donc que l'élément soit présent, et
    // on DIT combien de temps on a attendu.
    // 🔴 LE DIAGNOSTIC D'HORLOGE, ET POURQUOI IL EST DANS CE PILOTE.
    // `captureTime` peut manquer pour DEUX raisons qui ne se ressemblent pas :
    // l'agent n'annonce pas l'instant de capture au pair, ou le navigateur ne
    // le rend pas. Les confondre attribuerait au produit un défaut du
    // navigateur, ou l'inverse. On tranche par `getStats()` : la présence
    // d'un `remote-outbound-rtp` portant `remoteTimestamp` PROUVE que des
    // sender reports RTCP arrivent et sont lus.
    //
    // La `RTCPeerConnection` du client n'est exposée nulle part (vérifié :
    // `client/src/webrtc.ts:249` la garde locale), donc on l'attrape en
    // enveloppant le constructeur AVANT le chargement du document, puis en
    // rechargeant la page. ⚠️ Ce n'est pas un changement de produit : le
    // client n'est pas modifié, l'enveloppe vit le temps de la mesure.
    const ENVELOPPE = `(() => {
      const O = window.RTCPeerConnection;
      window.__pcLot3 = [];
      window.RTCPeerConnection = function (...a) {
        const p = new O(...a); window.__pcLot3.push(p); return p;
      };
      window.RTCPeerConnection.prototype = O.prototype;
    })()`;
    // ⚠️ LE RECHARGEMENT EST HORS DÉFAUT, et il faut dire pourquoi. MESURÉ le
    // 5 septembre 2026 : rechargée, la page de session cesse de répondre à
    // `Runtime.evaluate` (« CDP Runtime.evaluate sans réponse après 20 s »).
    // Le rôle `client` est EXCLUSIF par session : la page rechargée demande
    // un rôle que la page d'avant n'a pas encore rendu. Le diagnostic
    // d'horloge se fait donc sur le fil (voir `sr-rtcp.sh`), pas ici.
    if (arg('enveloppe', '0') === '1') {
        for (const s of sessions) {
            await s.cdp.envoyer('Page.enable');
            await s.cdp.envoyer('Page.addScriptToEvaluateOnNewDocument', { source: ENVELOPPE });
            await s.cdp.envoyer('Page.navigate', { url: s.url });
            await dormir(2500);
        }
    }
    for (const s of sessions) {
        for (let i = 0; i < 40; i += 1) {
            const n = await s.cdp.evaluer('document.querySelectorAll("video").length');
            if (n > 0) { s.attente_video_s = i; break; }
            await dormir(1000);
        }
        s.attache = await s.cdp.evaluer(SONDE);
        log(`sonde attachée sur ${s.url} : ${JSON.stringify(s.attache)} (après ${s.attente_video_s ?? '>40'} s)`);
    }

    // 🔴 UNE MIRE IMMOBILE NE PRODUIT AUCUNE IMAGE : Desktop Duplication
    // n'émet qu'au CHANGEMENT. Il faut donc animer la source. On le fait par
    // le CHEMIN DU PRODUIT — un `pointermove` sur l'élément vidéo, exactement
    // ce que `client/src/input.ts` écoute — qui déplace le curseur distant,
    // lequel est composité dans la capture. La cadence est CONNUE (30 Hz) et
    // elle est inscrite dans le relevé.
    //
    // ⚠️ CE N'EST PAS LA MIRE DU PLAN. `it-mire.ps1` ouvre un Chrome DANS
    // l'invité ; or la règle d'appartenance (lot 32I) ÉCARTE toute fenêtre que
    // desk n'a pas lancée, et desk ne lance pas ce Chrome-là. Le relevé porte
    // la substitution : ce n'est pas un détail d'instrument, c'est un autre
    // stimulus, et un lecteur doit pouvoir le savoir.
    releve.animation = { moyen: 'pointermove sur <video>', cadence_hz: 30 };
    if (arg('animer', '1') === '1') {
        for (const s of sessions) {
            await s.cdp.evaluer(`(() => {
              const v = document.querySelector('video');
              if (!v) return 'aucun <video>';
              let n = 0;
              window.__animLot3 = setInterval(() => {
                const e = v.getBoundingClientRect();
                const sw = v.videoWidth, sh = v.videoHeight;
                let r = e;
                if (sw && sh) {
                  const k = Math.min(e.width / sw, e.height / sh);
                  r = new DOMRect(e.x + (e.width - k * sw) / 2, e.y + (e.height - k * sh) / 2, k * sw, k * sh);
                }
                const a = (n++) * 0.12;
                v.dispatchEvent(new PointerEvent('pointermove', {
                  clientX: r.x + r.width * (0.5 + 0.35 * Math.cos(a)),
                  clientY: r.y + r.height * (0.5 + 0.35 * Math.sin(a)),
                  bubbles: true, pointerId: 1, pointerType: 'mouse' }));
              }, 33);
              return 'animation armée';
            })()`);
        }
        log('animation armée sur ' + sessions.length + ' session(s)');
    } else {
        releve.animation = { moyen: 'aucun', cadence_hz: 0 };
    }

    // ── ITEM 12 (3.10) : PROVOQUER DES `Resize`, POUR SUIVRE LE MAILLON ────
    // Le client emet un `Resize` sur son propre `ResizeObserver`
    // (client/src/resize-dom.ts, cable par main.ts). On change donc la
    // METRIQUE DE LA PAGE — le chemin du produit — et non une taille interne.
    // ⚠️ Chaque taille est TENUE assez longtemps pour que le lissage du client
    // la laisse partir ; une rafale serait coalescee et le journal ne porterait
    // qu'une demande.
    const TAILLES = arg('resize', '').split(',').filter(Boolean);
    if (TAILLES.length) {
        releve.resize = [];
        for (const t of TAILLES) {
            const [l, h] = t.split('x').map(Number);
            for (const s of sessions) {
                await s.cdp.envoyer('Emulation.setDeviceMetricsOverride',
                    { width: l, height: h, deviceScaleFactor: 1, mobile: false });
            }
            await dormir(6000);
            const vu = await sessions[0].cdp.evaluer(`(() => {
              const v = document.querySelector('video');
              return { innerWidth, innerHeight, dpr: devicePixelRatio,
                       video: v ? { cw: v.clientWidth, ch: v.clientHeight,
                                    vw: v.videoWidth, vh: v.videoHeight } : null };
            })()`);
            releve.resize.push({ demande: t, vu });
            log(`resize ${t} -> ${JSON.stringify(vu)}`);
        }
        for (const s of sessions) {
            await s.cdp.envoyer('Emulation.clearDeviceMetricsOverride').catch(() => {});
        }
    }

    log(`mesure en cours : ${DUREE_S} s, régime ${FENETRES} fenêtre(s)`);
    await dormir(DUREE_S * 1000);
    for (const s of sessions) {
        await s.cdp.evaluer('clearInterval(window.__animLot3);"arrêtée"').catch(() => {});
    }

    releve.amorces_posees = navigateur ? navigateur.posees.length : 0;
    releve.bras.mesure = [];
    const toutes = [];
    for (const s of sessions) {
        const ech = await s.cdp.evaluer('window.__latenceLot3()');
        const cpt = await s.cdp.evaluer('window.__latenceLot3Compteurs()');
        const lat = ech.map((e) => e.latence_ms);
        toutes.push(...lat);
        // Le diagnostic d'horloge : ce que le pair annonce réellement.
        const horloge = await s.cdp.evaluer(`(async () => {
          const p = (window.__pcLot3 || [])[0];
          if (!p) return { pc: 'AUCUNE (enveloppe non posée avant le chargement)' };
          const st = await p.getStats();
          const r = { pc: 'attrapée', types: {}, remote_outbound: [], inbound: [], sources: [] };
          st.forEach((v) => { r.types[v.type] = (r.types[v.type] || 0) + 1;
            if (v.type === 'remote-outbound-rtp') r.remote_outbound.push(
              { kind: v.kind, remoteTimestamp: v.remoteTimestamp, packetsSent: v.packetsSent, reportsSent: v.reportsSent });
            if (v.type === 'inbound-rtp' && v.kind === 'video') r.inbound.push(
              { framesDecoded: v.framesDecoded, framesReceived: v.framesReceived,
                estimatedPlayoutTimestamp: v.estimatedPlayoutTimestamp,
                jitterBufferDelay: v.jitterBufferDelay, jitterBufferEmittedCount: v.jitterBufferEmittedCount,
                // 🔴 LE SUBSTITUT. totalProcessingDelay court du PREMIER
                // PAQUET RECU de la trame a l'instant ou elle est rendue au
                // pipeline d'affichage : c'est le seul segment de la chaine
                // que le navigateur date des DEUX cotes. Il ne contient NI la
                // capture, NI l'encodage, NI le reseau.
                totalProcessingDelay: v.totalProcessingDelay,
                totalAssemblyTime: v.totalAssemblyTime,
                framesAssembledFromMultiplePackets: v.framesAssembledFromMultiplePackets,
                totalDecodeTime: v.totalDecodeTime,
                totalInterFrameDelay: v.totalInterFrameDelay,
                frameWidth: v.frameWidth, frameHeight: v.frameHeight,
                framesPerSecond: v.framesPerSecond, bytesReceived: v.bytesReceived });
            if (v.type === 'candidate-pair' && v.state === 'succeeded' && v.nominated) r.paire = {
                currentRoundTripTime: v.currentRoundTripTime,
                totalRoundTripTime: v.totalRoundTripTime,
                responsesReceived: v.responsesReceived,
                availableIncomingBitrate: v.availableIncomingBitrate };
          });
          for (const rc of p.getReceivers()) {
            if (rc.track && rc.track.kind === 'video' && rc.getSynchronizationSources) {
              r.sources.push(...rc.getSynchronizationSources().map((x) => ({
                source: x.source, timestamp: x.timestamp, captureTimestamp: x.captureTimestamp,
                senderCaptureTimeOffset: x.senderCaptureTimeOffset })));
            }
          }
          return r;
        })()`);
        releve.bras.mesure.push({
            url: s.url, attente_video_s: s.attente_video_s ?? null,
            compteurs: cpt, distribution: distribution(lat),
            diagnostic_horloge: horloge,
            echantillons_bruts: ech.slice(0, 400),
        });
        log(`horloge ${s.url} : ${JSON.stringify(horloge).slice(0, 700)}`);
        log(`${s.url} → ${JSON.stringify(distribution(lat))} (compteurs ${JSON.stringify(cpt)})`);
    }
    releve.distribution_agregee = distribution(toutes);
    log('AGRÉGÉ : ' + JSON.stringify(releve.distribution_agregee));
} catch (e) {
    releve.erreur = String(e && e.message ? e.message : e);
    log('ERREUR : ' + releve.erreur);
} finally {
    chrome.kill('SIGKILL');
    // 🔴 LE PROFIL CHROME PART AVEC LE PILOTE. Chaque exécution en laissait un
    // de ~100 Mio sous `tmpdir()` ; vingt-trois d'entre eux ont REMPLI le
    // tmpfs de 10 Gio de cette machine le 5 septembre 2026, et le symptôme
    // n'était pas « disque plein » mais **une commande qui ne rend RIEN** —
    // l'enveloppe du shell n'arrivant plus à écrire la sortie de l'enfant.
    await rm(profil, { recursive: true, force: true }).catch(() => {});
    await writeFile(SORTIE, JSON.stringify(releve, null, 2));
    log(`relevé écrit : ${SORTIE}`);
}
