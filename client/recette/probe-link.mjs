#!/usr/bin/env node
// Sonde jetable de la tâche 12 (recette du chantier réseau adaptatif) :
// échantillonne #status (texte de l'indicateur de lien) et #stats (overlay
// getStats()) à intervalle régulier pendant une session, pour observer
// l'évolution de la résolution/débit/texte d'alerte sous un profil netem
// donné. Dérivée du même patron CDP que client/recette/harness.mjs.
//
// Usage : node recette/probe-link.mjs <url> <durationMs> [intervalMs]

import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const url = process.argv[2] ?? 'http://127.0.0.1:5173/?session=demo';
const durationMs = Number(process.argv[3] ?? 60000);
const intervalMs = Number(process.argv[4] ?? 2000);
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
        if (result.exceptionDetails) {
            throw new Error(result.exceptionDetails.exception?.description ?? JSON.stringify(result.exceptionDetails));
        }
        return result.result.value;
    }
    close() { this.ws.close(); }
}

async function waitForDevtools(port, attempts = 50) {
    for (let i = 0; i < attempts; i += 1) {
        try {
            const response = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (response.ok) return;
        } catch {}
        await new Promise((resolve) => setTimeout(resolve, 200));
    }
    throw new Error('Chrome DevTools ne répond pas après le délai imparti');
}

async function main() {
    const port = 9222 + Math.floor(Math.random() * 1000);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-probe-link-'));
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
        const created = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
        const cdp = new Cdp(created.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');
        await cdp.send('Page.navigate', { url });
        const t0 = Date.now();
        console.log(`t=0ms navigation vers ${url}`);
        while (Date.now() - t0 < durationMs) {
            const snap = await cdp.eval(
                `({ status: document.querySelector('#status')?.textContent ?? null, hidden: document.querySelector('#status')?.dataset.hidden ?? null, stats: document.querySelector('#stats')?.textContent ?? null })`,
            );
            const t = Date.now() - t0;
            console.log(`t=${t}ms  hidden=${snap.hidden}  status="${snap.status}"  stats="${snap.stats}"`);
            await new Promise((resolve) => setTimeout(resolve, intervalMs));
        }
        cdp.close();
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => {});
    }
}

main().catch((error) => {
    console.error('échec de la sonde :', error);
    process.exit(1);
});
