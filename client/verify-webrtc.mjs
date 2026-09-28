#!/usr/bin/env node
// End-to-end verification harness: drives Chrome in headless
// mode through the DevTools protocol (CDP) and reads the real statistics
// of the WebRTC connection (`RTCPeerConnection.getStats()`), as the
// browser computes them — not a self-assessment of the client under test.
//
// Why CDP over a raw WebSocket rather than Puppeteer/Playwright: no
// extra dependency to install, Node 24 provides `WebSocket` and
// `fetch` natively, which is enough to drive Chrome through the
// documented protocol (https://chromedevtools.github.io/devtools-protocol/).
//
// Usage :
//   node client/verify-webrtc.mjs [url] [--duration=8000]
//   EXPECT_AUDIO=1 node client/verify-webrtc.mjs [url] [--duration=8000]
//
// ⚠️ Since P2, `RECETTE_EMAIL`, `RECETTE_MOTDEPASSE` and, outside
// http://127.0.0.1:8080, `PLATEFORME_URL` are ALSO required: without a token, a `client` peer is
// refused. The why and the how are in `recette/jeton-recette.mjs`.


//
// Output: two `getStats()` readings `duration` ms apart, to
// prove that `framesDecoded`/`framesReceived` (video) and
// `bytesReceived`/`packetsReceived` (audio, if any) increase — and not
// merely that they are non-zero. Exit code 0 if the video proof is made (and,
// with `EXPECT_AUDIO=1`, if the audio is seen as well), 1 otherwise — with the
// diagnosis (ICE state, connection state, page errors) in both cases.
// Without `EXPECT_AUDIO=1`, a session without an active audio track (pure video
// acceptance) never makes the harness fail.

import { spawn } from 'node:child_process';
import { attendreDevtools } from './recette/devtools.mjs';
import { semerJeton } from './recette/jeton-recette.mjs';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const url = process.argv[2] ?? 'http://localhost:5173/?session=demo';
const durationArg = process.argv.find((a) => a.startsWith('--duration='));
const sampleDelayMs = durationArg ? Number(durationArg.split('=')[1]) : 8000;
const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';
// Opt-in (review of task 10): by default, a session without an active audio
// track never makes the harness fail (it also serves pure video
// acceptance, `TEST_FILE`). `EXPECT_AUDIO=1` reverses that choice for an
// acceptance run where audio IS expected: the total absence of audio then becomes
// a failure, exactly like a stream frozen after it started. Without this
// opt-in, a regression that cut the audio entirely would be
// indistinguishable from a video-only session and would leave the harness green.
const expectAudio = process.env.EXPECT_AUDIO === '1';

/// Minimal CDP client: a WebSocket connection to the "page" endpoint,
/// with request/response matching by identifier.
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

/// Extracts the `inbound-rtp` counters of the video track from a serialised
/// `RTCStatsReport` (array of `[id, stats]` entries).
function extractVideoInboundStats(statsEntries) {
    const entry = statsEntries.find(([, stats]) => stats.type === 'inbound-rtp' && stats.kind === 'video');
    return entry ? entry[1] : null;
}

/// Audio counterpart of `extractVideoInboundStats` (task 10 of project A):
/// returns `bytesReceived`, `packetsReceived`, `packetsLost`, `jitter` and
/// `estimatedPlayoutTimestamp` of the audio `inbound-rtp` track — or `null` if
/// no audio entry exists in the report.
///
/// The client ALWAYS negotiates a `recvonly` audio transceiver
/// (`client/src/webrtc.ts`), but Chrome only MATERIALISES the matching
/// `inbound-rtp` entry after receiving at least one packet on
/// that track — verified during acceptance (§4 of the results report): a
/// `TEST_FILE` session (no audio source on the agent side) returns `null` here, not
/// an entry with `bytesReceived: 0`. Both cases (entry absent, entry
/// present at zero) are therefore possible and must be handled
/// equivalently by the caller — that is the role of the `?? 0` in `main()`.
function extractAudioInboundStats(statsEntries) {
    const entry = statsEntries.find(([, stats]) => stats.type === 'inbound-rtp' && stats.kind === 'audio');
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
            // The agent is a native peer, not a browser: it cannot
            // resolve the mDNS names (`*.local`) with which Chrome
            // anonymises its "host" ICE candidates by default. Without this
            // flag, the client's offer only announces candidates in
            // `xxxxxxxx-xxxx-....local`, unreachable by the agent: ICE stays
            // stuck in `checking` indefinitely (diagnosed in task 8 —
            // see the task report for the details of the symptoms).
            '--disable-features=WebRtcHideLocalIpsWithMdns',
            'about:blank',
        ],
        { stdio: 'ignore' },
    );

    let exitCode = 1;
    try {
        await attendreDevtools(port);

        // Creates a blank tab then connects to it, to be able to inject the
        // capture script BEFORE the real navigation to `url`.
        // Chrome 150 requires the PUT method for `/json/new` (GET is refused
        // with "Using unsafe HTTP verb GET", a text body and not JSON).
        const created = await (
            await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })
        ).json();
        const cdp = new Cdp(created.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');

        // Intercepts the first `RTCPeerConnection` created by the page: it is
        // through it that `getStats()` will be read, without depending on the internal code of
        // `webrtc.ts` (which exposes nothing globally).
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

        // Sub-block P2: without a token, the `client` handshake is refused.
        await semerJeton(cdp);
        console.log(`Navigating to ${url} (Chrome DevTools on port ${port})`);
        await cdp.send('Page.navigate', { url });

        // Waits for `connectSession` to have created the RTCPeerConnection.
        const pcAppeared = await pollUntil(() => cdp.eval('window.__pc !== null && window.__pc !== undefined'), 10_000);
        if (!pcAppeared) {
            throw new Error("no RTCPeerConnection created in the page after 10s — the client script did not start");
        }

        // Wait for the video to REALLY flow before opening the measurement
        // window. Without that, reading 1 is taken during negotiation
        // (ICE, DTLS, first keyframe): the reported delta then covers a
        // period where the stream did not exist yet, and underestimates the
        // steady-state rate all the more as the window is short — over
        // 10 s, a few seconds of negotiation are enough to make
        // a real rate of 55 fps pass for 20 fps.
        const videoFlowing = await pollUntil(async () => {
            const s = await sampleStats(cdp);
            return s && s.framesDecoded > 0;
        }, 20_000);
        if (!videoFlowing) {
            // Do NOT abort here: the counters that tell "nothing
            // arrives" (framesReceived=0, transport problem) from "it
            // arrives but nothing decodes" (framesReceived>0,
            // framesDecoded=0, H.264 stream problem — typically a missing
            // keyframe) are only printed by the readings below.
            // Exiting before reading them throws away the only information that
            // allows deciding.
            console.log('WARNING: no image decoded after 20s — readings taken anyway for diagnosis');
        }
        // Let the steady state settle (first keyframe absorbed, jitter
        // buffer stabilised) before timing.
        await new Promise((resolve) => setTimeout(resolve, 1500));

        const first = await sampleStats(cdp);
        console.log('--- Reading 1 ---');
        printSample(first);

        await new Promise((resolve) => setTimeout(resolve, sampleDelayMs));

        const second = await sampleStats(cdp);
        console.log(`--- Reading 2 (+${sampleDelayMs}ms) ---`);
        printSample(second);

        const framesDecodedDelta = (second.stats?.framesDecoded ?? 0) - (first.stats?.framesDecoded ?? 0);
        const framesReceivedDelta = (second.stats?.framesReceived ?? 0) - (first.stats?.framesReceived ?? 0);
        // The source is a captured window (Desktop Duplication + cropping
        // to the window, not the desktop), not a fixed-resolution test
        // file: depending on the window size on the agent side, the actual
        // dimensions vary from one session to another (784x592, 764x484... never
        // a fixed value — see milestone 1 acceptance, criterion 1). What
        // matters here is that an image was decoded with plausible
        // dimensions, not that they match a resolution expected
        // in advance.
        const width = second.stats?.frameWidth;
        const height = second.stats?.frameHeight;
        const PLAUSIBLE_MAX_DIMENSION = 8192; // well beyond any realistic screen here
        const dimensionsOk =
            Number.isInteger(width) && Number.isInteger(height) &&
            width > 0 && height > 0 &&
            width <= PLAUSIBLE_MAX_DIMENSION && height <= PLAUSIBLE_MAX_DIMENSION;

        // --- Audio (task 10 of project A) ---
        //
        // The client ALWAYS negotiates a `recvonly` audio transceiver
        // (`client/src/webrtc.ts`), but Chrome only materialises the audio
        // `inbound-rtp` entry in `getStats()` after receiving at
        // least one packet on that track — a `TEST_FILE` session (no
        // audio source on the agent side) returns `sample.audioStats === null`, not an
        // entry with `bytesReceived: 0` (verified during acceptance). Hence the `?? 0`
        // below: they treat "entry absent" and "entry present at
        // zero" equivalently, which is the wanted behaviour in
        // both cases. We therefore tell "no sound" (no byte received either
        // at reading 1 or at reading 2: expected silence, NOT a failure) from "some
        // sound arrived once then nothing more" (counter frozen after having
        // moved: that is precisely the false positive the brief warns
        // against — an isolated packet does not prove a stream).
        const audioBytesFirst = first.audioStats?.bytesReceived ?? 0;
        const audioBytesSecond = second.audioStats?.bytesReceived ?? 0;
        const audioPacketsFirst = first.audioStats?.packetsReceived ?? 0;
        const audioPacketsSecond = second.audioStats?.packetsReceived ?? 0;
        const audioBytesDelta = audioBytesSecond - audioBytesFirst;
        const audioPacketsDelta = audioPacketsSecond - audioPacketsFirst;
        // Honest absence: no byte observed at either of the two readings.
        // Without this double condition, a session that just starts to
        // receive sound between the two readings (bytesFirst=0,
        // bytesSecond>0, delta>0) would wrongly be classified "absent" while
        // it proves exactly what we are looking for.
        const audioAbsent = audioBytesFirst === 0 && audioBytesSecond === 0;
        const audioGrowing = audioBytesDelta > 0 && audioPacketsDelta > 0;

        // A/V offset (task 7: the RTCP `wallclock` announces the instant of
        // CAPTURE, not of writing — this measurement is the only objective
        // verification of that fix). `estimatedPlayoutTimestamp` is on
        // a clock shared by both tracks: their difference is the
        // offset as the receiver sees it. Positive ⇒ the audio is
        // AHEAD of the video (ITU-R BT.1359 annoyance threshold: 45 ms); negative
        // ⇒ the audio is LATE (threshold: 125 ms, a delay being
        // tolerated almost three times longer than an advance).
        let avSkewMs = null;
        if (!audioAbsent && second.audioStats?.estimatedPlayoutTimestamp != null && second.stats?.estimatedPlayoutTimestamp != null) {
            avSkewMs = second.audioStats.estimatedPlayoutTimestamp - second.stats.estimatedPlayoutTimestamp;
        }

        console.log('');
        console.log(`connectionState (final) : ${second.connectionState}`);
        console.log(`iceConnectionState (final) : ${second.iceConnectionState}`);
        console.log(`Δ framesDecoded over ${sampleDelayMs}ms: ${framesDecodedDelta}`);
        console.log(`Δ framesReceived over ${sampleDelayMs}ms: ${framesReceivedDelta}`);
        console.log(`plausible dimensions (>0, ≤ ${PLAUSIBLE_MAX_DIMENSION}px): ${dimensionsOk ? 'OK' : 'FAILURE'} (got ${width}x${height})`);
        console.log('');
        console.log(`Δ audio bytesReceived over ${sampleDelayMs}ms: ${audioBytesDelta}`);
        console.log(`Δ audio packetsReceived over ${sampleDelayMs}ms: ${audioPacketsDelta}`);
        console.log(`audio packetsLost (reading 2): ${second.audioStats?.packetsLost ?? 'absent'}`);
        console.log(`audio jitter (reading 2): ${(((second.audioStats?.jitter ?? 0)) * 1000).toFixed(1)} ms`);
        if (audioAbsent) {
            console.log(
                'audio: no byte received in the two readings (no active audio track — video-only session, or total silence).' +
                    (expectAudio ? ' EXPECT_AUDIO=1: this is treated as a failure.' : ''),
            );
        } else if (audioGrowing) {
            console.log('audio: bytesReceived and packetsReceived increase — PROOF that the audio goes through the chain.');
        } else {
            console.log('audio: bytes arrived but the counters stopped advancing (frozen) — this is NOT a proof of a continuous stream.');
        }
        if (avSkewMs === null) {
            // Two distinct causes not to be confused (review of task
            // 10): no audio track at all (nothing to compare), or an audio
            // track that goes through but whose getStats() entry does not (yet)
            // expose `estimatedPlayoutTimestamp` on this browser/this
            // platform — a single message could have suggested, in
            // that second case, that the audio does not go through at all while
            // the line just above proves the opposite.
            if (audioAbsent) {
                console.log('A/V offset: not measurable (no active audio track in this reading).');
            } else {
                console.log(
                    'A/V offset: not measurable — the audio track goes through (see above), but ' +
                        "`estimatedPlayoutTimestamp` is absent from getStats() for one of the two tracks " +
                        'on this browser/platform (see the keys listed in each reading).',
                );
            }
        } else {
            const sens = avSkewMs > 0 ? 'audio ahead of the video' : avSkewMs < 0 ? 'audio behind the video' : 'no offset measured';
            // ITU-R BT.1359 annoyance thresholds: 45 ms if the audio leads the image,
            // 125 ms if it lags it — the sign matters, an advance annoys almost
            // three times sooner than a delay.
            const seuil = avSkewMs > 0 ? 45 : 125;
            const dansLeSeuil = Math.abs(avSkewMs) <= seuil;
            console.log(
                `A/V offset (estimatedPlayoutTimestamp audio − video): ${avSkewMs.toFixed(1)} ms ` +
                    `(${sens}) — applicable ITU-R BT.1359 annoyance threshold: ${seuil} ms, ` +
                    `${dansLeSeuil ? 'within the threshold' : 'BEYOND THE THRESHOLD'} (informative measurement, does not condition the exit code)`,
            );
        }

        if (cdp.consoleLines.length > 0) {
            console.log('\n--- Page console ---');
            for (const line of cdp.consoleLines) console.log(line);
        }
        if (cdp.pageErrors.length > 0) {
            console.log('\n--- Page JS errors ---');
            for (const line of cdp.pageErrors) console.log(line);
        }

        if (framesDecodedDelta > 0 && framesReceivedDelta > 0 && dimensionsOk) {
            console.log('\nPROOF: the video goes through the chain (framesDecoded and framesReceived increase, plausible dimensions).');
            // The video is proven: it is here, and only here, that the audio
            // can make the harness fail. By default, only a stream SEEN
            // (at least one byte received) then frozen fails — a session
            // without an active audio track (`audioAbsent`) never makes
            // the harness fail: it also serves pure video acceptance
            // (`TEST_FILE`), which by construction has no sound to push
            // through. `EXPECT_AUDIO=1` (opt-in, review of task 10)
            // reverses that choice for an acceptance run where audio IS expected:
            // absence then becomes a failure too — otherwise a
            // regression that cut all the audio would be indistinguishable
            // from a video-only session and would leave the harness green. The
            // A/V offset never takes part in that choice (see
            // above: a measurement, not yet a proven criterion).
            if (audioAbsent && expectAudio) {
                console.log(
                    "\nFAILURE (audio): EXPECT_AUDIO=1 but no audio track was seen in the two readings. " +
                        'See chrome://webrtc-internals for details.',
                );
                exitCode = 1;
            } else if (audioAbsent || audioGrowing) {
                exitCode = 0;
            } else {
                console.log(
                    "\nFAILURE (audio): an audio track received bytes but bytesReceived/packetsReceived " +
                        "stopped advancing — an isolated packet does not prove a continuous stream. " +
                        'See chrome://webrtc-internals for details.',
                );
                exitCode = 1;
            }
        } else if (!dimensionsOk) {
            console.log(`\nFAILURE: reported dimensions not plausible (${width}x${height}). See chrome://webrtc-internals for details.`);
            exitCode = 1;
        } else {
            console.log('\nFAILURE: the counters show no decoded video (framesDecoded/framesReceived stagnant). See chrome://webrtc-internals for details.');
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
    const audioStats = extractAudioInboundStats(raw.entries);
    return {
        stats,
        audioStats,
        connectionState: raw.connectionState,
        iceConnectionState: raw.iceConnectionState,
    };
}

/// Describes the `estimatedPlayoutTimestamp` field of a `getStats()` entry
/// so as to tell apart the two possible causes of `avSkewMs === null`
/// (review of task 10): the **key** can be entirely absent from
/// the object (the browser does not implement it), or present but equal to
/// `null`/`undefined` (implemented but not yet produced for this
/// track). `a.estimatedPlayoutTimestamp ?? 'absent'` conflated these two cases
/// — only one of the two supports the conclusion "field not exposed by this
/// browser".
function decrireEstimatedPlayoutTimestamp(entry) {
    if (!entry) return 'n/a (entry absent)';
    const clePresente = 'estimatedPlayoutTimestamp' in entry;
    if (!clePresente) return 'KEY ABSENT from the entry';
    return `key present, value=${entry.estimatedPlayoutTimestamp}`;
}

function printSample(sample) {
    if (!sample.stats) {
        console.log('  no video inbound-rtp entry in getStats()');
    } else {
        const s = sample.stats;
        console.log(
            `  framesDecoded=${s.framesDecoded} framesReceived=${s.framesReceived} ` +
                `frameWidth=${s.frameWidth} frameHeight=${s.frameHeight} ` +
                `bytesReceived=${s.bytesReceived} packetsReceived=${s.packetsReceived} ` +
                `packetsLost=${s.packetsLost} keyFramesDecoded=${s.keyFramesDecoded}`,
        );
        console.log(
            `  [video] estimatedPlayoutTimestamp: ${decrireEstimatedPlayoutTimestamp(s)}`,
        );
        // Diagnosis requested in review: the complete list of the keys of
        // the entry, to check by facts rather than by deduction
        // which fields this browser actually produces on `inbound-rtp`.
        console.log(`  [video] keys of the getStats() entry: ${Object.keys(s).sort().join(', ')}`);
    }

    if (!sample.audioStats) {
        console.log('  no audio inbound-rtp entry in getStats()');
        return;
    }
    const a = sample.audioStats;
    console.log(
        `  [audio] bytesReceived=${a.bytesReceived} packetsReceived=${a.packetsReceived} ` +
            `packetsLost=${a.packetsLost} jitter=${((a.jitter ?? 0) * 1000).toFixed(1)}ms`,
    );
    console.log(
        `  [audio] estimatedPlayoutTimestamp : ${decrireEstimatedPlayoutTimestamp(a)}`,
    );
    console.log(`  [audio] keys of the getStats() entry: ${Object.keys(a).sort().join(', ')}`);
}

main().catch((error) => {
    console.error('verification harness failure:', error);
    process.exit(1);
});
