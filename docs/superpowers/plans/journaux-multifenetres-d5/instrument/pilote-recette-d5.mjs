#!/usr/bin/env node
// Recette D5 — tâche 11 : les trois critères du vivier d'encodeurs, en
// conditions de produit.
//
// Dérivé de `journaux-multifenetres-d4/instrument/pilote-recette-d4b.mjs`
// (même squelette CDP + vm-it + marqueurs). Ce qui change : la séquence n'est
// plus une simple montée, elle exerce le SOMMEIL — éviction par le focus,
// puis masquage — et relève les deux délais de réveil.
//
// Contraintes de protocole héritées, chacune ayant coûté une exécution :
//   1. la source BOUGE (page canvas animée, un `--user-data-dir` par fenêtre) ;
//   2. le navigateur est lancé AVANT le superviseur (DELAI_ATTENTE_VIEWPORT_MAX) ;
//   3. `--disable-popup-blocking` et les trois drapeaux anti-gel ;
//   4. AUCUNE capture d'écran CDP pendant la séquence ;
//   5. toute évaluation CDP sur une page portant un flux WebRTC est BORNÉE ;
//   6. l'instance de navigateur pilotée est vérifiée par PID écoutant ;
//   7. survie de la VM contrôlée après chaque rang ;
//   8. on tue par PID relevé, jamais par `pkill -f`.
//
// ── DEUX INSTRUMENTS À DÉCLARER DANS LE RAPPORT ──────────────────────────
//
// (a) LE FOCUS. `Emulation.setFocusEmulationEnabled` pose l'état de focus réel
//     de la page (`document.hasFocus()` en rend compte). Un navigateur sans
//     interface n'émet pas toujours les événements `focus`/`blur` qui vont
//     avec : le pilote les déclenche alors lui-même. L'état annoncé par le
//     produit reste celui que la page rapporte réellement — l'instrument
//     déclenche l'émission, il ne falsifie pas l'état.
//
// (b) LA VISIBILITÉ, ENTIÈREMENT IMPOSÉE PAR LE PILOTE, et c'est le point le
//     plus lourd de ce protocole. **Un Chrome sans interface rapporte
//     `document.hidden = true` pour TOUTE fenêtre d'arrière-plan** — relevé
//     lors d'un premier essai abandonné : à sept fenêtres, six pages sur sept
//     se déclaraient cachées. Cela ne modélise pas le scénario de la recette,
//     qui est « dix fenêtres visibles sur un bureau », et le produit s'y
//     endormirait entièrement pour une raison étrangère à ce qu'on mesure.
//
//     Le pilote redéfinit donc `document.hidden` / `visibilityState` sur un
//     drapeau `window.__cachee` qu'il pilote, **dès le chargement du document**
//     (`Page.addScriptToEvaluateOnNewDocument`), et déclenche
//     `visibilitychange` quand il le change. La visibilité annoncée est donc
//     celle du SCÉNARIO, pas celle du navigateur.
//
//     Ce qui est réellement exercé : tout le chemin à partir de
//     `attachVisibilite` — encodage du message, data channel, transport,
//     capteur, vivier, fil de fenêtre. Ce qui ne l'est PAS : le rapport de
//     visibilité du navigateur lui-même.

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

function racineDepot() {
    const r = spawnSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' });
    if (r.status !== 0) {
        throw new Error(`hors du depot git : impossible de deriver RACINE (git rev-parse a echoue : ${(r.stderr ?? '').trim()})`);
    }
    return r.stdout.trim();
}
const RACINE = process.env.RACINE
    ?? join(racineDepot(), '.claude/worktrees/chantier-multifenetres-d5');
const AIDE = process.env.AIDE
    ?? '/tmp/user/0/claude-0/-home-mallanic-Projects-Guacamole/41109bc6-2e4c-4722-a247-302540bd93f0/scratchpad/instrument';
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const COPIE_LOG = process.env.COPIE_LOG ?? '/tmp/agent-encours-d5.log';
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
const N_CIBLE = Number(process.env.N_CIBLE ?? 11);
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

/// La VM se met en veille prolongée toute seule (comportement documenté, cause
/// inconnue). Éprouvé par un ACCÈS réel : la présence de l'entrée de montage
/// CIFS ne prouve rien.
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
        attachee_capteur: compte('fenêtre attachée au capteur'),
        // --- Le sommeil (sous-bloc D5) ---------------------------------
        endormie: compte('fenêtre endormie'),
        reveillee: compte('fenêtre réveillée'),
        reveil_refuse: compte('réveil refusé'),
        visibilite_refusee: compte('visibilité refusée par le capteur'),
        // --- C2 : l'adaptation par la résolution ------------------------
        taille_changee: compte("taille d'encodage changée"),
        taille_refusee: compte("changement de taille d'encodage refusé"),
        refus_debit: compte("l'encodeur refuse le réglage du débit à chaud"),
        // --- Le vivier de sorties virtuelles ----------------------------
        sortie_creee: compte('sortie virtuelle créée'),
        sortie_rendue: compte('sortie virtuelle rendue au pilote'),
        creation_refusee: compte('création de sortie refusée'),
        trop_de_noms: compte('TOO_MANY_NAMES|0x80070044'),
        // --- Le reste ---------------------------------------------------
        reouverture: compte('accès à la duplication perdu, réouverture'),
        mutex_abandonne: compte('0x887a0026'),
        enfant_lance: compte('enfant lancé'),
        enfant_termine: compte('enfant terminé'),
        erreurs: compte('ERROR'),
    };
    log(`MARQUEURS (${etiquette}) ` + JSON.stringify(m));
    return m;
}

/// Les transitions de sommeil, avec leur horodatage et leur session : c'est la
/// pièce du critère 3 côté agent.
async function transitions(etiquette) {
    const plat = await journalPlat();
    const lignes = plat.split('\n').filter((l) =>
        /fenêtre endormie|fenêtre réveillée|réveil refusé/.test(l));
    const out = lignes.map((l) => {
        const t = l.match(/(\d{4}-\d\d-\d\dT[\d:.]+Z)/)?.[1] ?? '?';
        const s = l.match(/session=(\S+)/)?.[1] ?? '?';
        const d = l.match(/duree_ms=(\d+)/)?.[1];
        const quoi = /endormie/.test(l) ? 'dort' : (/réveil refusé/.test(l) ? 'refus' : 'réveil');
        return { t, s, quoi, duree_ms: d ? Number(d) : undefined };
    });
    log(`TRANSITIONS (${etiquette}) ${out.length} ligne(s)\n` + JSON.stringify(out, null, 1));
    return out;
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
    /// Une évaluation CDP sur une page portant un flux WebRTC actif peut ne
    /// JAMAIS rendre. Toute lecture passe par ici.
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

  // Instrument (b) — voir l'en-tête. La visibilité vient du SCÉNARIO, pas du
  // navigateur : posée AVANT tout script de la page, donc avant la toute
  // première annonce de \`attachVisibilite\`, sans quoi celle-ci partirait avec
  // la valeur du navigateur.
  window.__cachee = false;
  Object.defineProperty(document, 'hidden', {
    configurable: true, get: () => window.__cachee });
  Object.defineProperty(document, 'visibilityState', {
    configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
`;

const pages = new Map(); // sessionId -> {targetId, url}
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));
const nomDe = (url) => url.replace(/^.*\?session=/, 'w:');

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
    focalisee: document.hasFocus(), cachee: document.hidden,
    // \`#status\` est le bandeau des pages d'application (\`#statut\` est celui
    // de la page-shell — une exécution l'a lu par erreur et n'a rien vu). Il
    // porte le TEXTE de la raison du sommeil, et c'est le seul endroit
    // observable où elle apparaisse : ni l'agent ni l'enfant ne la journalisent.
    bandeau: document.querySelector('#status')?.textContent ?? null,
    horloge: Date.now(),
  };
})()`;

async function statsToutes(cdp, etiquette) {
    const entrees = appPages();
    const resultats = await Promise.all(entrees.map(async ([sid, p]) => {
        const s = await cdp.evalBorne(sid, STATS);
        return [nomDe(p.url), s];
    }));
    const out = Object.fromEntries(resultats);
    log(`STATS (${etiquette}) ` + JSON.stringify(out));
    return out;
}

/// Deux relevés espacés de `ms`, et la cadence par fenêtre qui s'en déduit.
/// La cadence est CALCULÉE (delta d'images / delta d'horloge de la page).
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
            kbps: Number((((ap.octets - av.octets) * 8) / dt / 1000).toFixed(0)),
            etat: ap.etat,
            focalisee: ap.focalisee,
            bandeau: ap.bandeau,
        };
    }
    log(`CADENCE NAVIGATEUR (${etiquette}) ` + JSON.stringify(out, null, 1));
    return out;
}

// ------------------------------------------------- pilotage de la visibilité

/// Instrument (a) — voir l'en-tête. Pose le focus réel sur `sidCible` et le
/// retire des autres, puis déclenche les événements que le navigateur sans
/// interface n'émet pas toujours de lui-même.
async function focaliser(cdp, sidCible, etiquette) {
    for (const [sid] of appPages()) {
        await cdp.send('Emulation.setFocusEmulationEnabled', { enabled: sid === sidCible }, sid)
            .catch((e) => log('  !! focus emulation', String(e).slice(0, 120)));
    }
    await cdp.send('Page.bringToFront', {}, sidCible).catch(() => { });
    await dodo(300);
    for (const [sid] of appPages()) {
        const cible = sid === sidCible;
        await cdp.evalBorne(sid,
            `(() => { window.dispatchEvent(new Event('${cible ? 'focus' : 'blur'}'));
                      return document.hasFocus(); })()`, 4000, false);
    }
    const etats = await Promise.all(appPages().map(async ([sid, p]) =>
        [nomDe(p.url), await cdp.evalBorne(sid,
            `document.hasFocus() + '/' + (document.hidden ? 'cachée' : 'visible')`, 4000, false)]));
    log(`FOCUS (${etiquette}) cible=${nomDe(pages.get(sidCible).url)} hasFocus/visibilité=`
        + JSON.stringify(Object.fromEntries(etats)));
}

/// Instrument (b) — voir l'en-tête. Pose l'override sur CHAQUE page et
/// annonce « visible » au produit.
///
/// **Pourquoi ici et pas seulement dans l'AMORCE** : une épreuve isolée
/// (`essai-visibilite.mjs`) a montré que `Page.addScriptToEvaluateOnNewDocument`
/// **ne court pas** sur une page ouverte par `window.open` — la course contre la
/// création du document est perdue —, et la page garde alors la visibilité que
/// le navigateur sans interface lui donne, c'est-à-dire `hidden` dès qu'une
/// autre fenêtre passe devant. Poser l'override explicitement, page par page,
/// est le seul moyen sûr. Le `dispatchEvent` qui suit fait resynchroniser le
/// produit sur l'état du SCÉNARIO : dix fenêtres visibles.
async function imposerVisibles(cdp, etiquette) {
    const etats = [];
    for (const [sid, p] of appPages()) {
        const r = await cdp.evalBorne(sid, `(() => {
            if (!Object.getOwnPropertyDescriptor(document, 'hidden')) {
                Object.defineProperty(document, 'hidden', {
                    configurable: true, get: () => window.__cachee });
                Object.defineProperty(document, 'visibilityState', {
                    configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
            }
            window.__cachee = false;
            document.dispatchEvent(new Event('visibilitychange'));
            return document.hidden;
        })()`, 5000, false);
        etats.push([nomDe(p.url), r]);
    }
    log(`VISIBILITÉ IMPOSÉE (${etiquette}) document.hidden=` + JSON.stringify(Object.fromEntries(etats)));
}

/// Instrument (b) — voir l'en-tête. Bascule le drapeau posé par la fonction
/// ci-dessus et
/// déclenche l'événement, comme le ferait une minimisation.
async function masquer(cdp, sid, cachee) {
    const r = await cdp.evalBorne(sid, `(() => {
        window.__cachee = ${cachee};
        document.dispatchEvent(new Event('visibilitychange'));
        return document.hidden;
    })()`, 5000, false);
    log(`MASQUAGE ${nomDe(pages.get(sid).url)} cachee=${cachee} → document.hidden=${JSON.stringify(r)}`);
}

/// Attend que `framesDecoded` d'une page se remette à croître, et rend le
/// délai. C'est le côté navigateur du critère 3 : le côté agent est le
/// `duree_ms` de « fenêtre réveillée ».
/// `sids` : les pages candidates. **Plusieurs, et non une seule** — c'est le
/// vivier qui choisit laquelle réveiller, et le pilote n'a pas à le deviner. Une
/// exécution précédente a guetté w-4 quand le LRU réveillait w-6 (la plus
/// récemment vue des endormies) : le geste avait réussi, la mesure l'avait
/// manqué.
async function attendreImages(cdp, sids, limiteMs = 25000) {
    const lire = async (sid) => {
        const s = await cdp.evalBorne(sid, STATS, 5000);
        return typeof s?.images === 'number' ? s.images : null;
    };
    const depart = new Map();
    for (const sid of sids) depart.set(sid, await lire(sid));
    const t = Date.now();
    for (; Date.now() - t < limiteMs;) {
        await dodo(250);
        for (const sid of sids) {
            const n = await lire(sid);
            const d = depart.get(sid);
            if (n != null && d != null && n > d) {
                const dt = Date.now() - t;
                const nom = nomDe(pages.get(sid).url);
                log(`RÉVEIL NAVIGATEUR ${nom} : première image ${dt} ms après la demande (images ${d} → ${n})`);
                return { nom, ms: dt, depart: d, arrivee: n };
            }
        }
    }
    const noms = sids.map((s) => nomDe(pages.get(s).url));
    log(`!! RÉVEIL NAVIGATEUR : aucune image de plus sur [${noms}] après ${limiteMs} ms`);
    return { nom: noms.join(','), ms: null };
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9992);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d5-'));
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
    log(`N_CIBLE=${N_CIBLE} chrome pid=${chrome.pid} port=${port}`);

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

        // ---- Étape 0 : la VM part sans AUCUNE fenêtre éligible.
        log('>>> ÉTAPE 0 : nettoyage — état de départ SANS fenêtre');
        vmItFichier('preparerd5', 'preparer-d5.ps1');
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

        /// Ouvre la fenêtre `n` et attend qu'une page d'application de plus se
        /// soit ouverte. Attendre le FAIT, jamais une durée.
        const ouvrirFenetre = async (n, attente = 40) => {
            const avant = appPages().length;
            log(`>>> OUVERTURE fenêtre ${n} (attendu : ${avant + 1} pages)`);
            vmIt(`ouvrird5-${n}`, [
                `$a = @(`,
                `  "--app=file:///C:/dev/anim-d4.html?n=${n}",`,
                `  "--user-data-dir=C:\\dev\\chrome-d5-${n}",`,
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
                    return { ouverte: true, secondes: (i + 1) * 2 };
                }
            }
            log(`   !! fenêtre ${n} : AUCUNE page ouverte après ${attente * 2}s (total ${appPages().length})`);
            return { ouverte: false, secondes: attente * 2 };
        };

        // ================================================== CRITÈRE 1 : montée
        const journal = [];
        for (let n = 1; n <= N_CIBLE; n += 1) {
            const r = await ouvrirFenetre(n);
            await dodo(6000);
            const e = await etat(`après fenêtre ${n}`);
            const m = await marqueurs(`après fenêtre ${n}`);
            const c = await cadenceNavigateur(cdp, `rang ${n}`, 5000);
            const diffusent = Object.values(c).filter((x) => x && x.images > 0).length;
            log(`RANG ${n} : pages=${appPages().length} DIFFUSENT=${diffusent} ouverte=${r.ouverte} endormies=${m.endormie - m.reveillee}`);
            vmVivante(`après rang ${n}`);
            journal.push({ rang: n, pages: appPages().length, diffusent, ouverte: r.ouverte, statut: e.statut, marqueurs: m });
            if (!r.ouverte) {
                log(`>>> ARRÊT DE LA MONTÉE au rang ${n} : aucune page de plus.`);
                await fenetresVm(`au rang bloquant ${n}`);
                break;
            }
        }
        log('SYNTHÈSE MONTÉE ' + JSON.stringify(journal.map(({ rang, pages, diffusent, ouverte, statut }) =>
            ({ rang, pages, diffusent, ouverte, statut })), null, 1));

        // La visibilité du SCÉNARIO, posée page par page avant tout geste : le
        // navigateur sans interface déclare cachée toute fenêtre d'arrière-plan.
        await imposerVisibles(cdp, 'après la montée');

        // Palier : de quoi laisser l'adaptation réseau demander des barreaux.
        const palier = await cadenceNavigateur(cdp, 'PALIER (20 s)', 20000);
        await transitions('fin de montée');
        await marqueurs('fin de montée');

        // ============================== CRITÈRE 1 (suite) : ÉVICTION par focus
        // Une endormie = une page dont `framesDecoded` ne bouge pas.
        const dort = (c) => Object.entries(c).filter(([, v]) => v && v.images === 0).map(([k]) => k);
        const veille = (c) => Object.entries(c).filter(([, v]) => v && v.images > 0).map(([k]) => k);
        log(`AVANT ÉVICTION : endormies=${JSON.stringify(dort(palier))} éveillées=${JSON.stringify(veille(palier))}`);

        const sidDe = (nom) => appPages().find(([, p]) => nomDe(p.url) === nom)?.[0];
        const c3 = [];
        const cible1 = dort(palier)[0];
        if (cible1) {
            log(`>>> ÉVICTION : on focalise ${cible1}, qui dort`);
            const tDemande = new Date().toISOString();
            await focaliser(cdp, sidDe(cible1), 'éviction');
            const reveil = await attendreImages(cdp, [sidDe(cible1)]);
            c3.push({ geste: 'focus sur une endormie', demande: tDemande, ...reveil });
            await dodo(4000);
            const apres = await cadenceNavigateur(cdp, 'APRÈS ÉVICTION', 6000);
            log(`APRÈS ÉVICTION : endormies=${JSON.stringify(dort(apres))} éveillées=${JSON.stringify(veille(apres))}`);
            await transitions('après éviction');
        } else {
            log('!! aucune page endormie au palier : le geste d\'éviction est SANS OBJET');
        }

        // ======================= CRITÈRE 1 (fin) : la seconde raison, `masquee`
        // Masquer une éveillée doit l'endormir ET rendre une place, donc
        // réveiller une endormie.
        const c = await cadenceNavigateur(cdp, 'AVANT MASQUAGE', 6000);
        const aMasquer = veille(c).find((n) => n !== cible1);
        const endormieAvant = dort(c).filter((n) => n !== cible1);
        if (aMasquer) {
            log(`>>> MASQUAGE : on cache ${aMasquer} (éveillée) ; attendu : elle dort, et une endormie se réveille`);
            const tDemande = new Date().toISOString();
            await masquer(cdp, sidDe(aMasquer), true);
            if (endormieAvant.length) {
                const reveil = await attendreImages(cdp, endormieAvant.map(sidDe));
                c3.push({ geste: 'masquage d\'une éveillée → réveil d\'une endormie', demande: tDemande, ...reveil });
            }
            await dodo(4000);
            const apres = await cadenceNavigateur(cdp, 'APRÈS MASQUAGE', 6000);
            log(`APRÈS MASQUAGE : endormies=${JSON.stringify(dort(apres))} éveillées=${JSON.stringify(veille(apres))}`);
            await transitions('après masquage');
        } else {
            log('!! aucune page éveillée à masquer');
        }

        // ------------------------------------------------------- bilan
        log('SYNTHÈSE C3 (délais de réveil, côté navigateur) ' + JSON.stringify(c3, null, 1));
        const fin = await marqueurs('FIN');
        await etat('FIN');
        await fenetresVm('fin de recette');
        vmVivante('fin de recette');
        log('SYNTHÈSE C2 ' + JSON.stringify({
            taille_changee: fin.taille_changee,
            taille_refusee: fin.taille_refusee,
            refus_debit: fin.refus_debit,
        }));

        copierLog();
        log('journal copié dans ' + COPIE_LOG);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
