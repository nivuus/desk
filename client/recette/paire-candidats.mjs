#!/usr/bin/env node
// Sonde de recette (chantier C volet 2, traversée NAT) : relève la paire de
// candidats RÉELLEMENT employée par le navigateur, le type des deux candidats
// qui la composent, son RTT et son débit.
//
// Pourquoi elle existe : `verify-webrtc.mjs` prouve que le flux traverse, mais
// ne dit pas PAR OÙ. Or c'est exactement la question de ce chantier — une
// session peut très bien fonctionner en direct et ne rien prouver du relais.
// Le type (`host` / `srflx` / `relay`) ne se lit que dans les entrées
// `local-candidate` / `remote-candidate` appariées à la paire nominée.
//
// Usage :
//   node client/recette/paire-candidats.mjs [url] [dureeMs]
//   FORCER_RELAIS=1 node client/recette/paire-candidats.mjs   (iceTransportPolicy: 'relay')
//
// `FORCER_RELAIS` s'applique en interceptant le constructeur de
// `RTCPeerConnection` dans la page, sans toucher au code du client : la
// modification n'a pas à être committée puis retirée, contrairement au réglage
// en dur que suggérait le plan.
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

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

async function waitForDevtools(port) {
    for (let i = 0; i < 50; i += 1) {
        try {
            const r = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (r.ok) return;
        } catch {}
        await new Promise((r) => setTimeout(r, 200));
    }
    throw new Error('devtools timeout');
}

/// Relève la paire employée, ses deux candidats, et les compteurs vidéo.
///
/// La paire retenue est celle marquée `selected` (Chrome), à défaut
/// `nominated` : les deux existent, et une paire nominée n'est pas
/// nécessairement celle qui porte le trafic.
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
                // Diagnostic : sans paire employée, ce sont les états de la
                // négociation et les candidats collectés qui disent pourquoi.
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
            'about:blank',
        ],
        { stdio: 'ignore' },
    );
    try {
        await waitForDevtools(port);
        const created = await (
            await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })
        ).json();
        const cdp = new Cdp(created.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');
        // Console et exceptions de la page : sans elles, un échec côté client
        // (allocation TURN refusée, réponse SDP jamais reçue) se présente comme
        // un simple « aucune paire employée », sans dire pourquoi.
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
        // Interception du constructeur : capture l'instance pour `getStats()`,
        // et impose `iceTransportPolicy: 'relay'` si on force le relais.
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
        await cdp.send('Page.navigate', { url });

        console.log(`mode : ${forcerRelais ? "RELAIS FORCÉ (iceTransportPolicy 'relay')" : 'libre (ICE choisit)'}`);
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
            console.log(`chemin employé : ${second.typeLocal} <- -> ${second.typeDistant}`);
            console.log(`RTT courant : ${(second.rttCourant * 1000).toFixed(1)} ms`);
            console.log(`images décodées : ${dImages} en ${dt.toFixed(2)} s => ${(dImages / dt).toFixed(1)} i/s`);
            console.log(`débit reçu : ${((dOctets * 8) / dt / 1e6).toFixed(2)} Mb/s`);
            console.log(`résolution : ${second.largeur}x${second.hauteur}`);
            const attendu = forcerRelais ? 'relay' : null;
            if (attendu && second.typeLocal !== attendu) {
                console.error(`ÉCHEC : type local ${second.typeLocal}, attendu ${attendu}`);
                process.exitCode = 1;
            } else if (dImages <= 0) {
                console.error('ÉCHEC : aucune image décodée sur la fenêtre de mesure');
                process.exitCode = 1;
            } else {
                console.log('PREUVE : le flux traverse par ce chemin (images décodées en hausse).');
            }
        } else {
            console.error("ÉCHEC : aucune paire de candidats n'a été employée");
            process.exitCode = 1;
        }
        if (journalPage.length) {
            console.log('');
            console.log('--- console de la page ---');
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
