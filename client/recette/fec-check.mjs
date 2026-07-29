#!/usr/bin/env node
// Sonde jetable (tâche 12) : lit directement les compteurs FEC/dissimulation
// de l'entrée inbound-rtp AUDIO (fecPacketsReceived, fecPacketsDiscarded,
// concealedSamples, concealmentEvents, packetsLost) à deux instants espacés
// — preuve plus directe que le débit agrégé (voir tâche 8 : LBRR redistribue
// les octets sous un débit cible, il ne les ajoute pas nécessairement).
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const url = process.argv[2] ?? 'http://127.0.0.1:5173/?session=demo';
const durationMs = Number(process.argv[3] ?? 20000);
const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.ready = new Promise((resolve) => this.ws.addEventListener('open', () => resolve(), { once: true }));
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
        const result = await this.send('Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
        if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
        return result.result.value;
    }
    close() { this.ws.close(); }
}

async function waitForDevtools(port) {
    for (let i = 0; i < 50; i += 1) {
        try { const r = await fetch(`http://127.0.0.1:${port}/json/version`); if (r.ok) return; } catch {}
        await new Promise((r) => setTimeout(r, 200));
    }
    throw new Error('devtools timeout');
}

async function sample(cdp) {
    return cdp.eval(
        `(async () => {
            const pc = window.__pc;
            const report = await pc.getStats();
            const a = Array.from(report.values()).find(s => s.type === 'inbound-rtp' && s.kind === 'audio');
            if (!a) return null;
            return {
                bytesReceived: a.bytesReceived, packetsReceived: a.packetsReceived,
                packetsLost: a.packetsLost, fecPacketsReceived: a.fecPacketsReceived,
                fecPacketsDiscarded: a.fecPacketsDiscarded, concealedSamples: a.concealedSamples,
                concealmentEvents: a.concealmentEvents, insertedSamplesForDeceleration: a.insertedSamplesForDeceleration,
                silentConcealedSamples: a.silentConcealedSamples, totalSamplesReceived: a.totalSamplesReceived,
                jitter: a.jitter, timestamp: a.timestamp,
            };
        })()`,
        true,
    );
}

async function main() {
    const port = 9222 + Math.floor(Math.random() * 1000);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-fec-'));
    const chrome = spawn(chromeBin, [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--autoplay-policy=no-user-gesture-required', '--disable-features=WebRtcHideLocalIpsWithMdns', 'about:blank',
    ], { stdio: 'ignore' });
    try {
        await waitForDevtools(port);
        const created = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
        const cdp = new Cdp(created.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
            source: `window.__pc=null;const N=window.RTCPeerConnection;window.RTCPeerConnection=function(...a){const p=new N(...a);window.__pc=p;return p;};window.RTCPeerConnection.prototype=N.prototype;`,
        });
        await cdp.send('Page.navigate', { url });
        for (let i = 0; i < 50; i += 1) {
            if (await cdp.eval('window.__pc != null')) break;
            await new Promise((r) => setTimeout(r, 200));
        }
        // Attendre une vraie entrée audio (pas seulement la création du PC) :
        // Chrome ne matérialise inbound-rtp audio qu'après le premier paquet.
        let s1 = null;
        for (let i = 0; i < 40; i += 1) {
            s1 = await sample(cdp);
            if (s1) break;
            await new Promise((r) => setTimeout(r, 500));
        }
        console.log('t0:', JSON.stringify(s1));
        await new Promise((r) => setTimeout(r, durationMs));
        const s2 = await sample(cdp);
        console.log(`t0+${durationMs}ms:`, JSON.stringify(s2));
        if (s1 && s2) {
            const dt = (s2.timestamp - s1.timestamp) / 1000;
            console.log('');
            console.log(`Δ bytesReceived=${s2.bytesReceived - s1.bytesReceived} sur ${dt.toFixed(2)}s => ${(((s2.bytesReceived - s1.bytesReceived) * 8) / dt / 1000).toFixed(1)} kb/s`);
            console.log(`Δ packetsLost=${s2.packetsLost - s1.packetsLost}`);
            console.log(`Δ fecPacketsReceived=${s2.fecPacketsReceived - s1.fecPacketsReceived}`);
            console.log(`Δ fecPacketsDiscarded=${s2.fecPacketsDiscarded - s1.fecPacketsDiscarded}`);
            console.log(`Δ concealedSamples=${s2.concealedSamples - s1.concealedSamples}`);
            console.log(`Δ concealmentEvents=${s2.concealmentEvents - s1.concealmentEvents}`);
            console.log(`Δ totalSamplesReceived=${s2.totalSamplesReceived - s1.totalSamplesReceived}`);
        }
        cdp.close();
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => {});
    }
}
main().catch((e) => { console.error(e); process.exit(1); });
