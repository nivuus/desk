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
//   EXPECT_AUDIO=1 node client/verify-webrtc.mjs [url] [--duration=8000]
//
// ⚠️ Depuis P2, il faut AUSSI `RECETTE_EMAIL`, `RECETTE_MOTDEPASSE` et, hors
// http://127.0.0.1:8080, `PLATEFORME_URL` : sans jeton, un pair `client` est
// refusé. Le pourquoi et le comment sont dans `recette/jeton-recette.mjs`.


//
// Sortie : deux relevés de `getStats()` espacés de `duration` ms, pour
// prouver que `framesDecoded`/`framesReceived` (vidéo) et
// `bytesReceived`/`packetsReceived` (audio, s'il y en a) augmentent — et pas
// seulement non nuls. Code de sortie 0 si la preuve vidéo est faite (et,
// avec `EXPECT_AUDIO=1`, que l'audio est également vu), 1 sinon — avec le
// diagnostic (état ICE, état de connexion, erreurs de page) dans les deux cas.
// Sans `EXPECT_AUDIO=1`, une session sans piste audio active (recette vidéo
// pure) ne fait jamais échouer le harnais.

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
// Opt-in (revue de la tâche 10) : par défaut, une session sans piste audio
// active ne fait jamais échouer le harnais (il sert aussi à la recette
// vidéo pure, `TEST_FILE`). `EXPECT_AUDIO=1` renverse ce choix pour une
// recette où l'audio EST attendu : l'absence totale d'audio devient alors
// un échec, exactement comme un flux figé après avoir démarré. Sans cet
// opt-in, une régression qui couperait totalement l'audio serait
// indiscernable d'une session vidéo seule et laisserait le harnais vert.
const expectAudio = process.env.EXPECT_AUDIO === '1';

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

/// Extrait les compteurs `inbound-rtp` de la piste vidéo depuis un rapport
/// `RTCStatsReport` sérialisé (tableau d'entrées `[id, stats]`).
function extractVideoInboundStats(statsEntries) {
    const entry = statsEntries.find(([, stats]) => stats.type === 'inbound-rtp' && stats.kind === 'video');
    return entry ? entry[1] : null;
}

/// Pendant audio de `extractVideoInboundStats` (tâche 10 du chantier A) :
/// rend `bytesReceived`, `packetsReceived`, `packetsLost`, `jitter` et
/// `estimatedPlayoutTimestamp` de la piste `inbound-rtp` audio — ou `null` si
/// aucune entrée audio n'existe dans le rapport.
///
/// Le client négocie TOUJOURS un transceiver audio `recvonly`
/// (`client/src/webrtc.ts`), mais Chrome ne MATÉRIALISE l'entrée
/// `inbound-rtp` correspondante qu'après réception d'au moins un paquet sur
/// cette piste — vérifié en recette (§4 du rapport de résultats) : une
/// session `TEST_FILE` (aucune source audio côté agent) rend `null` ici, pas
/// une entrée à `bytesReceived: 0`. Les deux cas (entrée absente, entrée
/// présente à zéro) sont donc possibles et doivent être traités de façon
/// équivalente par l'appelant — c'est le rôle des `?? 0` dans `main()`.
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
        await attendreDevtools(port);

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

        // Sous-bloc P2 : sans jeton, la poignée de main `client` est refusée.
        await semerJeton(cdp);
        console.log(`Navigating to ${url} (Chrome DevTools on port ${port})`);
        await cdp.send('Page.navigate', { url });

        // Attend que `connectSession` ait créé la RTCPeerConnection.
        const pcAppeared = await pollUntil(() => cdp.eval('window.__pc !== null && window.__pc !== undefined'), 10_000);
        if (!pcAppeared) {
            throw new Error("no RTCPeerConnection created in the page after 10s — the client script did not start");
        }

        // Attendre que la vidéo coule VRAIMENT avant d'ouvrir la fenêtre de
        // mesure. Sans cela, le relevé 1 est pris pendant la négociation
        // (ICE, DTLS, premier keyframe) : le delta rapporté couvre alors une
        // période où le flux n'existait pas encore, et sous-estime le débit
        // en régime établi d'autant plus que la fenêtre est courte — sur
        // 10 s, plusieurs secondes de négociation suffisent à faire passer
        // un débit réel de 55 i/s pour 20 i/s.
        const videoFlowing = await pollUntil(async () => {
            const s = await sampleStats(cdp);
            return s && s.framesDecoded > 0;
        }, 20_000);
        if (!videoFlowing) {
            // Ne PAS interrompre ici : les compteurs qui distinguent « rien
            // n'arrive » (framesReceived=0, problème de transport) de « ça
            // arrive mais rien ne se décode » (framesReceived>0,
            // framesDecoded=0, problème de flux H.264 — typiquement une image
            // clé manquante) ne sont imprimés que par les relevés ci-dessous.
            // Sortir avant de les lire, c'est jeter la seule information qui
            // permette de trancher.
            console.log('WARNING: no image decoded after 20s — readings taken anyway for diagnosis');
        }
        // Laisser le régime s'établir (premier keyframe absorbé, tampon de
        // gigue stabilisé) avant de chronométrer.
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

        // --- Audio (tâche 10 du chantier A) ---
        //
        // Le client négocie TOUJOURS un transceiver audio `recvonly`
        // (`client/src/webrtc.ts`), mais Chrome ne matérialise l'entrée
        // `inbound-rtp` audio dans `getStats()` qu'après réception d'au
        // moins un paquet sur cette piste — une session `TEST_FILE` (aucune
        // source audio côté agent) rend `sample.audioStats === null`, pas une
        // entrée à `bytesReceived: 0` (vérifié en recette). D'où les `?? 0`
        // ci-dessous : ils traitent « entrée absente » et « entrée présente à
        // zéro » de façon équivalente, ce qui est le comportement voulu dans
        // les deux cas. On distingue donc « pas de son » (aucun octet reçu ni
        // au relevé 1 ni au relevé 2 : silence attendu, PAS un échec) de « du
        // son est arrivé une fois puis plus rien » (compteur figé après avoir
        // bougé : c'est précisément le faux positif que le brief met en garde
        // contre — un paquet isolé ne prouve pas un flux).
        const audioBytesFirst = first.audioStats?.bytesReceived ?? 0;
        const audioBytesSecond = second.audioStats?.bytesReceived ?? 0;
        const audioPacketsFirst = first.audioStats?.packetsReceived ?? 0;
        const audioPacketsSecond = second.audioStats?.packetsReceived ?? 0;
        const audioBytesDelta = audioBytesSecond - audioBytesFirst;
        const audioPacketsDelta = audioPacketsSecond - audioPacketsFirst;
        // Absence honnête : aucun octet observé à aucun des deux relevés.
        // Sans cette double condition, une session qui démarre tout juste à
        // recevoir du son entre les deux relevés (bytesFirst=0,
        // bytesSecond>0, delta>0) serait à tort classée « absente » alors
        // qu'elle prouve exactement ce qu'on cherche.
        const audioAbsent = audioBytesFirst === 0 && audioBytesSecond === 0;
        const audioGrowing = audioBytesDelta > 0 && audioPacketsDelta > 0;

        // Décalage A/V (tâche 7 : le `wallclock` RTCP annonce l'instant de
        // CAPTURE, pas d'écriture — cette mesure est la seule vérification
        // objective de cette correction). `estimatedPlayoutTimestamp` est sur
        // une horloge commune aux deux pistes : leur différence est le
        // décalage tel que le récepteur le voit. Positif ⇒ l'audio est en
        // AVANCE sur la vidéo (seuil de gêne ITU-R BT.1359 : 45 ms) ; négatif
        // ⇒ l'audio est en RETARD (seuil : 125 ms, la gêne d'un retard étant
        // tolérée presque trois fois plus longtemps que celle d'une avance).
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
            // Deux causes distinctes à ne pas confondre (revue de la tâche
            // 10) : pas de piste audio du tout (rien à comparer), ou piste
            // audio qui traverse mais dont l'entrée getStats() n'expose pas
            // (encore) `estimatedPlayoutTimestamp` sur ce navigateur/cette
            // plateforme — un message unique aurait pu laisser croire, dans
            // ce second cas, que l'audio ne traverse pas du tout alors que
            // la ligne juste au-dessus prouve le contraire.
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
            // Seuils de gêne ITU-R BT.1359 : 45 ms si l'audio devance l'image,
            // 125 ms s'il la retarde — le signe compte, l'avance gêne presque
            // trois fois plus tôt que le retard.
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
            // La vidéo est prouvée : c'est ici, et seulement ici, que l'audio
            // peut faire échouer le harnais. Par défaut, seul un flux VU
            // (au moins un octet reçu) puis figé fait échouer — une session
            // sans piste audio active (`audioAbsent`) ne fait jamais échouer
            // le harnais : il sert aussi à la recette vidéo pure
            // (`TEST_FILE`), qui n'a par construction aucun son à faire
            // traverser. `EXPECT_AUDIO=1` (opt-in, revue de la tâche 10)
            // renverse ce choix pour une recette où l'audio EST attendu :
            // l'absence devient alors, elle aussi, un échec — sans quoi une
            // régression qui couperait tout l'audio serait indiscernable
            // d'une session vidéo seule et laisserait le harnais vert. Le
            // décalage A/V, lui, n'intervient jamais dans ce choix (voir
            // plus haut : mesure, pas encore critère éprouvé).
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

/// Décrit le champ `estimatedPlayoutTimestamp` d'une entrée `getStats()` de
/// façon à distinguer les deux causes possibles de `avSkewMs === null`
/// (revue de la tâche 10) : la **clé** peut être totalement absente de
/// l'objet (le navigateur ne l'implémente pas), ou présente mais valant
/// `null`/`undefined` (implémentée mais pas encore produite pour cette
/// piste). `a.estimatedPlayoutTimestamp ?? 'absent'` confondait ces deux cas
/// — un seul des deux soutient la conclusion « champ non exposé par ce
/// navigateur ».
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
        // Diagnostic demandé en revue : la liste complète des clés de
        // l'entrée, pour vérifier par les faits plutôt que par déduction
        // quels champs ce navigateur produit réellement sur `inbound-rtp`.
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
