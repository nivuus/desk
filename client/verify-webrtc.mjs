#!/usr/bin/env node
// Harnais de vérification de bout en bout : pilote Chrome en mode sans
// interface via le protocole DevTools (CDP) et lit les statistiques réelles
// de la connexion WebRTC (`RTCPeerConnection.getStats()`), telles que le
// navigateur les calcule — pas une auto-évaluation du client sous test.
//
// Pourquoi CDP en WebSocket brut plutôt que Puppeteer/Playwright : aucune
// dépendance supplémentaire à installer, Node 24 fournit `WebSocket` et
// `fetch` nativement, ce qui suffit à piloter Chrome par le protocole
// documenté (https://chromedevtools.github.io/devtools-protocol/).
//
// Usage :
//   node client/verify-webrtc.mjs [url] [--duration=8000]
//
// Sortie : deux relevés de `getStats()` espacés de `duration` ms, pour
// prouver que `framesDecoded`/`framesReceived` augmentent (et pas seulement
// non nuls). Code de sortie 0 si la preuve est faite, 1 sinon — avec le
// diagnostic (état ICE, état de connexion, erreurs de page) dans les deux cas.

import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const url = process.argv[2] ?? 'http://localhost:5173/?session=demo';
const durationArg = process.argv.find((a) => a.startsWith('--duration='));
const sampleDelayMs = durationArg ? Number(durationArg.split('=')[1]) : 8000;
const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';

/// Client CDP minimal : une connexion WebSocket vers l'endpoint « page »,
/// avec appariement requête/réponse par identifiant.
class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.ready = new Promise((resolve) => this.ws.addEventListener('open', () => resolve(), { once: true }));
        this.ws.addEventListener('message', (event) => this.onMessage(event));
        this.consoleLines = [];
        this.pageErrors = [];
    }

    onMessage(event) {
        const message = JSON.parse(String(event.data));
        if (message.id !== undefined && this.pending.has(message.id)) {
            const { resolve, reject } = this.pending.get(message.id);
            this.pending.delete(message.id);
            if (message.error) reject(new Error(JSON.stringify(message.error)));
            else resolve(message.result);
            return;
        }
        if (message.method === 'Runtime.consoleAPICalled') {
            const text = (message.params.args ?? [])
                .map((a) => a.value ?? a.description ?? '')
                .join(' ');
            this.consoleLines.push(`[${message.params.type}] ${text}`);
        }
        if (message.method === 'Runtime.exceptionThrown') {
            this.pageErrors.push(message.params.exceptionDetails.text);
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
        if (result.exceptionDetails) {
            throw new Error(result.exceptionDetails.exception?.description ?? JSON.stringify(result.exceptionDetails));
        }
        return result.result.value;
    }

    close() {
        this.ws.close();
    }
}

async function waitForDevtools(port, attempts = 50) {
    for (let i = 0; i < attempts; i += 1) {
        try {
            const response = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (response.ok) return;
        } catch {
            // Chrome pas encore prêt à accepter des connexions : on retente.
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
    }
    throw new Error('Chrome DevTools ne répond pas après le délai imparti');
}

/// Extrait les compteurs `inbound-rtp` de la piste vidéo depuis un rapport
/// `RTCStatsReport` sérialisé (tableau d'entrées `[id, stats]`).
function extractVideoInboundStats(statsEntries) {
    const entry = statsEntries.find(([, stats]) => stats.type === 'inbound-rtp' && stats.kind === 'video');
    return entry ? entry[1] : null;
}

async function main() {
    const port = 9222 + Math.floor(Math.random() * 1000);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-webrtc-verify-'));

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
            // L'agent est un pair natif, pas un navigateur : il ne sait pas
            // résoudre les noms mDNS (`*.local`) par lesquels Chrome
            // anonymise ses candidats ICE « host » par défaut. Sans ce
            // drapeau, l'offre du client n'annonce que des candidats en
            // `xxxxxxxx-xxxx-....local`, injoignables par l'agent : ICE reste
            // bloqué en `checking` indéfiniment (diagnostiqué en tâche 8 —
            // voir le rapport de tâche pour le détail des symptômes).
            '--disable-features=WebRtcHideLocalIpsWithMdns',
            'about:blank',
        ],
        { stdio: 'ignore' },
    );

    let exitCode = 1;
    try {
        await waitForDevtools(port);

        // Crée un onglet vierge puis s'y connecte, pour pouvoir injecter le
        // script de capture AVANT la navigation réelle vers `url`.
        // Chrome 150 exige la méthode PUT pour `/json/new` (GET est refusé
        // avec « Using unsafe HTTP verb GET », un corps texte et pas JSON).
        const created = await (
            await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })
        ).json();
        const cdp = new Cdp(created.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');

        // Intercepte la première `RTCPeerConnection` créée par la page : c'est
        // par elle qu'on lira `getStats()`, sans dépendre du code interne de
        // `webrtc.ts` (qui n'expose rien globalement).
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
            source: `
                window.__pc = null;
                const NativeRTCPeerConnection = window.RTCPeerConnection;
                window.RTCPeerConnection = function (...args) {
                    const pc = new NativeRTCPeerConnection(...args);
                    window.__pc = pc;
                    return pc;
                };
                window.RTCPeerConnection.prototype = NativeRTCPeerConnection.prototype;
            `,
        });

        console.log(`Navigation vers ${url} (Chrome DevTools sur le port ${port})`);
        await cdp.send('Page.navigate', { url });

        // Attend que `connectSession` ait créé la RTCPeerConnection.
        const pcAppeared = await pollUntil(() => cdp.eval('window.__pc !== null && window.__pc !== undefined'), 10_000);
        if (!pcAppeared) {
            throw new Error("aucune RTCPeerConnection créée dans la page après 10s — le script client n'a pas démarré");
        }

        const first = await sampleStats(cdp);
        console.log('--- Relevé 1 ---');
        printSample(first);

        await new Promise((resolve) => setTimeout(resolve, sampleDelayMs));

        const second = await sampleStats(cdp);
        console.log(`--- Relevé 2 (+${sampleDelayMs}ms) ---`);
        printSample(second);

        const framesDecodedDelta = (second.stats?.framesDecoded ?? 0) - (first.stats?.framesDecoded ?? 0);
        const framesReceivedDelta = (second.stats?.framesReceived ?? 0) - (first.stats?.framesReceived ?? 0);
        // La source est une fenêtre capturée (Desktop Duplication + recadrage
        // sur la fenêtre, pas le bureau), pas un fichier de test à résolution
        // fixe : selon la taille de la fenêtre côté agent, les dimensions
        // réelles varient d'une session à l'autre (784x592, 764x484... jamais
        // une valeur figée — voir la recette du jalon 1, critère 1). Ce qui
        // compte ici est qu'une image ait été décodée avec des dimensions
        // plausibles, pas qu'elles correspondent à une résolution attendue à
        // l'avance.
        const width = second.stats?.frameWidth;
        const height = second.stats?.frameHeight;
        const PLAUSIBLE_MAX_DIMENSION = 8192; // largement au-delà de tout écran réaliste ici
        const dimensionsOk =
            Number.isInteger(width) && Number.isInteger(height) &&
            width > 0 && height > 0 &&
            width <= PLAUSIBLE_MAX_DIMENSION && height <= PLAUSIBLE_MAX_DIMENSION;

        console.log('');
        console.log(`connectionState (final) : ${second.connectionState}`);
        console.log(`iceConnectionState (final) : ${second.iceConnectionState}`);
        console.log(`Δ framesDecoded sur ${sampleDelayMs}ms : ${framesDecodedDelta}`);
        console.log(`Δ framesReceived sur ${sampleDelayMs}ms : ${framesReceivedDelta}`);
        console.log(`dimensions plausibles (>0, ≤ ${PLAUSIBLE_MAX_DIMENSION}px) : ${dimensionsOk ? 'OK' : 'ÉCHEC'} (obtenu ${width}x${height})`);

        if (cdp.consoleLines.length > 0) {
            console.log('\n--- Console de la page ---');
            for (const line of cdp.consoleLines) console.log(line);
        }
        if (cdp.pageErrors.length > 0) {
            console.log('\n--- Erreurs JS de la page ---');
            for (const line of cdp.pageErrors) console.log(line);
        }

        if (framesDecodedDelta > 0 && framesReceivedDelta > 0 && dimensionsOk) {
            console.log('\nPREUVE : la vidéo traverse la chaîne (framesDecoded et framesReceived augmentent, dimensions plausibles).');
            exitCode = 0;
        } else if (!dimensionsOk) {
            console.log(`\nÉCHEC : dimensions rapportées non plausibles (${width}x${height}). Voir chrome://webrtc-internals pour le détail.`);
            exitCode = 1;
        } else {
            console.log('\nÉCHEC : les compteurs ne montrent pas de vidéo décodée (framesDecoded/framesReceived stagnants). Voir chrome://webrtc-internals pour le détail.');
            exitCode = 1;
        }

        cdp.close();
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => {});
    }

    process.exit(exitCode);
}

async function pollUntil(predicate, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        if (await predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 200));
    }
    return false;
}

async function sampleStats(cdp) {
    const raw = await cdp.eval(
        `(async () => {
            const pc = window.__pc;
            const report = await pc.getStats();
            return {
                entries: Array.from(report.entries()),
                connectionState: pc.connectionState,
                iceConnectionState: pc.iceConnectionState,
            };
        })()`,
        true,
    );
    const stats = extractVideoInboundStats(raw.entries);
    return { stats, connectionState: raw.connectionState, iceConnectionState: raw.iceConnectionState };
}

function printSample(sample) {
    if (!sample.stats) {
        console.log('  aucune entrée inbound-rtp vidéo dans getStats()');
        return;
    }
    const s = sample.stats;
    console.log(
        `  framesDecoded=${s.framesDecoded} framesReceived=${s.framesReceived} ` +
            `frameWidth=${s.frameWidth} frameHeight=${s.frameHeight} ` +
            `bytesReceived=${s.bytesReceived} packetsReceived=${s.packetsReceived} ` +
            `packetsLost=${s.packetsLost} keyFramesDecoded=${s.keyFramesDecoded}`,
    );
}

main().catch((error) => {
    console.error('échec du harnais de vérification :', error);
    process.exit(1);
});
