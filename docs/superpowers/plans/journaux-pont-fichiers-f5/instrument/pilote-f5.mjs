// Pilote de la recette F5 : monte le lecteur, éprouve le CACHE d'énumération,
// `Rafraichir`, et les portes P1/P3.
//
//     node pilote-f5.mjs <secondes-de-maintien> <fichier-json-de-sortie>
//
// 🔴 SON PRÉAMBULE DE MONTAGE EST REPRIS **VERBATIM** DE `pilote-f3.mjs`
// (l. 19-250), et il réutilise `commun-f1.mjs` et `injection-f2.js` PLUTÔT QUE
// DE LES RECOPIER. Ce préambule porte cinq pièges déjà payés — l'injection
// re-posée avant de naviguer (course dont le symptôme ne ressemble pas à une
// course), le jeton vérifié avant le clic, le test de montage qui porte sur le
// SUCCÈS et non sur la sous-chaîne « mont » (neuf minutes perdues en F1), la
// shell AVANT l'agent, et le repos déclaré. **Le réécrire les réintroduirait un
// par un.**
//
// ⚠️ AUCUNE capture d'écran CDP pendant la mesure : elle provoque un `Resize`,
// donc un `SHOW`, donc une session de plus (D1).
import fs from 'node:fs';
import { execFile, execFileSync } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-pont-fichiers/instrument/commun-f1.mjs';

/** L'instrument de F2, reemploye tel quel. */
const INSTRUMENT_F2 = '../../journaux-pont-fichiers-f2/instrument';

const ICI = path.dirname(fileURLToPath(import.meta.url));
const MAINTIEN_S = Number(process.argv[2] ?? 60);
const SORTIE = process.argv[3] ?? '/tmp/f5/pilote-f5.json';

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
const PORT_CDP = Number(process.env.PORT_CDP ?? 9450);
const UDD = process.env.UDD ?? '/tmp/f3/udd';

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

class FinSousCoupure extends Error {}

const resultat = { maintien_s: MAINTIEN_S, prefixe: PREFIXE, journal, erreurs: [] };
let chrome;
try {
    const paire = await obtenirPaire();
    dire('jeton de recette obtenu');

    const injection = fs.readFileSync(path.join(ICI, INSTRUMENT_F2, 'injection-f2.js'), 'utf8')
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

    // 🔴 `?faute-fichiers=1` ARME l'injection du canonicaliseur. **VARIABLE DE
    // BANC, jamais une configuration livrée** : sans elle, un `.faute-*` est un
    // chemin ordinaire, donc absent.
    //
    // ⚠️ **UNE INJECTION PROUVE QUE LA TABLE N'EST PAS DÉCORATIVE ; ELLE NE
    // PROUVE PAS QUE LA CAUSE EST ATTEIGNABLE EN EXPLOITATION.** Les deux
    // colonnes restent distinctes au document de résultats.
    const fautes = process.env.FAUTES_FICHIERS === '1' ? '&faute-fichiers=1' : '';
    resultat.fautes_armees = fautes !== '';
    const url = `${CLIENT_URL}/shell.html?signaling=${encodeURIComponent(SIGNALING_RELAIS)}&prefixe=${encodeURIComponent(PREFIXE)}${fautes}`;
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

    // ════════════════════════════════════════════════════════════════════
    // LE PROTOCOLE DU CRITÈRE ①, DANS L'ORDRE, ET IL N'EST PAS INDIFFÉRENT.
    //
    //  1. lister      — l'AMORÇAGE du cache. Sans lui il n'y a rien à servir,
    //                   et le critère serait VACUEUX.
    //  2. ajouter un fichier CÔTÉ NAVIGATEUR, hors du pont.
    //  3. relister    — le fichier doit être ABSENT.
    //  4. Rafraichir.
    //  5. relister    — le fichier doit être PRÉSENT.
    //
    // 🔴 RÈGLE D'ADMISSION (D4) : l'écart entre (1) et (3) est ENREGISTRÉ. S'il
    // dépasse `TTL_ENUMERATION` (30 s), l'exécution est DISQUALIFIÉE et non
    // rapportée — un cache expiré rendrait le fichier visible, donc un FAUX
    // ROUGE, et l'imputer au produit serait une erreur d'attribution.
    // ════════════════════════════════════════════════════════════════════
    const mesurer = async (phase) => {
        const { stdout } = await execFileAsync('bash', ['-c', `bash ${ICI}/mesurer-f5.sh ${phase}`],
            { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, timeout: 200000 });
        // 🔴 **LE BOM.** `Set-Content -Encoding utf8` de Windows PowerShell 5.1
        // écrit un `\uFEFF` en tête, et `JSON.parse` le REFUSE. Le symptôme est
        // un relevé dont TOUS les champs valent `undefined` — indiscernable
        // d'une mesure qui n'aurait rien trouvé, alors que le JSON est parfait.
        // *Payé sur place à la première exécution.*
        const propre = String(stdout).replace(/^\uFEFF/, '').trim();
        try { return JSON.parse(propre); } catch { return { erreur: 'json illisible', brut: propre.slice(0, 800) }; }
    };

    // ── PORTE P3, jouée AVANT tout le reste ─────────────────────────────
    // Un fichier créé DANS la VM apparaît-il sans que le filtre nous rappelle ?
    // 🔴 Le TÉMOIN qui rend le relevé lisible est le critère ① lui-même : un
    // fichier créé côté NAVIGATEUR ne doit, LUI, pas apparaître sans
    // aller-retour. Sans ce témoin, « le fichier est là » ne distinguerait pas
    // les deux mécanismes.
    dire('PORTE P3 : creation DANS la VM, puis relistage immediat');
    resultat.p3 = await mesurer('creer-dans-la-vm');
    dire(`P3 : present_apres=${resultat.p3.present_apres}`);

    // ── PORTE P1 : l'occupation disque ──────────────────────────────────
    dire('PORTE P1 : les candidates d occupation disque');
    resultat.p1 = await mesurer('occupation');

    // ── CRITÈRE ① ───────────────────────────────────────────────────────
    dire('(1) amorcage du cache');
    const t1 = Date.now();
    resultat.c1_amorcage = await mesurer('lister');
    dire(`(1) ${resultat.c1_amorcage.liste?.compte} entrees en ${resultat.c1_amorcage.liste?.ms} ms`);

    dire('(2) ajout d un fichier COTE NAVIGATEUR, hors du pont');
    resultat.c1_ajout = await cdp.evalBorne(sessionShell, `(async () => {
        const r = await navigator.storage.getDirectory();
        const d = await r.getDirectoryHandle('Mes documents');
        const f = await d.getFileHandle('AJOUTE-COTE-LOCAL.txt', { create: true });
        const w = await f.createWritable();
        await w.write('ajoute par le navigateur, jamais par le pont');
        await w.close();
        const noms = [];
        for await (const n of d.keys()) noms.push(n);
        return JSON.stringify({ ok: true, entrees: noms.length });
    })()`, 30000, true);
    dire(`(2) ${resultat.c1_ajout}`);

    dire('(3) relistage : le fichier doit etre ABSENT');
    resultat.c1_avant_rafraichir = await mesurer('lister');
    const t3 = Date.now();
    resultat.c1_ecart_ms = t3 - t1;
    resultat.c1_admissible = resultat.c1_ecart_ms < 30000;
    dire(`(3) ecart (1)->(3) = ${resultat.c1_ecart_ms} ms — admissible : ${resultat.c1_admissible}`);

    dire('(4) clic sur #rafraichir');
    resultat.c1_bouton = await cdp.evalBorne(sessionShell, `(() => {
        const b = document.querySelector('#rafraichir');
        if (!b) return 'BOUTON ABSENT';
        b.click();
        return 'clique';
    })()`, 8000, false);
    dire(`(4) ${resultat.c1_bouton}`);
    await dodo(2000);

    dire('(5) relistage : le fichier doit etre PRESENT');
    resultat.c1_apres_rafraichir = await mesurer('lister');

    // ── L'état des boutons et du bandeau ────────────────────────────────
    resultat.etat_boutons = await cdp.evalBorne(sessionShell, `JSON.stringify({
        rafraichir: !!document.querySelector('#rafraichir'),
        reprendre_present: !!document.querySelector('#reprendre-enregistrement'),
        reprendre_cache: document.querySelector('#reprendre-enregistrement')?.hidden ?? null,
        retenues: document.querySelector('#actions-fichiers')?.dataset.retenues ?? null,
        dues: document.querySelector('#ecritures-dues')?.dataset.dues ?? null,
        vues: document.querySelector('#ecritures-dues')?.dataset.vues ?? null,
    })`, 8000, false);
    dire(`boutons : ${resultat.etat_boutons}`);

    // ════════════════════════════════════════════════════════════════════
    // CRITÈRE ③ — LE RETOUR AVEC UN RÉPERTOIRE DIFFÉRENT RETIENT, ET LE DIT.
    //
    // 🔴 **CE QUE CE MONTAGE ÉPROUVE, ET CE QU'IL N'ÉPROUVE PAS.** Il crée un
    // SECOND répertoire OPFS, de nom différent, et remonte le lecteur dessus.
    // Le `name` d'une poignée OPFS est **la même valeur, lue au même endroit**,
    // que celui d'un répertoire choisi par `showDirectoryPicker()` : la RÈGLE
    // du nom est donc pleinement exercée.
    // ⛔ **LE MODÈLE DE PERMISSION NE L'EST PAS** — `showDirectoryPicker`,
    // `queryPermission`, `requestPermission` et l'activation utilisateur
    // transitoire ne sont appelés nulle part dans ce dépôt. **F5 mesure que la
    // règle du nom fonctionne ; il ne mesure pas qu'elle suffise.**
    if (process.env.CRITERE_3 === '1') {
        dire('CRITERE ③ : remontage sur un AUTRE repertoire');
        resultat.c3_second = await cdp.evalBorne(sessionShell, `(async () => {
            const r = await navigator.storage.getDirectory();
            const d = await r.getDirectoryHandle('Autre dossier', { create: true });
            await d.getFileHandle('temoin.txt', { create: true });
            return JSON.stringify({ nom: d.name });
        })()`, 30000, true);
        dire(`② second repertoire : ${resultat.c3_second}`);
        // ⚠️ **LE SÉLECTEUR FACTICE EST SURCHARGÉ ICI, ET PAS DANS
        // `injection-f2.js`.** Celui-ci est l'instrument de F2, que F3 et F4
        // réemploient : y ajouter un crochet pour F5 ferait qu'une recette
        // close dépendrait d'une édition faite pour une autre. La surcharge est
        // lue **à l'appel**, donc elle prend effet au clic suivant.
        await cdp.evalBorne(sessionShell, `(() => {
            window.showDirectoryPicker = async () => {
                const r = await navigator.storage.getDirectory();
                return r.getDirectoryHandle('Autre dossier', { create: true });
            };
            return 'surcharge posee';
        })()`, 8000, false);
        const b2 = await cdp.evalBorne(sessionShell, `(() => { const b = document.querySelector('#choisir-dossier'); const r = b.getBoundingClientRect(); return JSON.stringify({ x: Math.round(r.x + r.width/2), y: Math.round(r.y + r.height/2) }); })()`, 5000, false);
        const p2 = JSON.parse(b2);
        await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: p2.x, y: p2.y, button: 'left', clickCount: 1 }, sessionShell);
        await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: p2.x, y: p2.y, button: 'left', clickCount: 1 }, sessionShell);
        await dodo(8000);
        resultat.c3_etat = await cdp.evalBorne(sessionShell, `JSON.stringify({
            etat: document.querySelector('#etat-fichiers')?.textContent,
            dues: document.querySelector('#ecritures-dues')?.dataset.dues,
            texte: document.querySelector('#ecritures-dues')?.textContent,
            retenues: document.querySelector('#actions-fichiers')?.dataset.retenues,
            reprendre_cache: document.querySelector('#reprendre-enregistrement')?.hidden,
        })`, 8000, false);
        dire(`③ ${resultat.c3_etat}`);
    }

    // ════════════════════════════════════════════════════════════════════
    // CRITÈRE ④ — UN CACHE ARMÉ NE DOIT RIEN CASSER DE F2 NI DE F3.
    //
    // 🔴 **C'EST LE RISQUE N°1 DE F5, et sa rouge est le retrait de
    // l'invalidation.** Les trois cas sont joués APRÈS un `Rafraichir`, donc
    // sur un cache qui vient d'être repeuplé : créer, renommer, supprimer dans
    // la VM, et relister à chaque fois.
    if (process.env.CRITERE_4 === '1') {
        dire('CRITERE ④ : creer / renommer / supprimer dans la VM, cache arme');
        resultat.c4 = await mesurer('muter');
        const c4 = resultat.c4;
        if (c4 && c4.avant) {
            dire(`④ avant=${c4.avant.compte} creation=${c4.apres_creation.compte} ` +
                 `renommage=${c4.renommage}/${c4.apres_renommage.compte} ` +
                 `suppression=${c4.suppression}/${c4.apres_suppression.compte}`);
            resultat.c4_verdict = {
                creation_vue: c4.apres_creation.noms.includes('a-renommer.txt'),
                ancien_nom_disparu: !c4.apres_renommage.noms.includes('a-renommer.txt'),
                nouveau_nom_vu: c4.apres_renommage.noms.includes('RENOMME.txt'),
                supprime_disparu: !c4.apres_suppression.noms.includes('RENOMME.txt'),
            };
            dire(`④ verdict : ${JSON.stringify(resultat.c4_verdict)}`);
        }
    }

    // ════════════════════════════════════════════════════════════════════
    // TÂCHE 17 — LA LATENCE DE LISTAGE AUX RANGS DE F4, CACHE ARMÉ.
    // Imposée par le §0.1 au verdict VERT du critère ①.
    // ⚠️ **La PRÉDICTION du §0.2 est écrite AVANT la mesure** : si le cache est
    // consulté au bon endroit, un SECOND `Get-ChildItem` dans la fenêtre du TTL
    // doit coûter `lister=n:0`, et la latence doit approximativement se diviser
    // par deux. **Ce qui la falsifie** : un chaud aussi cher que le froid.
    // ⚠️ **Ce que cela NE ferme PAS** : le mécanisme des DEUX `Lister` par
    // geste (legs n°4 de F4) reste inexpliqué même si le cache en absorbe un.
    if (process.env.RANGS === '1') {
        resultat.rangs = {};
        for (const n of [10, 100, 1000]) {
            dire(`rang ${n} : peuplement OPFS`);
            const sous = `rang-${n}`;
            await cdp.evalBorne(sessionShell, `(async () => {
                const r = await navigator.storage.getDirectory();
                const d = await r.getDirectoryHandle('Mes documents');
                const s = await d.getDirectoryHandle(${JSON.stringify(sous)}, { create: true });
                for (let i = 0; i < ${n}; i += 1) {
                    await s.getFileHandle('f' + String(i).padStart(5, '0') + '.txt', { create: true });
                }
                return 'peuple';
            })()`, 180000, true);
            // Le répertoire est neuf côté navigateur : le pont ne peut pas le
            // connaître, et un `Rafraichir` garantit qu'aucune mémoire du parent
            // ne le cache.
            await cdp.evalBorne(sessionShell, `(document.querySelector('#rafraichir').click(), 'ok')`, 8000, false);
            await dodo(1500);
            resultat.rangs[n] = await mesurer(`rang-${sous}`);
            const m = resultat.rangs[n];
            dire(`rang ${n} : ${m.entrees} entrees | froid ${m.froid_ms} ms | chaud ${m.chaud_ms} ms | chaud2 ${m.chaud2_ms} ms | coherent ${m.coherent}`);
        }
    }

    resultat.arbre_opfs = await cdp.evalBorne(sessionShell, `window.__arbre()`, 30000, true);
    dire(`maintien de ${MAINTIEN_S} s`);
    await dodo(MAINTIEN_S * 1000);
} catch (e) {
    resultat.erreurs.push(String(e && e.stack ? e.stack : e));
    dire(`ERREUR : ${e}`);
} finally {
    try { if (chrome) chrome.kill(); } catch (e) { /* deja mort */ }
    fs.mkdirSync(path.dirname(SORTIE), { recursive: true });
    fs.writeFileSync(SORTIE, JSON.stringify(resultat, null, 2));
    dire(`releve ecrit : ${SORTIE}`);
    // 🔴 SORTIE EXPLICITE : le WebSocket CDP tient la boucle d'evenements
    // apres la mort de Chrome, et sans cela le harnais bascule le pilote en
    // arriere-plan alors que tout est fini (piege du presse-papier P1).
    process.exit(resultat.erreurs.length > 0 ? 1 : 0);
}
