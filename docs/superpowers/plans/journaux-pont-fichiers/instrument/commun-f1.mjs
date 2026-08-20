// Sous-bloc F1 — plomberie commune au pilote de recette du pont fichiers.
//
// La classe `Cdp`, le lancement de Chrome et l'attente de DevTools sont repris
// TELS QUELS de `journaux-multifenetres-d11/instrument/commun-d11.mjs` : ce
// depot parle CDP brut, il n'a ni puppeteer ni playwright, et diverger sur la
// plomberie ferait diverger les journaux d'un sous-bloc a l'autre.
//
// ---------------------------------------------------------------------------
// 🔴 CE QUE CET INSTRUMENT REMPLACE, ET CE QU'IL NE REMPLACE PAS
// ---------------------------------------------------------------------------
//
// `showDirectoryPicker()` est INUTILISABLE sous Chrome sans interface, et
// c'est MESURE, pas suppose (voir `sonde-picker.txt`) :
//
//   - sans interception CDP, l'appel rend `AbortError: The user aborted a
//     request.` -- il n'y a aucune interface pour afficher le selecteur ;
//   - avec `Page.setInterceptFileChooserDialog`, l'evenement
//     `Page.fileChooserOpened` est bien emis, mais l'appel rend
//     `AbortError: Intercepted by Page.setInterceptFileChooserDialog()` ;
//   - `Page.handleFileChooser` N'EXISTE PAS dans Chrome 151 (`-32601`), pas
//     plus que `Page.fileChooserAccepted`. Aucune commande n'ACCEPTE le
//     selecteur.
//
// L'hote n'a par ailleurs ni `DISPLAY`, ni socket X11, ni `Xvfb`, ni
// `xdotool` : un Chrome avec interface qui montrerait un vrai selecteur
// n'existe pas non plus.
//
// LA PARADE : `navigator.storage.getDirectory()` (OPFS) rend un VRAI
// `FileSystemDirectoryHandle` -- la meme classe, sans selecteur et sans geste.
// Verifie par mesure (`sonde-picker.txt`) : `instanceof
// FileSystemDirectoryHandle`, `getDirectoryHandle`/`getFileHandle`/`values`
// presents, `getFile()` rendant un vrai `File`, `slice()` reel, noms accentues
// et sous-repertoires supportes.
//
// On surcharge donc `window.showDirectoryPicker` pour qu'il rende une poignee
// OPFS peuplee depuis le VRAI jeu de donnees de l'hote, servi en HTTP.
//
// 🔴 LE POINT D'INJECTION EST CHOISI POUR ETRE LE PLUS BAS POSSIBLE.
// `choisirDossier()` (`client/src/fichiers/canal.ts:182`) lit
// `globalThis.showDirectoryPicker` A L'APPEL. En le surchargeant, TOUT le code
// produit tourne INCHANGE : `choisirDossier`, l'affectation `const racine:
// Racine = poignee` qui est le controle de compatibilite structurelle,
// `creerAdaptateur`, `creerServeur`, `connecterCanalFichiers`, et le
// gestionnaire de clic de `shell-page.ts`. Une injection plus haute -- par
// exemple remplacer `creerAdaptateur` -- aurait court-circuite le code sous
// test et rendu la recette VACUEUSE.
//
// ⚠️ CE QUI N'EST DONC PAS EXERCE, et qu'aucun critere de cette recette ne
// couvre : l'appel `showDirectoryPicker()` lui-meme, le modele de permission
// des repertoires choisis par l'utilisateur (`queryPermission` /
// `requestPermission` -- que `choisirDossier` n'appelle d'ailleurs pas), et
// l'activation utilisateur transitoire. Les octets, eux, viennent bien du
// disque de l'hote et traversent la chaine entiere.

import { spawn } from 'node:child_process';

export const dodo = (ms) => new Promise((r) => setTimeout(r, ms));
export const CHROME = process.env.CHROME ?? 'google-chrome';

export function drapeauxChrome(port, userDataDir, extra = []) {
    return [
        '--headless=new',
        `--remote-debugging-port=${port}`,
        `--user-data-dir=${userDataDir}`,
        '--no-first-run',
        '--no-default-browser-check',
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--remote-allow-origins=*',
        // Sans lui la page-shell ouvre ses fenetres dans le vide et la seule
        // trace est un message dans la page (D1).
        '--disable-popup-blocking',
        // Les trois anti-gel : sans eux une page jamais au premier plan gele
        // au bout de 5 minutes (chantier TURN).
        '--disable-background-timer-throttling',
        '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns',
        ...extra,
    ];
}

export function lancerChrome(port, userDataDir, extra = []) {
    return spawn(CHROME, drapeauxChrome(port, userDataDir, extra), { stdio: 'ignore' });
}

export async function attendreDevtools(port) {
    for (let i = 0; i < 120; i += 1) {
        try {
            const r = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (r.ok) return await r.json();
        } catch { /* pas encore pret */ }
        await dodo(250);
    }
    throw new Error('devtools timeout');
}

/** Chrome 150+ exige PUT sur /json/new (verify-webrtc.mjs:171). */
export async function ouvrirOnglet(port, url) {
    const r = await fetch(`http://127.0.0.1:${port}/json/new?${url}`, { method: 'PUT' });
    if (!r.ok) throw new Error(`/json/new a rendu ${r.status}`);
    return r.json();
}

export class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.handlers = [];
        this.ready = new Promise((res) =>
            this.ws.addEventListener('open', () => res(), { once: true }));
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
        if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 800));
        return r.result.value;
    }
    /** Evaluation BORNEE -- obligatoire sur toute page portant un flux WebRTC (D2). */
    async evalBorne(sessionId, expression, ms = 8000, awaitPromise = true) {
        return Promise.race([
            this.eval(sessionId, expression, awaitPromise),
            new Promise((r) => setTimeout(() => r({ __timeout: ms }), ms)),
        ]).catch((e) => ({ __erreur: String(e).slice(0, 300) }));
    }
}
