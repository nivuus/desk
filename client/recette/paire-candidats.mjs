#!/usr/bin/env node
// Acceptance probe (project C part 2, NAT traversal): reads the candidate
// pair ACTUALLY used by the browser, the type of the two candidates
// that make it up, its RTT and its bitrate.
//
// Why it exists: `verify-webrtc.mjs` proves that the stream gets through, but
// does not say WHICH WAY. Yet that is exactly the question of this project — a
// session may very well work directly and prove nothing about the relay.
// The type (`host` / `srflx` / `relay`) can only be read in the
// `local-candidate` / `remote-candidate` entries paired with the nominated pair.
//
// Usage :
//   node client/recette/paire-candidats.mjs [url] [dureeMs]
//   FORCER_RELAIS=1 node client/recette/paire-candidats.mjs   (iceTransportPolicy: 'relay')
//
// ⚠️ SINCE SUB-BLOCK P2, THIS INVOCATION IS NO LONGER ENOUGH against a guarded
// service: also set `RECETTE_EMAIL`, `RECETTE_MOTDEPASSE`, and `PLATEFORME_URL`
// if the service does not listen on http://127.0.0.1:8080. See
// `recette/jeton-recette.mjs` — without them, the tool WARNS and carries on.

//
// `FORCER_RELAIS` applies by intercepting the constructor of
// `RTCPeerConnection` in the page, without touching the client code: the
// change does not have to be committed then removed, unlike the hardcoded
// setting the plan suggested.
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { attendreDevtools } from './devtools.mjs';
import { semerJeton } from './jeton-recette.mjs';

const url = process.argv[2] ?? 'http://127.0.0.1:5174/?session=demo';
const durationMs = Number(process.argv[3] ?? 10000);
const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';
const forcerRelais = process.env.FORCER_RELAIS === '1';

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.ready = new Promise((resolve) =>
            this.ws.addEventListener('open', () => resolve(), { once: true }),
        );
        this.ws.addEventListener('message', (event) => this.onMessage(event));
    }
    onMessage(event) {
        const message = JSON.parse(String(event.data));
        if (message.id !== undefined && this.pending.has(message.id)) {
            const { resolve, reject } = this.pending.get(message.id);
            this.pending.delete(message.id);
            if (message.error) reject(new Error(JSON.stringify(message.error)));
            else resolve(message.result);
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
        const result = await this.send('Runtime.evaluate', {
            expression,
            awaitPromise,
            returnByValue: true,
        });
        if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
        return result.result.value;
    }
    close() {
        this.ws.close();
    }
}

/// Reads the pair in use, its two candidates, and the video counters.
///
/// The retained pair is the one marked `selected` (Chrome), failing that
/// `nominated`: both exist, and a nominated pair is not
/// necessarily the one carrying the traffic.
async function relever(cdp) {
    return cdp.eval(
        `(async () => {
            const pc = window.__pc;
            if (!pc) return null;
            const report = await pc.getStats();
            const toutes = Array.from(report.values());
            const paire = toutes.find(s => s.type === 'candidate-pair' && s.selected)
                ?? toutes.find(s => s.type === 'candidate-pair' && s.nominated)
                ?? toutes.find(s => s.type === 'candidate-pair' && s.state === 'succeeded');
            if (!paire) {
                // Diagnosis: without a pair in use, it is the negotiation
                // states and the collected candidates that say why.
                return {
                    paire: null,
                    etat: pc.iceConnectionState,
                    etatSignalisation: pc.signalingState,
                    etatCollecte: pc.iceGatheringState,
                    etatConnexion: pc.connectionState,
                    candidatsLocaux: toutes.filter(s => s.type === 'local-candidate')
                        .map(c => c.candidateType + ' ' + c.address + ':' + c.port),
                    candidatsDistants: toutes.filter(s => s.type === 'remote-candidate')
                        .map(c => c.candidateType + ' ' + c.address + ':' + c.port),
                    pairesEtats: toutes.filter(s => s.type === 'candidate-pair').map(p => p.state),
                };
            }
            const local = report.get(paire.localCandidateId);
            const distant = report.get(paire.remoteCandidateId);
            const video = toutes.find(s => s.type === 'inbound-rtp' && s.kind === 'video');
            return {
                etat: pc.iceConnectionState,
                typeLocal: local?.candidateType, adresseLocale: local?.address + ':' + local?.port,
                protocoleLocal: local?.protocol, relayLocal: local?.relayProtocol,
                typeDistant: distant?.candidateType, adresseDistante: distant?.address + ':' + distant?.port,
                rttCourant: paire.currentRoundTripTime, rttMoyen: paire.totalRoundTripTime,
                octetsRecus: paire.bytesReceived, octetsEnvoyes: paire.bytesSent,
                imagesDecodees: video?.framesDecoded, largeur: video?.frameWidth, hauteur: video?.frameHeight,
                gigueVideo: video?.jitter, pertesVideo: video?.packetsLost,
                horodatage: paire.timestamp,
            };
        })()`,
        true,
    );
}

async function main() {
    const port = 9222 + Math.floor(Math.random() * 1000);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-paire-'));
    const chrome = spawn(
        chromeBin,
        [
            '--headless=new',
            `--remote-debugging-port=${port}`,
            '--remote-allow-origins=*',
            `--user-data-dir=${userDataDir}`,
            '--no-sandbox',
            '--disable-dev-shm-usage',
            '--disable-gpu',
            '--autoplay-policy=no-user-gesture-required',
            '--disable-features=WebRtcHideLocalIpsWithMdns',
            // Without these three options, Chrome freezes the page after 5 minutes
            // (it is never in the foreground in headless mode): the
            // ICE traffic stops, the peer revokes consent ~30 s later
            // and the session drops. Observed during the acceptance run of 07/30/2026 — two
            // 11-minute sessions interrupted at 331 s and 340 s, one
            // relayed the other direct, which was at first wrongly blamed on the
            // relay. Any measurement longer than 5 minutes needs it.
            '--disable-background-timer-throttling',
            '--disable-backgrounding-occluded-windows',
            '--disable-renderer-backgrounding',
            'about:blank',
        ],
        { stdio: 'ignore' },
    );
    try {
        await attendreDevtools(port);
        const created = await (
            await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })
        ).json();
        const cdp = new Cdp(created.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');
        // Console and exceptions of the page: without them, a failure on the client side
        // (TURN allocation refused, SDP answer never received) shows up as
        // a mere "no pair in use", without saying why.
        const journalPage = [];
        cdp.ws.addEventListener('message', (event) => {
            const m = JSON.parse(String(event.data));
            if (m.method === 'Runtime.consoleAPICalled') {
                journalPage.push(
                    `[${m.params.type}] ` +
                        m.params.args.map((a) => a.value ?? a.description ?? '?').join(' '),
                );
            } else if (m.method === 'Runtime.exceptionThrown') {
                const d = m.params.exceptionDetails;
                journalPage.push(`[exception] ${d.exception?.description ?? d.text}`);
            }
        });
        // Interception of the constructor: captures the instance for `getStats()`,
        // and imposes `iceTransportPolicy: 'relay'` if the relay is forced.
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
            source: `
                window.__pc = null;
                const N = window.RTCPeerConnection;
                const forcer = ${forcerRelais};
                window.RTCPeerConnection = function (config, ...reste) {
                    const c = forcer ? { ...config, iceTransportPolicy: 'relay' } : config;
                    const p = new N(c, ...reste);
                    window.__pc = p;
                    return p;
                };
                window.RTCPeerConnection.prototype = N.prototype;
            `,
        });
        // Sub-block P2: without a token, the `client` handshake is refused.
        await semerJeton(cdp);
        await cdp.send('Page.navigate', { url });

        console.log(`mode: ${forcerRelais ? "FORCED RELAY (iceTransportPolicy 'relay')" : 'free (ICE chooses)'}`);
        for (let i = 0; i < 60; i += 1) {
            if (await cdp.eval('window.__pc != null')) break;
            await new Promise((r) => setTimeout(r, 200));
        }

        let premier = null;
        for (let i = 0; i < 60; i += 1) {
            premier = await relever(cdp);
            if (premier?.typeLocal) break;
            await new Promise((r) => setTimeout(r, 500));
        }
        console.log('t0 :', JSON.stringify(premier, null, 2));

        await new Promise((r) => setTimeout(r, durationMs));
        const second = await relever(cdp);
        console.log(`t0+${durationMs}ms :`, JSON.stringify(second, null, 2));

        if (premier?.typeLocal && second?.typeLocal) {
            const dt = (second.horodatage - premier.horodatage) / 1000;
            const dImages = second.imagesDecodees - premier.imagesDecodees;
            const dOctets = second.octetsRecus - premier.octetsRecus;
            console.log('');
            console.log(`path used: ${second.typeLocal} <- -> ${second.typeDistant}`);
            console.log(`current RTT: ${(second.rttCourant * 1000).toFixed(1)} ms`);
            console.log(`decoded images: ${dImages} in ${dt.toFixed(2)} s => ${(dImages / dt).toFixed(1)} i/s`);
            console.log(`received bitrate: ${((dOctets * 8) / dt / 1e6).toFixed(2)} Mb/s`);
            console.log(`resolution: ${second.largeur}x${second.hauteur}`);
            const attendu = forcerRelais ? 'relay' : null;
            if (attendu && second.typeLocal !== attendu) {
                console.error(`FAILURE: local type ${second.typeLocal}, expected ${attendu}`);
                process.exitCode = 1;
            } else if (dImages <= 0) {
                console.error('FAILURE: no image decoded over the measurement window');
                process.exitCode = 1;
            } else {
                console.log('PROOF: the stream goes through this path (decoded images rising).');
            }
        } else {
            console.error("FAILURE: no candidate pair was used");
            process.exitCode = 1;
        }
        if (journalPage.length) {
            console.log('');
            console.log('--- page console ---');
            for (const ligne of journalPage) console.log(ligne);
        }
        cdp.close();
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => {});
    }
}
main().catch((e) => {
    console.error(e);
    process.exit(1);
});
