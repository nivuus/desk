#!/usr/bin/env node
// Harnais de recette : pilote Chrome sans interface via CDP brut, dans le
// même esprit que client/verify-webrtc.mjs. Deux modes :
//   - stats   : relève l'overlay #stats et getStats() bruts pendant une durée
//               donnée, avec des molettes envoyées en continu pour simuler un
//               défilement réel (comme la tâche 12). Piloté avec
//               scroll-test.html (bandes de couleur) servi depuis
//               `python3 -m http.server 8099` sur cette machine, ouvert dans
//               Firefox côté VM.
//   - latency : mesure le délai « touche envoyée -> pixel visible changé »,
//               entièrement dans l'horloge JS d'une seule page (aucun aller-
//               retour Node<->Chrome dans l'intervalle chronométré). Piloté
//               avec latency-test.html (bascule noir/blanc sur Espace),
//               servi de la même façon.
//
// Usage (chaîne complète déjà montée — signaling, client Vite, agent lancé
// via scripts/run-agent.sh, Firefox sur la page de test correspondante et
// remis au premier plan APRÈS le dernier redémarrage de l'agent — voir
// docs/superpowers/plans/2026-07-27-jalon1-recette.md, chapitre « Ce qui a
// été appris », sur le vol de focus par schtasks /it) :
//   node recette/harness.mjs stats   <url> <durationMs>
//   node recette/harness.mjs latency <url> <trials>
//
// STATS_MODE=keyboard en variable d'environnement bascule le mode `stats`
// sur un défilement par Espace (diagnostic) plutôt que par molette.

import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { attendreDevtools } from './devtools.mjs';
import { semerJeton } from './jeton-recette.mjs';

const mode = process.argv[2];
const url = process.argv[3] ?? 'http://localhost:5173/?session=recette';
const arg4 = process.argv[4];
const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';

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
            const text = (message.params.args ?? []).map((a) => a.value ?? a.description ?? '').join(' ');
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
        const result = await this.send('Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
        if (result.exceptionDetails) {
            throw new Error(result.exceptionDetails.exception?.description ?? JSON.stringify(result.exceptionDetails));
        }
        return result.result.value;
    }
    close() {
        this.ws.close();
    }
}

async function pollUntil(predicate, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        if (await predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 200));
    }
    return false;
}

async function withChrome(fn) {
    const port = 9222 + Math.floor(Math.random() * 1000);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-recette-'));
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
        await attendreDevtools(port);
        const created = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
        const cdp = new Cdp(created.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
            source: `
                window.__pc = null;
                window.__sendCount = 0;
                const NativeRTCPeerConnection = window.RTCPeerConnection;
                window.RTCPeerConnection = function (...args) {
                    const pc = new NativeRTCPeerConnection(...args);
                    window.__pc = pc;
                    const origCreate = pc.createDataChannel.bind(pc);
                    pc.createDataChannel = function (label, opts) {
                        const ch = origCreate(label, opts);
                        const origSend = ch.send.bind(ch);
                        ch.send = function (data) { window.__sendCount++; return origSend(data); };
                        return ch;
                    };
                    return pc;
                };
                window.RTCPeerConnection.prototype = NativeRTCPeerConnection.prototype;
            `,
        });
        // Sous-bloc P2 : sans jeton, la poignée de main `client` est refusée.
        await semerJeton(cdp);
        await cdp.send('Page.navigate', { url });
        const pcAppeared = await pollUntil(() => cdp.eval('window.__pc !== null && window.__pc !== undefined'), 15_000);
        if (!pcAppeared) throw new Error('aucune RTCPeerConnection créée après 15s');
        // Attend l'état "prêt" (bandeau #status masqué par main.ts) avant de mesurer.
        await pollUntil(() => cdp.eval("document.querySelector('#status')?.dataset.hidden === 'true'"), 15_000);
        return await fn(cdp);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => {});
    }
}

async function modeStats(durationMs) {
    await withChrome(async (cdp) => {
        console.log('Connexion établie, envoi de molette continue pendant', durationMs, 'ms...');
        // Boucle de molette dans la page (un seul domaine d'horloge, pas de
        // rafale Node<->Chrome) : dispatch un WheelEvent réel sur #remote
        // toutes les ~60ms, comme un défilement utilisateur soutenu.
        const useKeyboard = process.env.STATS_MODE === 'keyboard';
        const wheelPromise = cdp.eval(
            `(async () => {
                const video = document.querySelector('#remote');
                const rect = video.getBoundingClientRect();
                const cx = rect.left + rect.width/2, cy = rect.top + rect.height/2;
                // La molette Windows agit sur la fenêtre SOUS LE CURSEUR : il
                // faut d'abord y positionner le curseur (le gestionnaire
                // onWheel n'envoie que le delta, jamais une position).
                video.dispatchEvent(new PointerEvent('pointermove', { clientX: cx, clientY: cy, bubbles: true, pointerId: 1, isPrimary: true }));
                await new Promise((r) => setTimeout(r, 150));
                const deadline = performance.now() + ${durationMs};
                let n = 0;
                while (performance.now() < deadline) {
                    if (${useKeyboard ? 'true' : 'false'}) {
                        // Diagnostic : Espace fait défiler une page vers le bas
                        // par défaut dans Firefox, sans dépendre de la position
                        // du curseur — sert à décorréler « molette cassée » de
                        // « pas de focus / fenêtre pas sous le curseur ». Voir
                        // la réserve « molette non reproduite » du document de
                        // recette : cette voie clavier n'est qu'un diagnostic,
                        // pas une mesure de défilement à la molette.
                        window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Space', key: ' ', bubbles: true, cancelable: true }));
                        window.dispatchEvent(new KeyboardEvent('keyup', { code: 'Space', key: ' ', bubbles: true, cancelable: true }));
                    } else {
                        const ev = new WheelEvent('wheel', { clientX: cx, clientY: cy, deltaY: 300, deltaMode: 0, bubbles: true, cancelable: true });
                        video.dispatchEvent(ev);
                    }
                    n++;
                    await new Promise((r) => setTimeout(r, 300));
                }
                return n;
            })()`,
            true,
        );
        const before = await sampleStats(cdp);
        await wheelPromise;
        const after = await sampleStats(cdp);
        const statsText = await cdp.eval("document.querySelector('#stats')?.textContent ?? ''");
        const sendCount = await cdp.eval('window.__sendCount');
        console.log('messages envoyés sur les canaux (input+control) :', sendCount);
        console.log('--- Avant ---');
        printSample(before);
        console.log('--- Après ---');
        printSample(after);
        console.log('Overlay #stats (dernier texte affiché) :', statsText);
        const dFrames = (after.stats?.framesDecoded ?? 0) - (before.stats?.framesDecoded ?? 0);
        const dt = (after.stats?.timestamp - before.stats?.timestamp) / 1000;
        console.log(`Δ framesDecoded=${dFrames} sur ${dt.toFixed(2)}s => ${(dFrames / dt).toFixed(2)} im/s`);
        console.log(`packetsLost=${after.stats?.packetsLost} framesDropped=${after.stats?.framesDropped}`);
        if (cdp.pageErrors.length) console.log('Erreurs JS :', cdp.pageErrors);
    });
}

async function sampleStats(cdp) {
    const raw = await cdp.eval(
        `(async () => {
            const pc = window.__pc;
            const report = await pc.getStats();
            const entries = Array.from(report.entries());
            const inbound = entries.find(([,s]) => s.type === 'inbound-rtp' && s.kind === 'video');
            return { stats: inbound ? inbound[1] : null, connectionState: pc.connectionState };
        })()`,
        true,
    );
    return raw;
}

function printSample(sample) {
    const s = sample.stats;
    if (!s) { console.log('  (aucune donnée inbound-rtp)'); return; }
    console.log(`  framesDecoded=${s.framesDecoded} frameWidth=${s.frameWidth} frameHeight=${s.frameHeight} packetsLost=${s.packetsLost} framesDropped=${s.framesDropped} t=${s.timestamp}`);
}

// Extrait uniquement freezeCount/totalFreezesDuration (léger, appelé deux
// fois par essai de latence pour corréler un essai lent à un gel détecté par
// le navigateur plutôt qu'à une simple hypothèse non vérifiée — voir la
// ronde de correction 1 du rapport de tâche 14).
async function sampleFreezes(cdp) {
    return cdp.eval(
        `(async () => {
            const pc = window.__pc;
            const report = await pc.getStats();
            const inbound = Array.from(report.values()).find((s) => s.type === 'inbound-rtp' && s.kind === 'video');
            return {
                freezeCount: inbound?.freezeCount ?? null,
                totalFreezesDuration: inbound?.totalFreezesDuration ?? null,
            };
        })()`,
        true,
    );
}

async function modeLatency(trials) {
    await withChrome(async (cdp) => {
        const results = [];
        for (let i = 0; i < trials; i++) {
            const freezeBefore = await sampleFreezes(cdp);
            const r = await cdp.eval(
                `(async () => {
                    const video = document.querySelector('#remote');
                    if (!video.videoWidth) return { error: 'pas encore de frame vidéo' };
                    const canvas = document.createElement('canvas');
                    canvas.width = video.videoWidth;
                    canvas.height = video.videoHeight;
                    const ctx = canvas.getContext('2d', { willReadFrequently: true });
                    function sampleCenter() {
                        ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
                        const d = ctx.getImageData((canvas.width/2)|0, (canvas.height/2)|0, 1, 1).data;
                        return [d[0], d[1], d[2]];
                    }
                    const before = sampleCenter();

                    // Espace = touche de bascule noir/blanc du plein écran de
                    // la page de test. Dispatché sur window, comme le vrai
                    // écouteur de client/src/input.ts (window.addEventListener
                    // ('keydown', ...)) — synthétique, mais suit exactement le
                    // même chemin de code que le clavier réel.
                    const t0 = performance.now();
                    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Space', key: ' ', bubbles: true, cancelable: true }));
                    window.dispatchEvent(new KeyboardEvent('keyup', { code: 'Space', key: ' ', bubbles: true, cancelable: true }));

                    return await new Promise((resolve) => {
                        let settled = false;
                        // Filet de sécurité INDÉPENDANT des callbacks de frame :
                        // si aucune nouvelle image n'arrive du tout (écran
                        // figé côté agent, panne de chaîne), requestVideoFrame-
                        // Callback ne serait jamais rappelé et la promesse ne
                        // se résoudrait jamais sans ce minuteur autonome.
                        const timer = setTimeout(() => {
                            if (settled) return;
                            settled = true;
                            resolve({ error: 'timeout (3s) sans nouvelle frame vidéo détectée du tout', before, after: sampleCenter() });
                        }, 3000);
                        function onFrame(now, metadata) {
                            if (settled) return;
                            const after = sampleCenter();
                            const diff = Math.abs(after[0]-before[0]) + Math.abs(after[1]-before[1]) + Math.abs(after[2]-before[2]);
                            // expectedDisplayTime : « vsync par lequel le
                            // navigateur s'attend à ce que la frame soit
                            // visible » (spec requestVideoFrameCallback) —
                            // c'est bien le moment d'AFFICHAGE recherché ici.
                            // presentationTime, à l'inverse, est le moment où
                            // le navigateur a SOUMIS la frame au compositeur,
                            // un cycle d'affichage plus tôt : gardé seulement
                            // en repli si expectedDisplayTime est absent d'une
                            // implémentation donnée.
                            const photonTime = metadata.expectedDisplayTime ?? metadata.presentationTime ?? now;
                            if (diff > 150) {
                                settled = true;
                                clearTimeout(timer);
                                resolve({ latencyMs: photonTime - t0, before, after, photonTime, t0 });
                                return;
                            }
                            video.requestVideoFrameCallback(onFrame);
                        }
                        video.requestVideoFrameCallback(onFrame);
                    });
                })()`,
                true,
            );
            const freezeAfter = await sampleFreezes(cdp);
            r.freezeCountDelta =
                freezeBefore.freezeCount !== null && freezeAfter.freezeCount !== null
                    ? freezeAfter.freezeCount - freezeBefore.freezeCount
                    : null;
            r.totalFreezesDurationDeltaMs =
                freezeBefore.totalFreezesDuration !== null && freezeAfter.totalFreezesDuration !== null
                    ? (freezeAfter.totalFreezesDuration - freezeBefore.totalFreezesDuration) * 1000
                    : null;
            results.push(r);
            console.log(`essai ${i + 1}/${trials} :`, JSON.stringify(r));
            // Laisse la vidéo se stabiliser avant l'essai suivant.
            await new Promise((resolve) => setTimeout(resolve, 1500));
        }
        const ok = results.filter((r) => typeof r.latencyMs === 'number');
        console.log('');
        console.log(`${ok.length}/${results.length} essais exploitables.`);
        if (ok.length) {
            const values = ok.map((r) => r.latencyMs).sort((a, b) => a - b);
            const sum = values.reduce((a, b) => a + b, 0);
            const mid = Math.floor(values.length / 2);
            // Médiane correcte : moyenne des deux valeurs centrales sur un
            // nombre pair d'essais, pas seulement l'élément d'indice
            // length/2 (qui donne le (n/2+1)-ième élément, pas le milieu).
            const median =
                values.length % 2 === 0 ? (values[mid - 1] + values[mid]) / 2 : values[mid];
            console.log('valeurs (ms) :', values.map((v) => v.toFixed(1)).join(', '));
            console.log(
                `min=${values[0].toFixed(1)} max=${values[values.length - 1].toFixed(1)} moyenne=${(sum / values.length).toFixed(1)} médiane=${median.toFixed(1)}`,
            );
            const withFreeze = ok.filter((r) => r.freezeCountDelta !== null);
            if (withFreeze.length) {
                console.log('');
                console.log('Corrélation gel détecté / latence de cet essai :');
                for (const r of withFreeze) {
                    console.log(
                        `  latence=${r.latencyMs.toFixed(1)}ms  freezeCountDelta=${r.freezeCountDelta}  totalFreezesDurationDelta=${r.totalFreezesDurationDeltaMs.toFixed(1)}ms`,
                    );
                }
            }
        }
        if (cdp.pageErrors.length) console.log('Erreurs JS :', cdp.pageErrors);
    });
}

if (mode === 'stats') {
    await modeStats(Number(arg4 ?? 10000));
} else if (mode === 'latency') {
    await modeLatency(Number(arg4 ?? 8));
} else {
    console.error('usage: node recette/harness.mjs <stats|latency> <url> <arg>');
    process.exit(1);
}
