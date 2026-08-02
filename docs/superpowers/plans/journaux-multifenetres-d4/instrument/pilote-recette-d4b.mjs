#!/usr/bin/env node
// Recette D4 — tâche 11 : les trois critères de réception de la capture
// mutualisée, en conditions de produit. **Rejeu de la tâche 9**, sur le
// binaire de la tâche 10, avec les quatre corrections de protocole que la
// recette 9 s'est elle-même prescrites :
//
//   1. la source BOUGE — une page animée (canvas, rAF) dans une fenêtre
//      Chrome `--app`, à la place du Bloc-notes immobile. Desktop Duplication
//      n'émet une trame qu'au changement du bureau : une mire fixe fait
//      mesurer zéro image et conclure à tort à une panne ;
//   2. la CEINTURE est éprouvée — le capteur est tué pendant que des
//      commandes circulent (le contrôleur d'adaptation en émet une par
//      seconde et par session), et l'on regarde si `commander` rend une
//      erreur ou suspend l'enfant ;
//   3. la survie de la VM est contrôlée APRÈS CHAQUE RANG, pas seulement à
//      la fin — la recette 9 a perdu une exécution à une hibernation ;
//   4. le « avant » du critère 3 se joue sur `7d7e254` (phase `cadence`
//      inchangée, employée sur les deux binaires).
//
// Dérivé de `journaux-multifenetres-d3/instrument/pilote-recette-d3.mjs`
// (même squelette CDP + vm-it + marqueurs), avec trois phases distinctes,
// choisies par la variable PHASE :
//
//   PHASE=montee   critère 1 — ouvrir des fenêtres ANIMÉES une par une jusqu'au
//                  refus, relever combien DIFFUSENT réellement (images décodées
//                  côté navigateur, pas « une page s'est ouverte »).
//   PHASE=capteur  critère 2 — N fenêtres diffusent, on tue le capteur PAR PID
//                  relevé, on regarde si les sessions survivent et si l'image
//                  revient.
//   PHASE=cadence  critère 3 — N fenêtres, deux relevés de `framesDecoded`
//                  espacés, plus les lignes de cadence des deux côtés.
//                  Employée telle quelle sur le binaire d'AVANT (source
//                  locale) et sur celui d'APRÈS (source distante) : c'est ce
//                  qui rend les deux chiffres opposables.
//
// Contraintes de protocole héritées de D1/D2/D3, chacune ayant coûté une
// exécution entière :
//   1. le navigateur est lancé AVANT le superviseur (DELAI_ATTENTE_VIEWPORT_MAX) ;
//   2. `--disable-popup-blocking` ;
//   3. AUCUNE capture d'écran CDP pendant la séquence ;
//   4. l'instance de navigateur pilotée est vérifiée par PID écoutant ;
//   5. les trois drapeaux anti-gel des pages d'arrière-plan ;
//   6. toute évaluation CDP sur une page portant un flux WebRTC est BORNÉE
//      (`evalBorne` ci-dessous) — elle peut ne jamais rendre ;
//   7. on tue par PID relevé, jamais par `pkill -f`.

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = '/home/mallanic/Projects/Guacamole';
const AIDE = process.env.AIDE ?? '/tmp/user/0/claude-0/-home-mallanic-Projects-Guacamole/dffb8a0c-ae33-4e14-a0b6-f1eab68129c9/scratchpad/instrument';
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const COPIE_LOG = process.env.COPIE_LOG ?? '/tmp/agent-encours-d4.log';
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
const PHASE = process.env.PHASE ?? 'montee';
const N_CIBLE = Number(process.env.N_CIBLE ?? 12);
const SANS_SUPERVISEUR = process.env.SANS_SUPERVISEUR === '1';

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(7);
    console.log(`[${dt}s ${new Date().toISOString()}] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- VM
function vmIt(nom, ps) {
    const r = spawnSync('bash', [join(AIDE, 'vm-it.sh'), nom, ps], { encoding: 'utf8', env: process.env });
    if (r.status !== 0) log(`!! vm-it ${nom} a échoué`, (r.stderr ?? '').slice(0, 400));
    return r;
}
function vmItFichier(nom, fichierPs) {
    const ps = spawnSync('cat', [join(AIDE, fichierPs)], { encoding: 'utf8' }).stdout ?? '';
    return vmIt(nom, ps);
}
function winrm(commande) {
    const r = spawnSync('node', [join(RACINE, 'scripts/winrm.js'), commande],
        { encoding: 'utf8', env: process.env, maxBuffer: 16 * 1024 * 1024 });
    if (r.status !== 0) log('!! winrm a échoué', (r.stderr ?? '').slice(0, 400));
    return r.stdout ?? '';
}

/// Correction de protocole n°3 : la VM se met en veille prolongée toute seule
/// (comportement documenté, cause inconnue). Un rang mesuré après une
/// hibernation ne mesure rien. On l'éprouve par un ACCÈS réel — la présence de
/// l'entrée de montage CIFS ne prouve rien —, après chaque rang.
function vmVivante(etiquette) {
    const virsh = spawnSync('virsh', ['domstate', 'Windows'], { encoding: 'utf8' }).stdout?.trim() ?? '?';
    const acces = spawnSync('bash', ['-c', 'ls /media/vm/dev/anim-d4.html >/dev/null 2>&1 && echo OUI || echo NON'],
        { encoding: 'utf8' }).stdout?.trim() ?? '?';
    log(`SURVIE VM (${etiquette}) virsh="${virsh}" acces_partage=${acces}`);
    return { virsh, acces };
}

async function fenetresVm(etiquette) {
    vmItFichier('listefen', 'listefen.ps1');
    await dodo(6000);
    const out = spawnSync('cat', ['/media/vm/dev/fenetres.txt'], { encoding: 'utf8' }).stdout ?? '';
    log(`--- fenêtres VM (${etiquette}) ---\n` + out.trimEnd());
    return out;
}

function copierLog() {
    spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
}
async function journalPlat() {
    copierLog();
    let texte = '';
    try { texte = await readFile(COPIE_LOG, 'utf8'); } catch { }
    return texte.replace(/\x1b\[[0-9;]*m/g, '');
}

/// Compte, dans la copie courante du journal, les lignes qui font le verdict.
/// Relevé, pas interprétation : les lignes elles-mêmes sont dans le journal
/// versé.
async function marqueurs(etiquette) {
    const plat = await journalPlat();
    const compte = (motif) => (plat.match(new RegExp(motif, 'g')) ?? []).length;
    const m = {
        lignes: plat.split('\n').length,
        cloture: compte('clôture de session amorcée'),
        capteur_lance: compte('capteur lancé'),
        capteur_relance: compte('capteur mort, relancé'),
        capteur_termine: compte('capteur terminé'),
        attachee_capteur: compte('fenêtre attachée au capteur'),
        rattache: compte('canal rattaché au capteur'),
        refus_capteur: compte("l'attache au capteur a été refusée|attache de la session"),
        reouverture: compte('accès à la duplication perdu, réouverture'),
        mutex_abandonne: compte('0x887a0026'),
        sortie_creee: compte('sortie virtuelle créée'),
        sortie_rendue: compte('sortie virtuelle rendue au pilote'),
        creation_refusee: compte('création de sortie refusée'),
        enfant_lance: compte('enfant lancé'),
        enfant_termine: compte('enfant terminé'),
        abandon_relances: compte("la session n'a pas tenu après"),
        erreurs: compte('ERROR'),
        // --- Ceinture (correction de protocole n°2) ---------------------
        // Si `commander` rend une ERREUR quand le capteur meurt, l'adaptation
        // le journalise. Si `commander` SUSPEND, la boucle de transport est
        // figée : aucune de ces lignes, et plus aucune image.
        refus_debit: compte("l'encodeur refuse le réglage du débit à chaud"),
        refus_taille: compte("changement de taille d'encodage refusé"),
        reponse_capteur: compte('réponse du capteur'),
        fin_fenetre: compte('fin de la fenêtre côté capteur'),
        cadence_capteur: compte('cadence du capteur'),
        cadence_piste: compte('cadence de la piste vidéo'),
    };
    log(`MARQUEURS (${etiquette}) ` + JSON.stringify(m));
    return m;
}

// ---------------------------------------------------------------- CDP
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
            } else if (m.method) {
                for (const h of this.handlers) h(m);
            }
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
    /// Piège documenté en D2 : une évaluation CDP sur une page portant un flux
    /// WebRTC actif peut ne JAMAIS rendre. Toute lecture de stats passe par
    /// ici.
    async evalBorne(sessionId, expression, ms = 6000, awaitPromise = true) {
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

const AMORCE = `
  window.__pc = null;
  const N = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;
`;

const pages = new Map(); // sessionId -> {targetId, url}
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));

const STATS = `(async () => {
  const pc = window.__pc;
  if (!pc) return { pc: null };
  const r = await pc.getStats(); const t = [...r.values()];
  const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
  const paire = t.find(x => x.type === 'candidate-pair' && (x.selected || x.nominated));
  return {
    etat: pc.iceConnectionState,
    images: v?.framesDecoded ?? null,
    l: v?.frameWidth ?? null, h: v?.frameHeight ?? null,
    octets: v?.bytesReceived ?? null,
    rtt: paire?.currentRoundTripTime ?? null,
    horloge: Date.now(),
  };
})()`;

/// Relève les stats de CHAQUE page d'application, en parallèle et borné :
/// séquentiellement, une page figée bloquerait toutes les suivantes, et le
/// temps écoulé entre la première et la dernière fausserait les cadences.
async function statsToutes(cdp, etiquette) {
    const entrees = appPages();
    const resultats = await Promise.all(entrees.map(async ([sid, p]) => {
        const s = await cdp.evalBorne(sid, STATS);
        return [p.url.replace(/^.*\?session=/, 'w:'), s];
    }));
    const out = Object.fromEntries(resultats);
    log(`STATS (${etiquette}) ` + JSON.stringify(out));
    return out;
}

/// Deux relevés espacés de `ms`, et la cadence par fenêtre qui s'en déduit.
/// La cadence est CALCULÉE (delta d'images / delta d'horloge de la page),
/// jamais estimée.
async function cadenceNavigateur(cdp, etiquette, ms) {
    const a = await statsToutes(cdp, `${etiquette} — relevé 1`);
    await dodo(ms);
    const b = await statsToutes(cdp, `${etiquette} — relevé 2`);
    const out = {};
    for (const k of Object.keys(b)) {
        const av = a[k], ap = b[k];
        if (!av || !ap || av.images == null || ap.images == null) { out[k] = null; continue; }
        const dt = (ap.horloge - av.horloge) / 1000;
        out[k] = {
            images: ap.images - av.images,
            secondes: Number(dt.toFixed(3)),
            i_par_s: Number(((ap.images - av.images) / dt).toFixed(2)),
            taille: `${ap.l}x${ap.h}`,
            octets: ap.octets - av.octets,
            kbps: Number((((ap.octets - av.octets) * 8) / dt / 1000).toFixed(0)),
            etat: ap.etat,
        };
    }
    log(`CADENCE NAVIGATEUR (${etiquette}) ` + JSON.stringify(out, null, 1));
    return out;
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9991);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d4-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new',
        `--remote-debugging-port=${port}`,
        '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`,
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--disable-gpu',
        '--window-size=1280,720',
        '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns',
        '--disable-popup-blocking',
        '--disable-background-timer-throttling',
        '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding',
        'about:blank',
    ], { stdio: 'ignore' });
    log(`PHASE=${PHASE} N_CIBLE=${N_CIBLE} chrome pid=${chrome.pid} port=${port}`);

    try {
        const version = await attendreDevtools(port);
        const qui = spawnSync('bash', ['-c',
            `ss -ltnp 2>/dev/null | grep ':${port} ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1`],
            { encoding: 'utf8' }).stdout.trim();
        log(`identité du navigateur : pid écoutant=${qui} pid lancé=${chrome.pid} version=${version.Browser}`);
        if (String(qui) !== String(chrome.pid)) {
            throw new Error(`le port ${port} est tenu par le pid ${qui}, pas par notre Chrome (${chrome.pid})`);
        }

        const cdp = new Cdp(version.webSocketDebuggerUrl);
        cdp.on(async (m) => {
            if (m.method === 'Target.attachedToTarget') {
                const { sessionId, targetInfo } = m.params;
                if (targetInfo.type !== 'page') {
                    await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
                    return;
                }
                pages.set(sessionId, { targetId: targetInfo.targetId, url: targetInfo.url });
                log(`+ page attachée  session=${sessionId.slice(0, 8)} url=${targetInfo.url}`);
                await cdp.send('Page.enable', {}, sessionId).catch(() => { });
                await cdp.send('Runtime.enable', {}, sessionId).catch(() => { });
                await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: AMORCE }, sessionId).catch(() => { });
                if (VIEWPORT_FORCE) {
                    const [L, H] = VIEWPORT_FORCE.split('x').map(Number);
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: L, height: H, deviceScaleFactor: 1, mobile: false }, sessionId)
                        .catch((e) => log('  !! imposition de viewport impossible', String(e).slice(0, 150)));
                }
                await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
            } else if (m.method === 'Target.detachedFromTarget') {
                const p = pages.get(m.params.sessionId);
                if (p) log(`- page détachée   session=${m.params.sessionId.slice(0, 8)} url=${p.url}`);
                pages.delete(m.params.sessionId);
            } else if (m.method === 'Target.targetInfoChanged') {
                for (const [, p] of pages) {
                    if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
                }
            }
        });
        await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
        await cdp.send('Target.setDiscoverTargets', { discover: true });

        // ---- Étape 0 : la VM part sans AUCUNE fenêtre Bloc-notes.
        log('>>> ÉTAPE 0 : nettoyage — état de départ SANS fenêtre');
        vmItFichier('preparerd4b', 'preparer-d4b.ps1');
        await dodo(6000);
        await fenetresVm('avant superviseur');

        // ---- La page-shell, AVANT le superviseur.
        await cdp.send('Target.createTarget', { url: URL_SHELL });
        log('shell demandée', URL_SHELL);
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut').textContent`));

        const etat = async (etiquette) => {
            const l = await cdp.evalBorne(sidShell,
                `Array.from(document.querySelectorAll('#fenetres li')).map(e=>e.textContent).join(' | ')`, 5000, false);
            const s = await cdp.evalBorne(sidShell, `document.querySelector('#statut').textContent`, 5000, false);
            log(`ÉTAT SHELL (${etiquette}) liste=[${l}] statut="${s}"`);
            log(`  pages d'application ouvertes (${appPages().length}) : ${appPages().map(([, p]) => p.url).join(' , ')}`);
            return { liste: l, statut: s };
        };

        // ---- Le superviseur, APRÈS la shell.
        if (!SANS_SUPERVISEUR) {
            log('>>> lancement du superviseur (SUPERVISEUR=1)');
            const sup = spawnSync('bash', ['-c',
                `cd ${RACINE} && SUPERVISEUR=1 SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
                { encoding: 'utf8', env: process.env });
            log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
            if ((sup.stderr ?? '').trim()) log('run-agent.sh STDERR :', sup.stderr.trim().replace(/\n/g, ' / '));
        }
        await dodo(6000);
        await marqueurs('superviseur démarré, 0 fenêtre');
        log('PIDs agent après démarrage : ' + winrm(
            "Get-Process agent -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id | Sort-Object").replace(/\s+/g, ' ').trim());

        /// Ouvre la fenêtre `n` et attend qu'une page d'application de plus se
        /// soit ouverte. Attendre le FAIT, jamais une durée.
        const ouvrirFenetre = async (n, attente = 40) => {
            const avant = appPages().length;
            log(`>>> OUVERTURE fenêtre ${n} (attendu : ${avant + 1} pages)`);
            const tOuverture = new Date().toISOString();
            // Correction de protocole n°1 : une source qui BOUGE. Une fenêtre
            // Chrome `--app` sur `anim-d4.html`, dont le canvas se redessine à
            // chaque rAF. Un profil par fenêtre (`--user-data-dir`), sans quoi
            // Chrome rattacherait la seconde à la première instance et
            // n'ouvrirait qu'un seul processus — mais surtout, une seule
            // fenêtre par instance : le critère « application à fenêtre
            // unique » de D2 est respecté (vérifié par `listefen`).
            vmIt(`ouvrird4-${n}`, [
                `$a = @(`,
                `  "--app=file:///C:/dev/anim-d4.html?n=${n}",`,
                `  "--user-data-dir=C:\\dev\\chrome-d4-${n}",`,
                `  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',`,
                `  '--window-size=1280,720','--window-position=${40 + n * 10},${40 + n * 10}',`,
                `  '--disable-features=CalculateNativeWinOcclusion',`,
                `  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',`,
                `  '--disable-renderer-backgrounding')`,
                `Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a`,
                `Start-Sleep -Seconds 2`,
            ].join('\n'));
            for (let i = 0; i < attente; i += 1) {
                await dodo(2000);
                if (appPages().length > avant) {
                    log(`   fenêtre ${n} : page ouverte en ${(i + 1) * 2}s (total ${appPages().length})`);
                    return { ouverte: true, t: tOuverture, secondes: (i + 1) * 2 };
                }
            }
            log(`   !! fenêtre ${n} : AUCUNE page ouverte après ${attente * 2}s (total ${appPages().length})`);
            return { ouverte: false, t: tOuverture, secondes: attente * 2 };
        };

        if (PHASE === 'montee') {
            // ---------------------------------------------------- CRITÈRE 1
            const journal = [];
            for (let n = 1; n <= N_CIBLE; n += 1) {
                const r = await ouvrirFenetre(n);
                await dodo(6000);
                const e = await etat(`après fenêtre ${n}`);
                const m = await marqueurs(`après fenêtre ${n}`);
                // Cadence courte : ce qui compte au critère 1 est COMBIEN
                // diffusent, pas à quelle vitesse — mais « diffuse » ne se
                // constate que par des images qui augmentent.
                const c = await cadenceNavigateur(cdp, `rang ${n}`, 5000);
                const diffusent = Object.values(c).filter((x) => x && x.images > 0).length;
                log(`RANG ${n} : pages=${appPages().length} DIFFUSENT=${diffusent} ouverte=${r.ouverte}`);
                vmVivante(`après rang ${n}`);
                journal.push({ rang: n, pages: appPages().length, diffusent, ouverte: r.ouverte, statut: e.statut, marqueurs: m });
                if (!r.ouverte) {
                    log(`>>> ARRÊT DE LA MONTÉE au rang ${n} : aucune page de plus.`);
                    await fenetresVm(`au rang bloquant ${n}`);
                    break;
                }
            }
            log('SYNTHÈSE MONTÉE ' + JSON.stringify(journal, null, 1));
            await cadenceNavigateur(cdp, 'PALIER FINAL (30 s)', 30000);
            await etat('FIN MONTÉE');
        }

        if (PHASE === 'capteur') {
            // ---------------------------------------------------- CRITÈRE 2
            for (let n = 1; n <= N_CIBLE; n += 1) {
                const r = await ouvrirFenetre(n);
                if (!r.ouverte) log(`!! fenêtre ${n} non ouverte — la suite continue avec ${appPages().length} pages`);
                vmVivante(`après ouverture ${n}`);
                await dodo(3000);
            }
            await dodo(8000);
            await etat('avant la mise à mort du capteur');
            const avant = await marqueurs('AVANT mise à mort');
            const cAvant = await cadenceNavigateur(cdp, 'AVANT mise à mort', 10000);

            // Le PID du capteur se lit dans le journal : c'est le superviseur
            // qui l'écrit (`capteur lancé pid=…`). On prend le DERNIER, pour
            // le cas où il aurait déjà été relancé.
            const plat = await journalPlat();
            const pids = [...plat.matchAll(/capteur lancé\s+pid=(\d+)|pid=(\d+).*capteur lancé/g)]
                .map((m) => Number(m[1] ?? m[2]));
            const pidCapteur = pids[pids.length - 1];
            log(`PIDs capteur vus dans le journal : [${pids.join(',')}] — cible = ${pidCapteur}`);
            if (!pidCapteur) throw new Error('aucun « capteur lancé pid= » dans le journal');
            const tousPid = winrm("Get-Process agent -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id | Sort-Object");
            log('PIDs agent vivants avant la mise à mort : ' + tousPid.replace(/\s+/g, ' ').trim());

            const tMort = new Date().toISOString();
            log(`>>> MISE À MORT DU CAPTEUR par PID ${pidCapteur} (jamais pkill -f) à ${tMort}`);
            const sortieTuerie = winrm(`Stop-Process -Id ${pidCapteur} -Force -ErrorAction SilentlyContinue; ` +
                `(Get-Process -Id ${pidCapteur} -ErrorAction SilentlyContinue) -eq $null`);
            log('résultat de la mise à mort (True = le PID a disparu) : ' + sortieTuerie.trim());

            // Attendre le FAIT : une ligne « capteur mort, relancé », puis des
            // rattachements.
            let vuRelance = null, vuRattache = 0;
            for (let i = 0; i < 40; i += 1) {
                await dodo(1000);
                const p = await journalPlat();
                const relances = [...p.matchAll(/(\S+Z)\s+WARN.*capteur mort, relancé/g)];
                const ratt = (p.match(/canal rattaché au capteur/g) ?? []).length;
                if (relances.length > avant.capteur_relance) vuRelance = relances[relances.length - 1][1];
                vuRattache = ratt;
                if (vuRelance && vuRattache >= avant.rattache + appPages().length) {
                    log(`   relance ET ${vuRattache - avant.rattache} rattachements vus à t+${i + 1}s`);
                    break;
                }
                if (i % 5 === 0) log(`   … t+${i + 1}s relance=${vuRelance ?? 'pas encore'} rattachements=${vuRattache}`);
            }
            const tFinAttente = new Date().toISOString();
            await dodo(4000);
            const cApres = await cadenceNavigateur(cdp, 'APRÈS relance du capteur', 10000);
            const apres = await marqueurs('APRÈS relance du capteur');
            await etat('après relance du capteur');
            log('BORNES TEMPORELLES (horodatage local du pilote, UTC)');
            log(`   mise à mort demandée : ${tMort}`);
            log(`   fin de l'attente du fait : ${tFinAttente}`);
            log('SYNTHÈSE CAPTEUR ' + JSON.stringify({
                pid_capteur_tue: pidCapteur,
                cadence_avant: cAvant, cadence_apres: cApres,
                delta_cloture: apres.cloture - avant.cloture,
                delta_relance: apres.capteur_relance - avant.capteur_relance,
                delta_rattache: apres.rattache - avant.rattache,
                delta_attachee: apres.attachee_capteur - avant.attachee_capteur,
                delta_enfant_lance: apres.enfant_lance - avant.enfant_lance,
                delta_enfant_termine: apres.enfant_termine - avant.enfant_termine,
            }, null, 1));
        }

        if (PHASE === 'cadence') {
            // ---------------------------------------------------- CRITÈRE 3
            for (let n = 1; n <= N_CIBLE; n += 1) {
                const r = await ouvrirFenetre(n);
                if (!r.ouverte) log(`!! fenêtre ${n} non ouverte`);
                vmVivante(`après ouverture ${n}`);
                await dodo(3000);
            }
            await dodo(10000);
            await etat('palier établi');
            await marqueurs('palier établi');
            // 30 s : trois périodes des compteurs de 10 s côté agent, donc au
            // moins deux lignes de cadence complètes par côté.
            const c = await cadenceNavigateur(cdp, `palier N=${N_CIBLE} (30 s)`, 30000);
            log('SYNTHÈSE CADENCE ' + JSON.stringify(c, null, 1));
            await marqueurs('fin de palier');
        }

        copierLog();
        log('journal copié dans ' + COPIE_LOG);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
