// Pilote de la recette F2 : monte le lecteur « Mes Fichiers », fait ECRIRE la
// VM dedans, et relit OPFS pour comparer les condensats.
//
//     node pilote-f2.mjs <secondes-de-maintien> <fichier-json-de-sortie>
//
// 🔴 IL REUTILISE `commun-f1.mjs` PLUTOT QUE DE LE RECOPIER. Une copie
// eprouverait la copie, pas l'instrument — c'est le patron du `TYPES_AGENT`
// ecrit a la main que ce depot paie depuis P1.
//
// ⚠️ AUCUNE capture d'ecran CDP pendant la mesure : elle provoque un `Resize`,
// donc un `SHOW`, donc une session de plus (D1).

import fs from 'node:fs';
import { execFile, execFileSync } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-pont-fichiers/instrument/commun-f1.mjs';

const ICI = path.dirname(fileURLToPath(import.meta.url));
const MAINTIEN_S = Number(process.argv[2] ?? 60);
const SORTIE = process.argv[3] ?? '/tmp/f2/pilote-f2.json';

const PLATEFORME_URL = process.env.PLATEFORME_URL ?? 'http://127.0.0.1:8080';
const CLIENT_URL = process.env.CLIENT_URL ?? 'http://127.0.0.1:5173';
const SIGNALING = process.env.SIGNALING_WS ?? 'ws://192.168.3.1:8080';
const BASE_JEU = process.env.BASE_JEU ?? 'http://127.0.0.1:5399';
const PREFIXE = process.env.PREFIXE_VM;
const PORT_CDP = Number(process.env.PORT_CDP ?? 9440);
const UDD = process.env.UDD ?? '/tmp/f2/udd';

if (!PREFIXE) throw new Error('PREFIXE_VM est obligatoire');

// 🔴 LA MESURE COTE VM EST ASYNCHRONE, ET C'EST UN DEFAUT D'INSTRUMENT PAYE SUR
// PLACE — voir `f2-arme-1-diagnostic.txt`.
//
// La premiere execution de cette recette employait `execFileSync`, comme le
// pilote de F1. Elle BLOQUE LA BOUCLE D'EVENEMENTS DE NODE pendant toute la
// mesure — deux minutes ici. Le pilote cesse alors de lire sa WebSocket CDP, la
// file de messages DevTools se remplit, et **le rendu de la page-shell se
// bloque avec elle** : le navigateur n'a plus servi une seule requete du pont.
//
// Le journal d'agent le montre sans ambiguite : VINGT commandes `expirée`
// pendant la mesure, puis — a la SECONDE ou `execFileSync` rend la main —
// vingt `réponse tardive ou inconnue : jetée`. **Le navigateur avait tout
// prepare et n'a rien pu emettre.**
//
// ⚠️ CE N'ETAIT PAS UNE PANNE DU PRODUIT, ET CELA S'EN LISAIT EXACTEMENT COMME
// UNE. C'est le piege maison « l'instrument detruit ce qu'il mesure », deja
// paye par la trace par paquet du chantier TURN et par la capture d'ecran CDP
// de D1 — ici sous une troisieme forme.
const execFileAsync = promisify(execFile);

const journal = [];
const dire = (m) => { const l = `[${new Date().toISOString()}] ${m}`; journal.push(l); console.log(l); };

// 🔴 `evalBorne` REND UN OBJET quand elle expire (`{__timeout}`) ou echoue
// (`{__erreur}`), jamais une chaine. Un `JSON.parse` pose dessus recoit
// « [object Object] » et LEVE : c'est ce qui a fait perdre l'execution
// `reprise-1` du 20 aout 2026 a son 51e echantillon, alors que le produit,
// lui, poussait correctement — le journal d'agent le montre. La lecture d'un
// compteur ne doit pas pouvoir tuer une recette.
//
// ⚠️ L'echantillon illisible est CONSERVE tel quel dans le suivi plutot que
// saute : un trou silencieux dans une serie se lit comme une serie continue.
async function lireCompteur(cdp, session) {
    const brut = await cdp.evalBorne(session, `window.__compteur()`, 15000, false);
    if (typeof brut === 'string') {
        try { return JSON.parse(brut); } catch { return { illisible: brut.slice(0, 200) }; }
    }
    return { illisible: JSON.stringify(brut).slice(0, 200) };
}

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

const resultat = { maintien_s: MAINTIEN_S, prefixe: PREFIXE, journal, erreurs: [] };
let chrome;
try {
    const paire = await obtenirPaire();
    dire('jeton de recette obtenu');

    const injection = fs.readFileSync(path.join(ICI, 'injection-f2.js'), 'utf8')
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

    // 🔴 L'INJECTION EST RE-POSÉE ICI, ET ATTENDUE, AVANT DE NAVIGUER.
    //
    // Le gestionnaire de `Target.attachedToTarget` est ASYNCHRONE : il inscrit
    // la session dans la table AVANT d'avoir fini de poser
    // `addScriptToEvaluateOnNewDocument`. La boucle ci-dessus peut donc rendre
    // une session dont l'injection n'est pas encore armée, et `Page.navigate`
    // partir sur une page qui ne verra jamais le jeton.
    //
    // ⚠️ **LE SYMPTÔME NE RESSEMBLE PAS À UNE COURSE** : sans jeton,
    // `shell-page.ts` fait `location.replace('connexion.html')`, la page-shell
    // n'existe plus, et `#etat-fichiers` rend `null` — ce qui se lit comme un
    // lecteur qui n'a pas monté. L'exécution `arme-2` a été perdue ainsi, et
    // son journal de page portait TROIS `[vite] connecting` pour une seule
    // navigation demandée.
    //
    // ⚠️ Le pilote de F1 porte la même course, et elle n'y a jamais été vue.
    await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: injection }, sessionShell);

    const console_page = [];
    cdp.on((m) => {
        if (m.method === 'Runtime.consoleAPICalled' && m.sessionId === sessionShell) {
            const t = (m.params.args ?? []).map((a) => a.value ?? a.description ?? a.type).join(' ');
            console_page.push({ t: Date.now(), niveau: m.params.type, texte: String(t).slice(0, 400) });
            if (console_page.length > 400) console_page.shift();
        }
    });
    resultat.console_page = console_page;

    const url = `${CLIENT_URL}/shell.html?signaling=${encodeURIComponent(SIGNALING)}&prefixe=${encodeURIComponent(PREFIXE)}`;
    dire(`navigation : ${url}`);
    await cdp.send('Page.navigate', { url }, sessionShell);
    await dodo(4000);

    for (let i = 0; i < 120; i += 1) {
        const p = await cdp.evalBorne(sessionShell, `String(window.__f2 && window.__f2.peuple)`, 5000, false);
        if (p && p !== 'undefined' && p !== 'null') { dire(`OPFS peuple : ${p} entrees`); resultat.opfs_entrees = Number(p); break; }
        await dodo(500);
    }

    // 🔴 L'ORDRE : LA SHELL D'ABORD, L'AGENT ENSUITE. Le signaling ne memorise
    // que les offres SDP ; une annonce `fenetre-ouverte` emise avant la
    // connexion de la shell est PERDUE SANS TRACE (D1), et passe
    // `DELAI_ATTENTE_VIEWPORT_MAX` une entree est ABANDONNEE et jamais
    // reproposee (D3).
    if (process.env.APRES_CONNEXION) {
        dire('la page-shell est connectee : lancement de l agent');
        const sortie = execFileSync('bash', ['-c', process.env.APRES_CONNEXION],
            { encoding: 'utf8', timeout: 180000 });
        dire(`agent lance : ${sortie.trim().split('\n').pop()}`);
        for (let i = 0; i < 90; i += 1) {
            const cibles = await (await fetch(`http://127.0.0.1:${PORT_CDP}/json/list`)).json();
            const n = cibles.filter((c) => c.type === 'page' && c.url.includes('session=')).length;
            if (n > 0) { dire(`${n} fenetre(s) d application apres ${i} s`); resultat.fenetres_ouvertes = n; break; }
            await dodo(1000);
        }
    }

    // 🔴 LE JETON EST VÉRIFIÉ AVANT LE CLIC. Sans lui la page se redirige, et
    // tout ce qui suit mesurerait `connexion.html`.
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

    // Repos declare : `#etat-fichiers` passe a « monte » quand le canal s'ouvre
    // COTE CLIENT ; le service du pont s'arme un peu plus tard cote agent.
    const repos = Number(process.env.REPOS_APRES_MONTAGE_MS ?? 8000);
    dire(`repos de ${repos} ms avant la mesure`);
    await dodo(repos);

    resultat.compteur_avant = await lireCompteur(cdp, sessionShell);
    dire(`compteur AVANT : ${JSON.stringify(resultat.compteur_avant)}`);

    // ── LA MESURE COTE VM, pendant que le lecteur est monte ────────────────
    dire('mesure cote VM (ecriture dans la racine) — ASYNCHRONE, voir l en-tete');
    try {
        const { stdout: sortie } = await execFileAsync('bash', ['-c', `bash ${ICI}/mesurer-f2.sh`],
            { encoding: 'utf8', timeout: 600000, maxBuffer: 64 * 1024 * 1024 });
        resultat.mesure_vm_brut = sortie;
        const ligne = sortie.split('\n').map((l) => l.trim()).filter((l) => l.startsWith('{')).pop();
        resultat.mesure_vm = ligne ? JSON.parse(ligne) : null;
        dire(`mesure VM : ${ligne ? 'JSON lu' : 'AUCUN JSON'}`);
    } catch (e) {
        resultat.mesure_vm_erreur = String(e).slice(0, 2000);
        dire(`mesure VM ECHOUEE : ${String(e).slice(0, 300)}`);
    }

    // ── Attendre que les ecritures dues redescendent a zero ────────────────
    // 🔴 ON ATTEND LE FAIT, jamais une duree.
    const suivi = [];
    for (let i = 0; i < 90; i += 1) {
        const c = await lireCompteur(cdp, sessionShell);
        suivi.push({ t: i, ...c });
        if (i > 3 && c.dues === 0 && c.vues > 0) break;
        await dodo(1000);
    }
    resultat.suivi_compteur = suivi;
    resultat.compteur_final = suivi.at(-1);
    dire(`compteur FINAL : ${JSON.stringify(resultat.compteur_final)}`);

    // ── LA RELECTURE D'OPFS : le critere ① et le critere ② ─────────────────
    resultat.relectures = {};
    for (const chemin of ['ecrit-par-la-vm.bin', 'projete.txt', 'vide.txt', 'dossier-neuf/dedans.txt', 'Casse.txt', 'CASSE.TXT']) {
        const v = await cdp.evalBorne(sessionShell, `window.__relire(${JSON.stringify(chemin)})`, 30000, true);
        resultat.relectures[chemin] = typeof v === 'string' ? JSON.parse(v) : v;
        dire(`relecture ${chemin} : ${JSON.stringify(resultat.relectures[chemin])}`);
    }
    const arbre = await cdp.evalBorne(sessionShell, `window.__arbre()`, 30000, true);
    resultat.arbre_opfs = typeof arbre === 'string' ? JSON.parse(arbre) : arbre;
    dire(`arbre OPFS : ${JSON.stringify(resultat.arbre_opfs)}`);

    // ── CRITERE ⑥ : la video ne perd pas une image ─────────────────────────
    const echantillons = [];
    const debut = Date.now();
    while ((Date.now() - debut) / 1000 < MAINTIEN_S) {
        await dodo(10000);
        const cibles = await (await fetch(`http://127.0.0.1:${PORT_CDP}/json/list`)).json();
        const pages = cibles.filter((c) => c.type === 'page' && c.url.includes('session='));
        const point = { t: Math.round((Date.now() - debut) / 1000), fenetres: pages.length, flux: [] };
        for (const p of pages) {
            const sid = sessions.get(p.id);
            if (!sid) { point.flux.push({ url: p.url.slice(-30), v: '{"sansSession":true}' }); continue; }
            const v = await cdp.evalBorne(sid, `(async () => {
                const pc = window.__pc;
                if (!pc) return JSON.stringify({ sansPc: true });
                const s = await pc.getStats();
                let d = null, l = null;
                s.forEach((r) => { if (r.type === 'inbound-rtp' && r.kind === 'video') { d = r.framesDecoded; l = r.packetsLost; } });
                return JSON.stringify({ framesDecoded: d, packetsLost: l });
            })()`, 8000, true);
            point.flux.push({ url: p.url.slice(-30), v });
        }
        echantillons.push(point);
        dire(`echantillon : ${JSON.stringify(point)}`);
    }
    resultat.echantillons = echantillons;
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
