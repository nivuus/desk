#!/usr/bin/env node
// LA PORTE P0 DE G5 — éliminatoire, jouée AVANT toute écriture de produit.
//
// LA QUESTION, et elle est UNE : peut-on faire installer une PWA par un
// navigateur SANS qu'aucune route authentifiée ne s'ouvre ?
//
// Un `<link rel="manifest">` est allé chercher par le navigateur SANS en-tête
// `Authorization`, exactement comme les icônes qu'il nomme, et le sous-projet
// ⑤ ne pose AUCUN cookie : son porteur vit dans `localStorage`, qui ne voyage
// sur aucune requête que le navigateur émet de lui-même. La voie candidate
// (V1) est un manifeste que la page AUTHENTIFIÉE construit elle-même, publie
// en `blob:`, et dont les icônes sont des `data:`.
//
// 🔴 CE QUE CET INSTRUMENT MESURE EST LE NAVIGATEUR, PAS LE PRODUIT. Aucun
// agent, aucune VM, aucune plateforme. Une page locale, un Chromium sans
// interface, et `Page.getAppManifest`.
//
// 🔴 P0-b EST LA MOITIÉ QUI COMPTE, ET ELLE SE JOUE EN PREMIER. Une sonde qui
// ne verrait que le cas favorable ne saurait pas dire si `errors` PEUT se
// remplir : elle serait verte en ne mesurant rien. C'est le patron que ce
// dépôt a payé six fois (F1 de D7, les quatre contrôles vacueux de D10, le
// témoin de mesurabilité de P1).
//
// Usage : node porte-p0.mjs <a|b|c|d|e|f> [journal.json]

import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { aplatPng } from './png.mjs';

const sonde = process.argv[2];
const sortie = process.argv[3];
const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';

if (!['a', 'b', 'c', 'd', 'e', 'f'].includes(sonde)) {
    console.error('usage : porte-p0.mjs <a|b|c|d|e|f> [journal.json]');
    process.exit(2);
}

// ─── le client CDP, sur le patron de client/recette/harness.mjs ───────────────

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
        if (m.method === 'Runtime.exceptionThrown') this.pageErrors.push(m.params.exceptionDetails.text);
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
        const r = await this.send('Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? JSON.stringify(r.exceptionDetails));
        return r.result.value;
    }
    close() { this.ws.close(); }
}

async function attendreDevtools(port, tentatives = 60) {
    for (let i = 0; i < tentatives; i += 1) {
        try { if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) return; } catch { /* pas prêt */ }
        await new Promise((r) => setTimeout(r, 200));
    }
    throw new Error('Chrome DevTools ne répond pas après le délai imparti');
}

// ─── les pages ────────────────────────────────────────────────────────────────

// ⚠️ Le manifeste est construit PAR LA PAGE, jamais servi. C'est la voie V1
// tout entière : `fetch` authentifié → objet en mémoire → `data:` pour
// l'icône → `Blob` → `createObjectURL` → `<link rel="manifest">`.
//
// 🔴 LES URL SONT ABSOLUES, ET C'EST UN RÉSULTAT DE P0 LUI-MÊME, PAS UN GOÛT.
//    Le premier jet de cet instrument les écrivait RELATIVES, comme n'importe
//    quel manifeste servi par HTTP. Mesuré, journal versé
//    (`p0-0-instrument-defectueux-url-relatives.json`) : sous un manifeste
//    `blob:`, la base de résolution est l'URL `blob:` elle-même, et Chromium
//    rend « property 'start_url' ignored, URL is invalid. » plus
//    `start-url-not-valid` à l'installabilité. Le mode `relatives` est
//    CONSERVÉ (sonde `f`) pour que le fait reste REJOUABLE, jamais raconté.
const PAGE_BLOB = (cote, b64, urls) => `<!doctype html>
<html lang="fr"><head><meta charset="utf-8"><title>P0 blob</title>
<link rel="icon" href="data:,">
</head><body><h1>P0</h1><script>
(() => {
  const abs = ${JSON.stringify(urls === 'absolues')};
  const base = abs ? location.origin : '';
  const manifeste = {
    name: 'Application temoin G5',
    short_name: 'Temoin',
    start_url: base + '/p0.html?app=00000000-0000-4000-8000-000000000000',
    scope: base + '/',
    id: base + '/temoin-00000000',
    display: 'standalone',
    background_color: '#0b0d10',
    theme_color: '#7aa2f7',
    icons: [{ src: 'data:image/png;base64,${b64}', sizes: '${cote}x${cote}', type: 'image/png', purpose: 'any' }],
  };
  const url = URL.createObjectURL(new Blob([JSON.stringify(manifeste)], { type: 'application/manifest+json' }));
  const lien = document.createElement('link');
  lien.rel = 'manifest';
  lien.href = url;
  document.head.appendChild(lien);
  window.__urlManifeste = url;
  window.__pose = true;
})();
window.__bip = null;
window.addEventListener('beforeinstallprompt', (e) => { window.__bip = String(e.type); });
<\/script></body></html>`;

// Le TÉMOIN : le même manifeste, servi par HTTP ordinaire. Ce qui diffère
// entre lui et P0-a est imputable au `blob:` ET À RIEN D'AUTRE.
const PAGE_HTTP = `<!doctype html>
<html lang="fr"><head><meta charset="utf-8"><title>P0 http</title>
<link rel="icon" href="data:,">
<link rel="manifest" href="/temoin.webmanifest">
</head><body><h1>P0 temoin</h1><script>
window.__pose = true;
window.__bip = null;
window.addEventListener('beforeinstallprompt', (e) => { window.__bip = String(e.type); });
<\/script></body></html>`;

function manifesteHttp(cote) {
    return JSON.stringify({
        name: 'Application temoin G5',
        short_name: 'Temoin',
        start_url: '/p0.html?app=00000000-0000-4000-8000-000000000000',
        scope: '/',
        id: '/temoin-00000000',
        display: 'standalone',
        background_color: '#0b0d10',
        theme_color: '#7aa2f7',
        icons: [{ src: `/icone-${cote}.png`, sizes: `${cote}x${cote}`, type: 'image/png', purpose: 'any' }],
    });
}

// ─── le serveur local ────────────────────────────────────────────────────────

function servir(cote, urls) {
    const png512 = aplatPng(512);
    const png128 = aplatPng(128);
    const b64 = (cote === 512 ? png512 : png128).toString('base64');
    const serveur = createServer((req, res) => {
        const chemin = req.url.split('?')[0];
        if (chemin === '/p0.html') {
            res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
            res.end(PAGE_BLOB(cote, b64, urls));
        } else if (chemin === '/temoin.html') {
            res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
            res.end(PAGE_HTTP);
        } else if (chemin === '/temoin.webmanifest') {
            res.writeHead(200, { 'content-type': 'application/manifest+json' });
            res.end(manifesteHttp(cote));
        } else if (chemin === '/icone-512.png') {
            res.writeHead(200, { 'content-type': 'image/png' });
            res.end(png512);
        } else if (chemin === '/icone-128.png') {
            res.writeHead(200, { 'content-type': 'image/png' });
            res.end(png128);
        } else {
            res.writeHead(404).end('non');
        }
    });
    return new Promise((r) => serveur.listen(0, '127.0.0.1', () => r({ serveur, port: serveur.address().port })));
}

// ─── la mesure ───────────────────────────────────────────────────────────────

const PLAN = {
    // 🔴 P0-b EN PREMIER : l'icône SOUS le seuil. Sans elle, P0-a serait verte
    //    en ne mesurant rien.
    b: { cote: 128, page: 'p0.html', quoi: 'blob: + icone 128x128 (SOUS le seuil)' },
    a: { cote: 512, page: 'p0.html', quoi: 'blob: + icone 512x512' },
    c: { cote: 512, page: 'temoin.html', quoi: 'TEMOIN : manifeste servi par HTTP ordinaire' },
    d: { cote: 512, page: 'p0.html', quoi: 'blob: 512, SANS service worker — une exigence de SW parait-elle ?' },
    e: { cote: 512, page: 'p0.html', quoi: 'beforeinstallprompt se declenche-t-il en --headless=new ?' },
    // La reprise du DÉFAUT trouvé au premier jet, gardée pour qu'il reste
    // rejouable : sous `blob:`, une URL relative n'a pas de base utilisable.
    f: { cote: 512, page: 'p0.html', quoi: 'blob: 512, URL RELATIVES — le defaut du premier jet', urls: 'relatives' },
};

async function jouer() {
    const { cote, page, quoi, urls = 'absolues' } = PLAN[sonde];
    const { serveur, port: portHttp } = await servir(cote, urls);
    const portCdp = 9500 + Math.floor(Math.random() * 400);
    const userDataDir = await mkdtemp(join(tmpdir(), 'p0-g5-'));
    const chrome = spawn(chromeBin, [
        '--headless=new',
        `--remote-debugging-port=${portCdp}`,
        '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`,
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--disable-gpu',
        'about:blank',
    ], { stdio: 'ignore' });

    const releve = { sonde, quoi, cote, urls, horodatage: new Date().toISOString() };
    let cdp;
    try {
        await attendreDevtools(portCdp);
        releve.navigateur = (await (await fetch(`http://127.0.0.1:${portCdp}/json/version`)).json()).Browser;
        const cree = await (await fetch(`http://127.0.0.1:${portCdp}/json/new?about:blank`, { method: 'PUT' })).json();
        cdp = new Cdp(cree.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');

        const url = `http://127.0.0.1:${portHttp}/${page}`;
        releve.url = url;
        await cdp.send('Page.navigate', { url });
        // On attend le FAIT (le script a posé son lien), jamais une durée seule.
        for (let i = 0; i < 60; i += 1) {
            if (await cdp.eval('window.__pose === true').catch(() => false)) break;
            await new Promise((r) => setTimeout(r, 100));
        }
        releve.urlManifestePosee = await cdp.eval('window.__urlManifeste ?? null').catch(() => null);

        // Le navigateur va chercher le manifeste de façon asynchrone : on laisse
        // passer un temps borné, puis on relève. Ce délai n'entre dans aucun
        // verdict — seuls `url`, `data` et `errors` en sortent.
        await new Promise((r) => setTimeout(r, 1500));

        const m = await cdp.send('Page.getAppManifest');
        // ⚠️ `Page.getAppManifest` rend un objet MÊME quand la page n'a pas de
        //    manifeste : c'est l'URL qu'il retourne qui tranche, pas le `data`.
        releve.manifesteUrl = m.url ?? null;
        releve.manifesteData = m.data ?? null;
        releve.errors = m.errors ?? [];
        releve.parsed = m.parsed ?? null;
        if (m.manifest) releve.manifest = m.manifest;

        // Relevé annexe : le CDP expose-t-il un verdict d'installabilité ?
        try {
            releve.installabilityErrors = (await cdp.send('Page.getInstallabilityErrors')).installabilityErrors;
        } catch (e) {
            releve.installabilityErrors = `INDISPONIBLE : ${e.message}`;
        }

        releve.serviceWorkersEnregistres = await cdp.eval(
            "(async () => navigator.serviceWorker ? (await navigator.serviceWorker.getRegistrations()).length : 'API absente')()",
            true,
        ).catch((e) => `erreur : ${e.message}`);
        // 🔵 `beforeinstallprompt` est relevé sur TOUTES les sondes, et pas
        //    seulement sur P0-e : c'est le seul témoin qui dise que Chromium
        //    tient l'application pour installable, et le comparer entre P0-a
        //    (512) et P0-b (128) est un second discriminant, INDÉPENDANT de
        //    `getInstallabilityErrors`.
        await new Promise((r) => setTimeout(r, 3000));
        releve.beforeinstallprompt = await cdp.eval('window.__bip');
        releve.console = cdp.consoleLines;
        releve.erreursPage = cdp.pageErrors;
    } catch (e) {
        releve.echec = e.message;
    } finally {
        cdp?.close();
        chrome.kill('SIGKILL');
        serveur.close();
        // Chrome écrit encore quand il meurt : le ménage est BEST-EFFORT, et
        // son échec ne doit jamais emporter un relevé déjà pris.
        await new Promise((r) => setTimeout(r, 300));
        await rm(userDataDir, { recursive: true, force: true }).catch(() => {});
    }

    const texte = JSON.stringify(releve, null, 2);
    if (sortie) await writeFile(sortie, texte + '\n');
    console.log(texte);
}

jouer().then(() => process.exit(0), (e) => { console.error(e); process.exit(1); });
