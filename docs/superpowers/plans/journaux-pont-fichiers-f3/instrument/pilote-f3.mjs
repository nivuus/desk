// Pilote de la recette F3 : monte le lecteur « Mes Fichiers », fait RENOMMER et
// SUPPRIMER par la VM, et relit OPFS pour voir si le poste local a suivi.
//
//     node pilote-f3.mjs <secondes-de-maintien> <fichier-json-de-sortie>
//
// 🔴 IL REUTILISE `commun-f1.mjs` **ET** `injection-f2.js` PLUTOT QUE DE LES
// RECOPIER. Une copie eprouverait la copie, pas l'instrument — c'est le patron
// du `TYPES_AGENT` ecrit a la main que ce depot paie depuis P1.
//
// ⚠️ L'INJECTION EST CELLE DE F2, MOT POUR MOT, et c'est un choix : elle peuple
// OPFS, expose `__relire` (taille + condensat SHA-256), `__arbre` et
// `__compteur`. F3 n'a besoin de RIEN de plus — ses criteres se lisent dans
// l'arbre d'OPFS et dans les condensats. En ecrire une seconde ferait deux
// instruments a maintenir pour un seul montage.
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

/** L'instrument de F2, reemploye tel quel. */
const INSTRUMENT_F2 = '../../journaux-pont-fichiers-f2/instrument';

const ICI = path.dirname(fileURLToPath(import.meta.url));
const MAINTIEN_S = Number(process.argv[2] ?? 60);
const SORTIE = process.argv[3] ?? '/tmp/f3/pilote-f3.json';

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

    // ── 🔴 FAIRE DIVERGER LE MIROIR, DÉLIBÉRÉMENT ────────────────────────
    //
    // Trois causes du §5 ne sont atteignables QUE si le poste local porte des
    // entrées que la VM ne connaît pas — c'est-à-dire si le miroir a dérivé :
    //
    //   `repertoire-non-vide` — supprimer dans la VM un répertoire que le poste
    //       local n'a pas fini de vider. **C'est le code DIAGNOSTIQUE de F3**,
    //       et le seul moyen de le produire est de faire dériver le miroir ;
    //   `deja-present`        — renommer vers un nom qui n'existe QUE côté
    //       local : Windows ne peut pas le résoudre pour nous, puisqu'il ne le
    //       voit pas ;
    //   `inattendue`          — via `casse-ambigue`, deux homonymes qui ne
    //       diffèrent que par la casse, ce qu'OPFS accepte (il est SENSIBLE à
    //       la casse, sonde S2) et que le canonicaliseur refuse de trancher.
    //
    // ⚠️ **C'EST UNE DÉRIVE FABRIQUÉE, et elle est déclarée comme telle.** Elle
    // n'établit pas qu'une dérive survienne en exploitation ; elle établit que
    // le pont la DÉNONCE au lieu de la subir.
    if (process.env.DIVERGER === '1') {
        const div = await cdp.evalBorne(sessionShell, `(async () => {
            const r = await navigator.storage.getDirectory();
            const d = await r.getDirectoryHandle('Mes documents');
            // (a) un répertoire que la VM croit vide, et qui ne l'est pas.
            const v = await d.getDirectoryHandle('vide-cote-vm', { create: true });
            await v.getFileHandle('inconnu-de-la-vm.txt', { create: true });
            // (b) un nom qui n'existe QUE côté local.
            await d.getFileHandle('occupe-cote-local.txt', { create: true });
            // (c) deux homonymes de casse — OPFS est SENSIBLE à la casse.
            await d.getFileHandle('ambigu.txt', { create: true });
            await d.getFileHandle('AMBIGU.TXT', { create: true });
            const noms = [];
            for await (const n of d.keys()) noms.push(n);
            return JSON.stringify(noms);
        })()`, 20000, true);
        resultat.divergence_posee = typeof div === 'string' ? JSON.parse(div) : div;
        dire(`divergence posee cote local : ${JSON.stringify(resultat.divergence_posee)}`);
    }

    resultat.compteur_avant = await lireCompteur(cdp, sessionShell);
    dire(`compteur AVANT : ${JSON.stringify(resultat.compteur_avant)}`);

    // ── CRITERE ③ : COUPER LE CANAL PENDANT UNE LECTURE EN VOL ────────────
    //
    // 🔴 LA COUPURE EST POSEE PENDANT QUE LA MESURE COURT, PAS AVANT NI APRES.
    //
    // Ce qu'on veut voir est ce qu'un pont fait d'une commande **deja partie**
    // quand le poste local disparait : il doit la solder par une erreur bornee
    // (`abandonnee`, puis `canal-ferme` pour celles qui suivent), et non la
    // laisser pendre. Un canal coupe AVANT la mesure ne mesurerait que le
    // refus a l'entree ; coupe APRES, il ne mesurerait rien du tout.
    //
    // ⚠️ On ferme la CIBLE de la page-shell, jamais le navigateur : le pont
    // doit voir la connexion WebRTC tomber, pas le processus mourir.
    if (process.env.COUPURE_SUR_MARQUE) {
        // 🔵 ON ATTEND LE FAIT, JAMAIS UNE DUREE — et c'est ici que ca compte
        // le plus : trois coupures calees sur une horloge sont tombees dans le
        // vide, le canal se fermant proprement avec RIEN en vol.
        const marque = '/media/vm/dev/marque-silence.txt';
        try { fs.rmSync(marque, { force: true }); } catch (e) { /* absent */ }
        (async () => {
            for (let i = 0; i < 600; i += 1) {
                if (fs.existsSync(marque)) {
                    // ⚠️ UN DELAI APRES LA MARQUE, ET IL EST DECLARE.
                    //
                    // La marque dit que la commande VA partir, pas qu'elle est
                    // ARRIVEE. Coupee dans l'intervalle, elle est refusee a
                    // l'entree (`canal-ferme`, mesure a l'appui) au lieu d'etre
                    // ABANDONNEE en vol — deux codes distincts, deux moments
                    // distincts. Ce delai n'est pas une attente a la place d'un
                    // fait : le fait est la marque, le delai ne fait que laisser
                    // la commande atteindre le pont.
                    const apres = Number(process.env.COUPURE_APRES_MARQUE_MS || 1500);
                    await dodo(apres);
                    resultat.coupure_posee_sur_marque_a_ms = i * 100 + apres;
                    await cdp.send('Target.closeTarget', { targetId: cible.targetId })
                        .then(() => dire(`COUPURE sur la marque, apres ${i * 100} ms d attente`))
                        .catch((e) => dire(`COUPURE echouee : ${String(e).slice(0, 150)}`));
                    return;
                }
                await dodo(100);
            }
            resultat.coupure_marque_jamais_vue = true;
            dire('COUPURE : la marque n est jamais apparue — RIEN N A ETE COUPE');
        })();
        dire('coupure du canal ARMEE sur la marque de commande en vol');
    }

    if (process.env.COUPURE_MS) {
        const attente = Number(process.env.COUPURE_MS);
        setTimeout(() => {
            cdp.send('Target.closeTarget', { targetId: cible.targetId })
                .then(() => dire(`COUPURE : cible de la page-shell fermee apres ${attente} ms`))
                .catch((e) => dire(`COUPURE echouee : ${String(e).slice(0, 150)}`));
            resultat.coupure_posee_a_ms = attente;
        }, attente);
        dire(`coupure du canal ARMEE a +${attente} ms`);
    }

    // ── LA MESURE COTE VM, pendant que le lecteur est monte ────────────────
    dire('mesure cote VM (ecriture dans la racine) — ASYNCHRONE, voir l en-tete');
    try {
        const { stdout: sortie } = await execFileAsync('bash', ['-c', `bash ${ICI}/mesurer-f3.sh`],
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
    // ⚠️ SOUS COUPURE, LA PAGE-SHELL N'EXISTE PLUS : tout ce qui suit la
    // relit, donc tout ce qui suit est SANS OBJET. On le DIT plutot que de
    // rendre des zeros qui se liraient comme des mesures.
    if (process.env.COUPURE_MS || process.env.COUPURE_SUR_MARQUE) {
        resultat.suite_sans_objet = 'canal coupe : compteur, relectures et arbre OPFS non relevables';
        dire('coupure posee : la suite du protocole est SANS OBJET, et le releve le dit');
        // ⚠️ On sort par une EXCEPTION MARQUEE, pas par un `return` : ce bloc
        // est au premier niveau du `try`, et un `return` y est illegal.
        throw new FinSousCoupure();
    }

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
    // 🔴 LES DEUX MOITIES DE CHAQUE CRITERE : ce qui doit avoir BOUGE, et ce
    // qui ne doit PLUS etre la. Ne relire que la cible laisserait passer une
    // COPIE — un renommage qui copie sans retirer la source, ce que le repli
    // local peut reellement produire.
    resultat.relectures = {};
    for (const chemin of [
        'projete.txt',                          // ① a — la SOURCE doit avoir DISPARU
        'renomme.txt',                          // ① a — la CIBLE doit etre la
        'sous-dossier/autre.txt',               // ① b — la SOURCE doit avoir DISPARU
        'dossier-renomme/autre.txt',            // ① b — la CIBLE
        'dossier-renomme/profond/feuille.txt',  // ① b — LE SOUS-REPERTOIRE
        'Casse.txt',                            // ② a — doit avoir DISPARU
        'a-effacer/un.txt',                     // ② b — doit avoir DISPARU
        'a-effacer/dedans/deux.txt',            // ② b — l'ENFANT du sous-repertoire
        'gros-lecture.bin',                     // temoin : rien ne doit lui arriver
        'vide-cote-vm/inconnu-de-la-vm.txt',    // ④ — NE DOIT PAS avoir ete detruit
        'occupe-cote-local.txt',                // ④ — NE DOIT PAS avoir ete ecrase
    ]) {
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
    if (e instanceof FinSousCoupure) { resultat.ok = true; }
    else {
    resultat.ok = false;
    resultat.erreurs.push(String(e).slice(0, 1000));
    dire(`ERREUR : ${String(e).slice(0, 500)}`);
    }
} finally {
    fs.writeFileSync(SORTIE, JSON.stringify(resultat, null, 1));
    dire(`resultat ecrit : ${SORTIE}`);
    if (chrome) chrome.kill();
    await dodo(500);
    process.exit(resultat.ok ? 0 : 1);
}
