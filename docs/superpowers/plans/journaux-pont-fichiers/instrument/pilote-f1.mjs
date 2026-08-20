// Pilote de la recette F1 : monte le lecteur « Mes Fichiers » depuis la
// page-shell, puis TIENT LA SESSION pendant que l'operateur mesure cote VM.
//
//     node pilote-f1.mjs <secondes-de-maintien> <fichier-json-de-sortie>
//
// Il ne juge RIEN cote VM : les criteres 1, 2, 3 et le legs de casse se
// relevent en PowerShell sur la VM. Ce pilote etablit la session, echantillonne
// ce que SEUL le navigateur peut voir (l'etat du lecteur, `framesDecoded` des
// fenetres d'application), et rend un JSON.
//
// ⚠️ AUCUNE capture d'ecran CDP pendant la mesure : elle provoque un `Resize`,
// donc un `SHOW`, donc une session de plus (D1).

import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Cdp, attendreDevtools, dodo, lancerChrome, ouvrirOnglet } from './commun-f1.mjs';

const ICI = path.dirname(fileURLToPath(import.meta.url));
const MAINTIEN_S = Number(process.argv[2] ?? 90);
const SORTIE = process.argv[3] ?? '/tmp/pilote-f1.json';

const PLATEFORME_URL = process.env.PLATEFORME_URL ?? 'http://127.0.0.1:8080';
const CLIENT_URL = process.env.CLIENT_URL ?? 'http://127.0.0.1:5173';
const SIGNALING = process.env.SIGNALING_WS ?? 'ws://192.168.3.1:8080';
const BASE_JEU = process.env.BASE_JEU ?? 'http://127.0.0.1:5399';
const PREFIXE = process.env.PREFIXE_VM;
const PORT_CDP = Number(process.env.PORT_CDP ?? 9411);
const UDD = process.env.UDD ?? '/tmp/udd-f1';

if (!PREFIXE) throw new Error('PREFIXE_VM est obligatoire (le prefixe rendu par admin:agent)');

const journal = [];
const dire = (m) => { const l = `[${new Date().toISOString()}] ${m}`; journal.push(l); console.log(l); };

/// Le jeton vient de la plateforme, JAMAIS forge ici (doctrine de
/// `client/recette/jeton-recette.mjs`).
async function obtenirPaire() {
    const r = await fetch(`${PLATEFORME_URL}/auth/connexion`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
            email: process.env.RECETTE_EMAIL,
            motdepasse: process.env.RECETTE_MOTDEPASSE,
        }),
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`/auth/connexion a rendu ${r.status} (${corps?.refus ?? 'sans motif'})`);
    return corps;
}

const resultat = { maintien_s: MAINTIEN_S, prefixe: PREFIXE, journal, etapes: [], erreurs: [] };

let chrome;
try {
    dire('obtention du jeton de recette');
    const paire = await obtenirPaire();

    const injection = fs.readFileSync(path.join(ICI, 'injection-f1.js'), 'utf8')
        .replaceAll('__JETON_ACCES__', paire.acces)
        .replaceAll('__JETON_RAFRAICHISSEMENT__', paire.rafraichissement)
        .replaceAll('__PREFIXE__', PREFIXE)
        .replaceAll('__BASE_JEU__', BASE_JEU);

    // 🔴 GARDE : un marqueur survivant serait seme comme jeton, la page ne
    // redirigerait pas (la chaine est non vide) et le seul symptome serait un
    // `jeton refuse (forme)` cote service. C'est arrive ; ce controle existe
    // pour que cela ne puisse plus etre silencieux.
    const restants = ['__JETON_ACCES__', '__JETON_RAFRAICHISSEMENT__', '__PREFIXE__', '__BASE_JEU__']
        .filter((m) => injection.includes(m));
    if (restants.length > 0) throw new Error(`marqueurs non substitues : ${restants.join(', ')}`);
    if (String(paire.acces).split('.').length !== 3) {
        throw new Error(`le jeton d acces n a pas la forme d un JWT : ${String(paire.acces).slice(0, 40)}`);
    }

    fs.rmSync(UDD, { recursive: true, force: true });
    chrome = lancerChrome(PORT_CDP, UDD);
    const ver = await attendreDevtools(PORT_CDP);
    dire(`chrome : ${ver.Browser}`);
    resultat.chrome = ver.Browser;

    // 🔴 CONNEXION AU NIVEAU NAVIGATEUR, ET AUTO-ATTACHE APLATIE.
    // `Page.addScriptToEvaluateOnNewDocument` pose sur UNE cible NE COURT PAS
    // sur les pages ouvertes par `window.open` (piege mesure au sous-bloc D5) --
    // or les fenetres d'application EN SONT. Sans auto-attache, l'injection
    // n'atteindrait jamais ces pages, `window.__pc` y serait absent, et le
    // critere 4 serait NON MESURABLE tout en ayant l'air simplement negatif.
    const cdp = new Cdp(ver.webSocketDebuggerUrl);
    const sessions = new Map(); // targetId -> sessionId
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
        // Toujours relacher, quoi qu'il arrive : une cible laissee en attente
        // de debogueur ne charge JAMAIS, et la page resterait blanche.
        try { await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId); } catch (e) { /* idem */ }
    });
    await cdp.send('Target.setAutoAttach', {
        autoAttach: true, waitForDebuggerOnStart: true, flatten: true,
    });

    const cible = await cdp.send('Target.createTarget', { url: 'about:blank' });
    // Attendre que l'auto-attache ait rendu la session de CETTE cible.
    let sessionShell;
    for (let i = 0; i < 80; i += 1) {
        sessionShell = sessions.get(cible.targetId);
        if (sessionShell) break;
        await dodo(100);
    }
    if (!sessionShell) throw new Error('aucune session CDP pour la page-shell');
    // 🔴 LE SEUL TEMOIN DE L'ANNONCE `fenetre-ouverte` EST COTE PAGE :
    // `Effet::AnnoncerOuverture` (`superviseur/boucle.rs`) appelle `envoyer`
    // SANS aucun `tracing::`, donc `agent.log` est muet la-dessus. Sans cette
    // capture, une annonce perdue et une annonce jamais emise se lisent
    // exactement pareil.
    const console_page = [];
    cdp.on((m) => {
        if (m.method === 'Runtime.consoleAPICalled' && m.sessionId === sessionShell) {
            const t = (m.params.args ?? []).map((a) => a.value ?? a.description ?? a.type).join(' ');
            console_page.push({ t: Date.now(), niveau: m.params.type, texte: String(t).slice(0, 400) });
            if (console_page.length > 400) console_page.shift();
        }
        if (m.method === 'Runtime.exceptionThrown' && m.sessionId === sessionShell) {
            console_page.push({ t: Date.now(), niveau: 'exception',
                texte: JSON.stringify(m.params.exceptionDetails).slice(0, 400) });
        }
    });
    resultat.console_page = console_page;
    // ⚠️ PAS de `Page.addScriptToEvaluateOnNewDocument` ICI : cette connexion
    // est au niveau NAVIGATEUR, ou le domaine `Page` n'existe pas (`-32601`).
    // L'injection est posee par session, dans le gestionnaire
    // `Target.attachedToTarget` ci-dessus -- c'est ce qui la fait atteindre
    // AUSSI les fenetres ouvertes par `window.open`.

    // La page-shell : 127.0.0.1 est un CONTEXTE SECURISE (OPFS et la FSA
    // l'exigent) ; `?signaling=` est explicite parce que le defaut viserait
    // `ws://127.0.0.1:8080`, alors que l'agent parle a 192.168.3.1.
    const url = `${CLIENT_URL}/shell.html?signaling=${encodeURIComponent(SIGNALING)}&prefixe=${encodeURIComponent(PREFIXE)}`;
    dire(`navigation : ${url}`);
    await cdp.send('Page.navigate', { url }, sessionShell);
    await dodo(4000);

    const etat0 = await cdp.evalBorne(sessionShell, `JSON.stringify({ href: location.href, f1: window.__f1 ? { peuple: window.__f1.peuple, erreurs: window.__f1.erreurs } : null })`, 8000, false);
    dire(`etat initial : ${etat0}`);
    resultat.etat_initial = etat0;

    // Attendre que OPFS soit peuple AVANT de cliquer : le picker surcharge
    // attend de toute facon, mais on veut la trace du peuplement.
    for (let i = 0; i < 120; i += 1) {
        const p = await cdp.evalBorne(sessionShell, `String(window.__f1 && window.__f1.peuple)`, 5000, false);
        if (p && p !== 'undefined' && p !== 'null') { dire(`OPFS peuple : ${p} entrees`); resultat.opfs_entrees = Number(p); break; }
        await dodo(500);
    }

    // 🔴 L'ORDRE EST OBLIGATOIRE, ET IL EST DOCUMENTE DANS `CLAUDE.md` :
    // « le signaling ne memorise que les offres SDP ; les annonces
    // `fenetre-ouverte` emises avant que la page-shell ne soit connectee sont
    // PERDUES SANS TRACE. Lancer le navigateur AVANT le superviseur. » D3 a
    // durci la regle : passe `DELAI_ATTENTE_VIEWPORT_MAX` (30 s), une entree
    // est ABANDONNEE et JAMAIS REPROPOSEE.
    //
    // Un premier essai de cette recette a lance l'agent en premier : zero
    // fenetre d'application, zero session video -- un echec de PROTOCOLE qui
    // se lisait comme une panne du produit.
    if (process.env.APRES_CONNEXION) {
        dire('la page-shell est connectee : lancement de l agent');
        try {
            const sortie = execFileSync('bash', ['-c', process.env.APRES_CONNEXION],
                { encoding: 'utf8', timeout: 180000 });
            dire(`agent lance : ${sortie.trim().split('\n').pop()}`);
        } catch (e) {
            dire(`lancement de l agent ECHOUE : ${String(e).slice(0, 300)}`);
            throw e;
        }
        // Attendre que des fenetres d'application apparaissent : c'est le FAIT
        // qu'on attend, jamais une duree (piege maison D3).
        for (let i = 0; i < 90; i += 1) {
            const cibles = await (await fetch(`http://127.0.0.1:${PORT_CDP}/json/list`)).json();
            const n = cibles.filter((c) => c.type === 'page' && c.url.includes('session=')).length;
            if (n > 0) { dire(`${n} fenetre(s) d application ouverte(s) apres ${i} s`); resultat.fenetres_ouvertes = n; break; }
            await dodo(1000);
        }
        if (!resultat.fenetres_ouvertes) {
            dire('AUCUNE fenetre d application apres 90 s');
            const diag = await cdp.evalBorne(sessionShell, `JSON.stringify({
                statut: (document.querySelector('#statut')||{}).textContent,
                liste: (document.querySelector('#liste')||{}).textContent,
                nbLi: document.querySelectorAll('#liste li').length,
            })`, 5000, false);
            dire(`diagnostic page-shell : ${diag}`);
            resultat.diagnostic_shell = diag;
        }
    }

    // 🔴 LE CLIC. Le gestionnaire est celui du produit ; le picker surcharge
    // n'exige aucune activation, mais on clique quand meme par `Input` plutot
    // que par `.click()` pour rester au plus pres du geste reel.
    dire('clic sur #choisir-dossier');
    const boite = await cdp.evalBorne(sessionShell, `(() => { const b = document.querySelector('#choisir-dossier'); const r = b.getBoundingClientRect(); return JSON.stringify({ x: Math.round(r.x + r.width/2), y: Math.round(r.y + r.height/2) }); })()`, 5000, false);
    const { x, y } = JSON.parse(boite);
    await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 }, sessionShell);
    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 }, sessionShell);

    // Attendre le montage, en lisant le TEXTE de #etat-fichiers.
    let monte = null;
    for (let i = 0; i < 60; i += 1) {
        const t = await cdp.evalBorne(sessionShell, `document.querySelector('#etat-fichiers').textContent`, 5000, false);
        if (t && String(t).trim() !== '') { monte = String(t); break; }
        await dodo(1000);
    }
    dire(`#etat-fichiers : ${JSON.stringify(monte)}`);
    resultat.etat_fichiers = monte;

    const f1 = await cdp.evalBorne(sessionShell, `JSON.stringify(window.__f1)`, 8000, false);
    resultat.trace_page = f1 ? JSON.parse(f1) : null;

    // ── La mesure cote VM, PENDANT que le lecteur est monte ─────────────────
    // Elle DOIT courir ici : la racine ProjFS ne sert rien sans le navigateur
    // qui la nourrit, et une mesure prise apres la fermeture de la page lirait
    // soit un cache, soit un echec, sans qu'on puisse les distinguer.
    // ⚠️ LE TEST PORTE SUR LE SUCCES, PAS SUR LA SOUS-CHAINE « mont ».
    // `shell.ts` ecrit « Lecteur ... monte sur ... » en cas de succes et
    // « Le lecteur ... n'a pas pu etre monte : ... » en cas d'echec : les DEUX
    // contiennent « mont ». Une premiere version testait `includes('mont')` et
    // a donc lance la mesure sur un pont NON MONTE (execution `exec4`), qui a
    // rendu des « introuvable » partout -- indiscernables, pour qui lit le seul
    // fichier de mesure, d'un pont monte mais vide.
    const bienMonte = typeof monte === 'string' && !monte.includes('n’a pas pu')
        && !monte.includes("n'a pas pu") && /mont[ée]/.test(monte);
    if (process.env.PENDANT_MAINTIEN && bienMonte) {
        // ⚠️ REPOS APRES LE MONTAGE, ET IL EST DECLARE.
        // `#etat-fichiers` passe a « monte » quand le canal s'ouvre COTE
        // CLIENT ; le service du pont, lui, s'arme un peu plus tard cote
        // agent. Une execution ou le premier `Get-ChildItem` est tombe 0,4 s
        // apres l'ouverture du canal a rendu une racine VIDE -- et elle l'est
        // RESTEE : ni `lecture complete`, ni `entrees` hydratees, la requete
        // n'a jamais atteint le navigateur. Ce repos ne CORRIGE rien : il
        // ecarte la course du chemin de mesure pour que les criteres 1 et 2
        // mesurent le pont plutot que cette course. La course elle-meme est
        // un constat de la recette, pas un detail d'instrument.
        const repos = Number(process.env.REPOS_APRES_MONTAGE_MS ?? 8000);
        dire(`lecteur monte : repos de ${repos} ms avant la mesure`);
        await dodo(repos);
        dire('mesure cote VM');
        try {
            const sortie = execFileSync('bash', ['-c', process.env.PENDANT_MAINTIEN],
                { encoding: 'utf8', timeout: 600000, maxBuffer: 64 * 1024 * 1024 });
            resultat.mesure_vm = sortie;
            dire(`mesure VM : ${sortie.split('\n').length} lignes`);
        } catch (e) {
            resultat.mesure_vm_erreur = String(e).slice(0, 2000);
            dire(`mesure VM ECHOUEE : ${String(e).slice(0, 300)}`);
        }
    } else if (process.env.PENDANT_MAINTIEN) {
        dire(`lecteur NON monte : la mesure cote VM est SAUTEE (elle n aurait rien a lire) — etat lu : ${JSON.stringify(monte)}`);
    }

    // ── Maintien, et echantillonnage de `framesDecoded` (critere 4) ─────────
    // Les fenetres d'application sont des onglets ouverts par `window.open`.
    const echantillons = [];
    const debut = Date.now();
    let n = 0;
    while ((Date.now() - debut) / 1000 < MAINTIEN_S) {
        await dodo(10000);
        n += 1;
        const cibles = await (await fetch(`http://127.0.0.1:${PORT_CDP}/json/list`)).json();
        const pages = cibles.filter((c) => c.type === 'page' && c.url.includes('session='));
        const point = { t: Math.round((Date.now() - debut) / 1000), fenetres: pages.length, flux: [] };
        for (const p of pages) {
            const sid = sessions.get(p.id);
            if (!sid) { point.flux.push({ url: p.url.slice(-40), v: '{"sansSession":true}' }); continue; }
            const v = await cdp.evalBorne(sid, `(async () => {
                const pc = window.__pc;
                if (!pc) return JSON.stringify({ sansPc: true });
                const s = await pc.getStats();
                let d = null, l = null, oct = null;
                s.forEach((r) => { if (r.type === 'inbound-rtp' && r.kind === 'video') {
                    d = r.framesDecoded; l = r.packetsLost; oct = r.bytesReceived; } });
                return JSON.stringify({ framesDecoded: d, packetsLost: l, octets: oct });
            })()`, 8000, true);
            point.flux.push({ url: p.url.slice(-40), v });
        }
        echantillons.push(point);
        dire(`echantillon ${n} : ${JSON.stringify(point)}`);
    }
    resultat.echantillons = echantillons;

    const finEtat = await cdp.evalBorne(sessionShell, `document.querySelector('#etat-fichiers').textContent`, 5000, false);
    resultat.etat_fichiers_fin = finEtat;
    dire(`#etat-fichiers (fin) : ${JSON.stringify(finEtat)}`);
    resultat.ok = true;
} catch (e) {
    resultat.ok = false;
    resultat.erreurs.push(String(e).slice(0, 1000));
    dire(`ERREUR : ${String(e).slice(0, 500)}`);
} finally {
    fs.writeFileSync(SORTIE, JSON.stringify(resultat, null, 1));
    dire(`resultat ecrit : ${SORTIE}`);
    if (chrome) chrome.kill();
    await dodo(500);
    process.exit(resultat.ok ? 0 : 1);
}
