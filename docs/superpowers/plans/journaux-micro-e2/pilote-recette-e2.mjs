#!/usr/bin/env node
// Pilote de la recette E2 (chantier E — microphone), tâches 11 à 13 du plan
// `docs/superpowers/plans/2026-08-20-micro-e2.md`.
//
// 🔵 UN VRAI `getUserMedia`, ET C'EST TOUTE LA DIFFÉRENCE AVEC E1. Le pilote de
// E1 (`client/recette/micro-e1.mjs`) posait un `OscillatorNode` sur le `sender`
// par `replaceTrack` : la permission, les contraintes de `client/src/micro.ts`
// (`echoCancellation`, `noiseSuppression`, `autoGainControl`), le bouton et le
// pipeline audio de Chrome n'étaient exercés par rien. Ici, Chrome ouvre un
// périphérique factice alimenté par un WAV
// (`--use-file-for-fake-audio-capture`), et **c'est le bouton `#micro` de la
// page qui est CLIQUÉ** : tout le chemin de production court.
//
// 🔵 LE VERDICT D'ÉCOUTE NE SE LIT PAS ICI. Il se lit sur CABLE Output, par un
// processus tiers (`micro-ecoute-e2.ps1`), c'est-à-dire là où une application
// Windows le lirait. Ce pilote établit la session, allume le micro, et relève
// ce que le NAVIGATEUR voit. Un compte d'octets ne distingue pas « du son » de
// « MON son » — doctrine payée au sous-bloc D7, et re-confirmée par la sonde
// de la tâche 1 de ce bloc, où une CRÊTE de 0,449795 a été relevée sur un
// spectre entièrement nul.
//
// ⚠️ L'ORIGINE DOIT ÊTRE SÛRE. `getUserMedia` n'existe pas sur une origine non
// sûre : `http://192.168.3.1:5173` (l'adresse qu'employait E1) n'en est PAS
// une, `http://127.0.0.1:5173` en est une. D'où le défaut de `--url` ci-dessous
// et les paramètres `?plateforme=`/`?signaling=` (`client/src/adresse-plateforme.ts`),
// sans lesquels la page déduirait l'adresse de la plateforme de sa propre
// origine, donc du port de Vite.
//
// Usage :
//   node pilote-recette-e2.mjs --scenario <instrument|ecoute> [options]
//
//   --wav <fichier>     le WAV joué à la place du microphone
//   --session <id>      le nom de session (préfixé par celui de la VM enrôlée)
//   --duree <ms>        la durée pendant laquelle le micro reste allumé
//   --url <url>         la page (défaut : 127.0.0.1:5173, origine SÛRE)
//   --sortie <f.json>   le compte rendu
//   --sans-micro        n'allume PAS le micro (témoin)
//   --releve-ms <ms>    période des relevés `getStats` (défaut 30 000)
//
// ⚠️ Poser `RECETTE_EMAIL`, `RECETTE_MOTDEPASSE` et `PLATEFORME_URL` : depuis le
// sous-bloc P2 un pair `client` sans jeton est REFUSÉ par la garde.

import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { attendreDevtools } from '../../../../client/recette/devtools.mjs';
import { semerJeton } from '../../../../client/recette/jeton-recette.mjs';

function argument(nom, defaut) {
    const i = process.argv.indexOf(`--${nom}`);
    return i >= 0 && process.argv[i + 1] !== undefined ? process.argv[i + 1] : defaut;
}
const drapeau = (nom) => process.argv.includes(`--${nom}`);

const scenario = argument('scenario', 'instrument');
const wav = argument('wav', '');
const session = argument('session', 'e2');
const duree = Number(argument('duree', '60000'));
const releveMs = Number(argument('releve-ms', '30000'));
const plateforme = process.env.PLATEFORME_URL ?? 'http://127.0.0.1:8091';
const signaling = argument('signaling', plateforme.replace(/^http/, 'ws'));
const url = argument(
    'url',
    `http://127.0.0.1:5173/?session=${encodeURIComponent(session)}` +
        `&plateforme=${encodeURIComponent(plateforme)}&signaling=${encodeURIComponent(signaling)}`,
);
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
    // ⚠️ BORNÉE. Une évaluation CDP sur une page portant un flux WebRTC actif
    // peut ne JAMAIS rendre (piège du sous-bloc D2, payé d'une exécution
    // entière). Sans cette borne, la tâche 13 se figerait au bout de dix
    // minutes sans rien écrire.
    async eval(expression, awaitPromise = false, plafondMs = 20000) {
        const appel = this.send('Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
        const borne = new Promise((_, rej) =>
            setTimeout(() => rej(new Error('évaluation CDP sans réponse dans le délai')), plafondMs),
        );
        const r = await Promise.race([appel, borne]);
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

async function jusqua(predicat, plafondMs, pas = 250) {
    const fin = Date.now() + plafondMs;
    while (Date.now() < fin) {
        if (await predicat()) return true;
        await dormir(pas);
    }
    return false;
}

/// L'interception de `RTCPeerConnection`, pour relever `getStats` sans toucher
/// une ligne de `client/`. Même technique que `FORCER_RELAIS` de
/// `paire-candidats.mjs` et que le pilote de E1.
///
/// ⚠️ Elle doit courir AVANT le script de la page, donc par
/// `Page.addScriptToEvaluateOnNewDocument`.
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

/// 🔴 LE CONTRÔLE D'INSTRUMENT, et il est le préalable de tout le reste
/// (tâche 11, step 1). Il exerce `getUserMedia` avec **les contraintes exactes
/// de `client/src/micro.ts`**, puis mesure la fréquence dominante de la piste
/// obtenue par un `AnalyserNode`.
///
/// ⚠️ POURQUOI CE SECOND ÉTAGE N'EST PAS DÉCORATIF. `noiseSuppression: true`
/// arme NS3, dont l'objet est d'effacer le bruit STATIONNAIRE — et une
/// sinusoïde continue en est un, du point de vue de l'algorithme. Un contrôle
/// qui se contenterait de « `getUserMedia` rend une piste » passerait sur un
/// périphérique dont le contenu est intégralement effacé, et toute la recette
/// mesurerait alors le silence en croyant mesurer une voix. **Lequel des deux
/// WAV survit est une mesure ; ce n'était pas prévisible.**
const CONTROLE_INSTRUMENT = (secondes) => `
    (async () => {
        const contraintes = {
            audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
        };
        let flux;
        try {
            flux = await navigator.mediaDevices.getUserMedia(contraintes);
        } catch (e) {
            return JSON.stringify({ erreur: e.name + ' : ' + e.message });
        }
        const pistes = flux.getAudioTracks();
        if (pistes.length === 0) return JSON.stringify({ erreur: 'aucune piste audio' });
        const piste = pistes[0];

        const ctx = new AudioContext({ sampleRate: 48000 });
        if (ctx.state !== 'running') await ctx.resume();
        const src = ctx.createMediaStreamSource(flux);
        const an = ctx.createAnalyser();
        an.fftSize = 8192;
        an.smoothingTimeConstant = 0;
        src.connect(an);

        const bacs = new Float32Array(an.frequencyBinCount);
        const cumul = new Float64Array(an.frequencyBinCount);
        let creteTemporelle = 0;
        const temps = new Float32Array(an.fftSize);
        const fin = Date.now() + ${secondes} * 1000;
        let tours = 0;
        while (Date.now() < fin) {
            await new Promise((r) => setTimeout(r, 50));
            an.getFloatFrequencyData(bacs);
            an.getFloatTimeDomainData(temps);
            for (let i = 0; i < temps.length; i++) {
                const a = Math.abs(temps[i]);
                if (a > creteTemporelle) creteTemporelle = a;
            }
            // On cumule en LINÉAIRE, jamais en dB : moyenner des décibels
            // pondère les creux comme les crêtes et déplace le maximum.
            for (let i = 0; i < bacs.length; i++) {
                if (Number.isFinite(bacs[i])) cumul[i] += Math.pow(10, bacs[i] / 10);
            }
            tours++;
        }
        let iMax = 0;
        for (let i = 1; i < cumul.length; i++) if (cumul[i] > cumul[iMax]) iMax = i;
        const parBac = ctx.sampleRate / an.fftSize;
        const dominanteDb = 10 * Math.log10(cumul[iMax] / Math.max(tours, 1));
        // Le plancher : la MÉDIANE des bacs, qui dit à combien la dominante
        // se détache. Une dominante qui ne se détache pas n'est pas une
        // dominante, c'est le premier bac d'un spectre plat.
        const tries = Array.from(cumul).sort((a, b) => a - b);
        const medianeDb = 10 * Math.log10(Math.max(tries[Math.floor(tries.length / 2)], 1e-30) / Math.max(tours, 1));

        // ⚠️ RELEVÉ AVANT l'arrêt de la piste. Une première rédaction le lisait après, et
        // rendait donc « ended » sur une piste parfaitement saine : le champ
        // décrivait le geste de l'instrument, pas l'état mesuré.
        const etatPiste = piste.readyState;
        piste.stop();
        await ctx.close();
        // ⚠️ Un -Infinity se sérialise en null dans un JSON, et un null se
        // lit comme « pas mesuré » alors qu'il veut dire « rigoureusement
        // aucune énergie ». On le nomme.
        const lisible = (x) => (Number.isFinite(x) ? x : String(x));
        return JSON.stringify({
            pisteObtenue: true,
            etiquette: piste.label,
            etatPiste,
            reglages: piste.getSettings ? piste.getSettings() : undefined,
            contraintesAppliquees: piste.getConstraints ? piste.getConstraints() : undefined,
            tours,
            resolutionHz: parBac,
            dominanteHz: iMax * parBac,
            dominanteDb: lisible(dominanteDb),
            medianeDb: lisible(medianeDb),
            detachementDb: lisible(dominanteDb - medianeDb),
            creteTemporelle,
        });
    })()
`;

/// L'état du bouton `#micro`, tel que `client/src/micro.ts` l'écrit.
const ETAT_BOUTON = `
    (() => {
        const b = document.querySelector('#micro');
        if (!b) return JSON.stringify({ erreur: 'aucun #micro dans le document' });
        return JSON.stringify({ hidden: b.hidden, disabled: b.disabled, etat: b.dataset.etat, titre: b.title });
    })()
`;

const CLIQUER_MICRO = `
    (() => {
        const b = document.querySelector('#micro');
        if (!b) return JSON.stringify({ erreur: 'aucun #micro' });
        if (b.hidden) return JSON.stringify({ erreur: 'bouton #micro masqué : ready ne portait pas mic:true' });
        b.click();
        return JSON.stringify({ clique: true });
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
                    kind: r.kind, packetsReceived: r.packetsReceived, packetsLost: r.packetsLost,
                    bytesReceived: r.bytesReceived, framesDecoded: r.framesDecoded,
                    framesDropped: r.framesDropped, frameWidth: r.frameWidth, frameHeight: r.frameHeight,
                });
            }
            if (r.type === 'outbound-rtp') {
                out.sortant.push({
                    kind: r.kind, packetsSent: r.packetsSent, bytesSent: r.bytesSent, mid: r.mid,
                    targetBitrate: r.targetBitrate,
                });
            }
            if (r.type === 'candidate-pair' && r.nominated && r.state === 'succeeded') {
                out.paire = { rtt: r.currentRoundTripTime, recu: r.bytesReceived, envoye: r.bytesSent };
            }
        });
        out.etatIce = pc.iceConnectionState;
        out.etatCnx = pc.connectionState;
        out.horodatage = new Date().toISOString();
        return JSON.stringify(out);
    })()
`;

async function avecChrome(fn) {
    const port = 9300 + Math.floor(Math.random() * 600);
    const profil = await mkdtemp(join(tmpdir(), 'chrome-micro-e2-'));
    const drapeaux = [
        '--headless=new',
        `--remote-debugging-port=${port}`,
        '--remote-allow-origins=*',
        `--user-data-dir=${profil}`,
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--disable-gpu',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns',
        // 🔴 LES TROIS DRAPEAUX DE DURÉE (tâche 11, step 2). Une page jamais
        // mise au premier plan GÈLE au bout de 5 minutes : deux sessions de
        // 11 minutes du chantier TURN se sont interrompues à 331 s et 340 s
        // pour cette seule raison, et la cause a d'abord été imputée au relais.
        // La tâche 13 dure DIX minutes : elle y est exposée par construction.
        '--disable-background-timer-throttling',
        '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding',
        // Le microphone : périphérique factice, permission accordée d'office,
        // et un contenu CONNU.
        '--use-fake-ui-for-media-stream',
        '--use-fake-device-for-media-stream',
    ];
    if (wav) drapeaux.push(`--use-file-for-fake-audio-capture=${wav}`);
    drapeaux.push('about:blank');
    const chrome = spawn(chromeBin, drapeaux, { stdio: 'ignore' });
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
        // ⚠️ `rm` court APRÈS `kill`, et Chrome écrit encore : un `rmdir` a
        // levé `ENOTEMPTY` et emporté le compte rendu d'une exécution entière
        // en E1. Le ménage d'un répertoire temporaire ne coûte jamais un relevé.
        await dormir(1500);
        await rm(profil, { recursive: true, force: true }).catch((e) => {
            console.warn(`ménage du profil Chrome incomplet (sans effet sur la mesure) : ${e.message}`);
        });
    }
}

async function principal() {
    const journal = { scenario, session, wav, url, duree, debut: new Date().toISOString(), etapes: [], stats: [] };
    const code = await avecChrome(async (cdp) => {
        if (scenario === 'instrument') {
            // Une origine sûre suffit : ce contrôle ne demande AUCUNE session.
            await cdp.send('Page.navigate', { url: 'http://127.0.0.1:5173/' });
            await dormir(2000);
            const secondes = Math.max(3, Math.round(duree / 1000));
            journal.controle = JSON.parse(await cdp.eval(CONTROLE_INSTRUMENT(secondes), true, secondes * 1000 + 20000));
            journal.console = cdp.consoleLines.slice(-40);
            journal.erreursPage = cdp.pageErrors;
            return journal.controle.erreur ? 1 : 0;
        }

        await cdp.send('Page.navigate', { url });

        // On attend le FAIT — le bouton micro qui PARAÎT —, pas une durée. Il
        // ne paraît que si l'agent a envoyé `ready` avec `mic: true`
        // (`client/src/main.ts`, `annoncerDisponibilite`), ce qui est le
        // critère ② à lui seul. ⚠️ Ce n'est PAS `framesDecoded > 0` : un
        // Bloc-notes immobile ne fait produire aucune image à Desktop
        // Duplication (piège de D4), et la session serait pourtant vivante.
        const parait = await jusqua(async () => {
            const b = JSON.parse((await cdp.eval(ETAT_BOUTON)) ?? '{}');
            return b.hidden === false;
        }, 90000);
        journal.boutonParait = parait;
        journal.etatBoutonInitial = JSON.parse((await cdp.eval(ETAT_BOUTON)) ?? '{}');
        journal.stats.push({ etiquette: 'apres-ready', ...JSON.parse((await cdp.eval(RELEVE_STATS, true)) ?? '{}') });
        if (!parait) {
            journal.etapes.push({ quoi: 'ready', issue: 'le bouton #micro est resté masqué 90 s' });
            journal.console = cdp.consoleLines.slice(-60);
            journal.erreursPage = cdp.pageErrors;
            return 1;
        }
        journal.etapes.push({ quoi: 'ready', issue: 'bouton visible', quand: new Date().toISOString() });

        if (!drapeau('sans-micro')) {
            journal.etapes.push({
                quoi: 'clic',
                quand: new Date().toISOString(),
                compte: JSON.parse(await cdp.eval(CLIQUER_MICRO)),
            });
            const actif = await jusqua(async () => {
                const b = JSON.parse((await cdp.eval(ETAT_BOUTON)) ?? '{}');
                return b.etat === 'actif';
            }, 20000);
            journal.microActif = actif;
            journal.etatBoutonApresClic = JSON.parse((await cdp.eval(ETAT_BOUTON)) ?? '{}');
            journal.etapes.push({ quoi: 'micro', issue: actif ? 'actif' : 'PAS actif', quand: new Date().toISOString() });
        } else {
            journal.microActif = false;
            journal.etapes.push({ quoi: 'micro', issue: 'non allumé (--sans-micro)' });
        }

        // Le palier. Les relevés sont PÉRIODIQUES pour que la tâche 13 puisse
        // lire une dérive plutôt qu'un total.
        const fin = Date.now() + duree;
        while (Date.now() < fin) {
            await dormir(Math.min(releveMs, Math.max(0, fin - Date.now())));
            try {
                const s = JSON.parse((await cdp.eval(RELEVE_STATS, true)) ?? '{}');
                s.etiquette = `t+${Math.round((duree - (fin - Date.now())) / 1000)}s`;
                s.etatBouton = JSON.parse((await cdp.eval(ETAT_BOUTON)) ?? '{}');
                journal.stats.push(s);
            } catch (e) {
                journal.stats.push({ etiquette: 'relevé en échec', erreur: e.message });
            }
        }
        journal.fin = new Date().toISOString();
        journal.console = cdp.consoleLines.slice(-60);
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
