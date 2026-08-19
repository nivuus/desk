// Sous-bloc D11 — plomberie commune aux quatre pilotes de recette, et LE
// PRÉDICAT DE DISTINCTION des flux (leg 8 de D10).
//
// ⚠️ Ce fichier n'est PAS dans la liste des fichiers créés par le plan de D11.
// Il est extrait plutôt que recopié quatre fois : la classe `Cdp`, le
// lancement de Chrome et les invariants de montage sont identiques aux quatre
// pilotes, et le prédicat de distinction DOIT être unique — deux copies qui
// divergeraient rendraient la recette ④ ininterprétable.
//
// ---------------------------------------------------------------------------
// INVARIANTS DE MONTAGE — hérités des sous-blocs D4 à D10, NON renégociés
// ---------------------------------------------------------------------------
//
// - Chrome `--app`, mire animée à cadence CONNUE et AFFICHÉE PAR LA MIRE
//   elle-même : sans ce chiffre, une capture lente et une source lente se
//   lisent pareil (D4).
// - UN `--user-data-dir` PAR FENÊTRE — SAUF pour les recettes ② et ③, qui
//   exigent au contraire un `--user-data-dir` PARTAGÉ pour obtenir un
//   `chrome.exe` UNIQUE, donc un seul groupe de PID (D8 : deux `notepad.exe`
//   seraient deux PID distincts, et le contrôle ne pourrait alors pas
//   échouer).
// - RELEVER les PID, jamais les supposer d'un nom d'exécutable : croiser
//   `Get-Process chrome` et les lignes `audio activé … pid=` de l'agent
//   lui-même — deux voies indépendantes, comme D8.
// - `--disable-popup-blocking` (sans quoi la page-shell ouvre dans le vide et
//   la seule trace est un message dans la page), et les TROIS anti-gel :
//   `--disable-background-timer-throttling`,
//   `--disable-backgrounding-occluded-windows`,
//   `--disable-renderer-backgrounding`. Sans eux, une page jamais au premier
//   plan GÈLE au bout de 5 minutes (chantier TURN).
// - Navigateur pilote sur l'HÔTE, JAMAIS sur la VM : sur la VM, la fenêtre de
//   la page-shell est elle-même capturée par le superviseur, ce qui boucle en
//   cascade d'ouvertures (D7).
// - AUCUNE capture d'écran CDP pendant une mesure (elle provoque un `Resize`,
//   donc un `SHOW`, donc une session de plus — D1), et TOUTE évaluation CDP
//   sur une page portant un flux WebRTC actif doit être BORNÉE : elle peut ne
//   JAMAIS rendre (D2). D'où `evalBorne`.
// - Le verdict audio se juge à la FRÉQUENCE DOMINANTE (`AnalyserNode`),
//   JAMAIS au compte d'octets : D7 a relevé `bytesReceived` en croissance sur
//   un spectre à −1000 dB. Le seuil se compare À LA DOMINANTE, jamais au
//   plancher de bruit — celui de D10 comparait au plancher (−158 dB) et NE
//   POUVAIT QUASIMENT PAS ÉCHOUER.

import { spawn } from 'node:child_process';

export const dodo = (ms) => new Promise((r) => setTimeout(r, ms));

export const CHROME = process.env.CHROME ?? 'google-chrome';

/** Les drapeaux communs, y compris les trois anti-gel. */
export function drapeauxChrome(port, userDataDir, extra = []) {
    return [
        '--headless=new',
        `--remote-debugging-port=${port}`,
        `--user-data-dir=${userDataDir}`,
        '--no-first-run',
        '--no-default-browser-check',
        // Requis quand le pilote tourne sous un compte privilegie : sans lui,
        // Chrome sort AVANT d'ouvrir le port de debogage, et le symptome est un
        // « devtools timeout » qui se lit comme une machine lente.
        '--no-sandbox',
        '--disable-popup-blocking',
        '--disable-background-timer-throttling',
        '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding',
        ...extra,
    ];
}

export function lancerChrome(port, userDataDir, extra = []) {
    return spawn(CHROME, drapeauxChrome(port, userDataDir, extra), { stdio: 'ignore' });
}

export async function attendreDevtools(port) {
    for (let i = 0; i < 80; i += 1) {
        try {
            const r = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (r.ok) return await r.json();
        } catch { /* pas encore prêt */ }
        await dodo(250);
    }
    throw new Error('devtools timeout');
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
        if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 600));
        return r.result.value;
    }
    /** Évaluation BORNÉE — obligatoire sur toute page portant un flux WebRTC. */
    async evalBorne(sessionId, expression, ms = 8000, awaitPromise = true) {
        return Promise.race([
            this.eval(sessionId, expression, awaitPromise),
            new Promise((r) => setTimeout(() => r({ __timeout: ms }), ms)),
        ]).catch((e) => ({ __erreur: String(e).slice(0, 200) }));
    }
}

// ---------------------------------------------------------------------------
// LE PRÉDICAT DE DISTINCTION (leg 8 de D10)
// ---------------------------------------------------------------------------
//
// D10 échantillonnait un sous-échantillon 8×8 de l'élément `<video>` VIVANT,
// page par page, par des allers-retours CDP indépendants (étalement mesuré :
// 187 et 245 ms) sur une source dont le fond DÉRIVE à chaque trame. Deux pages
// décodant le MÊME flux rendaient donc des empreintes différentes elles aussi :
// le contrôle NE POUVAIT PAS signaler une collision.
//
// Ici, on n'échantillonne QUE le marqueur d'identité de `anim-d11.html` : un
// aplat occupant l'octant haut-gauche, repeint à chaque trame mais dont la
// COULEUR ne dépend que de `n`. Deux pages sur le même flux rendent le même
// marqueur PAR CONSTRUCTION, quel que soit l'instant d'échantillonnage.
//
// Coordonnées DÉRIVÉES de `videoWidth`/`videoHeight`, jamais codées en dur :
// le flux peut être à n'importe quel barreau de l'échelle d'encodage.
export const EXPR_MARQUEUR = `(() => {
  // Element video en recette reelle ; canvas pour le controle de l'instrument
  // par lui-meme, qui tourne sur l'hote sans aucun flux WebRTC.
  //
  // AUCUN accent grave dans cette chaine : elle EST un litteral de gabarit, et
  // un accent grave la refermerait. node --check ne l'attrape PAS (le reste
  // reste syntaxiquement valide) -- l'erreur ne sort qu'a l'import.
  const v = document.querySelector('#remote');
  const c = document.querySelector('#c');
  let src = null, L = 0, H = 0;
  if (v && v.videoWidth > 0) { src = v; L = v.videoWidth; H = v.videoHeight; }
  else if (c && c.width > 0) { src = c; L = c.width; H = c.height; }
  if (!src) return { marqueur: null, raison: 'aucune source' };
  // Le marqueur occupe l'octant [0,L/8]x[0,H/8]. On moyenne un bloc de
  // L/32 x H/32 CENTRE sur (L/16, H/16) : il reste franchement a l'interieur
  // de l'octant a tout barreau, et la moyenne absorbe le bruit du codec.
  const bx = Math.max(1, Math.floor(L / 32)), by = Math.max(1, Math.floor(H / 32));
  const x0 = Math.floor(L / 16 - bx / 2), y0 = Math.floor(H / 16 - by / 2);
  const cv = document.createElement('canvas');
  cv.width = bx; cv.height = by;
  const g = cv.getContext('2d', { willReadFrequently: true });
  g.drawImage(src, x0, y0, bx, by, 0, 0, bx, by);
  const d = g.getImageData(0, 0, bx, by).data;
  let r = 0, vv = 0, b = 0, n = 0;
  for (let i = 0; i < d.length; i += 4) { r += d[i]; vv += d[i+1]; b += d[i+2]; n += 1; }
  r = Math.round(r / n); vv = Math.round(vv / n); b = Math.round(b / n);
  // Quantification par pas de 16 : absorbe le bruit de compression, tout en
  // laissant deux teintes voisines (47 degres d'ecart a saturation maximale)
  // franchement distinctes.
  const q = (x) => (x >> 4);
  const hex = (x) => x.toString(16).padStart(2, '0');
  return {
    marqueur: [q(r), q(vv), q(b)].join('-'),
    rgb: hex(r) + hex(vv) + hex(b),
    source: L + 'x' + H,
  };
})()`;

/** Deux marqueurs identiques = COLLISION : les deux pages voient le même flux. */
export function collision(a, b) {
    return a != null && b != null && a === b;
}
