// Pilote de la recette du bloc E3 : le micro en MULTI-FENÊTRES.
//
//     node pilote-e3.mjs <fichier-json-de-sortie>
//
// 🔴 SON PRÉAMBULE DE MONTAGE EST REPRIS DE `pilote-f5.mjs`, qui le tenait de
// `pilote-f3.mjs`. Il RÉEMPLOIE `commun-f1.mjs` plutôt que de le recopier.
// Cinq pièges y sont déjà payés, et les réécrire les réintroduirait un par un :
// l'injection RE-POSÉE et attendue avant de naviguer (course dont le symptôme
// ne ressemble pas à une course), le jeton vérifié avant tout clic, la SHELL
// AVANT l'agent (le signaling ne mémorise que les offres SDP), les évaluations
// BORNÉES sur toute page portant un flux WebRTC, et aucune capture d'écran CDP
// pendant la mesure (elle provoque un `Resize`, donc une session de plus).
//
// ⚠️ REPLI DE LA DÉCISION 9 DU PLAN, EMPLOYÉ ET DÉCLARÉ : **une seule
// tonalité**, pas deux. `--use-file-for-fake-audio-capture` est un drapeau de
// PROCESSUS, et la page-shell ouvre ses N fenêtres par `window.open` dans SON
// instance — les deux fenêtres portent donc nécessairement le même 440 Hz.
// Le discriminant retombe sur les deux autres pièces que la Décision 9 nomme :
// les lignes de journal à `session=` distincts, et l'état affiché par les deux
// clients. **Le juge sur CABLE Output ne peut pas, lui, distinguer un 440 Hz
// d'un 440 Hz** : il établit que QUELQU'UN est entendu, jamais LEQUEL.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFile, execFileSync } from 'node:child_process';
import { promisify } from 'node:util';
import { Cdp, lancerChrome, attendreDevtools, dodo }
    from '../../journaux-pont-fichiers/instrument/commun-f1.mjs';

const ICI = path.dirname(fileURLToPath(import.meta.url));
const SORTIE = process.argv[2] ?? '/tmp/e3/pilote-e3.json';
const PLATEFORME_URL = process.env.PLATEFORME_URL ?? 'http://127.0.0.1:8080';
const CLIENT_URL = process.env.CLIENT_URL ?? 'http://127.0.0.1:5173';
const SIGNALING = process.env.SIGNALING_WS ?? 'ws://192.168.3.1:8080';
const PREFIXE = process.env.PREFIXE_VM;
const PORT_CDP = Number(process.env.PORT_CDP ?? 9470);
const UDD = process.env.UDD ?? '/tmp/e3/udd';
const WAV = process.env.WAV ?? '/tmp/e3/ton-440.wav';
const RACINE = '/home/mallanic/Projects/Guacamole';

const execFileAsync = promisify(execFile);
const journal = [];
const dire = (m) => { const l = `[${new Date().toISOString()}] ${m}`; journal.push(l); console.log(l); };

// 🔴 LE JUGE EST ASYNCHRONE. `execFileSync` bloquerait la boucle d'événements
// de Node, donc la lecture de la WebSocket CDP, donc le rendu des pages — un
// chantier voisin l'a payé et son journal montrait vingt commandes expirées
// pendant la mesure. « L'instrument détruit ce qu'il mesure », troisième forme.
async function juger(etiquette, secondes) {
    dire(`juge « ${etiquette} » : ${secondes} s sur CABLE Output`);
    try {
        await execFileAsync('node', [`${RACINE}/scripts/winrm.js`,
            `powershell -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\e2-juge.ps1 -Etiquette e3-${etiquette} -Secondes ${secondes}`],
            { encoding: 'utf8', timeout: (secondes + 60) * 1000, cwd: RACINE });
    } catch (e) { dire(`juge « ${etiquette} » : ${String(e).slice(0, 200)}`); }
    try {
        const t = fs.readFileSync(`/media/vm/dev/e2-juge-e3-${etiquette}.log`, 'utf8');
        return t.split('\n').filter((l) => l.trim() !== '').slice(-14).join('\n');
    } catch (e) { return `journal du juge illisible : ${String(e).slice(0, 120)}`; }
}

async function obtenirPaire() {
    const r = await fetch(`${PLATEFORME_URL}/auth/connexion`, {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email: process.env.RECETTE_EMAIL, motdepasse: process.env.RECETTE_MOTDEPASSE }),
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`/auth/connexion a rendu ${r.status} (${corps?.refus ?? 'sans motif'})`);
    return corps;
}

const resultat = { prefixe: PREFIXE, wav: WAV, journal, phases: [], erreurs: [] };
let chrome;
try {
    const paire = await obtenirPaire();
    if (String(paire.acces).split('.').length !== 3) throw new Error("le jeton n a pas la forme d un JWT");
    dire('jeton de recette obtenu');

    const injection = fs.readFileSync(path.join(ICI, 'injection-e3.js'), 'utf8')
        .replaceAll('__JETON_ACCES__', paire.acces)
        .replaceAll('__JETON_RAFRAICHISSEMENT__', paire.rafraichissement)
        .replaceAll('__PREFIXE__', PREFIXE)
        .replaceAll('__SIGNALING__', SIGNALING);
    const restants = ['__JETON_ACCES__', '__JETON_RAFRAICHISSEMENT__', '__PREFIXE__', '__SIGNALING__']
        .filter((m) => injection.includes(m));
    if (restants.length > 0) throw new Error(`marqueurs non substitues : ${restants.join(', ')}`);

    fs.rmSync(UDD, { recursive: true, force: true });
    // 🔴 LE PÉRIPHÉRIQUE FACTICE ALIMENTÉ PAR UN FICHIER, et non un
    // `OscillatorNode` posé sur le `sender` : c'est ce qui exerce le VRAI
    // `getUserMedia`, la permission, et les trois contraintes de `micro.ts`
    // (`echoCancellation`, `noiseSuppression`, `autoGainControl`). Leçon de E2,
    // dont la Décision 6 a écarté le montage de E1 pour cette raison.
    chrome = lancerChrome(PORT_CDP, UDD, [
        '--use-fake-device-for-media-stream',
        '--use-fake-ui-for-media-stream',
        `--use-file-for-fake-audio-capture=${WAV}`,
    ]);
    const ver = await attendreDevtools(PORT_CDP);
    dire(`chrome : ${ver.Browser}`);
    resultat.chrome = ver.Browser;

    const cdp = new Cdp(ver.webSocketDebuggerUrl);
    const sessions = new Map();       // targetId -> sessionId
    const urls = new Map();           // targetId -> url
    cdp.on(async (m) => {
        if (m.method === 'Target.targetInfoChanged') {
            urls.set(m.params.targetInfo.targetId, m.params.targetInfo.url);
            return;
        }
        if (m.method !== 'Target.attachedToTarget') return;
        const { sessionId, targetInfo } = m.params;
        sessions.set(targetInfo.targetId, sessionId);
        urls.set(targetInfo.targetId, targetInfo.url);
        try {
            if (targetInfo.type === 'page') {
                await cdp.send('Page.enable', {}, sessionId);
                await cdp.send('Runtime.enable', {}, sessionId);
                await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: injection }, sessionId);
            }
        } catch (e) { /* cible deja partie */ }
        try { await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId); } catch (e) { /* idem */ }
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });

    const cible = await cdp.send('Target.createTarget', { url: 'about:blank' });
    let sessionShell;
    for (let i = 0; i < 80; i += 1) { sessionShell = sessions.get(cible.targetId); if (sessionShell) break; await dodo(100); }
    if (!sessionShell) throw new Error('aucune session CDP pour la page-shell');
    // L'injection est RE-POSÉE et attendue : le gestionnaire ci-dessus est
    // asynchrone et inscrit la session AVANT d'avoir fini de l'armer.
    await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: injection }, sessionShell);

    // Console de TOUTES les pages, la shell ET les fenêtres d'application.
    // 🔵 C'est le `console.warn` du client qui a dénoncé, chez un chantier
    // voisin, un message parti AVANT l'ouverture du canal. Sans lui, le
    // symptôme aurait été un silence.
    const console_pages = [];
    cdp.on((m) => {
        if (m.method !== 'Runtime.consoleAPICalled') return;
        const t = (m.params.args ?? []).map((a) => a.value ?? a.description ?? a.type).join(' ');
        console_pages.push({ t: Date.now(), sid: m.sessionId, niveau: m.params.type, texte: String(t).slice(0, 300) });
        if (console_pages.length > 600) console_pages.shift();
    });
    resultat.console_pages = console_pages;

    const url = `${CLIENT_URL}/shell.html?signaling=${encodeURIComponent(SIGNALING)}&prefixe=${encodeURIComponent(PREFIXE)}`;
    dire(`navigation : ${url}`);
    await cdp.send('Page.navigate', { url }, sessionShell);
    await dodo(4000);
    const jetonVu = await cdp.evalBorne(sessionShell,
        `JSON.stringify({ href: location.href, jeton: !!localStorage.getItem('guac.jeton.acces') })`, 8000, false);
    dire(`etat de la shell : ${jetonVu}`);
    resultat.etat_shell = jetonVu;
    if (!String(jetonVu).includes('"jeton":true')) throw new Error(`la page-shell n est pas dans l etat attendu : ${jetonVu}`);

    // 🔴 LA SHELL D'ABORD, L'AGENT ENSUITE (D1/D3).
    dire('la page-shell est connectee : lancement de l agent');
    const sortie = execFileSync('bash', ['-c', process.env.APRES_CONNEXION], { encoding: 'utf8', timeout: 240000 });
    dire(`agent lance : ${sortie.trim().split('\n').pop()}`);

    // Attente du FAIT — deux fenêtres d'application —, jamais d'une durée.
    let pages = [];
    for (let i = 0; i < 120; i += 1) {
        const cibles = await (await fetch(`http://127.0.0.1:${PORT_CDP}/json/list`)).json();
        pages = cibles.filter((c) => c.type === 'page' && c.url.includes('session='));
        if (pages.length >= 2) { dire(`${pages.length} fenetre(s) d application apres ${i} s`); break; }
        await dodo(1000);
    }
    resultat.fenetres_ouvertes = pages.length;
    if (pages.length < 2) throw new Error(`seulement ${pages.length} fenetre(s) d application : le livrable ② exige DEUX enfants`);

    // Les deux sessions CDP, appariées à leur nom de session de produit.
    const fen = [];
    for (const p of pages.slice(0, 2)) {
        let sid;
        for (let i = 0; i < 40; i += 1) { sid = sessions.get(p.id ?? p.targetId); if (sid) break; await dodo(100); }
        const nom = decodeURIComponent((p.url.match(/session=([^&]+)/) ?? [, '?'])[1]);
        fen.push({ nom, sid, url: p.url });
    }
    dire(`fenetres : ${fen.map((f) => f.nom).join(' , ')}`);
    resultat.fenetres = fen.map((f) => f.nom);
    if (fen.some((f) => !f.sid)) throw new Error('une fenetre sans session CDP');

    // 🔴 `evalBorne` REND UN OBJET quand elle expire (`{__timeout}`) ou échoue
    // (`{__erreur}`), JAMAIS une chaîne — et un `JSON.parse` posé dessus reçoit
    // « [object Object] », qui n'est pas du JSON. Payé à l'exécution 1, en
    // phase C : la page de l'enfant tué n'existe plus, et le pilote est tombé
    // sur une `SyntaxError` au lieu de RELEVER que la fenêtre a disparu — ce
    // qui est pourtant l'observation attendue à cet instant.
    const lireJson = (v, defaut) => {
        if (typeof v !== 'string') return { ...defaut, injoignable: JSON.stringify(v).slice(0, 200) };
        try { return JSON.parse(v); } catch (e) { return { ...defaut, illisible: String(v).slice(0, 200) }; }
    };
    const etatDe = async (f) => lireJson(await cdp.evalBorne(f.sid,
        `JSON.stringify({
            etat: (document.querySelector('#micro')||{}).dataset && document.querySelector('#micro').dataset.etat || null,
            cache: (document.querySelector('#micro')||{}).hidden,
            titre: (document.querySelector('#micro')||{}).title || null,
            statut: (document.querySelector('#status')||{}).textContent || null,
            statutCache: (document.querySelector('#status')||{}).dataset ? document.querySelector('#status').dataset.hidden : null,
            micState: (window.__e3||{}).micState || [],
            canaux: (window.__e3||{}).canaux
        })`, 8000, false), { etat: null, cache: null, titre: null, statut: null, micState: [], canaux: null });

    const allumer = async (f) => {
        const boite = await cdp.evalBorne(f.sid,
            `(() => { const b = document.querySelector('#micro'); if (!b || b.hidden) return 'null'; const r = b.getBoundingClientRect(); return JSON.stringify({ x: Math.round(r.x + r.width/2), y: Math.round(r.y + r.height/2) }); })()`, 8000, false);
        if (!boite || boite === 'null') throw new Error(`le bouton micro de ${f.nom} est absent ou cache`);
        const { x, y } = JSON.parse(String(boite));
        const t = Date.now();
        await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 }, f.sid);
        await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 }, f.sid);
        dire(`clic micro sur ${f.nom} a ${new Date(t).toISOString()}`);
        return t;
    };

    const relever = async (nom, secondesJuge) => {
        const etats = [];
        for (const f of fen) etats.push({ fenetre: f.nom, ...(await etatDe(f)) });
        const juge = secondesJuge ? await juger(nom, secondesJuge) : null;
        const phase = { phase: nom, t: new Date().toISOString(), etats, juge };
        resultat.phases.push(phase);
        for (const e of etats) {
            dire(`  [${nom}] ${e.fenetre} etat=${e.etat} cache=${e.cache} mic-state=${(e.micState||[]).length} titre=${JSON.stringify(String(e.titre).slice(0,80))}`);
        }
        return phase;
    };

    // 🔴 ATTENDRE LE FAIT — le bouton micro VISIBLE —, jamais une durée.
    //
    // Défaut d'instrument payé sur place à l'essai 1 : le pilote cliquait
    // quatre secondes après l'ouverture des fenêtres, et `#micro` était encore
    // `hidden` avec `dataset.etat` à `null`. **`dataset.etat` à `null` prouve
    // que `attacherBoutonMicro` n'avait pas encore couru** — il est écrit à
    // l'attache —, donc que la session WebRTC n'était pas établie. Ce n'était
    // pas `ready.mic: false` ; les deux se lisent pourtant pareil sur un
    // bouton caché, et c'est `dataset.etat` qui les départage.
    let visibles = 0;
    for (let i = 0; i < 90; i += 1) {
        const etats = [];
        for (const f of fen) etats.push(await etatDe(f));
        visibles = etats.filter((e) => e.cache === false).length;
        if (visibles === fen.length) { dire(`les ${visibles} boutons micro sont visibles apres ${i} s`); break; }
        if (i % 15 === 0 || i === 89) {
            const diag = [];
            for (const f of fen) {
                diag.push(JSON.parse(String(await cdp.evalBorne(f.sid, `JSON.stringify({
                    href: location.href,
                    statut: (document.querySelector('#status')||{}).textContent || null,
                    video: !!document.querySelector('video'),
                    etat: (document.querySelector('#micro')||{}).dataset ? document.querySelector('#micro').dataset.etat : null,
                    cache: (document.querySelector('#micro')||{}).hidden,
                    canaux: (window.__e3||{}).canaux,
                    injecte: !!window.__e3
                })`, 8000, false))));
            }
            dire(`  [attente ${i}s] ${JSON.stringify(diag)}`);
            resultat.diagnostic_bouton = diag;
        }
        await dodo(1000);
    }
    resultat.boutons_visibles = visibles;
    if (visibles < fen.length) throw new Error(`${visibles}/${fen.length} bouton(s) micro visible(s) : ready.mic est faux, ou la session n a pas abouti`);

    // ── Phase 0 : rien n'est allumé. Le témoin NÉGATIF de tout ce qui suit.
    await dodo(2000);
    resultat.t_avant_tout_clic = Date.now();
    await relever('0-repos', 0);

    // ── Phase A : le micro de la PREMIÈRE fenêtre, seul.
    resultat.t_clic_A = await allumer(fen[0]);
    await dodo(8000);
    await relever('A-une-seule', 10);

    // ── Phase B : le micro de la SECONDE. C'est le livrable ②.
    resultat.t_clic_B = await allumer(fen[1]);
    await dodo(8000);
    await relever('B-les-deux', 10);

    // ── Phase C : la REPRISE. On tue l'enfant gagnant, la perdante doit
    //    acquérir ET son client doit être RÉINFORMÉ. C'est R4 sur le chemin réel.
    if (process.env.SANS_REPRISE !== '1') {
        // 🔴 LE GAGNANT EST IDENTIFIÉ PAR LE JOURNAL, ET TUÉ PAR SON PID RELEVÉ.
        //
        // Jamais par `pkill -f` — ce piège est documenté depuis D2 et a été
        // rejoué par un chantier voisin cette semaine : un `pkill -f <motif>`
        // lancé depuis un shell dont la ligne de commande contient le motif tue
        // le shell lui-même. Et jamais par une heuristique de rang de PID :
        // « le plus jeune des agent » est FAUX (superviseur, capteur, pont,
        // puis les enfants), et un chantier a mesuré neuf minutes sur un
        // enfant tué au hasard.
        //
        // La chaîne est : `micro : cable acquis` porte le `session=` du
        // gagnant ; `enfant lancé` porte le couple `session=` / `pid=`.
        const plat = fs.readFileSync('/media/vm/dev/agent.log', 'utf8')
            .replace(/\x1b\[[0-9;]*m/g, '');
        // 🔴 LE GAGNANT SE DÉDUIT DU PERDANT, ET NON L'INVERSE — défaut
        // d'instrument payé à l'essai 5, qui cherchait `micro : cable acquis`.
        // **Cette ligne n'existe PAS pour le premier acquéreur** : `Issue::Accepte`
        // ne journalise rien du tout, seul `AccepteApresRefus` a sa trace. Le
        // gagnant est donc *celui qui n'a pas écrit le refus*.
        //
        // ⚠️ C'est exactement le patron « vérifier qu'un contrôle rend quelque
        // chose sur le VERT » : le motif cherché était introuvable sur un
        // produit qui fonctionnait, et le pilote a déclaré « phase C NON
        // JOUÉE » au lieu d'inventer un PID.
        const pids = {};
        for (const m of plat.matchAll(/enfant lancé session=(\S+) pid=(\d+)/g)) pids[m[1]] = m[2];
        const perdantes = new Set([...plat.matchAll(/session=(\S+)[^\n]*micro : une autre fenetre tient deja le cable/g)].map((m) => m[1]));
        for (const m of plat.matchAll(/micro : une autre fenetre tient deja le cable[^\n]*?session=(\S+)/g)) perdantes.add(m[1]);
        resultat.sessions_refusees = [...perdantes];
        const candidates = fen.map((f) => f.nom).filter((n) => !perdantes.has(n));
        const gagnante = perdantes.size > 0 && candidates.length === 1 ? candidates[0] : undefined;
        resultat.session_gagnante = gagnante ?? null;
        resultat.pids_enfants = pids;
        dire(`gagnante = ${gagnante ?? '<non trouvée>'} ; pids = ${JSON.stringify(pids)}`);
        const pid = gagnante ? pids[gagnante] : undefined;
        if (!pid) {
            dire('phase C NON JOUÉE : aucun PID d enfant apparié à la session gagnante');
            resultat.reprise_non_jouee = 'aucun PID apparié à la session gagnante';
        } else {
            dire(`phase C : mise a mort de l enfant gagnant, PID RELEVÉ ${pid} (session ${gagnante})`);
            try {
                const { stdout } = await execFileAsync('node', [`${RACINE}/scripts/winrm.js`,
                    `Stop-Process -Id ${pid} -Force; Start-Sleep 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count`],
                    { encoding: 'utf8', timeout: 90000, cwd: RACINE });
                dire(`mise a mort : agents restants = ${String(stdout).trim().split('\n').pop()}`);
                resultat.mise_a_mort = { pid, session: gagnante, sortie: String(stdout).trim().slice(-200) };
            } catch (e) { dire(`mise a mort : ${String(e).slice(0, 200)}`); }
        }
        await dodo(20000);
        await relever('C-reprise', 10);
    }

    dire('mesure terminee');
} catch (e) {
    resultat.erreurs.push(String(e?.stack ?? e).slice(0, 2000));
    dire(`ERREUR : ${String(e).slice(0, 400)}`);
} finally {
    try { chrome?.kill(); } catch (e) { /* deja mort */ }
    fs.mkdirSync(path.dirname(SORTIE), { recursive: true });
    fs.writeFileSync(SORTIE, JSON.stringify(resultat, null, 2));
    dire(`sortie : ${SORTIE}`);
    // 🔴 SORTIE EXPLICITE : la WebSocket CDP tient la boucle d'événements après
    // la mort de Chrome, et sans cela le harnais bascule en arrière-plan alors
    // que tout est fini — ce qui se lit comme une mesure interminable.
    process.exit(resultat.erreurs.length === 0 ? 0 : 1);
}
