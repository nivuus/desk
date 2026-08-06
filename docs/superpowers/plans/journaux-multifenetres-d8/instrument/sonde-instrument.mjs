#!/usr/bin/env node
// P2 du sous-bloc D8 : l'instrument de recette sait-il entrer en plein écran ?
//
// Deux questions, et une seule les deux : `requestFullscreen()` exige une
// activation utilisateur TRANSITOIRE, et D1 a relevé qu'un clic CDP n'en
// fournissait pas une pour `requestPointerLock` — sans que la cause soit
// isolée. Si celle-ci échoue, tout le protocole de recette de D8 change.
//
// Usage : node sonde-instrument.mjs
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.ready = new Promise((resolve) =>
            this.ws.addEventListener('open', () => resolve(), { once: true }),
        );
        this.ws.addEventListener('message', (event) => {
            const message = JSON.parse(String(event.data));
            if (message.id !== undefined && this.pending.has(message.id)) {
                const { resolve, reject } = this.pending.get(message.id);
                this.pending.delete(message.id);
                if (message.error) reject(new Error(JSON.stringify(message.error)));
                else resolve(message.result);
            }
        });
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
        const r = await this.send('Runtime.evaluate', {
            expression,
            awaitPromise,
            returnByValue: true,
        });
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
        return r.result.value;
    }
}

const PAGE = `data:text/html,<body style="margin:0"><div id="c" style="width:100vw;height:100vh;background:#333"></div>`;

async function main() {
    const profil = await mkdtemp(join(tmpdir(), 'd8-p2-'));
    const chrome = spawn(chromeBin, [
        '--headless=new',
        '--remote-debugging-port=9333',
        `--user-data-dir=${profil}`,
        '--no-first-run',
        // Cet hôte exécute la sonde en tant que root, où Chrome refuse de
        // démarrer sans --no-sandbox (« Running as root without --no-sandbox
        // is not supported »). Écart technique par rapport au code du brief,
        // sans effet sur ce qui est mesuré (armement du geste, plein écran,
        // verrou clavier) : documenté dans le journal et le rapport.
        '--no-sandbox',
        '--window-size=1280,720',
        PAGE,
    ]);
    // Attendre que le port réponde : un port qui répond ne prouve pas que
    // c'est le bon navigateur (D1), d'où le profil jetable ci-dessus.
    let cible;
    for (let i = 0; i < 50; i++) {
        try {
            const liste = await fetch('http://127.0.0.1:9333/json/list').then((r) => r.json());
            cible = liste.find((t) => t.type === 'page');
            if (cible) break;
        } catch {}
        await new Promise((r) => setTimeout(r, 200));
    }
    if (!cible) throw new Error('aucune page CDP');

    const cdp = new Cdp(cible.webSocketDebuggerUrl);
    await cdp.send('Runtime.enable');

    // Q2 d'abord : elle ne dépend d'aucun geste.
    const clavier = await cdp.eval(
        `({ present: typeof navigator.keyboard === 'object' && navigator.keyboard !== null,
            lock: typeof navigator.keyboard?.lock === 'function' })`,
    );

    // Q1 : armer la demande, puis fournir un clic CDP, puis relire l'état.
    await cdp.eval(`
        window.__verdict = { appele: false, rejet: null };
        document.addEventListener('pointerdown', () => {
            window.__verdict.appele = true;
            document.getElementById('c').requestFullscreen()
                .catch((e) => { window.__verdict.rejet = String(e); });
        }, { once: true });
    `);
    for (const type of ['mousePressed', 'mouseReleased']) {
        await cdp.send('Input.dispatchMouseEvent', {
            type, x: 40, y: 40, button: 'left', clickCount: 1,
        });
    }
    await new Promise((r) => setTimeout(r, 800));
    const pleinEcran = await cdp.eval(
        `({ ...window.__verdict, actif: document.fullscreenElement !== null })`,
    );

    console.log(JSON.stringify({ clavier, pleinEcran }, null, 2));
    const recu = pleinEcran.appele && pleinEcran.actif && clavier.lock;
    console.log(recu
        ? 'P2 RECU : l instrument sait entrer en plein ecran et verrouiller le clavier'
        : 'P2 REFUSE : voir le detail ci-dessus, et le repli du §4 de la spec');

    chrome.kill('SIGKILL');
    // Attendre la sortie réelle du processus avant de purger le profil :
    // sans cela, Chrome peut encore écrire dans le répertoire au moment du
    // `rm`, qui échoue alors en ENOTEMPTY. Bug d'instrument sans rapport
    // avec le verdict, corrigé ici (cf. consigne 5 du brief).
    await new Promise((resolve) => {
        if (chrome.exitCode !== null || chrome.signalCode !== null) return resolve();
        chrome.once('exit', resolve);
    });
    await rm(profil, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
    process.exit(recu ? 0 : 1);
}

main().catch((e) => { console.error(e); process.exit(2); });
