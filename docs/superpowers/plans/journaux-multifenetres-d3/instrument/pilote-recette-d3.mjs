#!/usr/bin/env node
// Recette D3 — tâche 11 : le critère 1 de réception en conditions de produit.
//
// Le critère : sur une séquence où une fenêtre est condamnée pendant que
// d'autres capturent, ZÉRO réouverture de duplication imputable à une
// RELANCE d'enfant (celles de l'ouverture initiale de la fenêtre condamnée et
// de son abandon final ne comptent pas), et AUCUNE session saine perdue.
//
// Dérivé de `journaux-multifenetres-d2/instrument/pilote-recette-d2.mjs`
// (même squelette CDP/vm-it), mais la séquence change de nature : D2 montait
// en NOMBRE de fenêtres (k -> k+1) ; D3 tue à répétition l'ENFANT d'UNE SEULE
// fenêtre déjà ouverte, pendant que trois autres capturent sans interruption.
// C'est le cas que les tâches 1-7 du sous-bloc D3 ont corrigé : la sortie
// virtuelle d'une fenêtre condamnée est désormais RETENUE d'une relance à
// l'autre au lieu d'être détruite puis recréée — et c'est la création qui
// abandonne le mutex des duplications DXGI voisines.
//
// Contraintes de protocole héritées de D1/D2, chacune ayant coûté une
// exécution entière là-bas :
//   1. le navigateur est lancé AVANT le superviseur ;
//   2. `--disable-popup-blocking` ;
//   3. AUCUNE capture d'écran CDP pendant la séquence (elle provoque un
//      `Resize`, donc un `SHOW`, donc une session et une sortie de plus) ;
//   4. l'instance de navigateur pilotée est vérifiée par PID écoutant ;
//   5. les trois drapeaux anti-gel des pages d'arrière-plan ;
//   6. une fenêtre PRÉEXISTANTE dont la page-shell tarde plus de 30 s est
//      abandonnée (DELAI_ATTENTE_VIEWPORT_MAX, tâche 6) — donc la VM part
//      d'un état SANS fenêtre Bloc-notes avant le superviseur, et les trois
//      fenêtres saines sont ouvertes APRÈS lui, une par une (le cas produit,
//      pas l'énumération initiale).

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = '/home/mallanic/Projects/Guacamole';
const AIDE = process.env.AIDE ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d3/instrument');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const COPIE_LOG = process.env.COPIE_LOG ?? '/tmp/agent-encours-d3.log';
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

/// Commande WinRM directe (session 0) : suffisant pour lister/tuer des
/// processus, contrairement à la capture ou l'injection d'entrée qui exigent
/// la session interactive (`vm-it.sh`).
function winrm(commande) {
    const r = spawnSync('node', [join(RACINE, 'scripts/winrm.js'), commande],
        { encoding: 'utf8', env: process.env, maxBuffer: 16 * 1024 * 1024 });
    if (r.status !== 0) log('!! winrm a échoué', (r.stderr ?? '').slice(0, 400));
    return r.stdout ?? '';
}

async function fenetresVm(etiquette) {
    vmIt('listefen', 'listefen.ps1');
    await dodo(6000);
    const out = spawnSync('cat', ['/media/vm/dev/fenetres.txt'], { encoding: 'utf8' }).stdout ?? '';
    log(`--- fenêtres VM (${etiquette}) ---\n` + out.trimEnd());
    return out;
}

/// Compte, dans la copie courante du journal de l'agent, les lignes qui font
/// le verdict. Relevé, pas interprétation : les lignes elles-mêmes sont
/// versées avec le journal.
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
        mutex_abandonne: compte('0x887a0026'),
        sortie_creee: compte('sortie virtuelle créée'),
        duplication_etablie: compte('duplication de sortie établie'),
        sortie_rendue: compte('sortie virtuelle rendue au pilote'),
        creation_refusee: compte('création de sortie refusée'),
        enfant_lance: compte('enfant lancé'),
        enfant_termine: compte('enfant terminé'),
        enfant_mort_seul: compte('enfant mort de lui-même'),
        abandon_relances: compte("la session n'a pas tenu après"),
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
`;

const pages = new Map(); // sessionId -> {targetId, url}
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));

async function main() {
    const port = Number(process.env.PORT_CDP ?? 9983);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d3-'));
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

        // ---- Étape 0 : la VM part sans AUCUNE fenêtre Bloc-notes/Paint/Wordpad.
        log('>>> ÉTAPE 0 : nettoyage — état de départ SANS fenêtre');
        vmIt('preparer', 'preparer-vide.ps1');
        await dodo(4000);
        await fenetresVm('avant superviseur');

        // ---- La page-shell, AVANT le superviseur (piège 1)
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

        // ---- Le superviseur, APRÈS la shell (piège 1)
        log('>>> lancement du superviseur (SUPERVISEUR=1)');
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        if ((sup.stderr ?? '').trim()) log('run-agent.sh STDERR :', sup.stderr.trim().replace(/\n/g, ' / '));
        await dodo(5000);
        await marqueurs('superviseur démarré, 0 fenêtre');

        // ---- Les TROIS sessions saines, une par une, en attendant chaque
        // fois la page navigateur avant la suivante.
        const attendrePages = async (n, quoi) => {
            for (let i = 0; i < 15; i += 1) {
                await dodo(2000);
                if (appPages().length >= n) { log(`   ${quoi} : ${n} page(s) d'application atteinte(s) en ${(i + 1) * 2}s`); return true; }
            }
            log(`   !! ${quoi} : ${n} page(s) NON atteinte(s) après 30s (pages=${appPages().length})`);
            return false;
        };

        log('>>> SESSION SAINE 1/3 : ouverture Bloc-notes (fenetre-1)');
        vmIt('ouvrir1', 'ouvrir-1.ps1');
        await attendrePages(1, 'saine 1');
        await dodo(3000);
        await etat('après saine 1');
        await marqueurs('après saine 1');

        log('>>> SESSION SAINE 2/3 : ouverture Bloc-notes (fenetre-2)');
        vmIt('ouvrir2', 'ouvrir-2.ps1');
        await attendrePages(2, 'saine 2');
        await dodo(3000);
        await etat('après saine 2');
        await marqueurs('après saine 2');

        log('>>> SESSION SAINE 3/3 : ouverture Bloc-notes (fenetre-3)');
        vmIt('ouvrir3', 'ouvrir-3-notepad.ps1');
        await attendrePages(3, 'saine 3');
        await dodo(3000);
        await etat('après saine 3');
        const avantCondamnation = await marqueurs('AVANT condamnation — trois sessions saines établies');

        // ---- Liste PROTÉGÉE : le superviseur lui-même + les trois enfants
        // sains. Tout PID `agent.exe` qui apparaît hors de cet ensemble, une
        // fois la fenêtre 4 ouverte, est l'enfant de la fenêtre condamnée.
        const brutProteges = winrm(
            "Get-Process agent -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id | Sort-Object");
        const proteges = [...brutProteges.matchAll(/\d+/g)].map((m) => Number(m[0]));
        log(`PIDs protégés (superviseur + 3 enfants sains) : [${proteges.join(',')}]`);
        if (proteges.length !== 4) {
            log(`!! ATTENTION : ${proteges.length} PID(s) agent trouvé(s), 4 attendus. La suite reste jouée mais ce compte est à vérifier au relevé.`);
        }

        // ---- CONDAMNATION : ouvrir une quatrième fenêtre, puis tuer son
        // enfant à répétition, par PID relevé côté Windows, jusqu'à l'abandon
        // après RELANCES_MAX (= 3, donc 4 lancements légitimes : l'original
        // + 3 relances).
        //
        // Le script est écrit dans un FICHIER sur le partage (comme le fait
        // déjà `scripts/run-agent.sh` pour `run-agent.ps1`) et invoqué par
        // `-File`, plutôt que passé inline à `winrm.js` : `nodejs-winrm`
        // enveloppe TOUJOURS la commande dans
        // `powershell -Command "& { <commande> }"` (`usePowershell=true`
        // fixe dans `scripts/winrm.js`), et les guillemets doubles de ce
        // script (`AppendLine("...")`) entraient en collision avec ceux de
        // cette enveloppe — échec de PREMIÈRE exécution, corrigé ici avant
        // tout relevé de critère (aucun tueur n'a réellement tourné).
        //
        // Le tueur est lancé AVANT le déclenchement de l'ouverture (et non
        // après) : il ne fait rien tant qu'aucun PID hors de la liste protégée
        // n'apparaît, donc démarrer tôt ne coûte rien et évite de manquer le
        // tout premier enfant par une course de lancement.
        const scriptTueur = `
$proteges = @(${proteges.join(',')})
$tues = New-Object System.Collections.Generic.List[int]
$sw = [Diagnostics.Stopwatch]::StartNew()
$log = New-Object System.Text.StringBuilder
while ($sw.Elapsed.TotalSeconds -lt 55) {
  $procs = Get-Process agent -ErrorAction SilentlyContinue
  foreach ($p in $procs) {
    if (($proteges -notcontains $p.Id) -and ($tues -notcontains $p.Id)) {
      $ts = (Get-Date).ToString('HH:mm:ss.fff')
      try {
        Stop-Process -Id $p.Id -Force -ErrorAction Stop
        $tues.Add($p.Id) | Out-Null
        [void]$log.AppendLine("$ts TUE pid=$($p.Id)")
      } catch {
        [void]$log.AppendLine("$ts ECHEC_TUE pid=$($p.Id) err=$($_.Exception.Message)")
      }
    }
  }
  Start-Sleep -Milliseconds 150
}
[void]$log.AppendLine("FIN nb_tues=$($tues.Count) tues=$($tues -join ',')")
$texte = $log.ToString()
$texte | Out-File -FilePath C:\\dev\\tueur-fenetre4.log -Encoding utf8
Write-Output $texte
`.trim();
        await writeFile('/media/vm/dev/tueur-fenetre4.ps1', scriptTueur, 'utf8');
        log('script du tueur écrit sur le partage : /media/vm/dev/tueur-fenetre4.ps1');

        log('>>> CONDAMNATION : lancement du tueur (fenêtre 4 pas encore ouverte)');
        const tueurPromise = new Promise((resolve) => {
            const p = spawn('node', [join(RACINE, 'scripts/winrm.js'),
                'powershell -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\tueur-fenetre4.ps1'],
                { env: process.env });
            let out = '';
            p.stdout.on('data', (d) => { out += d.toString(); });
            p.stderr.on('data', (d) => log('!! tueur STDERR', d.toString().slice(0, 300)));
            p.on('close', (code) => resolve({ code, out }));
        });

        // Laisser le tueur amorcer son premier tour avant de déclencher
        // l'ouverture, pour qu'aucun tour de scrutation ne soit perdu.
        await dodo(500);
        log('>>> CONDAMNATION : ouverture de la fenêtre 4 (fenetre-4)');
        const tCondamnationDeclenchee = new Date().toISOString();
        vmIt('ouvrir4', 'ouvrir-4-condamnee.ps1');

        log('>>> attente de l\'abandon (jusqu\'à RELANCES_MAX dépassé) — polling agent.log');
        let abandonVu = false;
        for (let i = 0; i < 40; i += 1) {
            await dodo(2000);
            spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
            let texte = '';
            try { texte = await readFile(COPIE_LOG, 'utf8'); } catch { }
            const plat = texte.replace(/\x1b\[[0-9;]*m/g, '');
            if (/la session n'a pas tenu après/.test(plat)) { abandonVu = true; log(`   abandon détecté en agent.log à t+${(i + 1) * 2}s`); break; }
            if (i % 5 === 0) log(`   … t+${(i + 1) * 2}s, abandon pas encore vu`);
        }
        if (!abandonVu) log('!! ABANDON NON DÉTECTÉ dans la fenêtre de polling (80s) — voir le journal complet');

        const { code: codeTueur, out: sortieTueur } = await tueurPromise;
        log(`tueur terminé (code=${codeTueur}) :\n` + sortieTueur.trim());

        const tCondamnationClose = new Date().toISOString();
        await dodo(2000);
        await etat('après condamnation');
        const apresCondamnation = await marqueurs('APRÈS condamnation — critère à lire ici');
        await fenetresVm('APRÈS condamnation (fenêtre 4 doit toujours exister côté Windows)');

        log('>>> BORNES TEMPORELLES DE LA SÉQUENCE (horodatage LOCAL, pas celui du journal agent)');
        log(`   déclenchement ouverture fenêtre condamnée : ${tCondamnationDeclenchee}`);
        log(`   fin de la fenêtre de polling (abandon vu=${abandonVu}) : ${tCondamnationClose}`);

        log('SYNTHÈSE ' + JSON.stringify({
            avant: avantCondamnation,
            apres: apresCondamnation,
            delta_reouverture: apresCondamnation.reouverture - avantCondamnation.reouverture,
            delta_cloture: apresCondamnation.cloture - avantCondamnation.cloture,
            delta_sortie_creee: apresCondamnation.sortie_creee - avantCondamnation.sortie_creee,
            delta_duplication_etablie: apresCondamnation.duplication_etablie - avantCondamnation.duplication_etablie,
            delta_enfant_lance: apresCondamnation.enfant_lance - avantCondamnation.enfant_lance,
            abandon_relances: apresCondamnation.abandon_relances,
        }, null, 1));

        // Copie finale, avant tout arrêt.
        spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
