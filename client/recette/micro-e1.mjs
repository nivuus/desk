#!/usr/bin/env node
// Pilote de la recette E1 (chantier E — microphone), tâche 14 du plan
// `docs/superpowers/plans/2026-08-19-micro.md`.
//
// 🔵 AUCUN MICROPHONE. La source montante est un `OscillatorNode` routé vers un
// `MediaStreamAudioDestinationNode`, passé au `sender` du transceiver `sendonly`
// par `replaceTrack` — décision 8 du plan. Aucune permission n'est demandée,
// aucun périphérique d'entrée n'est requis : la VM n'en a aucun, et ce pilote
// tourne sur un Chrome sans interface.
//
// 🔵 LE VERDICT SE LIT DANS `agent.log`, PAS ICI. Ce pilote établit la session,
// injecte le ton, et relève ce que le NAVIGATEUR voit (`getStats`) ; la
// fréquence retrouvée est jugée côté agent, sur le PCM décodé
// (`demarrage/micro.rs`, trace « micro mesuré »). Un compte d'octets ne
// distingue pas « du son » de « MON son » — doctrine payée au sous-bloc D7.
//
// ⚠️ LE `sender` NE VIENT PAS D'UNE MODIFICATION DU CLIENT. Le constructeur
// `RTCPeerConnection` est intercepté dans la page (même technique que
// `FORCER_RELAIS` de `paire-candidats.mjs`), et le transceiver est retrouvé par
// sa DIRECTION `sendonly`, seule de son espèce — jamais par un index de
// position, que l'ordre des `addTransceiver` rendrait fragile.
//
// Usage :
//   node recette/micro-e1.mjs --scenario ton|silence --session <id> --freq <Hz> \
//        --duree <ms> [--url <url>] [--sortie <fichier.json>]
//
// ⚠️ Poser `RECETTE_EMAIL`, `RECETTE_MOTDEPASSE` et au besoin `PLATEFORME_URL` :
// depuis le sous-bloc P2 un pair `client` sans jeton est REFUSÉ par la garde.

import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { attendreDevtools } from './devtools.mjs';
import { semerJeton } from './jeton-recette.mjs';

function argument(nom, defaut) {
    const i = process.argv.indexOf(`--${nom}`);
    return i >= 0 && process.argv[i + 1] !== undefined ? process.argv[i + 1] : defaut;
}

const scenario = argument('scenario', 'ton');
const session = argument('session', 'micro-e1');
const freq = Number(argument('freq', '440'));
const duree = Number(argument('duree', '60000'));
const url = argument('url', `http://192.168.3.1:5173/?session=${session}`);
const sortie = argument('sortie', '');
const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.ready = new Promise((r) => this.ws.addEventListener('open', () => r(), { once: true }));
        this.ws.addEventListener('message', (e) => this.onMessage(e));
        this.consoleLines = [];
        this.pageErrors = [];
    }
    onMessage(event) {
        const m = JSON.parse(String(event.data));
        if (m.id !== undefined && this.pending.has(m.id)) {
            const { resolve, reject } = this.pending.get(m.id);
            this.pending.delete(m.id);
            if (m.error) reject(new Error(JSON.stringify(m.error)));
            else resolve(m.result);
            return;
        }
        if (m.method === 'Runtime.consoleAPICalled') {
            const t = (m.params.args ?? []).map((a) => a.value ?? a.description ?? '').join(' ');
            this.consoleLines.push(`[${m.params.type}] ${t}`);
        }
        if (m.method === 'Runtime.exceptionThrown') {
            this.pageErrors.push(m.params.exceptionDetails.text);
        }
    }
    async send(method, params = {}) {
        await this.ready;
        const id = this.nextId++;
        return new Promise((resolve, reject) => {
            this.pending.set(id, { resolve, reject });
            this.ws.send(JSON.stringify({ id, method, params }));
        });
    }
    async eval(expression, awaitPromise = false) {
        const r = await this.send('Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
        if (r.exceptionDetails) {
            throw new Error(r.exceptionDetails.exception?.description ?? JSON.stringify(r.exceptionDetails));
        }
        return r.result.value;
    }
    close() {
        this.ws.close();
    }
}

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

async function jusqua(predicat, plafondMs) {
    const fin = Date.now() + plafondMs;
    while (Date.now() < fin) {
        if (await predicat()) return true;
        await dormir(250);
    }
    return false;
}

/// L'interception : elle doit courir AVANT le script de la page, donc par
/// `Page.addScriptToEvaluateOnNewDocument` et non par une évaluation après
/// navigation. ⚠️ Elle ne courrait PAS sur une page ouverte par `window.open`
/// (piège mesuré au sous-bloc D5) — ce pilote navigue directement.
const INTERCEPTION = `
    window.__pc = null;
    const Natif = window.RTCPeerConnection;
    window.RTCPeerConnection = function (...args) {
        const pc = new Natif(...args);
        window.__pc = pc;
        return pc;
    };
    window.RTCPeerConnection.prototype = Natif.prototype;
`;

/// Allume un oscillateur à `hz` et le pose sur le `sender` du transceiver
/// `sendonly`. Rend un compte rendu, jamais un objet `Window` — un
/// `Runtime.evaluate` qui rendrait une référence de fenêtre échoue en
/// « Object reference chain is too long » (piège du sous-bloc D5).
const allumer = (hz) => `
    (async () => {
        const pc = window.__pc;
        if (!pc) return JSON.stringify({ erreur: 'aucune RTCPeerConnection interceptee' });
        const tr = pc.getTransceivers().find((t) => t.direction === 'sendonly');
        if (!tr) {
            return JSON.stringify({
                erreur: 'aucun transceiver sendonly',
                directions: pc.getTransceivers().map((t) => t.direction + '/' + t.currentDirection),
            });
        }
        if (!window.__ctx) {
            window.__ctx = new AudioContext({ sampleRate: 48000 });
            window.__dest = window.__ctx.createMediaStreamDestination();
        }
        if (window.__ctx.state !== 'running') await window.__ctx.resume();
        const osc = window.__ctx.createOscillator();
        osc.frequency.value = ${hz};
        osc.connect(window.__dest);
        osc.start();
        window.__osc = osc;
        const piste = window.__dest.stream.getAudioTracks()[0];
        await tr.sender.replaceTrack(piste);
        window.__tr = tr;
        return JSON.stringify({
            hz: ${hz},
            etatContexte: window.__ctx.state,
            direction: tr.direction,
            directionCourante: tr.currentDirection,
            pisteVivante: piste.readyState,
        });
    })()
`;

/// Coupe l'oscillateur SANS toucher à la piste : `replaceTrack(null)` couperait
/// le flux RTP tout entier, alors que le critère ③ éprouve précisément ce que
/// fait l'agent quand le NAVIGATEUR se tait (DTX) sur une piste toujours en
/// place.
const eteindre = `
    (() => {
        if (!window.__osc) return JSON.stringify({ erreur: 'aucun oscillateur' });
        window.__osc.stop();
        window.__osc.disconnect();
        window.__osc = null;
        return JSON.stringify({ eteint: true });
    })()
`;

const RELEVE_STATS = `
    (async () => {
        const pc = window.__pc;
        if (!pc) return JSON.stringify({ erreur: 'aucune RTCPeerConnection' });
        const s = await pc.getStats();
        const out = { entrant: [], sortant: [], paire: null };
        s.forEach((r) => {
            if (r.type === 'inbound-rtp') {
                out.entrant.push({
                    kind: r.kind,
                    packetsReceived: r.packetsReceived,
                    packetsLost: r.packetsLost,
                    bytesReceived: r.bytesReceived,
                    framesDecoded: r.framesDecoded,
                    framesDropped: r.framesDropped,
                    frameWidth: r.frameWidth,
                    frameHeight: r.frameHeight,
                });
            }
            if (r.type === 'outbound-rtp') {
                out.sortant.push({
                    kind: r.kind,
                    packetsSent: r.packetsSent,
                    bytesSent: r.bytesSent,
                    mid: r.mid,
                });
            }
            if (r.type === 'candidate-pair' && r.nominated && r.state === 'succeeded') {
                out.paire = { rtt: r.currentRoundTripTime, recu: r.bytesReceived, envoye: r.bytesSent };
            }
        });
        out.horodatage = new Date().toISOString();
        return JSON.stringify(out);
    })()
`;

async function avecChrome(fn) {
    const port = 9222 + Math.floor(Math.random() * 1000);
    const profil = await mkdtemp(join(tmpdir(), 'chrome-micro-e1-'));
    const chrome = spawn(
        chromeBin,
        [
            '--headless=new',
            `--remote-debugging-port=${port}`,
            '--remote-allow-origins=*',
            `--user-data-dir=${profil}`,
            '--no-sandbox',
            '--disable-dev-shm-usage',
            '--disable-gpu',
            // Sans lui, l'`AudioContext` naît « suspended » et l'oscillateur ne
            // produit rien : la mesure lirait un silence de l'INSTRUMENT.
            '--autoplay-policy=no-user-gesture-required',
            '--disable-features=WebRtcHideLocalIpsWithMdns',
            // Une page jamais mise au premier plan gèle au bout de 5 minutes
            // (piège du chantier TURN) : ces trois drapeaux l'en empêchent.
            '--disable-background-timer-throttling',
            '--disable-backgrounding-occluded-windows',
            '--disable-renderer-backgrounding',
            'about:blank',
        ],
        { stdio: 'ignore' },
    );
    try {
        await attendreDevtools(port);
        const cree = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
        const cdp = new Cdp(cree.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');
        await semerJeton(cdp);
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: INTERCEPTION });
        try {
            return await fn(cdp);
        } finally {
            cdp.close();
        }
    } finally {
        chrome.kill();
        // ⚠️ `rm` COURT APRÈS `kill`, ET CHROME ÉCRIT ENCORE : un `rmdir` sur
        // `Default/` a levé `ENOTEMPTY` et emporté le compte rendu d'une
        // exécution entière, alors que la mesure, elle, avait abouti. Le
        // ménage d'un répertoire temporaire ne doit jamais coûter un relevé.
        await dormir(1500);
        await rm(profil, { recursive: true, force: true }).catch((e) => {
            console.warn(`ménage du profil Chrome incomplet (sans effet sur la mesure) : ${e.message}`);
        });
    }
}

async function principal() {
    const journal = { scenario, session, freq, duree, url, etapes: [], stats: [] };
    const code = await avecChrome(async (cdp) => {
        await cdp.send('Page.navigate', { url });
        // On attend le FAIT — une piste vidéo qui décode —, pas une durée.
        // `Session::run()` est la seule boucle qui draine `poll_output`, donc la
        // seule qui verra jamais un `MediaData` montant : sans session vivante,
        // la mesure ne dit rien (piège du sous-bloc D8).
        const vivante = await jusqua(async () => {
            const brut = await cdp.eval(RELEVE_STATS, true);
            const s = JSON.parse(brut ?? '{}');
            return (s.entrant ?? []).some((e) => e.kind === 'video' && (e.framesDecoded ?? 0) > 0);
        }, 90000);
        journal.sessionVivante = vivante;
        if (!vivante) {
            journal.etapes.push({ quoi: 'session', issue: 'aucune image decodee en 90 s' });
            return 1;
        }
        journal.etapes.push({ quoi: 'session', issue: 'vivante', quand: new Date().toISOString() });

        const releverStats = async (etiquette) => {
            const s = JSON.parse((await cdp.eval(RELEVE_STATS, true)) ?? '{}');
            s.etiquette = etiquette;
            journal.stats.push(s);
        };
        await releverStats('avant-injection');

        journal.etapes.push({
            quoi: 'allumage',
            quand: new Date().toISOString(),
            compte: JSON.parse(await cdp.eval(allumer(freq), true)),
        });

        if (scenario === 'ton') {
            await dormir(duree);
            await releverStats('fin-ton');
        } else if (scenario === 'silence') {
            // 20 s de ton, 60 s de silence, 30 s de ton : le critère ③ se lit
            // sur la CONTINUITÉ des lignes « micro mesuré » pendant le creux.
            await dormir(20000);
            await releverStats('avant-silence');
            journal.etapes.push({
                quoi: 'extinction',
                quand: new Date().toISOString(),
                compte: JSON.parse(await cdp.eval(eteindre, true)),
            });
            await dormir(60000);
            await releverStats('fin-silence');
            journal.etapes.push({
                quoi: 'rallumage',
                quand: new Date().toISOString(),
                compte: JSON.parse(await cdp.eval(allumer(freq), true)),
            });
            await dormir(30000);
            await releverStats('fin-reprise');
        } else {
            throw new Error(`scénario inconnu : ${scenario}`);
        }
        journal.console = cdp.consoleLines.slice(-40);
        journal.erreursPage = cdp.pageErrors;
        return 0;
    });
    journal.code = code;
    const texte = JSON.stringify(journal, null, 2);
    if (sortie) await writeFile(sortie, texte);
    console.log(texte);
    process.exit(code);
}

await principal();
