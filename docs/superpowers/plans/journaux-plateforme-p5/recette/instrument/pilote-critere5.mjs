// Le pilote du critere ⑤ : un NAVIGATEUR REEL, derriere le VRAI proxy.
//
// 🔴 IL N'EMPLOIE PAS `--ignore-certificate-errors`, QUI RENDRAIT LE CONTROLE
// VACUEUX. Il epingle la seule cle publique du certificat de recette, par
// `--ignore-certificate-errors-spki-list=<SPKI>` : toute AUTRE erreur de
// certificat resterait fatale.
//
// ⚠️ ET IL N'EMPLOIE PAS `--virtual-time-budget`, dont la tache 14 a mesure
// qu'il N'ATTEND PAS UNE E/S RESEAU REELLE (3 « ouvert » et 2 « attente » sur
// 5, a budget 8 s comme a 20 s). Il attend un FAIT — que `window.__p5` porte
// un verdict de WebSocket et un statut de requete — avec une borne de temps.
// Seul « WS-REFUSE » vaut comme preuve de refus ; « attente » est un faux
// negatif d'instrument, jamais un verdict.
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

const url = process.argv[2];
const spki = process.argv[3];
const etiquette = process.argv[4] ?? '(sans etiquette)';
const BORNE_MS = 20000;

const profil = mkdtempSync(path.join(tmpdir(), 'p5-chrome-'));
const port = 9222 + Math.floor(Math.random() * 700);
const chrome = spawn('/usr/bin/google-chrome', [
    '--headless=new',
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${profil}`,
    `--ignore-certificate-errors-spki-list=${spki}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-gpu',
    // ⚠️ `--no-sandbox` EST IMPOSE PAR L'ENVIRONNEMENT, PAS CHOISI : la
    // session tourne en root, et Chrome REFUSE DE DEMARRER sans lui
    // (« Running as root without --no-sandbox is not supported », mesure du
    // 20 aout 2026). Il n'affaiblit rien de ce que ce pilote MESURE — la CSP,
    // le contenu mixte et CORS sont appliques par le moteur de rendu, pas par
    // le bac a sable de processus.
    '--no-sandbox',
    'about:blank',
], { stdio: ['ignore', 'pipe', 'pipe'] });

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

async function cible() {
    for (let i = 0; i < 100; i++) {
        try {
            const r = await fetch(`http://127.0.0.1:${port}/json/list`);
            const l = await r.json();
            const p = l.find((t) => t.type === 'page');
            if (p?.webSocketDebuggerUrl) return p.webSocketDebuggerUrl;
        } catch { /* pas encore la */ }
        await dormir(100);
    }
    throw new Error('CDP injoignable : le navigateur ne peut pas etre pilote — la tache ECHOUE.');
}

const ws = new (await import('ws')).WebSocket(await cible());
await new Promise((r, j) => { ws.once('open', r); ws.once('error', j); });

let id = 0;
const attentes = new Map();
const journalConsole = [];
ws.on('message', (d) => {
    const m = JSON.parse(d.toString());
    if (m.id && attentes.has(m.id)) { attentes.get(m.id)(m); attentes.delete(m.id); return; }
    if (m.method === 'Log.entryAdded') {
        const e = m.params.entry;
        journalConsole.push({ source: e.source, niveau: e.level, texte: e.text });
    }
    if (m.method === 'Runtime.consoleAPICalled') {
        journalConsole.push({ source: 'console', niveau: m.params.type, texte: (m.params.args ?? []).map((a) => a.value ?? a.description).join(' ') });
    }
});
const envoyer = (method, params = {}) => new Promise((r) => { const n = ++id; attentes.set(n, r); ws.send(JSON.stringify({ id: n, method, params })); });

await envoyer('Log.enable');
await envoyer('Runtime.enable');
await envoyer('Page.enable');
await envoyer('Network.enable');
await envoyer('Page.navigate', { url });

let etat = null;
const debut = Date.now();
while (Date.now() - debut < BORNE_MS) {
    await dormir(400);
    const r = await envoyer('Runtime.evaluate', {
        expression: 'JSON.stringify(window.__p5 ?? null)',
        returnByValue: true,
    });
    const v = r.result?.result?.value;
    if (v && v !== 'null') {
        etat = JSON.parse(v);
        if (etat.ws !== 'attente' && etat.fetchStatut !== null) break;
    }
}

const securite = await envoyer('Runtime.evaluate', {
    expression: 'JSON.stringify({ protocole: location.protocol, securise: window.isSecureContext })',
    returnByValue: true,
});

console.log(`--- [${etiquette}] ---`);
console.log(`  url                    : ${url}`);
console.log(`  contexte               : ${securite.result?.result?.value}`);
console.log(`  SCRIPT_OK (le script a tourne) : ${etat ? etat.SCRIPT_OK === true : 'NON — aucun etat, le script n\'a pas tourne'}`);
if (etat) {
    console.log(`  (a) protocole de la page      : ${etat.protocole}`);
    console.log(`  (b) violations CSP dans la page : ${etat.violationsCsp.length}`);
    for (const v of etat.violationsCsp) console.log(`        ${v.directive} bloque ${v.bloque}`);
    console.log(`  (c) montee ${etat.wsUrl} : ${etat.ws}${etat.wsFermeture ? ` (code ${etat.wsFermeture.code})` : ''}`);
    console.log(`  (d) POST /auth/connexion      : statut ${etat.fetchStatut}, corps lisible = ${etat.fetchLisible}`);
    console.log(`      champs du corps (NOMS seuls) : ${JSON.stringify(etat.fetchChamps)}`);
    if (etat.fetchErreur) console.log(`      erreur : ${etat.fetchErreur}`);
}
const csp = journalConsole.filter((e) => /Content Security Policy|Refused to/i.test(e.texte));
console.log(`  journal console — entrees mentionnant la CSP : ${csp.length}`);
for (const e of csp) console.log(`        [${e.source}/${e.niveau}] ${e.texte.slice(0, 220)}`);
console.log(`  journal console — entrees d'erreur : ${journalConsole.filter((e) => e.niveau === 'error').length}`);
for (const e of journalConsole.filter((x) => x.niveau === 'error')) console.log(`        ${e.texte.slice(0, 220)}`);

ws.close();
chrome.kill('SIGKILL');
rmSync(profil, { recursive: true, force: true });
