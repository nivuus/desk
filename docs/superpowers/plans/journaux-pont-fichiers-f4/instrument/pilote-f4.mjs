// Pilote de la recette F4 : monte le lecteur, peuple un GABARIT dans OPFS, fait
// mesurer la VM par un chronometre, et releve le differentiel de l'histogramme
// du pont autour de chaque geste.
//
//     node pilote-f4.mjs <fichier-json-de-sortie>
//
// 🔴 IL REUTILISE `commun-f1.mjs` **ET** `injection-f2.js` PAR LECTURE DE LEUR
// FICHIER D'ORIGINE, jamais par copie : « une copie eprouverait la copie, pas
// l'instrument » (F3). `injection-f4.js` est CONCATENE apres celle de F2, il ne
// la remplace pas.
//
// 🔴 DEUX INSTRUMENTS, ET LEURS ROLES NE SONT PAS INTERCHANGEABLES (plan §0.5) :
//   - le chronometre de la VM ARBITRE — c'est lui qui repond a la question du
//     cadrage, parce que c'est lui qui mesure ce qu'une application ATTEND ;
//   - l'histogramme du pont DECOMPOSE — il repond a « ou le temps passe », et
//     il ne voit que la traversee pont -> navigateur -> pont.
//   Leur DIFFERENCE est un residu NOMME, jamais une grandeur mesuree : c'est
//   une soustraction entre deux horloges sur deux machines.
//
// ⚠️ AUCUN appel synchrone bloquant ici, sauf le lancement de l'agent : un
// `execFileSync` bloque la boucle d'evenements de Node, donc la lecture de la
// WebSocket CDP, DONC LE RENDU DE LA PAGE-SHELL (F2 : vingt `commande expirée`).
// ⚠️ AUCUNE capture d'ecran CDP pendant une mesure (D1).
// ⚠️ AUCUN parcours de la racine par l'instrument (`etat.rs:307-323`).

import fs from 'node:fs';
import { execFile, execFileSync } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-pont-fichiers/instrument/commun-f1.mjs';

const INSTRUMENT_F2 = '../../journaux-pont-fichiers-f2/instrument';
const ICI = path.dirname(fileURLToPath(import.meta.url));
const SORTIE = process.argv[2] ?? '/tmp/f4/pilote-f4.json';

const PLATEFORME_URL = process.env.PLATEFORME_URL ?? 'http://127.0.0.1:8080';
const CLIENT_URL = process.env.CLIENT_URL ?? 'http://127.0.0.1:5173';
const SIGNALING = process.env.SIGNALING_WS ?? 'ws://192.168.3.1:8080';
// 🔴 CHANTIER auth-pomerium : le relais a quitté `/` pour `/signal`.
// `?signaling=` est EXPLICITE côté client
// (client/src/adresse-plateforme.ts::adresseSignaling) et NE REÇOIT AUCUN
// AJOUT — SIGNALING reste la BASE, c'est ce pilote qui fournit l'URL du
// relais à la page.
const SIGNALING_RELAIS = `${SIGNALING.replace(/\/+$/, '')}/signal`;
const BASE_JEU = process.env.BASE_JEU ?? 'http://127.0.0.1:5399';
const PREFIXE = process.env.PREFIXE_VM;
const PORT_CDP = Number(process.env.PORT_CDP ?? 9460);
const UDD = process.env.UDD ?? '/tmp/f4/udd';
const AGENT_LOG = process.env.AGENT_LOG ?? '/media/vm/dev/agent.log';
const REPOS_MESURE = process.env.REPOS_MESURE ?? '25';

if (!PREFIXE) throw new Error('PREFIXE_VM est obligatoire');

const execFileAsync = promisify(execFile);
const journal = [];
const dire = (m) => { const l = `[${new Date().toISOString()}] ${m}`; journal.push(l); console.log(l); };

/**
 * 🔴 `evalBorne` REND UN OBJET quand elle expire (`{__timeout}`) ou echoue
 * (`{__erreur}`), jamais une chaine : un `JSON.parse` pose dessus LEVE, et cela
 * a tue le pilote de `reprise-1` a son 51e echantillon alors que le produit
 * fonctionnait.
 *
 * ⚠️ L'echantillon illisible est CONSERVE tel quel plutot que saute : un trou
 * silencieux dans une serie se lit comme une serie continue.
 */
async function lire(cdp, session, expr, ms = 20000, attendre = true) {
    const brut = await cdp.evalBorne(session, expr, ms, attendre);
    if (typeof brut === 'string') {
        try { return JSON.parse(brut); } catch { return { illisible: brut.slice(0, 300) }; }
    }
    return { illisible: JSON.stringify(brut).slice(0, 300) };
}

async function obtenirPaire() {
    const r = await fetch(`${PLATEFORME_URL}/auth/connexion`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email: process.env.RECETTE_EMAIL, motdepasse: process.env.RECETTE_MOTDEPASSE }),
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`/auth/connexion a rendu ${r.status} (${corps?.refus ?? 'sans motif'})`);
    return corps;
}

const resultat = { prefixe: PREFIXE, journal, erreurs: [], gabarits: [], mesures: [] };
let chrome;
try {
    const paire = await obtenirPaire();
    dire('jeton de recette obtenu');

    // 🔴 L'INJECTION DE F2 EST LUE DEPUIS SON REPERTOIRE D'ORIGINE, PUIS CELLE
    // DE F4 LUI EST CONCATENEE. Deux fichiers, une seule source pour chacun.
    const injection = [
        fs.readFileSync(path.join(ICI, INSTRUMENT_F2, 'injection-f2.js'), 'utf8'),
        fs.readFileSync(path.join(ICI, 'injection-f4.js'), 'utf8'),
    ].join('\n')
        .replaceAll('__JETON_ACCES__', paire.acces)
        .replaceAll('__JETON_RAFRAICHISSEMENT__', paire.rafraichissement)
        .replaceAll('__PREFIXE__', PREFIXE)
        .replaceAll('__BASE_JEU__', BASE_JEU);
    const restants = ['__JETON_ACCES__', '__JETON_RAFRAICHISSEMENT__', '__PREFIXE__', '__BASE_JEU__']
        .filter((m) => injection.includes(m));
    if (restants.length > 0) throw new Error(`marqueurs non substitues : ${restants.join(', ')}`);
    if (String(paire.acces).split('.').length !== 3) throw new Error('le jeton n a pas la forme d un JWT');

    fs.rmSync(UDD, { recursive: true, force: true });
    chrome = lancerChrome(PORT_CDP, UDD);
    const ver = await attendreDevtools(PORT_CDP);
    dire(`chrome : ${ver.Browser}`);
    resultat.chrome = ver.Browser;

    const cdp = new Cdp(ver.webSocketDebuggerUrl);
    const sessions = new Map();
    cdp.on(async (m) => {
        if (m.method !== 'Target.attachedToTarget') return;
        const { sessionId, targetInfo } = m.params;
        sessions.set(targetInfo.targetId, sessionId);
        try {
            if (targetInfo.type === 'page') {
                await cdp.send('Page.enable', {}, sessionId);
                await cdp.send('Runtime.enable', {}, sessionId);
                await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: injection }, sessionId);
            }
        } catch (e) { /* cible deja partie */ }
        // 🔴 TOUJOURS : « une cible laissee en attente de debogueur ne charge JAMAIS ».
        try { await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId); } catch (e) { /* idem */ }
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });

    const cible = await cdp.send('Target.createTarget', { url: 'about:blank' });
    let sessionShell;
    for (let i = 0; i < 80; i += 1) {
        sessionShell = sessions.get(cible.targetId);
        if (sessionShell) break;
        await dodo(100);
    }
    if (!sessionShell) throw new Error('aucune session CDP pour la page-shell');
    // 🔴 RE-POSER L'INJECTION **ET L'ATTENDRE** AVANT DE NAVIGUER : le
    // gestionnaire d'auto-attache est ASYNCHRONE, et sans jeton `shell-page.ts`
    // se redirige — le symptome est `#etat-fichiers` a `null`, qui SE LIT COMME
    // UN LECTEUR NON MONTE (F2, execution `arme-2` perdue).
    await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: injection }, sessionShell);

    // 🔴 LA CONSOLE DE LA PAGE-SHELL EST CAPTUREE, ET C'EST LA PORTE P0 QUI
    // L'EXIGE : `client/src/fichiers/canal.ts:134` ne journalise qu'un
    // `console.warn('trame fichiers non traitée', e)` quand `canal.send()`
    // echoue. Sans cette capture, un REFUS DU NAVIGATEUR serait indiscernable
    // d'un SILENCE DU PRODUIT.
    //
    // ⚠️ `Runtime.consoleAPICalled` rend la chaine « Object » pour tout argument
    // objet (piege S4) : les champs se relevent dans `preview.properties`.
    const console_page = [];
    cdp.on((m) => {
        if (m.method !== 'Runtime.consoleAPICalled' || m.sessionId !== sessionShell) return;
        const args = (m.params.args ?? []).map((a) => {
            if (a.value !== undefined) return String(a.value);
            if (a.preview?.properties) {
                return '{' + a.preview.properties.map((p) => `${p.name}:${p.value}`).join(',') + '}';
            }
            return a.description ?? a.type;
        });
        console_page.push({ t: new Date().toISOString(), niveau: m.params.type, texte: args.join(' ').slice(0, 500) });
        if (console_page.length > 600) console_page.shift();
    });
    resultat.console_page = console_page;

    const url = `${CLIENT_URL}/shell.html?signaling=${encodeURIComponent(SIGNALING_RELAIS)}&prefixe=${encodeURIComponent(PREFIXE)}`;
    dire(`navigation : ${url}`);
    await cdp.send('Page.navigate', { url }, sessionShell);
    await dodo(4000);

    for (let i = 0; i < 120; i += 1) {
        const p = await cdp.evalBorne(sessionShell, `String(window.__f2 && window.__f2.peuple)`, 5000, false);
        if (p && p !== 'undefined' && p !== 'null') { dire(`OPFS peuple : ${p} entrees`); resultat.opfs_entrees = Number(p); break; }
        await dodo(500);
    }

    // 🔴 L'ORDRE : LA SHELL D'ABORD, L'AGENT ENSUITE (D1 : une annonce emise
    // avant la connexion de la shell est PERDUE SANS TRACE).
    if (process.env.APRES_CONNEXION) {
        dire('la page-shell est connectee : lancement de l agent');
        const s = execFileSync('bash', ['-c', process.env.APRES_CONNEXION], { encoding: 'utf8', timeout: 180000 });
        dire(`agent lance : ${s.trim().split('\n').pop()}`);
    }

    const jetonVu = await cdp.evalBorne(sessionShell, `JSON.stringify({ href: location.href, jeton: !!localStorage.getItem('guac.jeton.acces'), bouton: !!document.querySelector('#choisir-dossier') })`, 8000, false);
    dire(`etat avant le clic : ${jetonVu}`);
    resultat.etat_avant_clic = jetonVu;
    if (!String(jetonVu).includes('"jeton":true') || !String(jetonVu).includes('"bouton":true')) {
        throw new Error(`la page-shell n est pas dans l etat attendu : ${jetonVu}`);
    }

    dire('clic sur #choisir-dossier');
    const boite = await cdp.evalBorne(sessionShell, `(() => { const b = document.querySelector('#choisir-dossier'); const r = b.getBoundingClientRect(); return JSON.stringify({ x: Math.round(r.x + r.width/2), y: Math.round(r.y + r.height/2) }); })()`, 5000, false);
    const { x, y } = JSON.parse(boite);
    await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 }, sessionShell);
    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 }, sessionShell);

    let monte = null;
    for (let i = 0; i < 60; i += 1) {
        const t = await cdp.evalBorne(sessionShell, `document.querySelector('#etat-fichiers').textContent`, 5000, false);
        if (t && String(t).trim() !== '') { monte = String(t); break; }
        await dodo(1000);
    }
    dire(`#etat-fichiers : ${JSON.stringify(monte)}`);
    resultat.etat_fichiers = monte;
    // ⚠️ LE TEST PORTE SUR LE SUCCES, PAS SUR LA SOUS-CHAINE « mont » : les DEUX
    // messages de `shell.ts` la contiennent, et F1 a mesure NEUF MINUTES sur un
    // pont NON monte pour l'avoir oublie.
    const bienMonte = typeof monte === 'string' && !monte.includes('n’a pas pu')
        && !monte.includes("n'a pas pu") && /mont[ée]/.test(monte);
    resultat.bien_monte = bienMonte;
    if (!bienMonte) throw new Error(`lecteur NON monte : ${JSON.stringify(monte)}`);

    await dodo(Number(process.env.REPOS_APRES_MONTAGE_MS ?? 8000));

    if (process.env.NEUTRALISER_MOVE === '1') {
        resultat.move_neutralise = await lire(cdp, sessionShell, `window.__neutraliserMove()`, 10000, false);
        dire(`move neutralise : ${JSON.stringify(resultat.move_neutralise)}`);
    }

    // ── LES GABARITS ──────────────────────────────────────────────────────
    // ⚠️ ON ATTEND LE FAIT — le nombre d'entrees REELLEMENT presentes —, jamais
    // une duree : « une mesure lancee sur un gabarit a demi ecrit rendrait un
    // chiffre qui ne veut rien dire ». Le peuplement se chronometre lui-meme.
    for (const spec of (process.env.GABARITS ?? '').split(',').map((s) => s.trim()).filter(Boolean)) {
        const t0 = Date.now();
        let v;
        if (spec.startsWith('listage:')) {
            const n = Number(spec.split(':')[1]);
            v = await lire(cdp, sessionShell, `window.__gabaritListage(${n})`, 600000, true);
            const compte = await lire(cdp, sessionShell, `window.__compteEntrees(${JSON.stringify('listage/' + n)})`, 60000, true);
            v = { ...v, ...compte };
        } else {
            const [, sous, nom, taille] = spec.split(':');
            v = await lire(cdp, sessionShell, `window.__gabaritFichier(${JSON.stringify(sous)}, ${JSON.stringify(nom)}, ${Number(taille)}, 424242)`, 900000, true);
        }
        const releve = { spec, ms_peuplement: Date.now() - t0, ...v };
        resultat.gabarits.push(releve);
        dire(`gabarit ${spec} : ${JSON.stringify(releve)}`);
    }

    // ── LA MESURE COTE VM ─────────────────────────────────────────────────
    // Le PLAN de gestes est passe tel quel au chronometre de la VM. Chaque
    // geste y porte ses `debut_iso`/`fin_iso` (horloge de la VM), et c'est ce
    // qui permet d'attribuer les lignes `traversees` d'`agent.log` — ecrites
    // par la MEME horloge — sans synchroniser quoi que ce soit entre machines.
    if (process.env.PLAN_VM) {
        dire(`mesure VM (ASYNCHRONE) : ${process.env.PLAN_VM}`);
        const t0 = Date.now();
        try {
            const { stdout } = await execFileAsync('bash',
                ['-c', `bash ${ICI}/mesurer-f4.sh ${JSON.stringify(process.env.PLAN_VM)} ${REPOS_MESURE}`],
                { encoding: 'utf8', timeout: 3_600_000, maxBuffer: 64 * 1024 * 1024 });
            resultat.mesure_vm_brut = stdout;
            const ligne = stdout.split('\n').map((l) => l.trim()).filter((l) => l.startsWith('{')).pop();
            resultat.mesure_vm = ligne ? JSON.parse(ligne) : null;
            dire(`mesure VM : ${ligne ? `${resultat.mesure_vm?.releves?.length ?? 0} gestes` : 'AUCUN JSON'} en ${Math.round((Date.now() - t0) / 1000)} s`);
        } catch (e) {
            resultat.mesure_vm_erreur = String(e).slice(0, 3000);
            dire(`mesure VM ECHOUEE : ${String(e).slice(0, 300)}`);
        }
    }

    // ── LE JOURNAL D'AGENT, COPIE PAR LE PILOTE ───────────────────────────
    // ⚠️ Le pilote en prend une copie ICI, tant que la page-shell vit : les
    // lignes de recensement qui bornent le dernier geste y sont deja.
    try {
        resultat.agent_log_octets = fs.statSync(AGENT_LOG).size;
    } catch (e) { resultat.agent_log_octets = null; }

    // ── TEMOIN « la video est intacte » — GRATUIT ICI ─────────────────────
    // ⚠️ IL A DEJA ETE NON MESURABLE UNE FOIS (F2, critere ⑥ : `window.__pc`
    // absent). S'il l'est de nouveau, ON LE DIT ; on ne le remplace pas par un
    // raisonnement.
    const cibles = await (await fetch(`http://127.0.0.1:${PORT_CDP}/json/list`)).json();
    const pages = cibles.filter((c) => c.type === 'page' && c.url.includes('session='));
    resultat.fenetres_application = pages.length;
    resultat.flux = [];
    for (const p of pages) {
        const sid = sessions.get(p.id);
        if (!sid) { resultat.flux.push({ url: p.url.slice(-30), v: { sansSession: true } }); continue; }
        const v = await lire(cdp, sid, `(async () => { const pc = window.__pc; if (!pc) return JSON.stringify({ sansPc: true }); const s = await pc.getStats(); let d = null, l = null; s.forEach((r) => { if (r.type === 'inbound-rtp' && r.kind === 'video') { d = r.framesDecoded; l = r.packetsLost; } }); return JSON.stringify({ framesDecoded: d, packetsLost: l }); })()`, 8000, true);
        resultat.flux.push({ url: p.url.slice(-30), v });
    }
    dire(`fenetres d application : ${pages.length} — flux : ${JSON.stringify(resultat.flux)}`);

    resultat.etat_injection = await lire(cdp, sessionShell, `window.__f4Etat()`, 10000, false);
    resultat.ok = true;
} catch (e) {
    resultat.ok = false;
    resultat.erreurs.push(String(e).slice(0, 1500));
    dire(`ERREUR : ${String(e).slice(0, 600)}`);
} finally {
    fs.mkdirSync(path.dirname(SORTIE), { recursive: true });
    fs.writeFileSync(SORTIE, JSON.stringify(resultat, null, 1));
    dire(`resultat ecrit : ${SORTIE}`);
    if (chrome) chrome.kill();
    await dodo(500);
    process.exit(resultat.ok ? 0 : 1);
}
