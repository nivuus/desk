#!/usr/bin/env node
// Pilote de recette du lot 31 : le CHIFFRE-JUGE du chemin d'encodage.
//
// 🔴 CE QUI JUGE N'EST PAS QU'UN ENCODEUR SE CREE. Un encodeur qui
// s'instancie, se configure et n'emet jamais rendrait tous les controles
// verts. Le juge est une IMAGE QUI ARRIVE AU NAVIGATEUR : le delta de
// `framesDecoded` de la piste `inbound-rtp` video, entre deux releves
// `getStats()` espaces d'un palier.
//
// ⚠️ `bytesReceived` NE JUGE PAS : ce depot a mesure qu'il croit sur un
// spectre audio a -1000 dB. L'analogue video est un flux qui porte des
// octets sans jamais rendre une image decodable.
//
// 🔴 ET LA MIRE DOIT BOUGER. Desktop Duplication n'emet qu'au CHANGEMENT du
// bureau : une mire immobile rendrait framesDecoded=0 sur les DEUX bras, et
// le rouge serait vacueux. `mire.ps1` anime a 10 Hz et AFFICHE sa cadence.
//
// Usage :
//   RECETTE_EMAIL=... RECETTE_MOTDEPASSE=... PLATEFORME_URL=http://h:p \
//     node pilote-lot31.mjs <url-shell> <palier_ms>
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { attendreDevtools } from '../../../../../client/recette/devtools.mjs';
import { semerJeton } from '../../../../../client/recette/jeton-recette.mjs';

const urlShell = process.argv[2];
const palierMs = Number(process.argv[3] ?? 15000);
const PORT = 9333;

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.n = 1;
        this.attente = new Map();
        this.pret = new Promise((r) => this.ws.addEventListener('open', () => r(), { once: true }));
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.attente.has(m.id)) {
                const { resolve, reject } = this.attente.get(m.id);
                this.attente.delete(m.id);
                m.error ? reject(new Error(JSON.stringify(m.error))) : resolve(m.result);
            }
        });
    }
    async envoyer(method, params = {}) {
        await this.pret;
        const id = this.n++;
        return new Promise((resolve, reject) => {
            this.attente.set(id, { resolve, reject });
            this.ws.send(JSON.stringify({ id, method, params }));
        });
    }
    async evaluer(expr) {
        const r = await this.envoyer('Runtime.evaluate', {
            expression: expr, awaitPromise: true, returnByValue: true,
        });
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
        return r.result.value;
    }
    /// `semerJeton` (partage avec les autres pilotes) appelle `.send()` :
    /// l'alias evite de dupliquer le semeur pour une difference de nom.
    async send(method, params) { return this.envoyer(method, params); }
    fermer() { try { this.ws.close(); } catch {} }
}

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

async function cibles() {
    const r = await fetch(`http://127.0.0.1:${PORT}/json`);
    return await r.json();
}

/// Le releve `inbound-rtp` video : ce qui juge, et ce qui NE juge pas.
const RELEVE = `(async () => {
  if (!window.__pc) return null;
  const s = await window.__pc.getStats();
  let v = null, a = null;
  s.forEach((r) => {
    if (r.type === 'inbound-rtp' && r.kind === 'video') v = r;
    if (r.type === 'inbound-rtp' && r.kind === 'audio') a = r;
  });
  return {
    ice: window.__pc.iceConnectionState,
    framesDecoded: v ? (v.framesDecoded ?? 0) : null,
    framesReceived: v ? (v.framesReceived ?? 0) : null,
    bytesVideo: v ? (v.bytesReceived ?? 0) : null,
    bytesAudio: a ? (a.bytesReceived ?? 0) : null,
    t: Date.now(),
  };
})()`;

const profil = await mkdtemp(join(tmpdir(), 'lot31-'));
const chrome = spawn('google-chrome', [
    '--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profil}`,
    '--no-sandbox', '--no-first-run',
    // ⚠️ Sans ces trois-la, Chrome gele une page jamais mise au premier plan
    // et la session tombe vers 331-340 s.
    '--disable-background-timer-throttling',
    '--disable-backgrounding-occluded-windows',
    '--disable-renderer-backgrounding',
    // 🔴 La shell ouvre chaque fenetre par `window.open`, SANS geste
    // humain : le bloqueur de fenetres surgissantes de Chrome l'annule en
    // silence, et le symptome se lit « la shell n'a recu aucune fenetre ».
    '--disable-popup-blocking',
    'about:blank',
], { stdio: 'ignore' });

try {
    await attendreDevtools(PORT);
    const liste = await cibles();
    const page = liste.find((c) => c.type === 'page');
    const shell = new Cdp(page.webSocketDebuggerUrl);
    await shell.envoyer('Page.enable');
    await shell.envoyer('Runtime.enable');
    await semerJeton(shell);
    // 🔴 ON SE CONNECTE PAR LE FORMULAIRE, comme un humain. Semer le jeton ne
    // suffit PAS : `connexion.html` reste sur son ecran tant qu'on n'a pas
    // valide -- mesure, pas supposition (le premier essai est reste sur
    // « Connexion | Courriel | Mot de passe », prefixe=null). Et c'est ce
    // parcours qui pose `guac.prefixe`, sans lequel la shell ne rejoint pas
    // la session de controle de l'agent.
    await shell.envoyer('Page.navigate', { url: urlShell });
    await dormir(3000);
    const email = process.env.RECETTE_EMAIL;
    const mdp = process.env.RECETTE_MOTDEPASSE;
    await shell.evaluer(`(() => {
        const e = document.querySelector('#email');
        const m = document.querySelector('#motdepasse');
        if (!e || !m) return 'formulaire absent';
        const poser = (el, v) => {
            const d = Object.getOwnPropertyDescriptor(el.constructor.prototype, 'value');
            d.set.call(el, v);
            el.dispatchEvent(new Event('input', { bubbles: true }));
        };
        poser(e, ${JSON.stringify(email)});
        poser(m, ${JSON.stringify(mdp)});
        document.querySelector('#valider').click();
        return 'soumis';
    })()`);
    for (let i = 0; i < 30; i += 1) {
        await dormir(1000);
        const p = await shell.evaluer("localStorage.getItem('guac.prefixe')").catch(() => null);
        if (p) { console.log(`prefixe pose : ${p}`); break; }
    }
    console.log(`page courante : ${await shell.evaluer('document.location.href')}`);

    // La page de session est ouverte par la shell : on la CHERCHE parmi les
    // cibles, plutot que de supposer qu'une amorce injectee y court -- ce
    // depot a mesure que `Page.addScriptToEvaluateOnNewDocument` NE COURT PAS
    // sur une page ouverte par `window.open`.
    let session = null;
    for (let i = 0; i < 120 && !session; i += 1) {
        await dormir(1000);
        for (const c of await cibles()) {
            if (c.type !== 'page' || c.id === page.id) continue;
            const essai = new Cdp(c.webSocketDebuggerUrl);
            await essai.envoyer('Runtime.enable');
            const vu = await essai.evaluer('typeof window.__pc').catch(() => 'erreur');
            if (vu === 'object') { session = essai; console.log(`page de session : ${c.url}`); break; }
            essai.fermer();
        }
    }
    if (!session) throw new Error("aucune page de session n'est apparue : la shell a-t-elle recu une fenetre ?");

    const a = await session.evaluer(RELEVE);
    console.log('releve A ' + JSON.stringify(a));
    await dormir(palierMs);
    const b = await session.evaluer(RELEVE);
    console.log('releve B ' + JSON.stringify(b));

    const d = (b?.framesDecoded ?? 0) - (a?.framesDecoded ?? 0);
    const secondes = ((b?.t ?? 0) - (a?.t ?? 0)) / 1000;
    console.log(`\nCHIFFRE-JUGE framesDecoded_delta=${d} palier_s=${secondes.toFixed(1)} ` +
                `cadence=${secondes > 0 ? (d / secondes).toFixed(2) : 'n/a'} i/s`);
    console.log(`TEMOIN ice=${b?.ice} bytesVideo_delta=${(b?.bytesVideo ?? 0) - (a?.bytesVideo ?? 0)} ` +
                `bytesAudio_delta=${(b?.bytesAudio ?? 0) - (a?.bytesAudio ?? 0)}`);
    console.log(d > 0 ? 'VERDICT: des images arrivent au navigateur' : 'VERDICT: AUCUNE image');
} finally {
    chrome.kill('SIGKILL');
    // ⚠️ Le menage ne doit JAMAIS masquer l'erreur du corps. La premiere
    // version laissait `rm` lever ENOTEMPTY depuis le `finally`, ce qui
    // REMPLACAIT la vraie cause par une erreur de repertoire -- et le
    // diagnostic partait du mauvais cote.
    await rm(profil, { recursive: true, force: true }).catch(() => {});
}
