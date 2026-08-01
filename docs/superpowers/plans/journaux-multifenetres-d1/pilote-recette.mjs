// PIÈCE VERSÉE — instrument de la recette D1 du 1er août 2026, tel qu'il a
// tourné pour l'exécution F. Il n'est pas du code produit et n'est pas
// maintenu ; il est versé parce que plusieurs affirmations du rapport de
// résultats ne se lisent nulle part ailleurs :
//
//   - `--disable-popup-blocking` et les trois drapeaux anti-gel de Chrome ;
//   - `Emulation.setDeviceMetricsOverride`, qui impose le viewport pair des
//     exécutions B et F sans toucher au code produit ;
//   - `SANS_NOUVELLE_FENETRE`, qui saute délibérément les points 2 et 6 ;
//   - la commande sonore exacte du point 5 : quatre `Windows Ding.wav` joués
//     par `Media.SoundPlayer.PlaySync()`, et non `[Console]::Beep`, qui donne
//     un faux négatif documenté sur cette VM.
//
// ⚠️ CE FICHIER A ÉTÉ MODIFIÉ ENTRE LES EXÉCUTIONS, et c'est son état FINAL.
// Il est donc l'instrument exact de F seulement. Pour A, il n'avait ni
// l'imposition de viewport, ni les sauts, ni la réorganisation qui remonte le
// point 5 avant le point 3 ; pour B, il avait l'imposition de viewport mais
// pas les deux autres. Les chemins de code que ces variables gouvernent
// disent lesquels étaient actifs. Le point 5 n'a été joué qu'en F.
//
// Les chemins codés en dur pointent vers un répertoire de travail éphémère,
// qui n'existe plus : ce fichier se lit, il ne se rejoue pas tel quel.

#!/usr/bin/env node
// Recette D1 — pilotage du navigateur par CDP brut (même approche que
// client/recette/paire-candidats.mjs), plus déclenchement des actions Windows
// par tâche planifiée /IT (mécanisme de la tâche 6).
//
// Tout est journalisé avec horodatage sur stdout : c'est la pièce.

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = '/home/mallanic/Projects/Guacamole';
const AIDE = '/tmp/user/0/claude-0/-home-mallanic-Projects-Guacamole/b24a1a5a-167f-4286-98f7-e73ff7946fd6/scratchpad/recette';
const CAPTURES = process.env.CAPTURES ?? join(AIDE, 'captures');
const ETIQ = process.env.ETIQ ?? '';
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(6);
    console.log(`[${dt}s] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- VM
function vmIt(nom, ps) {
    const r = spawnSync('bash', [join(AIDE, 'vm-it.sh'), nom, ps], {
        encoding: 'utf8',
        env: process.env,
    });
    if (r.status !== 0) log(`!! vm-it ${nom} a échoué`, r.stderr?.slice(0, 400));
    return r;
}
function winrm(ps) {
    const r = spawnSync('node', [join(RACINE, 'scripts/winrm.js'), ps], {
        encoding: 'utf8',
        env: process.env,
        maxBuffer: 8 << 20,
    });
    return (r.stdout ?? '') + (r.stderr ?? '');
}

/// Liste les fenêtres de la session interactive.
function fenetresVm(etiquette) {
    vmIt('listefen', `
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$f = New-Object System.IO.StreamWriter("C:\\dev\\fenetres.txt", $false, (New-Object System.Text.UTF8Encoding($false)))
Add-Type @"
using System;using System.Runtime.InteropServices;
public class W { [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L,T,R,B; } }
"@
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 } | ForEach-Object {
  $r = New-Object W+RECT
  [void][W]::GetWindowRect($_.MainWindowHandle, [ref]$r)
  $f.WriteLine(("{0}\`t{1}\`t0x{2}\`t{3}x{4}+{5}+{6}\`t{7}" -f $_.Id, $_.ProcessName, $_.MainWindowHandle.ToString("x"), ($r.R-$r.L), ($r.B-$r.T), $r.L, $r.T, $_.MainWindowTitle))
}
$f.Close()
`);
    return new Promise((r) => setTimeout(r, 5000)).then(() => {
        const out = spawnSync('cat', ['/media/vm/dev/fenetres.txt'], { encoding: 'utf8' }).stdout ?? '';
        log(`--- fenêtres VM (${etiquette}) ---\n` + out.trimEnd());
        return out;
    });
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
            if (r.ok) return (await r.json()).webSocketDebuggerUrl;
        } catch { }
        await dodo(250);
    }
    throw new Error('devtools timeout');
}

// Script injecté dans CHAQUE page avant son exécution : capte la
// RTCPeerConnection pour pouvoir lire les stats, et rien d'autre.
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

async function main() {
    await mkdir(CAPTURES, { recursive: true });
    const port = Number(process.env.PORT_CDP ?? 9971);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d1-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new',
        `--remote-debugging-port=${port}`,
        '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`,
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--disable-gpu',
        '--window-size=1280,800',
        '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns',
        // LE point de ce sous-bloc : la shell ouvre une fenêtre par fenêtre
        // Windows, depuis un gestionnaire de message WebSocket — donc hors
        // geste utilisateur. Sans ceci, le bloqueur de pop-ups les refuse
        // toutes et rien ne s'ouvre.
        '--disable-popup-blocking',
        // Anti-gel des pages d'arrière-plan (recette du 30/07/2026).
        '--disable-background-timer-throttling',
        '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding',
        'about:blank',
    ], { stdio: 'ignore' });

    let superviseur = null;
    try {
        const wsUrl = await attendreDevtools(port);
        const cdp = new Cdp(wsUrl);

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
                if (process.env.VIEWPORT_FORCE) {
                    const [L, H] = process.env.VIEWPORT_FORCE.split('x').map(Number);
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: L, height: H, deviceScaleFactor: 1, mobile: false }, sessionId).catch((e) => log('  !! override viewport impossible', String(e).slice(0, 150)));
                }
                await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
            } else if (m.method === 'Target.detachedFromTarget') {
                const p = pages.get(m.params.sessionId);
                if (p) log(`- page détachée   session=${m.params.sessionId.slice(0, 8)} url=${p.url}`);
                pages.delete(m.params.sessionId);
            } else if (m.method === 'Target.targetInfoChanged') {
                for (const [sid, p] of pages) {
                    if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
                }
            }
        });

        await cdp.send('Target.setAutoAttach', {
            autoAttach: true, waitForDebuggerOnStart: true, flatten: true,
        });
        await cdp.send('Target.setDiscoverTargets', { discover: true });

        // ---- Ouvrir la shell
        const { targetId } = await cdp.send('Target.createTarget', { url: URL_SHELL });
        log('shell demandée', URL_SHELL, targetId);
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut').textContent`));

        // ---- Lancer le superviseur (APRÈS que la shell soit connectée : le
        //      signaling ne mémorise que les offres, pas les annonces)
        log('>>> lancement du superviseur');
        superviseur = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh stdout :', (superviseur.stdout ?? '').trim().replace(/\n/g, ' / '));
        if ((superviseur.stderr ?? '').trim()) log('run-agent.sh STDERR :', superviseur.stderr.trim().replace(/\n/g, ' / '));
        // Copie périodique du journal de l'agent : si la VM disparaît, on
        // garde ce qui a été écrit.
        setInterval(() => { spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${AIDE}/agent-encours.log 2>/dev/null`]); }, 5000).unref?.();

        const etat = async (etiquette) => {
            const l = await cdp.eval(sidShell,
                `Array.from(document.querySelectorAll('#fenetres li')).map(e=>e.textContent).join(' | ')`);
            const s = await cdp.eval(sidShell, `document.querySelector('#statut').textContent`);
            log(`ÉTAT SHELL (${etiquette}) liste=[${l}] statut="${s}"`);
            log(`  pages ouvertes : ${[...pages.values()].map((p) => p.url).join(' , ')}`);
            return l;
        };

        // ---- Point 1 : les fenêtres déjà ouvertes
        for (let i = 0; i < 24; i += 1) {
            await dodo(2500);
            const n = [...pages.values()].filter((p) => !p.url.includes('shell.html')).length;
            if (i % 4 === 0) await etat(`t+${(i + 1) * 2.5}s`);
            if (n >= 2) break;
        }
        await dodo(20000);   // laisser les sessions WebRTC s'établir
        await etat('POINT 1 — fenêtres préexistantes');
        await statsToutes(cdp, 'POINT 1');
        if (!process.env.SANS_CAPTURE_P1) await capturer(cdp, 'point1');

        // ---- Point 2 : ouvrir le Bloc-notes
        if (process.env.SANS_NOUVELLE_FENETRE) {
            log('>>> POINT 2 SAUTÉ volontairement (SANS_NOUVELLE_FENETRE) : ce passage vise les points 3/4/5/7/8 sur un arrangement STABLE');
        } else {
        log('>>> POINT 2 : ouverture du Bloc-notes sur la VM');
        vmIt('opennotepad', `Start-Process notepad; Start-Sleep -Seconds 2`);
        for (let i = 0; i < 16; i += 1) { await dodo(2500); if (i % 4 === 3) await etat(`p2 t+${(i + 1) * 2.5}s`); }
        await etat('POINT 2 — après ouverture du Bloc-notes');
        await fenetresVm('après ouverture Bloc-notes');
        await statsToutes(cdp, 'POINT 2');
        await capturer(cdp, 'point2');
        }

        // ---- Point 5 : son
        log('>>> POINT 5 : lecture d’un son sur la VM');
        const avant = await statsToutes(cdp, 'POINT 5 avant son');
        vmIt('sonner', `1..4 | ForEach-Object { (New-Object Media.SoundPlayer 'C:\\Windows\\Media\\Windows Ding.wav').PlaySync(); Start-Sleep -Milliseconds 300 }`);
        await dodo(9000);
        const apres = await statsToutes(cdp, 'POINT 5 après son');
        log('DELTA AUDIO :', JSON.stringify(deltaAudio(avant, apres)));

        // ---- Point 3 : frappe clavier ciblée
        log('>>> POINT 3 : frappe clavier dans la fenêtre du Bloc-notes');
        const cible = [...pages].find(([, p]) => !p.url.includes('shell.html'));
        // On tape dans CHAQUE page d'application un texte distinct, pour voir
        // où chacun atterrit.
        let k = 0;
        for (const [sid, p] of pages) {
            if (p.url.includes('shell.html')) continue;
            k += 1;
            const texte = `PAGE${k}`;
            log(`   frappe "${texte}" dans ${p.url}`);
            await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: 400, y: 300, button: 'left', clickCount: 1 }, sid).catch(() => { });
            await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 400, y: 300, button: 'left', clickCount: 1 }, sid).catch(() => { });
            await dodo(500);
            for (const c of texte) {
                await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', text: c, key: c, code: `Key${c}`, windowsVirtualKeyCode: c.charCodeAt(0) }, sid).catch(() => { });
                await dodo(120);
                await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: c, code: `Key${c}`, windowsVirtualKeyCode: c.charCodeAt(0) }, sid).catch(() => { });
                await dodo(120);
            }
            await dodo(1500);
        }
        await dodo(3000);
        await fenetresVm('après frappe');
        await capturer(cdp, 'point3');

        // ---- Point 6 : fermer le Bloc-notes
        if (process.env.SANS_NOUVELLE_FENETRE) {
            log('>>> POINT 6 SAUTÉ volontairement (SANS_NOUVELLE_FENETRE)');
        } else {
        log('>>> POINT 6 : fermeture du Bloc-notes sur la VM');
        vmIt('killnotepad', `Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force`);
        await dodo(15000);
        await etat('POINT 6 — après fermeture du Bloc-notes');
        await fenetresVm('après fermeture Bloc-notes');
        }

        // ---- Point 7 : fermer une page navigateur
        const restante = [...pages].find(([, p]) => !p.url.includes('shell.html'));
        if (restante) {
            log('>>> POINT 7 : fermeture de la PAGE', restante[1].url);
            await cdp.send('Target.closeTarget', { targetId: restante[1].targetId });
            await dodo(4000);
            await etat('POINT 7 — après fermeture de la page');
            await fenetresVm('après fermeture de la page navigateur');
            log('   clic sur « Rouvrir »');
            await cdp.eval(sidShell, `document.querySelector('#fenetres button')?.click(); 'clic'`);
            await dodo(8000);
            await etat('POINT 7 — après Rouvrir');
            await statsToutes(cdp, 'POINT 7 après Rouvrir');
        } else {
            log('!! POINT 7 non exercé : aucune page d’application ouverte');
        }

        // ---- Point 8 : déplacer une fenêtre Windows hors de sa sortie
        log('>>> POINT 8 : déplacement d’une fenêtre Windows hors de sa sortie');
        vmIt('bouger', `
Add-Type @"
using System;using System.Runtime.InteropServices;
public class M { [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr a,int x,int y,int cx,int cy,uint f); }
"@
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 -and $_.ProcessName -ne 'explorer' } | ForEach-Object {
  [void][M]::SetWindowPos($_.MainWindowHandle,[IntPtr]::Zero,10,10,700,500,0x0010)
}
`);
        await dodo(12000);
        await fenetresVm('après déplacement forcé');
        await capturer(cdp, 'point8');
        await statsToutes(cdp, 'POINT 8');

        await etat('FIN');
        // Console des pages, utile en diagnostic
        for (const [sid, p] of pages) {
            const c = await cdp.eval(sid, `(window.__console||[]).slice(-15).join('\\n')`).catch(() => '');
            if (c) log(`--- console ${p.url} ---\n${c}`);
        }
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
    }
}

async function statsToutes(cdp, etiquette) {
    const out = {};
    for (const [sid, p] of pages) {
        if (p.url.includes('shell.html')) continue;
        const s = await cdp.eval(sid, `(async () => {
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
              audio: a ? { energie: a.totalAudioEnergy, octets: a.bytesReceived, samples: a.totalSamplesReceived } : null,
              rtt: paire?.currentRoundTripTime,
              viewport: window.innerWidth + 'x' + window.innerHeight,
            };
        })()`, true).catch((e) => ({ erreur: String(e).slice(0, 200) }));
        out[p.url] = s;
    }
    log(`STATS (${etiquette}) ` + JSON.stringify(out, null, 1));
    return out;
}

function deltaAudio(a, b) {
    const d = {};
    for (const k of Object.keys(b)) {
        const av = a[k]?.audio, ap = b[k]?.audio;
        d[k] = {
            energieAvant: av?.energie ?? null, energieApres: ap?.energie ?? null,
            delta: av && ap ? ap.energie - av.energie : null,
            octetsDelta: av && ap ? ap.octets - av.octets : null,
            pisteAudio: ap ? 'oui' : 'non',
        };
    }
    return d;
}

async function capturer(cdp, nom) {
    let i = 0;
    for (const [sid, p] of pages) {
        i += 1;
        try {
            const r = await cdp.send('Page.captureScreenshot', { format: 'png' }, sid);
            const f = join(CAPTURES, `${ETIQ}${nom}-${i}-${p.url.includes('shell') ? 'shell' : 'app'}.png`);
            await writeFile(f, Buffer.from(r.data, 'base64'));
            log(`   capture ${f}  (${p.url})`);
        } catch (e) { log(`   capture impossible pour ${p.url}: ${String(e).slice(0, 120)}`); }
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
