#!/usr/bin/env node
// Acceptance harness: drives a headless Chrome over raw CDP, in the
// same spirit as client/verify-webrtc.mjs. Two modes:
//   - stats   : reads the #stats overlay and raw getStats() during a given
//               duration, with wheel events sent continuously to simulate a
//               real scroll (as in task 12). Driven with
//               scroll-test.html (colour bands) served from
//               `python3 -m http.server 8099` on this machine, opened in
//               Firefox on the VM side.
//   - latency : measures the "key sent -> visible pixel changed" delay,
//               entirely within the JS clock of a single page (no Node<->Chrome
//               round trip in the timed interval). Driven
//               with latency-test.html (black/white toggle on Space),
//               served the same way.
//
// Usage (whole chain already set up — signaling, Vite client, agent launched
// through scripts/run-agent.sh, Firefox on the matching test page and
// brought to the foreground AFTER the last restart of the agent — see
// docs/superpowers/plans/2026-07-27-jalon1-recette.md, chapter "What was
// learned", on focus theft by schtasks /it):
//   node recette/harness.mjs stats   <url> <durationMs>
//   node recette/harness.mjs latency <url> <trials>
//
// ⚠️ SINCE SUB-BLOCK P2, THIS INVOCATION IS NO LONGER ENOUGH against a guarded
// service: also set `RECETTE_EMAIL`, `RECETTE_MOTDEPASSE`, and `PLATEFORME_URL`
// if the service does not listen on http://127.0.0.1:8080. See
// `recette/jeton-recette.mjs` — without them, the tool WARNS and carries on.

//
// STATS_MODE=keyboard as an environment variable switches the `stats` mode
// to scrolling with Space (diagnostic) rather than with the wheel.

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
        // Sub-block P2: without a token, the `client` handshake is refused.
        await semerJeton(cdp);
        await cdp.send('Page.navigate', { url });
        const pcAppeared = await pollUntil(() => cdp.eval('window.__pc !== null && window.__pc !== undefined'), 15_000);
        if (!pcAppeared) throw new Error('no RTCPeerConnection created after 15s');
        // Waits for the "ready" state (#status banner hidden by main.ts) before measuring.
        await pollUntil(() => cdp.eval("document.querySelector('#status')?.dataset.hidden === 'true'"), 15_000);
        return await fn(cdp);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => {});
    }
}

async function modeStats(durationMs) {
    await withChrome(async (cdp) => {
        console.log('Connection established, sending continuous wheel for', durationMs, 'ms...');
        // Wheel loop inside the page (a single clock domain, no
        // Node<->Chrome burst): dispatches a real WheelEvent on #remote
        // every ~60ms, like a sustained user scroll.
        const useKeyboard = process.env.STATS_MODE === 'keyboard';
        const wheelPromise = cdp.eval(
            `(async () => {
                const video = document.querySelector('#remote');
                const rect = video.getBoundingClientRect();
                const cx = rect.left + rect.width/2, cy = rect.top + rect.height/2;
                // The Windows wheel acts on the window UNDER THE CURSOR: the
                // cursor must first be positioned there (the onWheel
                // handler only sends the delta, never a position).
                video.dispatchEvent(new PointerEvent('pointermove', { clientX: cx, clientY: cy, bubbles: true, pointerId: 1, isPrimary: true }));
                await new Promise((r) => setTimeout(r, 150));
                const deadline = performance.now() + ${durationMs};
                let n = 0;
                while (performance.now() < deadline) {
                    if (${useKeyboard ? 'true' : 'false'}) {
                        // Diagnosis: Space scrolls a page down by default
                        // in Firefox, without depending on the cursor
                        // position — used to decorrelate "broken wheel" from
                        // "no focus / window not under the cursor". See
                        // the "wheel not reproduced" reservation of the acceptance
                        // document: this keyboard path is only a diagnosis,
                        // not a measurement of wheel scrolling.
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
        console.log('messages sent on the channels (input+control):', sendCount);
        console.log('--- Before ---');
        printSample(before);
        console.log('--- After ---');
        printSample(after);
        console.log('Overlay #stats (last text displayed):', statsText);
        const dFrames = (after.stats?.framesDecoded ?? 0) - (before.stats?.framesDecoded ?? 0);
        const dt = (after.stats?.timestamp - before.stats?.timestamp) / 1000;
        console.log(`Δ framesDecoded=${dFrames} over ${dt.toFixed(2)}s => ${(dFrames / dt).toFixed(2)} fps`);
        console.log(`packetsLost=${after.stats?.packetsLost} framesDropped=${after.stats?.framesDropped}`);
        if (cdp.pageErrors.length) console.log('JS errors:', cdp.pageErrors);
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
    if (!s) { console.log('  (no inbound-rtp data)'); return; }
    console.log(`  framesDecoded=${s.framesDecoded} frameWidth=${s.frameWidth} frameHeight=${s.frameHeight} packetsLost=${s.packetsLost} framesDropped=${s.framesDropped} t=${s.timestamp}`);
}

// Only extracts freezeCount/totalFreezesDuration (light, called twice
// per latency trial to correlate a slow trial with a freeze detected by
// the browser rather than with a mere unverified hypothesis — see
// correction round 1 of the task 14 report).
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
                    if (!video.videoWidth) return { error: 'no video frame yet' };
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

                    // Space = the black/white toggle key of the test page's
                    // fullscreen. Dispatched on window, like the real
                    // listener of client/src/input.ts (window.addEventListener
                    // ('keydown', ...)) — synthetic, but follows exactly the
                    // same code path as the real keyboard.
                    const t0 = performance.now();
                    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Space', key: ' ', bubbles: true, cancelable: true }));
                    window.dispatchEvent(new KeyboardEvent('keyup', { code: 'Space', key: ' ', bubbles: true, cancelable: true }));

                    return await new Promise((resolve) => {
                        let settled = false;
                        // Safety net INDEPENDENT of the frame callbacks:
                        // if no new image arrives at all (screen
                        // frozen on the agent side, chain failure), requestVideoFrame-
                        // Callback would never be called back and the promise would
                        // never resolve without this standalone timer.
                        const timer = setTimeout(() => {
                            if (settled) return;
                            settled = true;
                            resolve({ error: 'timeout (3s) without any new video frame detected', before, after: sampleCenter() });
                        }, 3000);
                        function onFrame(now, metadata) {
                            if (settled) return;
                            const after = sampleCenter();
                            const diff = Math.abs(after[0]-before[0]) + Math.abs(after[1]-before[1]) + Math.abs(after[2]-before[2]);
                            // expectedDisplayTime: "the vsync at which the
                            // browser expects the frame to be
                            // visible" (requestVideoFrameCallback spec) —
                            // that is indeed the DISPLAY instant sought here.
                            // presentationTime, conversely, is the instant when
                            // the browser SUBMITTED the frame to the compositor,
                            // one display cycle earlier: kept only
                            // as a fallback if expectedDisplayTime is missing from a
                            // given implementation.
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
            console.log(`trial ${i + 1}/${trials}:`, JSON.stringify(r));
            // Lets the video settle before the next trial.
            await new Promise((resolve) => setTimeout(resolve, 1500));
        }
        const ok = results.filter((r) => typeof r.latencyMs === 'number');
        console.log('');
        console.log(`${ok.length}/${results.length} usable trials.`);
        if (ok.length) {
            const values = ok.map((r) => r.latencyMs).sort((a, b) => a - b);
            const sum = values.reduce((a, b) => a + b, 0);
            const mid = Math.floor(values.length / 2);
            // Correct median: average of the two central values on an
            // even number of trials, not only the element at index
            // length/2 (which gives the (n/2+1)-th element, not the middle).
            const median =
                values.length % 2 === 0 ? (values[mid - 1] + values[mid]) / 2 : values[mid];
            console.log('values (ms):', values.map((v) => v.toFixed(1)).join(', '));
            console.log(
                `min=${values[0].toFixed(1)} max=${values[values.length - 1].toFixed(1)} mean=${(sum / values.length).toFixed(1)} median=${median.toFixed(1)}`,
            );
            const withFreeze = ok.filter((r) => r.freezeCountDelta !== null);
            if (withFreeze.length) {
                console.log('');
                console.log('Correlation of detected freeze / latency of this trial:');
                for (const r of withFreeze) {
                    console.log(
                        `  latency=${r.latencyMs.toFixed(1)}ms  freezeCountDelta=${r.freezeCountDelta}  totalFreezesDurationDelta=${r.totalFreezesDurationDeltaMs.toFixed(1)}ms`,
                    );
                }
            }
        }
        if (cdp.pageErrors.length) console.log('JS errors:', cdp.pageErrors);
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
