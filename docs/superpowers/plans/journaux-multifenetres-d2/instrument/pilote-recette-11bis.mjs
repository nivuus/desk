#!/usr/bin/env node
// Recette D2 — tâche 11 BIS : exécution de vérification du réessai
// d'ouverture. Repris tel quel de la tâche 11, à deux compteurs près
// (`ouverture_reessayee`, `ouverture_abandonnee`) : c'est la comparabilité
// avec le passage D qui fait le relevé.
//
// Le critère : partant de k fenêtres capturées, ouvrir une application de plus
// ne doit tuer aucune session préexistante, jusqu'à cinq fenêtres simultanées.
//
// Pilotage du navigateur par CDP brut (même approche que le pilote de D1,
// `docs/superpowers/plans/journaux-multifenetres-d1/pilote-recette.mjs`), et
// déclenchement des actions Windows par tâche planifiée /IT (`vm-it.sh`) — la
// session interactive est la seule où une fenêtre soit visible du superviseur.
//
// Contraintes de protocole héritées des pièges de D1, chacune ayant coûté une
// exécution entière là-bas :
//   1. le navigateur est lancé AVANT le superviseur (le signaling ne mémorise
//      que les offres SDP ; une annonce `fenetre-ouverte` émise avant que la
//      page-shell soit connectée est perdue sans trace) ;
//   2. `--disable-popup-blocking`, sans quoi la shell n'ouvre rien ;
//   3. AUCUNE capture d'écran CDP pendant la séquence — en D1 elle provoquait
//      un `Resize`, donc un `SHOW`, donc une session, donc une sortie. Les
//      captures sont prises à la toute fin, après le relevé du critère ;
//   4. l'instance de navigateur pilotée est vérifiée (un port de débogage qui
//      répond ne prouve pas que c'est le bon navigateur) ;
//   5. les trois drapeaux anti-gel des pages d'arrière-plan.

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, writeFile, mkdir, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

function racineDepot() {
    const r = spawnSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' });
    if (r.status !== 0) {
        throw new Error(`hors du depot git : impossible de deriver RACINE (git rev-parse a echoue : ${(r.stderr ?? '').trim()})`);
    }
    return r.stdout.trim();
}
const RACINE = racineDepot();
const AIDE = process.env.AIDE ?? '/tmp/user/0/claude-0/-home-mallanic-Projects-Guacamole/ecebb0d5-b8a9-404c-a078-652572f86f2e/scratchpad/recette';
const CAPTURES = process.env.CAPTURES ?? join(AIDE, 'captures');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const COPIE_LOG = join(AIDE, 'agent-encours.log');
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(7);
    console.log(`[${dt}s] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- VM
function vmIt(nom, fichierPs) {
    const ps = spawnSync('cat', [join(AIDE, fichierPs)], { encoding: 'utf8' }).stdout ?? '';
    const r = spawnSync('bash', [join(AIDE, 'vm-it.sh'), nom, ps], { encoding: 'utf8', env: process.env });
    if (r.status !== 0) log(`!! vm-it ${nom} a échoué`, (r.stderr ?? '').slice(0, 400));
    return r;
}

async function fenetresVm(etiquette) {
    vmIt('listefen', 'listefen.ps1');
    await dodo(6000);
    const out = spawnSync('cat', ['/media/vm/dev/fenetres.txt'], { encoding: 'utf8' }).stdout ?? '';
    log(`--- fenêtres VM (${etiquette}) ---\n` + out.trimEnd());
    return out;
}

/// Compte, dans la copie courante du journal de l'agent, les lignes qui font
/// le verdict. C'est un relevé, pas une interprétation : les lignes elles-mêmes
/// sont versées avec le journal.
async function marqueurs(etiquette) {
    spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
    let texte = '';
    try { texte = await readFile(COPIE_LOG, 'utf8'); } catch { }
    const plat = texte.replace(/\x1b\[[0-9;]*m/g, '');
    const compte = (motif) => (plat.match(new RegExp(motif, 'g')) ?? []).length;
    const m = {
        lignes: plat.split('\n').length,
        cloture: compte('clôture de session amorcée'),
        reouverture: compte('accès à la duplication perdu, réouverture'),
        // Le journal écrit le HRESULT en minuscules : `0x887a0026`.
        mutex_abandonne: compte('0x887a0026'),
        duplication_refusee: compte('0x887A0022'),
        // Tâche 11 bis : les deux lignes du réessai d'ouverture. La première
        // est la preuve que le réessai TRAVAILLE ; la seconde qu'il a fini par
        // renoncer, et son `attendu_ms` dit après combien de temps.
        ouverture_reessayee: compte("duplication indisponible à l'ouverture, nouvel essai"),
        ouverture_abandonnee: compte('ouverture de la duplication abandonnée'),
        sortie_creee: compte('sortie virtuelle créée'),
        sortie_rendue: compte('sortie virtuelle rendue au pilote'),
        enfant_lance: compte('enfant lancé'),
        enfant_termine: compte('enfant terminé'),
        enfant_mort_seul: compte('enfant mort de lui-même'),
        premier_plan_obtenu: compte('premier plan obtenu'),
        premier_plan_refuse: compte('SetForegroundWindow refusé'),
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
  window.__console = [];
  for (const n of ['log','warn','error']) {
    const o = console[n];
    console[n] = (...x) => { window.__console.push(n + ': ' + x.join(' ')); o(...x); };
  }
`;

const pages = new Map(); // sessionId -> {targetId, url}
/// Les pages d'APPLICATION : ni la page-shell, ni l'`about:blank` que Chrome
/// ouvre au démarrage. (Le premier passage de cette recette comptait
/// `about:blank` parmi elles, ce qui décalait tous les comptes de un.)
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));

async function main() {
    await mkdir(CAPTURES, { recursive: true });
    const port = Number(process.env.PORT_CDP ?? 9982);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d2-'));
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
    log(`chrome lancé pid=${chrome.pid} port=${port} profil=${userDataDir}`);

    try {
        const version = await attendreDevtools(port);
        // Piège 4 : vérifier que le processus qui ÉCOUTE sur ce port est bien
        // celui qu'on vient de lancer. Un Chrome sans interface survit à la
        // mort de son pilote, et un port qui répond ne prouve rien.
        const qui = spawnSync('bash', ['-c',
            `ss -ltnp 2>/dev/null | grep ':${port} ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1`],
            { encoding: 'utf8' }).stdout.trim();
        log(`identité du navigateur : pid écoutant=${qui} pid lancé=${chrome.pid} version=${version.Browser} profil déclaré=${version['User-Agent'] ? 'n/a' : ''}`);
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
                // Le viewport annoncé par la page décide de la résolution de
                // la sortie virtuelle demandée au pilote. Sans imposition, un
                // pop-up de ce Chrome annonce 1280×632, et le pilote rend une
                // sortie 1280×720 — mode qu'il sait faire —, écart de 88 px
                // très au-delà de la tolérance d'appariement de 4 px : la
                // fenêtre ne s'ouvre jamais. On impose donc une taille que le
                // pilote rend à l'identique. (Même mécanisme que le
                // `VIEWPORT_FORCE` du pilote de D1.)
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

        // ---- La page-shell, AVANT le superviseur
        await cdp.send('Target.createTarget', { url: URL_SHELL });
        log('shell demandée', URL_SHELL);
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut').textContent`));

        const etat = async (etiquette) => {
            const l = await cdp.eval(sidShell,
                `Array.from(document.querySelectorAll('#fenetres li')).map(e=>e.textContent).join(' | ')`);
            const s = await cdp.eval(sidShell, `document.querySelector('#statut').textContent`);
            log(`ÉTAT SHELL (${etiquette}) liste=[${l}] statut="${s}"`);
            log(`  pages d'application ouvertes (${appPages().length}) : ${appPages().map(([, p]) => p.url).join(' , ')}`);
            return l;
        };

        const stats = async (etiquette) => {
            const out = {};
            for (const [sid, p] of appPages()) {
                out[p.url] = await cdp.eval(sid, `(async () => {
                    const pc = window.__pc;
                    if (!pc) return { pc: null };
                    const r = await pc.getStats(); const t = [...r.values()];
                    const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
                    const a = t.find(x => x.type === 'inbound-rtp' && x.kind === 'audio');
                    const paire = t.find(x => x.type === 'candidate-pair' && (x.selected || x.nominated));
                    return {
                      etat: pc.iceConnectionState,
                      images: v?.framesDecoded, l: v?.frameWidth, h: v?.frameHeight,
                      octetsV: v?.bytesReceived,
                      octetsA: a?.bytesReceived ?? null,
                      rtt: paire?.currentRoundTripTime,
                      viewport: window.innerWidth + 'x' + window.innerHeight,
                    };
                })()`, true).catch((e) => ({ erreur: String(e).slice(0, 200) }));
            }
            log(`STATS (${etiquette}) ` + JSON.stringify(out, null, 1));
            return out;
        };

        // ---- Le superviseur, APRÈS la shell
        log('>>> lancement du superviseur (SUPERVISEUR=1)');
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        if ((sup.stderr ?? '').trim()) log('run-agent.sh STDERR :', sup.stderr.trim().replace(/\n/g, ' / '));
        const copie = setInterval(() => {
            spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
        }, 5000);
        copie.unref?.();

        // ---- k = 1 : la fenêtre préexistante
        for (let i = 0; i < 20; i += 1) {
            await dodo(2000);
            if (appPages().length >= 1) break;
        }
        await dodo(20000);
        await etat('k=1 — fenêtre préexistante');
        await stats('k=1');
        await marqueurs('k=1');

        // ---- Montée : une application de plus à la fois
        //
        // Cinq Bloc-notes sur cinq fichiers distincts, et non cinq
        // applications différentes : le premier passage a montré que Paint
        // ouvre DEUX fenêtres éligibles (« Paint » et « UIRibbonWorkPane »),
        // ce qui rend le compte « une application ouverte = une fenêtre de
        // plus » faux et brouille la lecture du critère.
        const releves = [];
        const etape = async (nom, fichier, quoi) => {
            const n = appPages().length;
            log(`>>> ÉTAPE : ${quoi}  (pages d'application avant = ${n})`);
            const avant = await marqueurs(`avant ${quoi}`);
            vmIt(nom, fichier);
            for (let i = 0; i < 7; i += 1) {
                await dodo(5000);
                log(`   … t+${(i + 1) * 5}s  pages=${appPages().length}`);
            }
            const apres = await marqueurs(`après ${quoi}`);
            await etat(`après ${quoi}`);
            await stats(`après ${quoi}`);
            releves.push({ quoi, pagesAvant: n, pagesApres: appPages().length, avant, apres });
            log(`RELEVÉ ${quoi} : pages ${n} -> ${appPages().length}, ` +
                `clôtures ${avant.cloture} -> ${apres.cloture}, ` +
                `réouvertures ${avant.reouverture} -> ${apres.reouverture}, ` +
                `duplication refusée (0x887A0022) ${avant.duplication_refusee} -> ${apres.duplication_refusee}, ` +
                `ouvertures réessayées ${avant.ouverture_reessayee} -> ${apres.ouverture_reessayee}, ` +
                `ouvertures abandonnées ${avant.ouverture_abandonnee} -> ${apres.ouverture_abandonnee}`);
            return apres;
        };

        for (const [nom, fichier, quoi] of [
            ['ouvrirn2', 'ouvrir-n2.ps1', 'Bloc-notes 2'],
            ['ouvrirn3', 'ouvrir-n3.ps1', 'Bloc-notes 3'],
            ['ouvrirn4', 'ouvrir-n4.ps1', 'Bloc-notes 4'],
            ['ouvrirn5', 'ouvrir-n5.ps1', 'Bloc-notes 5'],
        ]) {
            await etape(nom, fichier, quoi);
        }

        // ---- Épreuve du plafond : une fermeture RÉELLE, puis une ouverture.
        //
        // Le premier passage a buté sur un refus de duplication
        // (`0x887A0022`, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE) au moment
        // d'ouvrir une cinquième fenêtre. Deux explications tiennent : un
        // plafond sur le NOMBRE de duplications simultanées, ou une
        // indisponibilité TRANSITOIRE pendant la refonte de la topologie.
        // Fermer une fenêtre puis en ouvrir une autre départage : si la place
        // libérée suffit, c'est un plafond.
        //
        // Cette fermeture est aussi la seule `clôture de session` SOLLICITÉE
        // de la séquence — celle qui correspond à une fenêtre réellement
        // fermée, et que le critère autorise explicitement.
        await etape('fermer2', 'fermer-2.ps1', 'FERMETURE réelle du Bloc-notes 2');
        await etape('ouvrirn6', 'ouvrir-n6.ps1', 'Bloc-notes 6 (après la fermeture)');

        log('>>> ÉTAT FINAL — le critère se lit ici');
        await etat('FIN');
        const statsFin = await stats('FIN');
        await marqueurs('FIN');
        await fenetresVm('FIN');
        log('SYNTHÈSE ' + JSON.stringify(releves.map((r) => ({
            quoi: r.quoi, pages: `${r.pagesAvant}->${r.pagesApres}`,
            cloture: `${r.avant.cloture}->${r.apres.cloture}`,
            reouverture: `${r.avant.reouverture}->${r.apres.reouverture}`,
            refus_duplication: `${r.avant.duplication_refusee}->${r.apres.duplication_refusee}`,
            ouverture_reessayee: `${r.avant.ouverture_reessayee}->${r.apres.ouverture_reessayee}`,
            ouverture_abandonnee: `${r.avant.ouverture_abandonnee}->${r.apres.ouverture_abandonnee}`,
        })), null, 1));

        // ---- Après le relevé du critère seulement : clavier, puis captures.
        if (!process.env.SANS_CLAVIER) {
            log('>>> CLAVIER (hors critère, après relevé) : frappe dans chaque page');
            let k = 0;
            for (const [sid, p] of appPages()) {
                k += 1;
                const texte = `PAGE${k}`;
                log(`   frappe "${texte}" dans ${p.url}`);
                await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: 400, y: 300, button: 'left', clickCount: 1 }, sid).catch(() => { });
                await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 400, y: 300, button: 'left', clickCount: 1 }, sid).catch(() => { });
                await dodo(600);
                for (const c of texte) {
                    await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', text: c, key: c, code: `Key${c}`, windowsVirtualKeyCode: c.charCodeAt(0) }, sid).catch(() => { });
                    await dodo(120);
                    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: c, code: `Key${c}`, windowsVirtualKeyCode: c.charCodeAt(0) }, sid).catch(() => { });
                    await dodo(120);
                }
                await dodo(1200);
            }
            await dodo(3000);
            await marqueurs('après clavier');
            vmIt('relire', 'relire.ps1');
            await dodo(7000);
            const relu = spawnSync('cat', ['/media/vm/dev/relecture.txt'], { encoding: 'utf8' }).stdout ?? '';
            log('--- relecture WM_GETTEXT des Bloc-notes ---\n' + relu.trimEnd());
        }

        log('>>> CAPTURES (fin de séquence seulement — piège 3)');
        let i = 0;
        for (const [sid, p] of pages) {
            i += 1;
            try {
                // Borné : au premier passage, `Page.captureScreenshot` sur une
                // page portant un flux WebRTC actif n'a JAMAIS rendu, et le
                // pilote y est resté suspendu sans fin (aucune capture versée).
                const r = await Promise.race([
                    cdp.send('Page.captureScreenshot', { format: 'png' }, sid),
                    dodo(20000).then(() => { throw new Error('capture non rendue en 20 s'); }),
                ]);
                const f = join(CAPTURES, `fin-${i}-${p.url.includes('shell') ? 'shell' : 'app'}.png`);
                await writeFile(f, Buffer.from(r.data, 'base64'));
                log(`   capture ${f}  (${p.url})`);
            } catch (e) { log(`   capture impossible pour ${p.url}: ${String(e).slice(0, 150)}`); }
        }
        await etat('APRÈS CAPTURES (hors critère)');
        await marqueurs('APRÈS CAPTURES (hors critère)');
        for (const [sid, p] of pages) {
            const c = await cdp.eval(sid, `(window.__console||[]).slice(-12).join('\\n')`).catch(() => '');
            if (c) log(`--- console ${p.url} ---\n${c}`);
        }
        spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
