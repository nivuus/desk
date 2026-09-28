#!/usr/bin/env node
// Driver of acceptance run E1 (project E — microphone), task 14 of the plan
// `docs/superpowers/plans/2026-08-19-micro.md`.
//
// 🔵 NO MICROPHONE. The upstream source is an `OscillatorNode` routed to a
// `MediaStreamAudioDestinationNode`, passed to the `sender` of the `sendonly` transceiver
// through `replaceTrack` — decision 8 of the plan. No permission is requested,
// no input device is required: the VM has none, and this driver
// runs on a headless Chrome.
//
// 🔵 THE VERDICT IS READ IN `agent.log`, NOT HERE. This driver establishes the session,
// injects the tone, and reads what the BROWSER sees (`getStats`); the
// recovered frequency is judged on the agent side, on the decoded PCM
// (`demarrage/micro.rs`, trace "mic measured"). A byte count does not
// tell "some sound" from "MY sound" — a doctrine paid for in sub-block D7.
//
// ⚠️ THE `sender` DOES NOT COME FROM A CHANGE TO THE CLIENT. The
// `RTCPeerConnection` constructor is intercepted in the page (same technique as
// `FORCER_RELAIS` of `paire-candidats.mjs`), and the transceiver is found by
// its `sendonly` DIRECTION, the only one of its kind — never by a position
// index, which the order of the `addTransceiver` calls would make fragile.
//
// Usage:
//   node recette/micro-e1.mjs --scenario ton|silence --session <id> --freq <Hz> \
//        --duree <ms> [--url <url>] [--sortie <file.json>]
//
// ⚠️ Set `RECETTE_EMAIL`, `RECETTE_MOTDEPASSE` and if needed `PLATEFORME_URL`:
// since sub-block P2 a `client` peer without a token is REFUSED by the guard.

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

/// The interception: it must run BEFORE the page script, hence through
/// `Page.addScriptToEvaluateOnNewDocument` and not through an evaluation after
/// navigation. ⚠️ It would NOT run on a page opened by `window.open`
/// (a trap measured in sub-block D5) — this driver navigates directly.
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

/// Starts an oscillator at `hz` and sets it on the `sender` of the
/// `sendonly` transceiver. Returns a report, never a `Window` object — a
/// `Runtime.evaluate` that returned a window reference fails with
/// "Object reference chain is too long" (trap of sub-block D5).
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

/// Stops the oscillator WITHOUT touching the track: `replaceTrack(null)` would cut
/// the whole RTP stream, whereas criterion ③ exercises precisely what
/// the agent does when the BROWSER goes quiet (DTX) on a track still in
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
            // Without it, the `AudioContext` is born "suspended" and the oscillator
            // produces nothing: the measurement would read a silence of the INSTRUMENT.
            '--autoplay-policy=no-user-gesture-required',
            '--disable-features=WebRtcHideLocalIpsWithMdns',
            // A page never brought to the foreground freezes after 5 minutes
            // (trap of the TURN project): these three flags prevent it.
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
        // ⚠️ `rm` RUNS AFTER `kill`, AND CHROME IS STILL WRITING: an `rmdir` on
        // `Default/` threw `ENOTEMPTY` and took away the report of a
        // whole run, although the measurement itself had succeeded. Cleaning
        // up a temporary directory must never cost a report.
        await dormir(1500);
        await rm(profil, { recursive: true, force: true }).catch((e) => {
            console.warn(`incomplete Chrome profile clean-up (no effect on the measurement): ${e.message}`);
        });
    }
}

async function principal() {
    const journal = { scenario, session, freq, duree, url, etapes: [], stats: [] };
    const code = await avecChrome(async (cdp) => {
        await cdp.send('Page.navigate', { url });
        // We wait for the FACT — a video track that decodes —, not a duration.
        // `Session::run()` is the only loop that drains `poll_output`, hence the
        // only one that will ever see an upstream `MediaData`: without a live session,
        // the measurement says nothing (trap of sub-block D8).
        const vivante = await jusqua(async () => {
            const brut = await cdp.eval(RELEVE_STATS, true);
            const s = JSON.parse(brut ?? '{}');
            return (s.entrant ?? []).some((e) => e.kind === 'video' && (e.framesDecoded ?? 0) > 0);
        }, 90000);
        journal.sessionVivante = vivante;
        if (!vivante) {
            journal.etapes.push({ quoi: 'session', issue: 'no image decoded in 90 s' });
            return 1;
        }
        journal.etapes.push({ quoi: 'session', issue: 'vivante', quand: new Date().toISOString() });

        const releverStats = async (etiquette) => {
            const s = JSON.parse((await cdp.eval(RELEVE_STATS, true)) ?? '{}');
            s.etiquette = etiquette;
            journal.stats.push(s);
        };
        await releverStats('before-injection');

        journal.etapes.push({
            quoi: 'allumage',
            quand: new Date().toISOString(),
            compte: JSON.parse(await cdp.eval(allumer(freq), true)),
        });

        if (scenario === 'ton') {
            await dormir(duree);
            await releverStats('fin-ton');
        } else if (scenario === 'silence') {
            // 20 s of tone, 60 s of silence, 30 s of tone: criterion ③ is read
            // on the CONTINUITY of the "mic measured" lines during the gap.
            await dormir(20000);
            await releverStats('before-silence');
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
            throw new Error(`unknown scenario: ${scenario}`);
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
