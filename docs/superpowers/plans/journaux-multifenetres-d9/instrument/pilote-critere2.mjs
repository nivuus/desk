#!/usr/bin/env node
// Sous-bloc D9, tâche 15 — pilote de la recette ② : l'audio mort, deux
// montages (réarmement, promotion), plus la non-régression du rattachement.
//
// Dérivé de `docs/superpowers/plans/journaux-multifenetres-d9/instrument/pilote-critere1.mjs`
// (Cdp, vmIt, vmVivante, copierLog, purgerFenetresVMRescapees, journalPlat,
// horodate/sessionDeLigne/champ, attendreDevtools/attendreLeFait, ouvrirFenetre,
// l'amorce d'interception RTCPeerConnection — repris quasi tels quels) et de
// `docs/superpowers/plans/journaux-multifenetres-d8/instrument/pilote-recette-d8.mjs`
// (`expressionReleveFrequence`, COPIÉE TELLE QUELLE depuis
// `pilote-recette-d7.mjs::expressionReleveFrequence` — même doctrine : niveaux
// par fréquence assignée jamais le seul argmax, `ctx.resume()` vérifié,
// repli par `<video>.srcObject`, `ctx.close()` en `finally` — non reformulée
// pour ne pas la faire dériver de l'original).
//
// ============================================================================
// CE QUE CE PILOTE MESURE, ET COMMENT
// ============================================================================
//
// MONTAGE A (réarmement) : une seule fenêtre Chrome `--app` jouant un ton pur
// à fréquence connue (`ton.html?hz=…&gain=0.3`, D7). Aucune voisine à
// promouvoir dans son groupe de PID : c'est la branche du répit
// (`REPIT_REARMEMENT_AUDIO`, `agent/src/capteur/sommeil.rs`) qui doit
// financer le retour du son, en réélisant la MÊME session après son délai.
//
// MONTAGE B (promotion) : deux fenêtres Chrome `--app` partageant un SEUL
// `--user-data-dir`, donc un seul `chrome.exe` (mécanisme déjà établi par D7,
// repris tel quel). Chacune joue le MÊME ton (gain 0.3) : la session porteuse
// entend le MÉLANGE du groupe de PID (D7), donc les deux fenêtres rendent la
// MÊME dominante quand l'une porte — ce qui compte ici n'est PAS la fréquence
// mais QUELLE SESSION porte, avant et après la mort de la porteuse.
//
// Dans les deux montages : `node scripts/winrm.js 'Restart-Service Audiosrv
// -Force'` provoque la mort de capture — cause nommément transitoire
// (`agent/src/audio.rs:69-70`) qui épuise `LECTURES_ECHOUEES_MAX` (10 essais)
// côté fil de capture WASAPI de l'enfant porteur.
//
// Le verdict se lit À LA DOMINANTE REÇUE (`AnalyserNode`), JAMAIS à un compte
// d'octets — D7 a mesuré `bytesReceived` croissant sur un spectre à −1000 dB,
// ce qui aurait fait conclure À TORT qu'une fenêtre entend encore quelque
// chose.
//
// ============================================================================
// CE QUE LA LECTURE DU CODE (avant d'écrire ce pilote) ÉTABLIT, ET QUE CE
// PILOTE EXISTE POUR CONFRONTER À LA RÉALITÉ DE LA VM
// ============================================================================
//
// `AudioSource::set_actif` (`agent/src/windows_audio.rs::set_actif`) N'ÉCRIT
// QU'UN ATOMIQUE (`self.emet.store`) — il ne reconstruit RIEN. Le fil de
// capture WASAPI, une fois `capture_morte` posé, EXÉCUTE UN `return;` ET NE
// REVIENT JAMAIS (`agent/src/windows_audio.rs`, la ligne juste après
// « lecture audio échouée, capture arrêtée définitivement »).
// `agent/src/demarrage/audio.rs::brancher` — qui construit le
// `WindowsAudioSource` — n'est appelé QU'UNE FOIS, au démarrage de l'enfant
// (`demarrage.rs::executer`, pas de boucle).
//
// Le commentaire de `REPIT_REARMEMENT_AUDIO` (`sommeil.rs`) affirme que
// « réélire la même session construit une activation process loopback
// NEUVE » — mais RIEN dans le chemin réélection → `Audio{actif:true}` →
// `appliquer_audio` → `set_actif` ne reconstruit de fil de capture. Le
// rapport de la tâche 9 le dit lui-même : « le comportement de bout en bout
// … n'a été vérifié que par lecture de code … aucun test neuf n'exerce le
// chemin complet ». CE PILOTE EST LA PREMIÈRE VÉRIFICATION SUR LA VM. Il ne
// suppose PAS le résultat — il l'observe et le rapporte, quel qu'il soit.
//
// ============================================================================
// GARDE-FOUS REPRIS DE D8/D9-T14 (mêmes raisons)
// ============================================================================
//   - `--user-data-dir` PAR FENÊTRE (montage A) ou PARTAGÉ (montage B, EXPRÈS).
//   - Aucune capture d'écran CDP pendant une mesure.
//   - `Get-Process agent` vérifié après CHAQUE tentative (fait par l'appelant
//     shell, voir le rapport de tâche).
//   - Survie de la VM contrôlée après chaque rang.
//   - `agent.log` copié APRÈS la fin réelle de l'exécution.
//   - Le navigateur PILOTE tourne sur l'HÔTE, jamais sur la VM.
//   - Une session WebRTC VIVANTE est requise (leg D7) : un répondeur de
//     viewport nu donne un faux négatif indiscernable du défaut — ce pilote
//     attend `iceConnectionState === 'connected'` avant toute mesure.
//
// ============================================================================
// PARAMÈTRES
// ============================================================================
//   MONTAGE      — 'a' (réarmement, 1 fenêtre) ou 'b' (promotion, 2 fenêtres
//                  même user-data-dir).
//   ETIQUETTE    — suffixe des fichiers de sortie.
//   PORT_SHELL   — port du serveur vite (5173, inchangé — seul l'AGENT change
//                  entre les commits, pas le client, voir le rapport de tâche
//                  pour la justification).
//   OBSERVATION_S — durée d'observation post-Audiosrv (défaut 55 s : au-delà
//                  de REARMEMENTS_MAX(5)×REPIT_REARMEMENT_AUDIO(5s)=25s pour
//                  laisser le temps à un abandon définitif de s'exprimer si
//                  le réarmement échoue, avec marge).

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
const TON_HTML = join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d7/instrument/ton.html');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const PORT_SHELL = process.env.PORT_SHELL ?? '5173';
const URL_SHELL = `http://${HOTE}:${PORT_SHELL}/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const MONTAGE = (process.env.MONTAGE ?? 'a').toLowerCase();
const COPIE_LOG = process.env.COPIE_LOG ?? `/tmp/agent-critere-2-${ETIQUETTE}.log`;
const SORTIE_JSON = process.env.SORTIE_JSON
    ?? join(RACINE, `docs/superpowers/plans/journaux-multifenetres-d9/critere-2-${ETIQUETTE}.json`);
const BITRATE = process.env.BITRATE ?? '8000000';
const HZ_A = Number(process.env.HZ_A ?? 660);
const HZ_B = Number(process.env.HZ_B ?? 770);
const OBSERVATION_S = Number(process.env.OBSERVATION_S ?? 55);
// Déclencheur de la mort de capture. Par défaut celui du brief
// (`Restart-Service Audiosrv -Force`). Une PREMIÈRE exécution du contrôle
// ROUGE avec ce déclencheur a montré, PAR LE JOURNAL (`agent.log`, zéro ligne
// `lecture audio échouée` sur 70+ s), qu'il ne fait JAMAIS échouer
// `capture.read()` — le son s'éteint côté SOURCE (la fenêtre Chrome sur la
// VM perd son rendu, sans doute) mais la capture WASAPI, elle, continue de
// lire avec succès (silence). `DECLENCHEUR=stop_attente_start` bascule sur
// une coupure DÉLIBÉRÉMENT SOUTENUE (`Stop-Service` puis attente PUIS
// `Start-Service`, plutôt qu'un `Restart-Service` dont la fenêtre
// d'indisponibilité réelle n'est pas mesurée) : ~1,5 s d'indisponibilité
// CONFIRMÉE, au-delà du budget de réessai total du fil de capture
// (`LECTURES_ECHOUEES_MAX`(10) × backoff croissant plafonné à 200 ms ≈ 1,1 s
// au pire, `agent/src/audio.rs`). Les deux restent des redémarrages du
// service audio — la cause nommée par `agent/src/audio.rs:69-70` — mais
// seul le second est confirmé, par le journal, atteindre le code qu'il est
// censé exercer.
const DECLENCHEUR = process.env.DECLENCHEUR ?? 'restart_force';
function commandeDeclencheur() {
    if (DECLENCHEUR === 'stop_attente_start') {
        return 'Stop-Service Audiosrv -Force; Start-Sleep -Milliseconds 1500; Start-Service Audiosrv';
    }
    // Second essai de validation (voir le rapport de tâche) : même
    // `stop_attente_start` mesuré SANS AUCUNE erreur de lecture (audiodg.exe
    // redémarre pourtant bien, StartTime confirmé après coup). Troisième
    // essai : tuer directement `audiodg.exe` (le processus SERVEUR COM que le
    // client WASAPI de la capture appelle), plutôt que de passer par le
    // service — une rupture de connexion RPC avec le processus serveur est
    // le cas le plus proche d'une vraie invalidation de client documentée
    // dans la littérature WASAPI, et le plus distinct des deux essais
    // précédents (qui redémarraient le service SANS tuer le processus en
    // cours d'appel).
    if (DECLENCHEUR === 'tuer_audiodg') {
        return "Stop-Process -Name audiodg -Force -ErrorAction SilentlyContinue; 'audiodg tue'";
    }
    // Quatrième essai de validation : « changement de périphérique », le
    // TROISIÈME motif nommé par `agent/src/audio.rs:69-70` (les deux
    // premiers essais ciblaient « redémarrage du service », le troisième
    // était hors motif). Désactive puis réactive les deux points de sortie
    // actifs (`Get-PnpDevice -Class AudioEndpoint`), en 1,5 s d'écart.
    if (DECLENCHEUR === 'changement_peripherique') {
        return "$d = Get-PnpDevice -Class AudioEndpoint | Where-Object { $_.Status -eq 'OK' }; "
            + "$d | Disable-PnpDevice -Confirm:$false; Start-Sleep -Milliseconds 1500; "
            + "$d | Enable-PnpDevice -Confirm:$false; 'peripheriques bascules : ' + ($d.InstanceId -join ',')";
    }
    return 'Restart-Service Audiosrv -Force';
}

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(7);
    console.log(`[${dt}s ${new Date().toISOString()}] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));
const maintenantIso = () => new Date().toISOString();

// ---------------------------------------------------------------- VM (session interactive)
function vmIt(nom, ps) {
    const userName = process.env.WINDOWS_ADMIN_USERNAME ?? 'Administrateur';
    const password = process.env.WINDOWS_ADMIN_PASSWORD ?? '';
    spawnSync('bash', ['-c', `cat > /media/vm/dev/it-${nom}.ps1`], { input: ps, encoding: 'utf8' });
    const commande = [
        `schtasks /delete /tn it-${nom} /f 2>$null;`,
        `schtasks /create /tn it-${nom} /f /it /ru '${userName}' /rp '${password}'`,
        `/sc once /st 00:00 /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\it-${nom}.ps1';`,
        `schtasks /run /tn it-${nom}`,
    ].join(' ');
    const r = spawnSync('node', [join(RACINE, 'scripts/winrm.js'), commande], { encoding: 'utf8', env: process.env });
    if (r.status !== 0) log(`!! vm-it ${nom} a échoué`, (r.stderr ?? '').slice(0, 400));
    return r;
}
// Commande WinRM DIRECTE (pas /it) — utile pour un ordre qui n'a besoin
// d'aucun contexte de session interactive (Audiosrv est un service, pas une
// app en session 1).
function vmDirect(commande) {
    const r = spawnSync('node', [join(RACINE, 'scripts/winrm.js'), commande], { encoding: 'utf8', env: process.env });
    log(`VM-DIRECT [${commande.slice(0, 60)}…] status=${r.status} `
        + (r.stdout ?? '').trim().slice(0, 300) + (r.stderr ? ' STDERR:' + r.stderr.slice(0, 200) : ''));
    return r;
}
function vmVivante(etiquette) {
    const virsh = spawnSync('virsh', ['domstate', 'Windows'], { encoding: 'utf8' }).stdout?.trim() ?? '?';
    const acces = spawnSync('bash', ['-c', 'ls /media/vm/dev/ton.html >/dev/null 2>&1 && echo OUI || echo NON'],
        { encoding: 'utf8' }).stdout?.trim() ?? '?';
    log(`SURVIE VM (${etiquette}) virsh="${virsh}" acces_partage=${acces}`);
    return { virsh, acces };
}
function copierLog() {
    spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
}
async function purgerFenetresVMRescapees() {
    const script = [
        'Get-Process chrome -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue',
        'Start-Sleep -Seconds 2',
        '$n = (Get-Process chrome -ErrorAction SilentlyContinue | Measure-Object).Count',
        '$agents = (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count',
        '@{ chrome_restants = $n; agents_restants = $agents } | ConvertTo-Json -Compress | Out-File -Encoding ascii C:\\dev\\purge-rescapees.json',
    ].join('\n');
    vmIt('purge-rescapees', script);
    await dodo(5000);
    let resultat = null;
    try { resultat = JSON.parse(await readFile('/media/vm/dev/purge-rescapees.json', 'utf8')); } catch { }
    log('PURGE FENÊTRES/AGENTS RESCAPÉS (avant lancement du superviseur) ' + JSON.stringify(resultat));
    if (!resultat || resultat.chrome_restants !== 0 || resultat.agents_restants !== 0) {
        throw new Error(`état VM non propre avant lancement : ${JSON.stringify(resultat)}`);
    }
}
async function journalPlat() {
    copierLog();
    let texte = '';
    try { texte = await readFile(COPIE_LOG, 'utf8'); } catch { }
    return texte.replace(/\x1b\[[0-9;]*m/g, '');
}
const horodate = (l) => l.match(/(\d{4}-\d\d-\d\dT[\d:.]+Z)/)?.[1];
const sessionDeLigne = (l) => l.match(/fenetre\{session=([^}]+)\}/)?.[1]
    ?? l.match(/\bsession=(\S+)/)?.[1] ?? null;
const champ = (l, nom) => l.match(new RegExp(`\\b${nom}=(\\S+)`))?.[1] ?? null;

/// Toutes les lignes du journal contenant l'une des sous-chaînes du brief
/// (step 4) : « capture audio morte », « réarmement programmé »,
/// « abandon définitif », « ordre audio applique » — plus la ligne de mort
/// CÔTÉ ENFANT (« capture arrêtée définitivement », windows_audio.rs) et
/// l'ouverture (« audio activé »), bornées à une fenêtre temporelle.
async function lignesAudio(debutIso, finIso, motifs) {
    const plat = await journalPlat();
    const out = [];
    for (const l of plat.split('\n')) {
        if (!motifs.some((m) => l.includes(m))) continue;
        const t = horodate(l);
        if (!t || (debutIso && t < debutIso) || (finIso && t > finIso)) continue;
        out.push({ t, session: sessionDeLigne(l), ligne: l.trim() });
    }
    return out;
}
const MOTIFS_MORT_ENFANT = ['lecture audio échouée', 'capture arrêtée définitivement'];
const MOTIFS_REGISTRE = ['capture audio morte', 'réarmement programmé', 'abandon définitif'];
const MOTIFS_ORDRE = ['ordre audio applique'];
const MOTIFS_OUVERTURE = ['audio activé'];

// ---------------------------------------------------------------- CDP (repris de D9-T14/D8)
class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.handlers = [];
        this.ready = new Promise((res) => this.ws.addEventListener('open', () => res(), { once: true }));
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.pending.has(m.id)) {
                const { resolve, reject } = this.pending.get(m.id);
                this.pending.delete(m.id);
                if (m.error) reject(new Error(JSON.stringify(m.error)));
                else resolve(m.result);
            } else if (m.method) { for (const h of this.handlers) h(m); }
        });
    }
    on(h) { this.handlers.push(h); }
    async send(method, params = {}, sessionId) {
        await this.ready;
        const id = this.nextId++;
        const msg = { id, method, params };
        if (sessionId) msg.sessionId = sessionId;
        return new Promise((resolve, reject) => {
            this.pending.set(id, { resolve, reject });
            this.ws.send(JSON.stringify(msg));
        });
    }
    async eval(sessionId, expression, awaitPromise = false) {
        const r = await this.send('Runtime.evaluate',
            { expression, awaitPromise, returnByValue: true }, sessionId);
        if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 600));
        return r.result.value;
    }
    async evalBorne(sessionId, expression, ms = 8000, awaitPromise = true) {
        return Promise.race([
            this.eval(sessionId, expression, awaitPromise),
            new Promise((r) => setTimeout(() => r({ __timeout: ms }), ms)),
        ]).catch((e) => ({ __erreur: String(e).slice(0, 200) }));
    }
}
async function attendreDevtools(port) {
    for (let i = 0; i < 80; i += 1) {
        try {
            const r = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (r.ok) return await r.json();
        } catch { }
        await dodo(250);
    }
    throw new Error('devtools timeout');
}
async function attendreLeFait(etiquette, test, maxS) {
    const debut = Date.now();
    while ((Date.now() - debut) / 1000 < maxS) {
        const r = await test();
        if (r) {
            log(`FAIT ATTEINT (${etiquette}) après ${((Date.now() - debut) / 1000).toFixed(1)} s`);
            return r;
        }
        await dodo(2000);
    }
    log(`!! FAIT NON ATTEINT (${etiquette}) après ${maxS} s`);
    return null;
}

// L'AMORCE — interception de RTCPeerConnection (pour exposer window.__pc aux
// évaluations ultérieures), reprise de D9-T14/D8.
const AMORCE = `
(() => {
  if (window.__amorceD9C2) return;
  window.__amorceD9C2 = true;
  window.__pc = null;
  const N = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;
})();
`;

// ---------------------------------------------------------------- audio (COPIÉ TEL QUEL de D7/D8, voir l'en-tête de fichier)
const SENTINEL_DB = -1000;
function expressionReleveFrequence(hzsAssignes) {
    return `(async () => {
  const CIBLES = ${JSON.stringify(hzsAssignes)};
  const SENTINEL = ${SENTINEL_DB};
  const pc = window.__pc;
  let piste = pc ? pc.getReceivers().map(r => r.track).find(t => t && t.kind === 'audio') : null;
  if (!piste) {
    const v = document.querySelector('video');
    piste = v?.srcObject?.getAudioTracks?.()[0] ?? null;
  }
  if (!piste) return { erreur: pc ? 'aucune piste audio' : 'aucune PeerConnection exposee' };
  const ctx = new AudioContext();
  try {
    await ctx.resume();
    if (ctx.state !== 'running') {
      return { erreur: 'AudioContext non demarre etat=' + ctx.state };
    }
    const analyseur = ctx.createAnalyser();
    analyseur.fftSize = 8192;
    ctx.createMediaStreamSource(new MediaStream([piste])).connect(analyseur);
    await new Promise(r => setTimeout(r, 1500));
    const bins = new Float32Array(analyseur.frequencyBinCount);
    analyseur.getFloatFrequencyData(bins);
    const db = (x) => (Number.isFinite(x) ? Math.round(x) : SENTINEL);
    const binDe = (f) => Math.max(0, Math.min(bins.length - 1,
      Math.round(f * analyseur.fftSize / ctx.sampleRate)));
    let meilleur = 0;
    for (let i = 1; i < bins.length; i++) if (bins[i] > bins[meilleur]) meilleur = i;
    const finis = [...bins].filter(Number.isFinite);
    const plancher_db = finis.length ? Math.round(finis.reduce((a, b) => a + b, 0) / finis.length) : SENTINEL;
    let statsAudio = null;
    if (pc) {
      try {
        const rapport = await pc.getStats();
        const entree = [...rapport.values()].find((x) => x.type === 'inbound-rtp' && x.kind === 'audio');
        if (entree) statsAudio = { bytes_recus: entree.bytesReceived ?? null, paquets_recus: entree.packetsReceived ?? null };
      } catch { }
    }
    return {
      hz: Math.round(meilleur * ctx.sampleRate / analyseur.fftSize),
      db: db(bins[meilleur]), plancher_db, sample_rate: ctx.sampleRate,
      niveaux: CIBLES.map((f) => ({ f, db: db(bins[binDe(f)]) })),
      piste_muted: piste.muted, piste_ready_state: piste.readyState,
      stats_audio: statsAudio,
    };
  } finally {
    await ctx.close();
  }
})()`;
}
const ETAT_ICE = `(() => { const pc = window.__pc; return pc ? pc.iceConnectionState : null; })()`;

// ---------------------------------------------------------------- ouverture des fenêtres VM (dérivé de D9-T14, sid CDP connu directement)
const pages = new Map();
let dernierSidAppPage = null;
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));
let indexAttachePage = 0;

async function ouvrirFenetreTon(n, hz, gain, profilChromeVm) {
    const fichier = `ton-d9c2-${n}-${hz}.html`;
    spawnSync('bash', ['-c', `cp ${TON_HTML} /media/vm/dev/${fichier}`]);
    const avant = appPages().length;
    log(`  · ouverture fenêtre ${n} (${fichier}?hz=${hz}&gain=${gain}, profil=${profilChromeVm})`);
    vmIt(`ouvrird9c2-${n}`, [
        '$a = @(',
        `  "--app=file:///C:/dev/${fichier}?hz=${hz}&gain=${gain}",`,
        `  "--user-data-dir=${profilChromeVm}",`,
        "  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',",
        `  '--window-size=1280,720','--window-position=${30 + n * 12},${30 + n * 12}',`,
        "  '--autoplay-policy=no-user-gesture-required',",
        "  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',",
        "  '--disable-renderer-backgrounding')",
        "Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a",
        'Start-Sleep -Seconds 2',
    ].join('\n'));
    for (let i = 0; i < 30; i += 1) {
        await dodo(2000);
        if (appPages().length > avant) return { ok: true, hz, sid: dernierSidAppPage, marqueur: fichier };
    }
    log(`  !! fenêtre ${n} : aucune page de plus après 60 s`);
    return { ok: false, hz, sid: null, marqueur: fichier };
}

/// Session agent ("w-N") lue SUR LA PAGE elle-même (vérité vivante), et
/// attente de la connexion WebRTC — garde-fou D7 : une session doit être
/// VIVANTE pour que la mesure audio dise quoi que ce soit.
async function sessionEtConnexion(cdp, sid, etiquette) {
    const session = await cdp.evalBorne(sid, `new URLSearchParams(location.search).get('session')`, 4000, false);
    const etat = await attendreLeFait(`ICE connected (${etiquette}, session=${session})`, async () => {
        const e = await cdp.evalBorne(sid, ETAT_ICE, 4000, false);
        return e === 'connected' ? e : null;
    }, 40);
    return { session, ice: etat };
}

// ---------------------------------------------------------------- PID (Montage B) — voie INDÉPENDANTE de la résolution agent
/// Résout les PID chrome.exe portant `--user-data-dir=<dossierPartage>` par
/// `Get-CimInstance Win32_Process` (WMI), filtré sur le `CommandLine` —
/// **indépendant** de la dérivation agent (`GetWindowThreadProcessId` sur le
/// hwnd, `agent/src/demarrage/audio.rs::pid_de_fenetre`). Piège du sous-bloc
/// D8 rejoué à l'identique si on l'oublie : deux `notepad.exe` sont deux PID
/// distincts — ici on VÉRIFIE plutôt que de supposer que le partage de
/// `--user-data-dir` a bien produit UN SEUL PID.
async function resoudrePidChromeVoie(dossierPartage) {
    const marqueurJson = 'pid-groupe-d9c2.json';
    // `dossierPartage` (ex. C:\dev\chrome-…-groupe) est injecté dans une
    // chaîne PowerShell À GUILLEMETS SIMPLES : le backslash n'y a aucune
    // signification spéciale (contrairement aux guillemets doubles), donc
    // AUCUN échappement n'est nécessaire — même convention que
    // `psStyleFenetre` (repris de D8/D9-T14), qui interpole un `marqueur` de
    // la même façon dans un `-like '*…*'`.
    // DEUX BUGS TROUVÉS EN COURS D'EXÉCUTION (tâche 15) :
    // 1. `.join('; ')` sur des lignes qui se terminent déjà par `|` produisait
    //    `… |;  Where-Object …`, un élément de pipeline VIDE que PowerShell
    //    refuse net (« Un élément de canal vide n'est pas autorisé »).
    //    Corrigé en joignant par `\n`, comme les scripts multi-lignes de
    //    `vmIt` ci-dessous.
    // 2. Le `\n` seul ne suffisait PAS non plus : `vmDirect` passe la
    //    commande à `winrm.js`, qui l'enveloppe TOUJOURS dans
    //    `powershell -Command "& { … }"` (CLAUDE.md, piège déjà documenté) —
    //    un script multi-lignes inline à guillemets doubles y entre en
    //    collision avec cette enveloppe et le symptôme est la MÊME erreur
    //    « élément de canal vide », le pipeline étant tronqué au premier saut
    //    de ligne. Remède établi par ce dépôt : écrire le script sur le
    //    partage et l'invoquer par `-File` — exactement ce que `vmIt` fait
    //    déjà. Bascule sur `vmIt`, qui n'exige PAS la session interactive
    //    pour un simple `Get-CimInstance` (elle n'est utile que pour
    //    atteindre GDI/USER32, sans rapport ici).
    vmIt('pid-groupe', [
        `$p = Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" |`,
        ` Where-Object { $_.CommandLine -like '*${dossierPartage}*' } |`,
        ` Select-Object ProcessId,CommandLine`,
        `$p | ConvertTo-Json -Compress | Out-File -Encoding ascii C:\\dev\\${marqueurJson}`,
    ].join('\n'));
    await dodo(3000);
    let brut = '';
    try { brut = await readFile(`/media/vm/dev/${marqueurJson}`, 'utf8'); } catch { }
    let parsed = null;
    try { parsed = JSON.parse(brut); } catch { }
    const liste = Array.isArray(parsed) ? parsed : (parsed ? [parsed] : []);
    const pidsUniques = [...new Set(liste.map((p) => p.ProcessId))];
    log(`PID CHROME (voie Get-CimInstance/CommandLine) dossier=${dossierPartage} `
        + `processus_trouves=${liste.length} pids_uniques=${JSON.stringify(pidsUniques)}`);
    return { liste, pidsUniques };
}

/// Les lignes « audio activé … pid=… » du journal (voie agent, INDÉPENDANTE
/// de Get-CimInstance ci-dessus), une par session.
async function pidsAgentDepuisJournal() {
    const plat = await journalPlat();
    const out = [];
    for (const l of plat.split('\n')) {
        if (!l.includes('audio activé')) continue;
        out.push({ t: horodate(l), session: sessionDeLigne(l), pid: champ(l, 'pid') });
    }
    return out;
}

function cpuHote(etiquette) {
    const charge = spawnSync('bash', ['-c', 'cat /proc/loadavg'], { encoding: 'utf8' }).stdout?.trim() ?? '';
    log(`CPU HÔTE (${etiquette}) loadavg=${charge}`);
    return { loadavg: charge };
}

// ---------------------------------------------------------------- phase MONTAGE A (réarmement)
async function phaseMontageA(cdp) {
    const r1 = await ouvrirFenetreTon(1, HZ_A, 0.3, `C:\\dev\\chrome-d9c2-${ETIQUETTE}-a`);
    if (!r1.ok || !r1.sid) throw new Error('fenêtre A non ouverte');
    await dodo(2000);
    const { session, ice } = await sessionEtConnexion(cdp, r1.sid, 'montage A');
    log(`MONTAGE A — fenêtre ouverte session=${session} ice=${ice}`);
    if (ice !== 'connected') {
        throw new Error(`session non connectée (ice=${ice}) — mesure invalide, garde-fou D7`);
    }

    const avantIso = maintenantIso();
    const avant = await cdp.evalBorne(r1.sid, expressionReleveFrequence([HZ_A]), 8000, true);
    log(`AVANT (montage A, session=${session}) ` + JSON.stringify(avant));

    const debutDeclencheur = maintenantIso();
    log(`>>> déclenchement (${DECLENCHEUR}) : ${commandeDeclencheur()}`);
    const restart = vmDirect(commandeDeclencheur());

    // Observation périodique — ATTEINDRE LE FAIT (la dominante revient),
    // borné à OBSERVATION_S, sans le supposer.
    const releves = [];
    const debutObs = Date.now();
    while ((Date.now() - debutObs) / 1000 < OBSERVATION_S) {
        await dodo(5000);
        const r = await cdp.evalBorne(r1.sid, expressionReleveFrequence([HZ_A]), 8000, true);
        const t = maintenantIso();
        releves.push({ t, ...r });
        log(`  relevé montage A (+${((Date.now() - debutObs) / 1000).toFixed(1)}s) ` + JSON.stringify(r));
    }
    const finObs = maintenantIso();

    const lignesMortEnfant = await lignesAudio(debutDeclencheur, finObs, MOTIFS_MORT_ENFANT);
    const lignesRegistre = await lignesAudio(debutDeclencheur, finObs, MOTIFS_REGISTRE);
    const lignesOrdre = await lignesAudio(debutDeclencheur, finObs, MOTIFS_ORDRE);

    const dominanteDansTolerance = (r) => r && Number.isFinite(r.hz)
        && Math.abs(r.hz - HZ_A) <= (48000 / 8192) // résolution d'un bin FFT
        && (r.niveaux?.[0]?.db ?? SENTINEL_DB) > SENTINEL_DB;
    const dernier = releves[releves.length - 1] ?? null;
    const dominanteRevenue = dominanteDansTolerance(dernier);
    // Le PREMIER relevé (le plus proche du déclenchement) où le niveau à HZ_A
    // retombe au plancher : la preuve que la mort a bien été OBSERVÉE avant
    // toute reprise éventuelle, pas seulement supposée du fait des messages
    // de journal.
    const premierEffondrement = releves.findIndex((r) => (r.niveaux?.[0]?.db ?? SENTINEL_DB) <= SENTINEL_DB);

    return {
        montage: 'a', session, ice_avant: ice, avant, debut_declencheur: debutDeclencheur,
        restart_service: { status: restart.status, stdout: (restart.stdout ?? '').trim() },
        releves, fin_observation: finObs,
        lignes_mort_enfant: lignesMortEnfant, lignes_registre: lignesRegistre, lignes_ordre: lignesOrdre,
        effondrement_observe: premierEffondrement >= 0,
        index_premier_effondrement: premierEffondrement,
        dominante_revenue: dominanteRevenue,
        dernier_releve: dernier,
    };
}

// ---------------------------------------------------------------- phase MONTAGE B (promotion)
async function phaseMontageB(cdp) {
    const dossierPartage = `C:\\dev\\chrome-d9c2-${ETIQUETTE}-groupe`;
    const r1 = await ouvrirFenetreTon(1, HZ_B, 0.3, dossierPartage);
    if (!r1.ok || !r1.sid) throw new Error('fenêtre B1 non ouverte');
    await dodo(2000);
    const r2 = await ouvrirFenetreTon(2, HZ_B, 0.3, dossierPartage);
    if (!r2.ok || !r2.sid) throw new Error('fenêtre B2 non ouverte');
    await dodo(2000);

    const c1 = await sessionEtConnexion(cdp, r1.sid, 'montage B fenêtre 1');
    const c2 = await sessionEtConnexion(cdp, r2.sid, 'montage B fenêtre 2');
    log(`MONTAGE B — fenêtres ouvertes session1=${c1.session} ice1=${c1.ice} `
        + `session2=${c2.session} ice2=${c2.ice}`);
    if (c1.ice !== 'connected' || c2.ice !== 'connected') {
        throw new Error(`sessions non connectées (ice1=${c1.ice}, ice2=${c2.ice}) — garde-fou D7`);
    }

    // Résolution du PID par DEUX voies indépendantes (brief : "relever les PID
    // AVANT de conclure, jamais les supposer d'un nom d'exécutable").
    const pidVoieCim = await resoudrePidChromeVoie(dossierPartage);
    await dodo(1000);
    const pidVoieAgent = await pidsAgentDepuisJournal();
    const pidsAgentDesDeuxSessions = pidVoieAgent.filter((l) => l.session === c1.session || l.session === c2.session);
    log(`PID (voie agent, agent.log "audio activé") pour les deux sessions : `
        + JSON.stringify(pidsAgentDesDeuxSessions));
    const memePidCim = pidVoieCim.pidsUniques.length === 1;
    const memePidAgent = new Set(pidsAgentDesDeuxSessions.map((l) => l.pid)).size === 1
        && pidsAgentDesDeuxSessions.length === 2;

    const avantIso = maintenantIso();
    const avant1 = await cdp.evalBorne(r1.sid, expressionReleveFrequence([HZ_B]), 8000, true);
    const avant2 = await cdp.evalBorne(r2.sid, expressionReleveFrequence([HZ_B]), 8000, true);
    log(`AVANT (montage B) session1=${c1.session} ` + JSON.stringify(avant1));
    log(`AVANT (montage B) session2=${c2.session} ` + JSON.stringify(avant2));
    const porteuseAvant = (avant1.niveaux?.[0]?.db ?? SENTINEL_DB) > SENTINEL_DB ? c1.session
        : (avant2.niveaux?.[0]?.db ?? SENTINEL_DB) > SENTINEL_DB ? c2.session : null;
    log(`PORTEUSE AVANT déclenchement : ${porteuseAvant}`);

    const debutDeclencheur = maintenantIso();
    log(`>>> déclenchement (${DECLENCHEUR}) : ${commandeDeclencheur()}`);
    const restart = vmDirect(commandeDeclencheur());

    const releves = [];
    const debutObs = Date.now();
    while ((Date.now() - debutObs) / 1000 < OBSERVATION_S) {
        await dodo(5000);
        const r1v = await cdp.evalBorne(r1.sid, expressionReleveFrequence([HZ_B]), 8000, true);
        const r2v = await cdp.evalBorne(r2.sid, expressionReleveFrequence([HZ_B]), 8000, true);
        const t = maintenantIso();
        releves.push({ t, [c1.session]: r1v, [c2.session]: r2v });
        log(`  relevé montage B (+${((Date.now() - debutObs) / 1000).toFixed(1)}s) `
            + `${c1.session}=${JSON.stringify(r1v)} ${c2.session}=${JSON.stringify(r2v)}`);
    }
    const finObs = maintenantIso();

    const lignesMortEnfant = await lignesAudio(debutDeclencheur, finObs, MOTIFS_MORT_ENFANT);
    const lignesRegistre = await lignesAudio(debutDeclencheur, finObs, MOTIFS_REGISTRE);
    const lignesOrdre = await lignesAudio(debutDeclencheur, finObs, MOTIFS_ORDRE);

    const porte = (r) => (r?.niveaux?.[0]?.db ?? SENTINEL_DB) > SENTINEL_DB;
    const dernier = releves[releves.length - 1] ?? null;
    const porteuseApres = dernier
        ? (porte(dernier[c1.session]) ? c1.session : porte(dernier[c2.session]) ? c2.session : null)
        : null;
    const dominanteRevenueQuelquePart = !!porteuseApres;
    const promotionObservee = dominanteRevenueQuelquePart && porteuseAvant && porteuseApres !== porteuseAvant;

    return {
        montage: 'b', session1: c1.session, session2: c2.session,
        pid_voie_cim: pidVoieCim, pid_voie_agent: pidsAgentDesDeuxSessions,
        meme_pid_confirme_par_les_deux_voies: memePidCim && memePidAgent,
        avant1, avant2, porteuse_avant: porteuseAvant,
        debut_declencheur: debutDeclencheur,
        restart_service: { status: restart.status, stdout: (restart.stdout ?? '').trim() },
        releves, fin_observation: finObs,
        lignes_mort_enfant: lignesMortEnfant, lignes_registre: lignesRegistre, lignes_ordre: lignesOrdre,
        porteuse_apres: porteuseApres,
        dominante_revenue_quelque_part: dominanteRevenueQuelquePart,
        promotion_observee: promotionObservee,
        dernier_releve: dernier,
    };
}

// ---------------------------------------------------------------- phase RATTACHEMENT (step 6, non-régression)
/// Le montage du critère 2 de D4, repris tel quel : tuer le CAPTEUR (jamais
/// l'enfant) par PID relevé, et vérifier que la ou les sessions saines ne
/// perdent rien — 0 `clôture de session amorcée`, 0 enfant qui se termine.
/// Sans rapport avec le déclencheur audio de ce fichier : c'est une
/// non-régression sur le chemin de reprise D2/D4 (rétention de sortie,
/// rattachement du canal MÉDIA), qui doit continuer de tenir alors que ce
/// sous-bloc a ajouté un canal de COMMANDES neuf (`AudioMort`) sur la même
/// connexion. **La course du leg 2 (identité de session par génération) n'est
/// PAS exercée ici** — voir le brief et le rapport de tâche.
async function phaseRattachement(cdp) {
    const r1 = await ouvrirFenetreTon(1, HZ_A, 0.3, `C:\\dev\\chrome-d9c2-${ETIQUETTE}-rattach`);
    if (!r1.ok || !r1.sid) throw new Error('fenêtre rattachement non ouverte');
    await dodo(2000);
    const { session, ice } = await sessionEtConnexion(cdp, r1.sid, 'rattachement');
    log(`RATTACHEMENT — fenêtre ouverte session=${session} ice=${ice}`);
    if (ice !== 'connected') {
        throw new Error(`session non connectée (ice=${ice}) — mesure invalide`);
    }
    await dodo(3000);

    // PID du CAPTEUR — relevé, jamais supposé (même doctrine que le PID
    // Chrome). `agent::superviseur::lanceur: capteur lancé pid=…`.
    const plat0 = await journalPlat();
    const ligneLancement = plat0.split('\n').reverse().find((l) => l.includes('capteur lancé pid='));
    const pidCapteur = ligneLancement?.match(/pid=(\d+)/)?.[1];
    log(`PID CAPTEUR relevé (ligne "capteur lancé") = ${pidCapteur ?? '(introuvable)'}`);
    if (!pidCapteur) throw new Error('PID du capteur introuvable dans agent.log');

    const debutMort = maintenantIso();
    const mort = vmDirect(`Stop-Process -Id ${pidCapteur} -Force; 'capteur ${pidCapteur} tue'`);
    log(`>>> capteur tué (pid=${pidCapteur}) ` + JSON.stringify({ status: mort.status, stdout: (mort.stdout ?? '').trim() }));

    // Le capteur relancé, et le canal rattaché — attendus tous deux, sans
    // les supposer.
    const relance = await attendreLeFait('capteur relancé', async () => {
        const p = await journalPlat();
        return p.includes('capteur mort, relancé') || null;
    }, 20);
    const rattache = await attendreLeFait('canal rattaché au capteur', async () => {
        const p = await journalPlat();
        const lignes = p.split('\n').filter((l) => l.includes('canal rattaché au capteur') && horodate(l) >= debutMort);
        return lignes.length > 0 ? lignes : null;
    }, 20);

    await dodo(6000); // marge après rattachement, avant le relevé final

    const finObs = maintenantIso();
    const plat = await journalPlat();
    const clotures = plat.split('\n').filter((l) => l.includes('clôture de session amorcée')
        && horodate(l) && horodate(l) >= debutMort);
    const enfantsTermines = plat.split('\n').filter((l) => /ExitStatus|enfant.*termin/i.test(l)
        && horodate(l) && horodate(l) >= debutMort);
    // Preuve positive que la session est TOUJOURS vivante après coup, pas
    // seulement l'absence de clôture : le témoin est la reprise du flux
    // vidéo, une ligne « cadence du capteur » postérieure au rattachement.
    const cadenceApres = await attendreLeFait('cadence du capteur après rattachement', async () => {
        const p = await journalPlat();
        return p.split('\n').some((l) => l.includes('cadence du capteur') && l.includes(`session=${session}`)
            && horodate(l) >= debutMort) || null;
    }, 15);

    return {
        session, pid_capteur: pidCapteur, debut_mort: debutMort, mort: { status: mort.status },
        capteur_relance: !!relance, canal_rattache: !!rattache, lignes_rattache: rattache ?? [],
        cloture_de_session_amorcee: clotures.length, lignes_cloture: clotures,
        enfants_termines: enfantsTermines.length, lignes_enfants_termines: enfantsTermines,
        session_vivante_apres: !!cadenceApres,
        fin_observation: finObs,
    };
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9996);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d9c2-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns', '--disable-popup-blocking',
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', 'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} MONTAGE=${MONTAGE} PORT_SHELL=${PORT_SHELL} OBSERVATION_S=${OBSERVATION_S} chrome pid=${chrome.pid}`);

    const releve = { etiquette: ETIQUETTE, montage: MONTAGE, port_shell: PORT_SHELL };
    try {
        const version = await attendreDevtools(port);
        const qui = spawnSync('bash', ['-c',
            `ss -ltnp 2>/dev/null | grep ':${port} ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1`],
            { encoding: 'utf8' }).stdout.trim();
        log(`identité du navigateur : pid écoutant=${qui} pid lancé=${chrome.pid} version=${version.Browser}`);
        if (String(qui) !== String(chrome.pid)) throw new Error(`port ${port} tenu par ${qui}, pas ${chrome.pid}`);
        releve.navigateur = version.Browser;

        const cdp = new Cdp(version.webSocketDebuggerUrl);
        cdp.on(async (m) => {
            if (m.method === 'Target.attachedToTarget') {
                const { sessionId, targetInfo } = m.params;
                if (targetInfo.type !== 'page') {
                    await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
                    return;
                }
                pages.set(sessionId, { targetId: targetInfo.targetId, url: targetInfo.url });
                indexAttachePage += 1;
                const estInitiale = indexAttachePage === 1;
                const estShell = targetInfo.url.includes('shell.html');
                const estAppPage = !estInitiale && !estShell;
                log(`+ page attachée  session=${sessionId.slice(0, 8)} url=${targetInfo.url} `
                    + `index=${indexAttachePage} initiale=${estInitiale} shell=${estShell} app=${estAppPage}`);
                await cdp.send('Page.enable', {}, sessionId).catch(() => { });
                await cdp.send('Runtime.enable', {}, sessionId).catch(() => { });
                await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: AMORCE }, sessionId).catch(() => { });
                await cdp.send('Runtime.evaluate', { expression: AMORCE, returnByValue: true }, sessionId).catch(() => { });
                if (estAppPage) {
                    dernierSidAppPage = sessionId;
                    // BUG TROUVÉ EN COURS D'EXÉCUTION (premier essai, contrôle
                    // ROUGE) : sans cette ligne, une fenêtre Chrome `--app`
                    // headless atterrit "naturellement" à 1280×632 (~88 px de
                    // moins que demandé — chiffre déjà documenté ailleurs pour
                    // d'autres navigateurs), que le pilote SudoVDA refuse de
                    // faire correspondre à la sortie virtuelle déjà alignée
                    // sur 1280×720 par un essai antérieur (persistance de
                    // registre, CLAUDE.md D8/D9-2bis) — `sortie créée mais
                    // introuvable dans la topologie DXGI`, en boucle, aucune
                    // fenêtre ne s'attache jamais. Repris de
                    // `pilote-critere1.mjs` (tâche 14), sans la complexité du
                    // dpr (non pertinente ici, dpr=1 partout dans ce pilote).
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false }, sessionId).catch(() => { });
                }
                await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
            } else if (m.method === 'Target.detachedFromTarget') {
                pages.delete(m.params.sessionId);
            } else if (m.method === 'Target.targetInfoChanged') {
                for (const [, p] of pages) {
                    if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
                }
            }
        });
        await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
        await cdp.send('Target.setDiscoverTargets', { discover: true });

        log('>>> ÉTAPE 0 : préparation — VM sans fenêtre éligible');
        releve.cpu_repos = cpuHote('au repos, avant toute session');
        await purgerFenetresVMRescapees();

        await cdp.send('Target.createTarget', { url: URL_SHELL });
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut')?.textContent`));

        log('>>> lancement du superviseur');
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 BITRATE=${BITRATE} `
            + `SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        await dodo(6000);

        if (MONTAGE === 'a') {
            releve.phase = await phaseMontageA(cdp);
        } else if (MONTAGE === 'b') {
            releve.phase = await phaseMontageB(cdp);
        } else if (MONTAGE === 'rattachement') {
            releve.phase = await phaseRattachement(cdp);
        } else {
            throw new Error(`MONTAGE inconnu : ${MONTAGE} (attendu 'a', 'b' ou 'rattachement')`);
        }

        releve.survie_finale = vmVivante('fin de mesure');
        await writeFile(SORTIE_JSON, JSON.stringify(releve, null, 1));
        log('relevé écrit dans ' + SORTIE_JSON);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
        await dodo(8000);
        copierLog();
        log('journal copié dans ' + COPIE_LOG + ' (après la fermeture du navigateur)');
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
