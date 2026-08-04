#!/usr/bin/env node
// Sous-bloc D7, tâche 11 — l'INSTRUMENT de la recette d'isolation audio par
// fenêtre (la recette elle-même, avec ses `critere-*.log`, est la tâche 13).
//
// Dérivé de `docs/superpowers/plans/journaux-multifenetres-d6/instrument/pilote-recette-d6.mjs`
// (même approche CDP, mêmes conventions, mêmes drapeaux de lancement) — pas
// réinventé, adapté.
//
// CE QUE CE PILOTE PROUVE, ET POURQUOI `bytesReceived` NE SUFFIT PAS :
//
//   « Chaque fenêtre entend son application et elle seule » ne se prouve pas
//   par de l'octet qui arrive : du son arrive, on ignore LEQUEL. Ce pilote
//   ouvre N fenêtres jouant chacune un ton PUR à une fréquence CONNUE et
//   DISTINCTE (`ton.html?hz=…`), branche un `AnalyserNode` sur la piste audio
//   réellement REÇUE par chaque page-session, et relève le bin le plus
//   énergique. Le critère d'isolation devient alors : la page qui devrait
//   entendre 440 Hz lit-elle bien ~440 — et PAS 880 ?
//
// Contraintes de protocole héritées de D4/D5/D6 (chacune a coûté une mesure) :
//   1. `--user-data-dir` PAR FENÊTRE, sinon Chrome rejoint son instance
//      existante et on compte des lancements au lieu de fenêtres ;
//   2. `AudioContext` démarre `suspended` sans activation utilisateur : ce
//      pilote appelle `ctx.resume()` et VÉRIFIE `ctx.state === 'running'`
//      avant toute mesure — voir `expressionReleveFrequence` ci-dessous — sinon on
//      mesure un silence et on conclut à tort à une panne d'isolation ;
//   3. toute évaluation CDP sur une page portant un flux WebRTC actif est
//      BORNÉE (`Cdp.evalBorne`, `Promise.race` avec délai) : elle peut ne
//      JAMAIS rendre ;
//   4. AUCUNE capture d'écran CDP pendant une mesure ;
//   5. visibilité ET focus sont IMPOSÉS page par page (`imposerScenario`) : un
//      Chrome sans interface rapporte `document.hidden = true` pour toute
//      fenêtre d'arrière-plan, et la cible passe par `blur` PUIS `focus` — la
//      déduplication de `client/src/visibilite.ts` peut sinon faire
//      DISPARAÎTRE le focus ;
//   6. Chrome pour une mesure de plus de 5 minutes porte les trois drapeaux
//      anti-gel (`--disable-background-timer-throttling`,
//      `--disable-backgrounding-occluded-windows`,
//      `--disable-renderer-backgrounding`) ;
//   7. `--disable-popup-blocking`, sans quoi la page-shell voit toutes ses
//      fenêtres refusées en silence.
//
// CE QUE CE FICHIER NE FAIT PAS : il ne pilote pas les cinq critères de la
// tâche 13 (arbitrage par PID, survie au sommeil, budget rendu, non-régression
// mono-fenêtre) — seul le critère ① (isolation par fréquence) y est câblé de
// bout en bout, en exemple d'emploi. Les fonctions exportées (`Cdp`,
// `expressionReleveFrequence`, `imposerScenario`, `AMORCE`) sont réutilisables
// telles quelles par la tâche 13 pour les critères restants.
//
// CORRECTIF ROUND 1/5 (revue de tâche 11) — cinq changements de fond :
//   - un simple argmax ne prouve QUE « le ton le plus fort », jamais « et pas
//     les autres » : une fenêtre qui entendrait 440 en plein ET 880 à -10 dB
//     argmaxerait quand même sur 440 et passerait « OK » à tort. Le verdict
//     lit maintenant le niveau à CHAQUE fréquence assignée, pas seulement au
//     pic (`niveaux`, `jugerIsolation`) ;
//   - `ctx` n'était jamais fermé : jusqu'à dix contextes web audio vivants en
//     parallèle sur un hôte dont la charge a déjà faussé des mesures d'un
//     facteur 19 (D6). Fermé en `finally` ;
//   - la piste audio se lit désormais AUSSI via `<video>.srcObject` (le
//     `MediaStream` unique de `client/src/webrtc.ts`), qui ne dépend d'aucune
//     course avec l'amorce ;
//   - `bytesReceived`/`packetsReceived`/`muted`/`readyState` sont relevés
//     comme DISCRIMINANTS annexes (jamais comme preuve d'isolation à eux
//     seuls) ;
//   - l'assignation fenêtre↔fréquence n'est plus SUPPOSÉE de l'ordre
//     d'ouverture : elle est confrontée à ce que la page-shell affiche avoir
//     réellement reçu du produit (mécanisme corrigé au round 3, voir plus
//     bas — la version de ce round-ci, par `document.title`, ne fonctionnait
//     pas).
//
// CORRECTIF ROUND 2/5 — un repli latent qui ouvrait la porte au lieu de la
// fermer (`plancher_db` manquant substitué par `SENTINEL_DB`, qui aurait
// PASSÉ la marge de signal au lieu de la faire échouer) est devenu sa propre
// erreur explicite. Voir `jugerIsolation`.
//
// CORRECTIF ROUND 3/5 — mesuré sur la VM (recette du 4 août 2026, critère ①,
// deux fenêtres à 440 et 880 Hz, isolation à 85 et 94 dB de marge) :
// `document.title` N'ATTEINT PAS la légende que le superviseur relaie pour
// une fenêtre Chrome `--app` sur `file://` — cette légende est le CHEMIN DE
// FICHIER, identique pour toutes les fenêtres puisque la chaîne de requête
// n'y figure pas. Le garde-fou du round 1 était donc en permanence rouge, y
// compris sur une mesure d'isolation par ailleurs parfaite. Remède :
// l'identité vit maintenant dans le NOM DE FICHIER (`ton-440.html`,
// `ton-880.html`, copiés par fréquence), et le contrôle devient un test de
// CONTENANCE par fenêtre plutôt qu'une comparaison positionnelle de tableaux.
// Voir `verifierAssignationParFichier`.

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
const AIDE = process.env.AIDE
    ?? join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d7/instrument');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const COPIE_LOG = process.env.COPIE_LOG ?? `/tmp/agent-recette-d7-${ETIQUETTE}.log`;
const SORTIE_JSON = process.env.SORTIE_JSON
    ?? join(RACINE, `docs/superpowers/plans/journaux-multifenetres-d7/recette-${ETIQUETTE}.json`);
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
// Fréquences des N fenêtres, dans l'ordre d'ouverture. Deux par défaut : le
// couple du critère ① de la tâche 13 (`ton.html?hz=440` / `?hz=880`).
const HZS = (process.env.HZS ?? '440,880').split(',').map(Number).filter(Number.isFinite);
// Gain de chaque fenêtre, même longueur que HZS ou une seule valeur répétée.
// `GAINS=0.25,0` fait de la SECONDE fenêtre une fenêtre « muette » réaliste
// (étape 3 du brief : le graphe audio tourne, l'énergie est nulle) sans
// changer HZS.
const GAINS = (process.env.GAINS ?? '0.25').split(',').map(Number);
const gainDe = (i) => GAINS[i] ?? GAINS[GAINS.length - 1] ?? 0.25;
// Délai avant la première lecture, pour laisser l'échelle DXGI/NVENC et la
// négociation audio (voir `agent/src/audio.rs`) se poser après l'ouverture de
// la dernière fenêtre. Pas calibré : large par construction.
const DELAI_STABILISATION_S = Number(process.env.DELAI_STABILISATION_S ?? 15);
// Index (0-based) de la fenêtre focalisée par `imposerScenario`. Sans objet
// pour le critère ① seul (aucune des deux fenêtres n'a besoin du focus pour
// conserver son encodeur tant que N <= `vivier::PLAFOND_EVEIL` = 8), mais
// câblé pour que la tâche 13 réutilise ce pilote sur le critère ② (arbitrage
// par PID), qui EXIGE un déplacement de focus.
const INDEX_FOCUS = Number(process.env.INDEX_FOCUS ?? 0);
// Tolérance de correspondance fréquence mesurée / fréquence assignée. Les
// deux fréquences par défaut (440, 880) sont séparées de 440 Hz ; la
// résolution du bin FFT (voir expressionReleveFrequence, fftSize=8192) est
// `sampleRate / fftSize` — RELEVÉE à 5,383 Hz en vérification locale (Chrome
// Linux, `ctx.sampleRate = 44100`), et attendue autour de 5,86 Hz sur la VM
// Windows (le sondage audio de juillet 2026, CLAUDE.md, y a mesuré un mixage
// à 48 000 Hz — mais PAS pour un `AudioContext` de navigateur, dont le taux
// n'a pas été mesuré ici et peut différer). Dans les deux cas, une tolérance
// de 25 Hz reste très en dessous des 440 Hz qui séparent les deux tons tout
// en absorbant largement l'arrondi du bin (1 à 3 Hz mesurés localement).
const TOLERANCE_HZ = Number(process.env.TOLERANCE_HZ ?? 25);
// L'argmax seul ne prouve QUE « le ton le plus fort », jamais « et pas les
// autres » (revue de tâche 11, FINDING 1) : une fenêtre qui entendrait 440 en
// plein ET 880 à -10 dB argmaxerait quand même sur 440. Le verdict lit donc le
// niveau à CHAQUE fréquence assignée (`niveaux`, calculé côté page) et exige
// DEUX marges, toutes deux NON CALIBRÉES — choisies par analogie avec un
// rapport signal/interférence confortable à l'oreille, jamais mesurées sur ce
// produit ni ajustées à son bruit de fond réel :
//   - `MARGE_ISOLATION_DB` : écart minimal entre le niveau à SA fréquence et
//     le niveau à CHAQUE AUTRE fréquence assignée — c'est elle qui distingue
//     « on entend surtout la sienne » de « on n'entend QUE la sienne » ;
//   - `MARGE_SIGNAL_DB` : écart minimal entre le niveau à SA fréquence et le
//     PLANCHER du spectre — sans elle, un pic à peine au-dessus du bruit de
//     fond compterait comme une isolation réussie.
const MARGE_ISOLATION_DB = Number(process.env.MARGE_ISOLATION_DB ?? 10);
const MARGE_SIGNAL_DB = Number(process.env.MARGE_SIGNAL_DB ?? 20);
// En-dessous de tout plancher RÉEL observé en vérification locale (-210 dB,
// silence quasi total) : sert à représenter -Infinity (silence numérique
// exact, non représentable en JSON — `JSON.stringify(-Infinity) === 'null'`)
// par une valeur NUMÉRIQUE distincte de « champ absent » (`null` propre).
// Revue de tâche 11, FINDING 6 : avant ce correctif, `db` ET `plancher_db`
// utilisaient tous deux `null` pour « silence total » ET pour « valeur
// manquante », strictement indiscernables une fois relus dans un journal.
const SENTINEL_DB = -1000;
// Préparer la VM avant de lancer (kill des processus résiduels, purge des
// sorties virtuelles orphelines) : mis à '0' pour enchaîner plusieurs mesures
// sans reperdre l'état déjà en place.
const PREPARER = process.env.PREPARER !== '0';

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(7);
    console.log(`[${dt}s ${new Date().toISOString()}] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- VM
//
// Exécute un script PowerShell dans la SESSION INTERACTIVE de la VM (session
// 1), par tâche planifiée /IT — même mécanisme que `scripts/run-agent.sh` et
// que `vm-it.sh` de D6. Réimplémenté ICI (plutôt que référencé depuis le
// répertoire d'un autre sous-bloc) pour que cette tâche ne livre que les deux
// fichiers prescrits, sans dépendance croisée entre répertoires de sous-bloc.
async function vmIt(nom, ps) {
    await writeFile(`/media/vm/dev/it-${nom}.ps1`, ps);
    const userName = process.env.WINDOWS_ADMIN_USERNAME ?? 'Administrateur';
    const cmd = `schtasks /delete /tn it-${nom} /f 2>$null; `
        + `schtasks /create /tn it-${nom} /f /it /ru '${userName}' /rp '${process.env.WINDOWS_ADMIN_PASSWORD}' `
        + `/sc once /st 00:00 `
        + `/tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\it-${nom}.ps1'; `
        + `schtasks /run /tn it-${nom}`;
    const r = spawnSync('node', [join(RACINE, 'scripts/winrm.js'), cmd], { encoding: 'utf8', env: process.env });
    if (r.status !== 0) log(`!! vm-it ${nom} a échoué`, (r.stderr ?? '').slice(0, 400));
    return r;
}

function vmVivante(etiquette) {
    const virsh = spawnSync('virsh', ['domstate', 'Windows'], { encoding: 'utf8' }).stdout?.trim() ?? '?';
    const acces = spawnSync('bash', ['-c', 'ls /media/vm/dev/ton.html >/dev/null 2>&1 && echo OUI || echo NON'],
        { encoding: 'utf8' }).stdout?.trim() ?? '?';
    log(`SURVIE VM (${etiquette}) virsh="${virsh}" acces_partage=${acces}`);
    return { virsh, acces };
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

/// Les marqueurs de fin, dans l'esprit de D6 : un décompte de lignes-clé pour
/// un coup d'œil rapide sur `agent.log`, sans remplacer la lecture détaillée.
async function marqueurs(etiquette) {
    const plat = await journalPlat();
    const compte = (motif) => (plat.match(new RegExp(motif, 'g')) ?? []).length;
    const m = {
        lignes: plat.split('\n').length,
        enfant_lance: compte('enfant lancé'),
        attachee_capteur: compte('fenêtre attachée au capteur'),
        cloture: compte('clôture de session amorcée'),
        audio_active: compte('audio activé'),
        ordre_audio_actif_true: compte('ordre audio applique[^\\n]*actif=true'),
        ordre_audio_actif_false: compte('ordre audio applique[^\\n]*actif=false'),
        erreurs: compte('ERROR'),
    };
    log(`MARQUEURS (${etiquette}) ` + JSON.stringify(m));
    return m;
}

// ---------------------------------------------------------------- CDP
export class Cdp {
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
    // Lecon 3 : JAMAIS d'évaluation non bornée sur une page portant un flux
    // WebRTC actif — `Page.captureScreenshot` comme une relecture ordinaire
    // peuvent ne jamais rendre (constaté en D2, reconstaté en D5).
    async evalBorne(sessionId, expression, ms = 8000, awaitPromise = true) {
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

// L'amorce pose deux choses sur chaque page attachée, AVANT que son propre
// script ne s'exécute (`Page.addScriptToEvaluateOnNewDocument`) et à nouveau
// juste après l'attache (`Runtime.evaluate` direct) — cette redondance est
// nécessaire : `addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page
// ouverte par `window.open` (course perdue contre la création du document,
// éprouvée en D5) :
//
//   1. la capture de `window.__pc` — `expressionReleveFrequence` s'en sert en
//      premier recours (avec un repli sur `<video>.srcObject`, FINDING 4) pour trouver
//      la piste audio reçue. `client/src/webrtc.ts` N'EXPOSE RIEN de global :
//      c'est ce monkey-patch, et lui seul, qui rend la PeerConnection visible
//      au script de recette (même technique qu'en D6, où elle sert `__liens`
//      pour les messages `link` du canal de contrôle) ;
//   2. les overrides de visibilité et de focus, pilotés par le SCÉNARIO
//      (`imposerScenario`) et non par le navigateur — indispensable dès qu'un
//      Chrome sans interface est de la partie (leçon 5).
// Revue de tâche 11, FINDING 7 (mineur) : cette substitution du constructeur
// remplace `window.RTCPeerConnection` par une fonction ordinaire, qui NE PORTE
// PAS les membres STATIQUES de l'original (`generateCertificate` notamment —
// `RTCPeerConnection.prototype` est réassigné juste après, mais les statiques
// ne le sont pas). Inerte aujourd'hui : rien dans `client/src/webrtc.ts`
// n'appelle de membre statique. Silencieux si un futur changement du client en
// dépendait — à surveiller, pas à corriger à vide.
export const AMORCE = `
(() => {
  if (window.__amorceD7) return;
  window.__amorceD7 = true;
  window.__pc = null;
  const N = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;

  window.__cachee = false;
  window.__focalisee = false;
  Object.defineProperty(document, 'hidden', {
    configurable: true, get: () => window.__cachee });
  Object.defineProperty(document, 'visibilityState', {
    configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
  document.hasFocus = () => window.__focalisee;
})();
`;

// Branche un `AnalyserNode` sur la piste audio REÇUE et rend, pour CHAQUE
// fréquence assignée (`hzsAssignes` — toutes les fenêtres en jeu, pas
// seulement celle de la page courante), le niveau du bin qui lui correspond —
// PAS SEULEMENT celui du bin le plus énergique. C'est ce qui rend le critère
// d'isolation MESURABLE au lieu de déclaratif : `bytesReceived` dit que du son
// arrive, jamais LEQUEL.
//
// Revue de tâche 11, FINDING 1 (Critical) — POURQUOI l'argmax seul ne
// suffisait pas, avec l'exemple qui l'a montré : une fenêtre qui entendrait
// SA fréquence en plein ET celle de sa voisine à -10 dB argmaxerait quand même
// sur la sienne, jugerait « OK », alors qu'elle entend audiblement sa voisine.
// L'argmax du brief est CONSERVÉ ci-dessous (verbatim de sa forme, utile en
// diagnostic — `hz`/`db` bruts dans le journal), mais `jugerIsolation` ne s'y
// fie plus SEUL : elle lit `niveaux`, le niveau à CHAQUE fréquence assignée,
// et exige `db(propre) - db(chaque autre) >= MARGE_ISOLATION_DB` en plus de
// `db(propre) - plancher >= MARGE_SIGNAL_DB`. Le calcul du bin par fréquence
// se fait ICI, côté page (elle seule connaît `ctx.sampleRate` au moment de la
// mesure), pour ne pas dupliquer cette conversion côté Node.
//
// `ctx.resume()` + la vérification `ctx.state === 'running'` (leçon 2) sont
// une addition à ce que le brief esquissait : un `AudioContext` fraîchement
// créé démarre `suspended` sans activation utilisateur, et un contexte
// suspendu ne fait tourner NI son graphe NI son analyseur — `getFloatFrequencyData`
// y rendrait un plancher identique à celui d'une vraie isolation réussie.
// Sans ce contrôle, cette fonction ne distinguerait pas « le contexte n'a
// jamais tourné » de « la piste est bien silencieuse », exactement le faux
// négatif que l'étape 3 du brief existe pour révéler par ailleurs — mieux
// vaut le détecter ICI, à la source, que le laisser confondre les deux cas
// en aval. Vérifié activement en local (rapport de tâche 11, essai 3) : sans
// activation possible, `ctx.resume()` ne se résout pas dans le délai borné de
// l'appelant — ce contrôle intercepterait donc réellement ce cas.
//
// Revue de tâche 11, FINDING 2 (Important) — `'aucune piste audio'` est
// INATTEIGNABLE sur ce produit : `client/src/webrtc.ts` appelle
// `pc.addTransceiver('audio', { direction: 'recvonly' })` INCONDITIONNELLEMENT
// (ligne 206), donc un récepteur ET une piste EXISTENT même quand l'agent
// n'envoie jamais un seul paquet. Le cas « pas de piste du tout » ne se
// présentera donc essentiellement jamais ; le cas réel d'une fenêtre
// silencieuse se lira comme `hz` improbable ou un pic quelconque au niveau du
// bruit — indiscernable, SANS AIDE SUPPLÉMENTAIRE, d'« un ton a été reçu et
// mal mesuré ». `piste.muted`, `piste.readyState` et les compteurs
// `inbound-rtp` audio de `pc.getStats()` sont donc relevés en plus, comme
// DISCRIMINANTS annexes — jamais comme preuve d'isolation à eux seuls : c'est
// le seul emploi légitime de `bytesReceived` dans cet instrument.
//
// Revue de tâche 11, FINDING 4 (Important) — la piste ne dépend plus
// SEULEMENT de gagner la course de l'AMORCE contre `window.open`
// (`Page.addScriptToEvaluateOnNewDocument` ne court pas sur une page ouverte
// ainsi, éprouvé en D5) : `client/src/webrtc.ts` place les DEUX pistes reçues
// dans un `MediaStream` UNIQUE affecté à `<video>.srcObject` (ligne ~228-246).
// Cette piste est donc réatteignable, SANS AVOIR À GAGNER AUCUNE COURSE, via
// `document.querySelector('video').srcObject.getAudioTracks()[0]` — un repli
// à deux lignes qui élimine toute une classe d'exécutions VM perdues pour rien
// si l'AMORCE arrivait après coup.
//
// Revue de tâche 11, FINDING 3 (Important) — `ctx.close()` en `finally` :
// laissé ouvert, chaque appel ajoutait un `AudioContext` vivant de plus,
// jusqu'à dix (un par page, sur toutes les pages, à chaque relevé) — ce
// dépôt a déjà DEUX FOIS payé le prix d'un instrument qui perturbe ce qu'il
// mesure (la trace par paquet du chantier TURN, la capture d'écran CDP du
// sous-bloc D1). Fermer le contexte est le geste symétrique de l'ouvrir.
export function expressionReleveFrequence(hzsAssignes) {
    return `(async () => {
  const CIBLES = ${JSON.stringify(hzsAssignes)};
  const SENTINEL = ${SENTINEL_DB};
  const pc = window.__pc;
  let piste = pc ? pc.getReceivers().map(r => r.track).find(t => t && t.kind === 'audio') : null;
  // FINDING 4 : repli qui ne dépend d'aucune course avec l'AMORCE.
  if (!piste) {
    const v = document.querySelector('video');
    piste = v?.srcObject?.getAudioTracks?.()[0] ?? null;
  }
  if (!piste) return { erreur: pc ? 'aucune piste audio' : 'aucune PeerConnection exposee' };
  const ctx = new AudioContext();
  try {
    await ctx.resume();
    if (ctx.state !== 'running') {
      return { erreur: 'AudioContext non demarre etat=' + ctx.state };
    }
    const analyseur = ctx.createAnalyser();
    analyseur.fftSize = 8192;
    ctx.createMediaStreamSource(new MediaStream([piste])).connect(analyseur);
    await new Promise(r => setTimeout(r, 1500));
    const bins = new Float32Array(analyseur.frequencyBinCount);
    analyseur.getFloatFrequencyData(bins);
    // FINDING 6 : -Infinity (silence numerique exact) n'est pas representable
    // en JSON (devient null, indiscernable d'un champ absent) -> clampe sur
    // SENTINEL, une valeur numerique tres en dessous de tout plancher reel.
    const db = (x) => (Number.isFinite(x) ? Math.round(x) : SENTINEL);
    const binDe = (f) => Math.max(0, Math.min(bins.length - 1,
      Math.round(f * analyseur.fftSize / ctx.sampleRate)));
    let meilleur = 0;
    for (let i = 1; i < bins.length; i++) if (bins[i] > bins[meilleur]) meilleur = i;
    // FINDING 6 (suite) : la moyenne du plancher filtre les bins non finis —
    // un seul -Infinity dans un tableau moyenné rend TOUT le plancher
    // -Infinity, précisément quand le spectre est partiellement silencieux et
    // que ce chiffre importe le plus.
    const finis = [...bins].filter(Number.isFinite);
    const plancher_db = finis.length ? Math.round(finis.reduce((a, b) => a + b, 0) / finis.length) : SENTINEL;
    let statsAudio = null;
    if (pc) {
      try {
        const rapport = await pc.getStats();
        const entree = [...rapport.values()].find((x) => x.type === 'inbound-rtp' && x.kind === 'audio');
        if (entree) {
          statsAudio = {
            bytes_recus: entree.bytesReceived ?? null,
            paquets_recus: entree.packetsReceived ?? null,
          };
        }
      } catch { }
    }
    return {
      hz: Math.round(meilleur * ctx.sampleRate / analyseur.fftSize),
      db: db(bins[meilleur]),
      plancher_db,
      sample_rate: ctx.sampleRate,
      niveaux: CIBLES.map((f) => ({ f, db: db(bins[binDe(f)]) })),
      piste_muted: piste.muted,
      piste_ready_state: piste.readyState,
      stats_audio: statsAudio,
    };
  } finally {
    await ctx.close();
  }
})()`;
}

const pages = new Map();
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));
const nomDe = (url) => url.replace(/^.*\?session=/, 'w:');
const rang = (n) => Number(String(n).match(/(\d+)\s*$/)?.[1] ?? 0);
const nomsTries = () => appPages().map(([, p]) => nomDe(p.url)).sort((a, b) => rang(a) - rang(b));

/// Impose le scénario page par page : toutes visibles, UNE SEULE focalisée
/// (index `cibleIndex` dans l'ordre trié des sessions). Leçon 5, verbatim de
/// D6 : la cible passe ELLE AUSSI par `blur` puis `focus`, jamais `focus`
/// seul — une page tout juste ouverte peut s'annoncer focalisée avant que
/// l'amorce n'ait posé l'override (course perdue par
/// `Page.addScriptToEvaluateOnNewDocument` sur une page issue de
/// `window.open`), et la déduplication de `client/src/visibilite.ts` ferait
/// alors disparaître silencieusement le focus qu'on demande.
export async function imposerScenario(cdp, cibleIndex, etiquette) {
    const ordre = appPages();
    const noms = nomsTries();
    const cible = noms[cibleIndex];
    for (const passe of ['blur', 'focus']) {
        for (const [sid, p] of ordre) {
            const veutFocus = passe === 'focus' && nomDe(p.url) === cible;
            if (passe === 'focus' && !veutFocus) continue;
            await cdp.evalBorne(sid, `(() => {
                if (!Object.getOwnPropertyDescriptor(document, 'hidden')) {
                    Object.defineProperty(document, 'hidden', {
                        configurable: true, get: () => window.__cachee });
                    Object.defineProperty(document, 'visibilityState', {
                        configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
                }
                document.hasFocus = () => window.__focalisee;
                window.__cachee = false;
                window.__focalisee = ${veutFocus};
                document.dispatchEvent(new Event('visibilitychange'));
                window.dispatchEvent(new Event(${veutFocus ? "'focus'" : "'blur'"}));
                return document.hidden + '/' + document.hasFocus();
            })()`, 6000, false);
        }
    }
    const etats = [];
    for (const [sid, p] of ordre) {
        const r = await cdp.evalBorne(sid, `document.hidden + '/' + document.hasFocus()`, 6000, false);
        etats.push([nomDe(p.url), r]);
    }
    const focalisees = etats.filter(([, v]) => String(v).endsWith('/true')).map(([k]) => k);
    log(`SCÉNARIO IMPOSÉ (${etiquette}) cible=${cible} hidden/hasFocus=` + JSON.stringify(Object.fromEntries(etats)));
    log(`  → focalisées effectives : ${JSON.stringify(focalisees)} (une seule attendue)`);
    return { cible, etats: Object.fromEntries(etats), focalisees };
}

/// Relève le niveau à CHAQUE fréquence assignée sur toutes les pages
/// d'application, bornée page par page (leçon 3). `hzsAssignes` doit couvrir
/// TOUTES les fenêtres réellement ouvertes (pas seulement celle qu'on lit) :
/// c'est ce qui permet à `jugerIsolation` de comparer le niveau propre de
/// chaque page au niveau de CHAQUE autre fréquence en jeu.
async function frequencesToutes(cdp, etiquette, hzsAssignes) {
    const entrees = appPages();
    const expression = expressionReleveFrequence(hzsAssignes);
    const resultats = await Promise.all(entrees.map(async ([sid, p]) =>
        [nomDe(p.url), await cdp.evalBorne(sid, expression)]));
    const out = Object.fromEntries(resultats);
    log(`FRÉQUENCES (${etiquette}) ` + JSON.stringify(out));
    return out;
}

/// Le critère ① : chaque fenêtre entend-elle SA fréquence assignée, ET RIEN
/// QUE ELLE — pas seulement « SA fréquence est-elle la plus forte » ?
/// `assignation` associe un nom de session (`w:N`) à son hz, VÉRIFIÉ par
/// `verifierAssignationParFichier` (FINDING 5, round 3) plutôt que supposé de l'ordre
/// d'ouverture.
///
/// Revue de tâche 11, FINDING 1 (Critical) : un simple argmax passerait à
/// tort une fuite PARTIELLE (sa fréquence en plein, une autre à -10 dB) —
/// les deux argmaxeraient sur leur propre ton. Le verdict exige donc DEUX
/// marges lues sur `niveaux` (voir `expressionReleveFrequence`), toutes deux
/// NON CALIBRÉES (voir leur définition avec `MARGE_ISOLATION_DB` /
/// `MARGE_SIGNAL_DB` plus haut) :
///   - `ecart_isolation_db = db(propre) - max(db(chaque autre))` doit
///     dépasser `MARGE_ISOLATION_DB` ;
///   - `ecart_signal_db = db(propre) - plancher_db` doit dépasser
///     `MARGE_SIGNAL_DB`.
/// L'argmax du brief (`argmax_hz`) est conservé en DIAGNOSTIC seulement — il
/// ne décide plus seul du verdict.
export function jugerIsolation(freqs, assignation) {
    const jugements = {};
    let toutesBonnes = true;
    for (const [nom, attendu] of Object.entries(assignation)) {
        const r = freqs[nom];
        if (!r || !Array.isArray(r.niveaux)) {
            jugements[nom] = { attendu, mesure: r, verdict: 'ERREUR (pas de niveaux)' };
            toutesBonnes = false;
            continue;
        }
        // Revue de tâche 11, FINDING 8 (Important, round 2) — `plancher_db`
        // MANQUANT ou mal formé n'est PAS substitué par `SENTINEL_DB` : ce
        // serait l'inverse du fail-closed voulu. `SENTINEL_DB` signifie
        // « silence mesuré » partout ailleurs dans ce fichier (une vraie
        // lecture, très basse) ; l'employer ici pour un champ ABSENT ferait
        // `ecartSignalDb = niveauPropre - (-1000)`, qui franchit
        // `MARGE_SIGNAL_DB` pour PRESQUE N'IMPORTE QUELLE valeur — une mesure
        // de plancher manquante se lirait comme un succès facile de la marge
        // de signal, exactement la classe de défaut que ce correctif entier
        // existe pour éliminer. Une valeur manquante doit rester une ERREUR,
        // jamais un nombre qui décide la porte.
        if (typeof r.plancher_db !== 'number') {
            jugements[nom] = { attendu, mesure: r, verdict: 'ERREUR (plancher_db absent ou invalide)' };
            toutesBonnes = false;
            continue;
        }
        // `niveauPropre ?? SENTINEL_DB` (ligne suivante) pointe dans l'autre
        // sens EXPRÈS : une fréquence propre absente de `niveaux` doit rendre
        // l'écart très NÉGATIF (donc `ok = false`), pas l'ouvrir. Les deux
        // fallbacks de cette fonction doivent donc TOUJOURS fermer la porte,
        // jamais l'ouvrir — c'est pour ça que `plancher_db` manquant est une
        // ERREUR explicite ci-dessus plutôt qu'un `?? SENTINEL_DB` par
        // symétrie avec `niveauPropre` : la même substitution aurait ici
        // l'effet contraire de celui recherché, selon qu'elle apparaît côté
        // MINUEND ou côté SOUSTRAIT d'une soustraction.
        const niveauPropre = r.niveaux.find((x) => x.f === attendu)?.db ?? SENTINEL_DB;
        const autres = r.niveaux.filter((x) => x.f !== attendu);
        const pireAutreDb = autres.length ? Math.max(...autres.map((x) => x.db)) : SENTINEL_DB;
        const ecartIsolationDb = niveauPropre - pireAutreDb;
        const ecartSignalDb = niveauPropre - r.plancher_db;
        const ok = ecartIsolationDb >= MARGE_ISOLATION_DB && ecartSignalDb >= MARGE_SIGNAL_DB;
        if (!ok) toutesBonnes = false;
        jugements[nom] = {
            attendu,
            niveau_propre_db: niveauPropre,
            pire_autre_db: pireAutreDb,
            plancher_db: r.plancher_db,
            ecart_isolation_db: ecartIsolationDb,
            ecart_signal_db: ecartSignalDb,
            // Diagnostic seulement — voir le commentaire de la fonction.
            argmax_hz: r.hz, argmax_db: r.db,
            argmax_correspond: typeof r.hz === 'number' && Math.abs(r.hz - attendu) <= TOLERANCE_HZ,
            // Discriminants annexes (FINDING 2) — jamais une preuve à eux seuls.
            piste_muted: r.piste_muted, piste_ready_state: r.piste_ready_state, stats_audio: r.stats_audio,
            verdict: ok ? 'OK' : 'FAUX',
        };
    }
    return { toutesBonnes, jugements };
}

/// Revue de tâche 11, FINDING 5 (Important) puis FINDING 9 (round 3) —
/// confronte l'assignation SUPPOSÉE (construite par l'ordre d'ouverture, dans
/// `ouvrirFenetreTon`) à ce que le produit affirme avoir réellement capté.
///
/// CE QUI A ÉTÉ MESURÉ, PAS SUPPOSÉ (round 3, recette du 4 août 2026 sur la
/// VM) : la légende que le superviseur relaie pour une fenêtre Chrome `--app`
/// sur `file://` est le CHEMIN DE FICHIER (observé : `"_/C:/dev/ton.html"`),
/// **pas** `document.title` — et la chaîne de requête (`?hz=…`) n'y figure
/// pas, donc deux fenêtres ouvertes sur le MÊME fichier portent la MÊME
/// légende, mot pour mot. Le mécanisme du round 1 (comparer `document.title`
/// posé par `ton.html` au texte affiché par la page-shell) était donc
/// TOUJOURS FAUX, y compris sur une mesure d'isolation à 85 et 94 dB de
/// marge — un garde-fou qui ne peut jamais être vert est pire qu'aucun
/// garde-fou. Ce relevé ne distingue pas SI Chrome ne propage simplement pas
/// `document.title` à la légende d'une fenêtre `--app`, ou si le superviseur
/// échantillonne la légende à la détection de la fenêtre, avant que le script
/// de la page ne s'exécute — les deux mécanismes rendraient le même symptôme,
/// et rien ici ne permet de trancher.
///
/// LE REMÈDE : l'identité vit maintenant dans le NOM DE FICHIER, pas la
/// chaîne de requête ni le titre — `ouvrirFenetreTon` copie `ton.html` vers
/// un fichier PROPRE À CHAQUE FRÉQUENCE (`ton-440.html`, `ton-880.html`, …)
/// et ouvre CE fichier. La légende observée diffère alors nécessairement
/// d'une fenêtre à l'autre, et PORTE la fréquence.
///
/// Le contrôle n'est plus POSITIONNEL (l'ordre des `<li>` n'a jamais été
/// vérifié non plus, et une fenêtre « éligible » supplémentaire — D4, Paint
/// en ouvre deux — pourrait encore le décaler) : pour chaque fréquence
/// assignée, on vérifie qu'AU MOINS UNE légende observée CONTIENT
/// `ton-${hz}.html`, et qu'AUCUNE légende n'est partagée entre deux
/// fréquences différentes — c'est ce second contrôle qui aurait détecté le
/// défaut de ce round-ci (deux légendes rigoureusement identiques pour deux
/// fenêtres distinctes). Une containance par fenêtre ne dépend d'aucun ordre.
export async function verifierAssignationParFichier(cdp, sidShell, assignation) {
    const titresObserves = await cdp.evalBorne(sidShell,
        `[...document.querySelectorAll('#fenetres li')].map(li => li.textContent.replace(/ — (ouverte|fermée) $/, ''))`,
        6000, false);
    const observes = Array.isArray(titresObserves) ? titresObserves : [];
    const jetons = Object.fromEntries(
        Object.entries(assignation).map(([nom, hz]) => [nom, `ton-${hz}.html`]));
    const correspondances = {};
    let correspond = observes.length > 0;
    for (const [nom, jeton] of Object.entries(jetons)) {
        const trouvees = observes.filter((t) => t.includes(jeton));
        correspondances[nom] = { jeton, occurrences: trouvees.length };
        if (trouvees.length < 1) correspond = false;
    }
    // Aucune légende ne doit être PARTAGÉE entre deux fenêtres : c'est
    // exactement le défaut observé ce round-ci (deux légendes identiques
    // pour deux fréquences différentes). Note : si `assignation` assignait
    // deux fois la MÊME fréquence à deux fenêtres (non exercé, HZS par
    // défaut est toujours composé de valeurs distinctes), ce contrôle serait
    // ATTENDU de refuser une légende partagée qui serait pourtant légitime —
    // cas dégénéré non traité ici.
    const legendesUniques = new Set(observes);
    if (legendesUniques.size < observes.length) correspond = false;
    log(`VÉRIFICATION ASSIGNATION (légendes shell, par fichier) attendues=${JSON.stringify(jetons)} `
        + `observees=${JSON.stringify(observes)} correspondances=${JSON.stringify(correspondances)} `
        + `correspond=${correspond}`);
    if (!correspond) {
        log("  !! DÉSYNCHRONISATION assignation<->légende : l'assignation fenêtre/fréquence "
            + "N'EST PAS CONFIRMÉE par le produit (légende partagée, ou fréquence introuvable "
            + "dans aucune légende observée)");
    }
    return { correspond, jetons, titresObserves: observes };
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9994);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d7r-'));
    // Sept leçons de D1..D6, toutes présentes dans ces drapeaux :
    //  - popup-blocking désactivé (7) ;
    //  - les trois drapeaux anti-gel (6) ;
    //  - autoplay sans geste, pour que les `AudioContext` créés par
    //    `expressionReleveFrequence` PUISSENT passer à `running` sur
    //    `ctx.resume()` (sans quoi la vérification de la leçon 2 échouerait
    //    toujours, y compris sur une isolation par ailleurs correcte).
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns', '--disable-popup-blocking',
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', 'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} HZS=${JSON.stringify(HZS)} GAINS=${JSON.stringify(GAINS)} `
        + `DELAI_STABILISATION_S=${DELAI_STABILISATION_S} chrome pid=${chrome.pid}`);

    const releve = { etiquette: ETIQUETTE, hz: HZS, gains: HZS.map((_, i) => gainDe(i)), phases: {} };
    try {
        const version = await attendreDevtools(port);
        const qui = spawnSync('bash', ['-c',
            `ss -ltnp 2>/dev/null | grep ':${port} ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1`],
            { encoding: 'utf8' }).stdout.trim();
        log(`identité du navigateur : pid écoutant=${qui} pid lancé=${chrome.pid} version=${version.Browser}`);
        if (String(qui) !== String(chrome.pid)) throw new Error(`port ${port} tenu par ${qui}, pas ${chrome.pid}`);
        releve.navigateur = version.Browser;

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
                await cdp.send('Runtime.evaluate', { expression: AMORCE, returnByValue: true }, sessionId).catch(() => { });
                if (VIEWPORT_FORCE) {
                    const [L, H] = VIEWPORT_FORCE.split('x').map(Number);
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: L, height: H, deviceScaleFactor: 1, mobile: false }, sessionId).catch(() => { });
                }
                await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
            } else if (m.method === 'Target.detachedFromTarget') {
                pages.delete(m.params.sessionId);
            } else if (m.method === 'Target.targetInfoChanged') {
                for (const [, p] of pages) {
                    if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
                }
            }
        });
        await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
        await cdp.send('Target.setDiscoverTargets', { discover: true });

        // `ton.html` doit être présent sur `C:\dev` : le copier depuis l'hôte
        // (le montage CIFS `/media/vm` EST `C:\dev`) le rend inutile de s'en
        // souvenir comme étape manuelle séparée avant la tâche 13.
        spawnSync('bash', ['-c', `cp ${join(AIDE, 'ton.html')} /media/vm/dev/ton.html`]);
        log('ton.html copié vers /media/vm/dev/ton.html');

        if (PREPARER) {
            log('>>> préparation VM : purge des processus et sorties résiduels');
            await vmIt('preparerd7', [
                `Get-Process notepad, mspaint, wordpad, chrome, agent -ErrorAction SilentlyContinue | Stop-Process -Force`,
                `Start-Sleep -Seconds 3`,
                `Get-ChildItem 'C:\\dev' -Directory -Filter 'chrome-d7-*' -ErrorAction SilentlyContinue |`,
                `  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue`,
            ].join('\n'));
            // Les sorties virtuelles survivent à un `Stop-Process -Force`
            // (constaté en D5) : purger avant de lancer, sinon le vivier
            // démarre déjà entamé.
            spawnSync('bash', ['-c',
                `cd ${RACINE} && MULTIFENETRE_VDD_PURGE=1 RUST_LOG=info scripts/run-agent.sh`],
                { encoding: 'utf8', env: process.env });
            await dodo(4000);
        }
        releve.survie_avant = vmVivante('avant lancement');

        await cdp.send('Target.createTarget', { url: URL_SHELL });
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut')?.textContent`));

        log('>>> lancement du superviseur');
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 `
            + `RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        await dodo(6000);

        const assignation = {};
        const ouvrirFenetreTon = async (n, hz, gain) => {
            const avant = appPages().length;
            // FINDING 9 (round 3) : la légende de fenêtre relayée par le
            // superviseur pour une fenêtre Chrome `--app` sur `file://` est
            // le CHEMIN DE FICHIER, pas `document.title` — mesuré sur la VM,
            // pas supposé (voir `verifierAssignationParFichier`). L'identité
            // par fréquence doit donc vivre dans le NOM DE FICHIER : chaque
            // fréquence obtient sa propre copie de `ton.html`. La chaîne de
            // requête (`?hz=…&gain=…`) reste nécessaire par ailleurs : c'est
            // elle que lit le script de la page pour choisir le ton à jouer
            // — le nom de fichier ne sert qu'à l'identité observable
            // côté produit, pas à paramétrer `ton.html`.
            const fichier = `ton-${hz}.html`;
            spawnSync('bash', ['-c', `cp ${join(AIDE, 'ton.html')} /media/vm/dev/${fichier}`]);
            log(`  · ouverture fenêtre ${n} (${fichier}?hz=${hz}&gain=${gain})`);
            await vmIt(`ouvrird7-${n}`, [
                `$a = @(`,
                `  "--app=file:///C:/dev/${fichier}?hz=${hz}&gain=${gain}",`,
                `  "--user-data-dir=C:\\dev\\chrome-d7-${n}",`,
                `  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',`,
                `  '--window-size=1280,720','--window-position=${30 + n * 12},${30 + n * 12}',`,
                `  '--disable-features=CalculateNativeWinOcclusion',`,
                // Sans ce drapeau, l'oscillateur de `ton.html` resterait
                // `suspended` faute de tout geste utilisateur possible sur
                // une fenêtre lancée par tâche planifiée — la source elle-même
                // ne jouerait jamais rien, quelle que soit la qualité de la
                // capture en aval.
                `  '--autoplay-policy=no-user-gesture-required',`,
                `  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',`,
                `  '--disable-renderer-backgrounding')`,
                `Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a`,
                `Start-Sleep -Seconds 2`,
            ].join('\n'));
            for (let i = 0; i < 30; i += 1) {
                await dodo(2000);
                if (appPages().length > avant) {
                    const nouveau = nomsTries().find((nom) => !(nom in assignation));
                    if (nouveau) assignation[nouveau] = hz;
                    return true;
                }
            }
            log(`  !! fenêtre ${n} : aucune page de plus après 60 s`);
            return false;
        };

        log(`>>> OUVERTURE — ${HZS.length} fenêtre(s) : ${JSON.stringify(HZS)}`);
        let ouvertes = 0;
        for (let n = 0; n < HZS.length; n += 1) {
            if (!(await ouvrirFenetreTon(n + 1, HZS[n], gainDe(n)))) break;
            ouvertes += 1;
        }
        log(`  ${ouvertes}/${HZS.length} fenêtres ouvertes ; assignation=` + JSON.stringify(assignation));

        // FINDING 5 / FINDING 9 : l'assignation ci-dessus est une HYPOTHÈSE
        // (construite de l'ordre d'ouverture) — on la confronte à ce que le
        // produit affirme avant de s'en servir pour juger l'isolation.
        const verifAssignation = await verifierAssignationParFichier(cdp, sidShell, assignation);

        const scenario = await imposerScenario(cdp, INDEX_FOCUS, 'critère ①');
        log(`>>> stabilisation ${DELAI_STABILISATION_S} s avant lecture`);
        await dodo(DELAI_STABILISATION_S * 1000);

        const freqs = await frequencesToutes(cdp, 'critère ① — isolation', Object.values(assignation));
        const jugement = jugerIsolation(freqs, assignation);
        // Le verdict global n'est valable QUE si l'assignation elle-même est
        // confirmée (FINDING 5) : un jugement "OK" sur une assignation
        // désynchronisée ne prouverait rien.
        const toutesBonnesEtAssignationSure = jugement.toutesBonnes && verifAssignation.correspond;
        log(`CRITÈRE ① — ISOLATION toutesBonnes=${jugement.toutesBonnes} `
            + `assignation_confirmee=${verifAssignation.correspond} `
            + `verdict_final=${toutesBonnesEtAssignationSure} ` + JSON.stringify(jugement.jugements));

        releve.phases.isolation = {
            ouvertes, assignation, verifAssignation, scenario, frequences: freqs, jugement,
            verdict_final: toutesBonnesEtAssignationSure,
        };
        releve.survie_apres = vmVivante('après critère ①');
        releve.marqueurs = await marqueurs('FIN');
        await writeFile(SORTIE_JSON, JSON.stringify(releve, null, 1));
        log('relevé écrit dans ' + SORTIE_JSON);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
        // Copier le journal APRÈS la fermeture réelle du navigateur, pas
        // seulement après l'arrêt du pilote : les enfants ne se terminent
        // qu'à la fermeture de leur page, et leurs dernières lignes
        // partiraient sinon avec le journal suivant (leçon D4/D6).
        await dodo(8000);
        copierLog();
        log('journal copié dans ' + COPIE_LOG + ' (après la fermeture du navigateur)');
    }
}

// N'exécute `main()` que si ce fichier est le point d'entrée du process —
// permet à la tâche 13 d'importer `Cdp`, `expressionReleveFrequence`,
// `AMORCE` et `imposerScenario` sans relancer une recette complète.
if (import.meta.url === `file://${process.argv[1]}`) {
    main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
}
