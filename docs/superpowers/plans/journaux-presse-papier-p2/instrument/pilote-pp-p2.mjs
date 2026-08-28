#!/usr/bin/env node
// Sous-bloc P2 (presse-papier) — le pilote de recette, tâche 22.
//
// Il mesure les critères ②, ③ et ④ dans une seule exécution. Deux exécutions
// du bras armé, deux du bras désarmé (`PRESSE_PAPIER_GARDE=0`).
//
// ---------------------------------------------------------------------------
// CE QUI EST OBSERVÉ, ET POURQUOI CE POINT D'OBSERVATION ET PAS UN AUTRE
// ---------------------------------------------------------------------------
//
// 🔴 Les messages `clipboard` AGENT → CLIENT sont comptés SUR LE CANAL DE
// CONTRÔLE, par un écouteur INDÉPENDANT du produit — jamais sur les appels à
// `writeText`. `PressePapierLocal` dédoublonne LUI AUSSI, si bien que compter
// les écritures mesurerait le garde du CLIENT et non celui de l'AGENT. C'est
// la leçon la plus réutilisable de la recette de P1, et elle est reprise telle
// quelle.
//
// 🔴 DEUX FAITS DISTINCTS SONT RELEVÉS SÉPARÉMENT, et les confondre ferait
// imputer l'un à l'autre :
//   - le PRESSE-PAPIER DE LA VM porte-t-il le texte ? (l'agent a écrit) ;
//   - le BLOC-NOTES porte-t-il le texte ? (l'injection de `Ctrl+V` a eu lieu).
// Le premier sans le second désignerait l'injection ; le second sans le
// premier serait impossible. Les deux se lisent par le LECTEUR, en session 1.
//
// ⚠️ LA SESSION 1 EST OBLIGATOIRE POUR LES DEUX, et pour deux raisons
// distinctes : `Get-Clipboard` par WinRM (session 0) rend `-1`, le
// presse-papier étant par station de fenêtres (sonde P0 de P1) ; et
// `EnumWindows` depuis la session 0 ne voit AUCUNE fenêtre de la session 1,
// donc un `WM_GETTEXT` lancé par WinRM ne trouverait jamais le Bloc-notes.
//
// ---------------------------------------------------------------------------
// LE PRESSE-PAPIER DU NAVIGATEUR EST AMORCÉ PAR UNE VRAIE COPIE
// ---------------------------------------------------------------------------
//
// `Ctrl+C` de confiance sur un `<textarea>`, JAMAIS `navigator.clipboard.
// writeText` — qui échoue hors activation utilisateur (sonde annexe du §0,
// deux exécutions) et demanderait de toute façon une permission. C'est le
// geste exact de la sonde du §0, repris.
//
// 🔴 ET IL SE FAIT DANS UN ONGLET SÉPARÉ, ce qui n'est pas un détail :
// `client/src/input.ts` appelle `preventDefault()` sur chaque `keydown` qui
// n'est PAS un raccourci de collage — un `Ctrl+C` frappé dans la page de
// session serait donc empêché, et rien ne serait copié.
//
// ---------------------------------------------------------------------------
// INVARIANTS DE MONTAGE, hérités et non renégociés
// ---------------------------------------------------------------------------
//
//   - navigateur pilote sur l'HÔTE, jamais sur la VM ;
//   - fenêtre VM (le Bloc-notes) ouverte AVANT le superviseur ;
//   - LECTEUR à demeure, un seul processus, né AVANT le superviseur : une
//     tâche planifiée par lecture ouvrirait une console ÉLIGIBLE à la capture
//     (D11), et l'instrument détruirait ce qu'il mesure (payé en P1) ;
//   - page-shell ouverte AVANT le superviseur (le signaling ne mémorise pas
//     les annonces `fenetre-ouverte` — D1) ;
//   - `waitForDebuggerOnStart` + `setDiscoverTargets`, et BALAYAGE de
//     `Target.getTargets` : l'auto-attache ne suffit pas (mesuré en P1) ;
//   - aucune capture d'écran CDP, toute évaluation BORNÉE (D1, D2) ;
//   - identité obligatoire depuis P3 de la plateforme : sans
//     `AGENT_VM`/`AGENT_SECRET` l'agent est refusé et le symptôme se lit
//     exactement comme une panne du produit.
//
// Usage :
//   node pilote-pp-p2.mjs --etiquette=arme-1 [--desarme=1] --identite=<fichier>

import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-multifenetres-d11/instrument/commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const ETIQUETTE = arg('etiquette', 'arme-1');
const DESARME = arg('desarme', '') === '1';
const SORTIE = arg('sortie', `pp2-${ETIQUETTE}.json`);
function racineDepot() {
    const r = spawnSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' });
    if (r.status !== 0) {
        throw new Error(`hors du depot git : impossible de deriver RACINE (git rev-parse a echoue : ${(r.stderr ?? '').trim()})`);
    }
    return r.stdout.trim();
}
const RACINE = process.env.RACINE ?? racineDepot();
const P2 = `${RACINE}/docs/superpowers/plans/journaux-presse-papier-p2`;
const VMIT = `${RACINE}/docs/superpowers/plans/journaux-multifenetres-d10/instrument/vm-it.sh`;
const IDENTITE = arg('identite', '');
const PLATEFORME_URL = arg('plateforme', 'http://127.0.0.1:8090');
const CLIENT_URL = arg("client", "http://127.0.0.1:5174");
const SIGNALING_WS = arg('signaling', `ws://${process.env.HOTE ?? '192.168.3.1'}:8090`);
// 🔴 CHANTIER auth-pomerium : le relais a quitté `/` pour `/signal`.
// SIGNALING_WS RESTE LA BASE — c'est elle qui va dans SIGNALING_URL au
// lancement de l'agent (`agent/src/signaling.rs::url_du_relais` y ajoute
// `/signal` lui-même). `?signaling=` côté NAVIGATEUR, lui, est EXPLICITE et
// NE REÇOIT AUCUN AJOUT (client/src/adresse-plateforme.ts::adresseSignaling) :
// c'est ce pilote qui doit fournir l'URL du relais, pas sa seule base.
const SIGNALING_RELAIS = `${SIGNALING_WS.replace(/\/+$/, '')}/signal`;
const NONCE = `${ETIQUETTE}-${Math.random().toString(36).slice(2, 8)}`;
const log = (...a) => console.log(new Date().toISOString(), ...a);

const winrm = (c) => {
    const r = spawnSync('node', [`${RACINE}/scripts/winrm.js`, c],
        { encoding: 'utf8', env: process.env, timeout: 180000 });
    return (r.stdout ?? '') + (r.stderr ? `\n[stderr] ${r.stderr}` : '');
};
const vmIt = (nom, ps) => {
    const r = spawnSync('bash', [VMIT, nom, ps], { encoding: 'utf8', env: process.env, timeout: 120000 });
    if (r.status !== 0) log(`!! vm-it ${nom} a échoué`, (r.stderr ?? '').slice(0, 300));
    return r.status;
};

// L'amorce. Elle pose UN observateur indépendant et ne touche à rien d'autre.
const AMORCE = `(() => {
  if (window.__pp2) return;
  window.__pp2 = { messages: [], erreurs: [] };
  // L'ECOUTEUR EST POSE EN ENVELOPPANT createDataChannel, AVANT que le
  // produit n'attache le sien : c'est ce qui le rend INDEPENDANT de lui.
  try {
    const P = window.RTCPeerConnection;
    if (P && P.prototype && P.prototype.createDataChannel) {
      const c = P.prototype.createDataChannel;
      P.prototype.createDataChannel = function (label, opts) {
        const ch = c.call(this, label, opts);
        try {
          if (label === 'control') {
            ch.addEventListener('message', (e) => {
              try {
                const m = JSON.parse(e.data);
                if (m && m.type === 'clipboard') {
                  window.__pp2.messages.push({
                    t: Date.now(),
                    octets: m.bytes,
                    nul: m.text === null,
                    debut: m.text === null ? null : String(m.text).slice(0, 40),
                  });
                }
              } catch (err) { window.__pp2.erreurs.push(String(err).slice(0, 120)); }
            });
          }
        } catch (err) { window.__pp2.erreurs.push(String(err).slice(0, 120)); }
        return ch;
      };
    } else { window.__pp2.erreurs.push('RTCPeerConnection absent'); }
  } catch (e) { window.__pp2.erreurs.push('hameconnage createDataChannel : ' + String(e).slice(0, 120)); }
  // La page-shell ouvre '/?session=...' SANS parametre 'signaling' : main.ts
  // retombe alors sur ws://<hote>:8080 en dur, or cette recette parle a une
  // SECONDE instance sur 8090. Geste d'INSTRUMENT, declare : main.ts
  // documente lui-meme ce parametre comme prevu pour faciliter les essais.
  try {
    const o = window.open.bind(window);
    window.open = function (url, ...r) {
      try {
        if (typeof url === 'string' && url.includes('?session=') && !url.includes('signaling=')) {
          url += (url.includes('?') ? '&' : '?') + 'signaling=' + encodeURIComponent('__SIGNALING__');
        }
      } catch (e) { window.__pp2.erreurs.push(String(e).slice(0, 120)); }
      return o(url, ...r);
    };
  } catch (e) { window.__pp2.erreurs.push('hameconnage window.open : ' + String(e).slice(0, 120)); }
})();`;

const EXPR_ETAT = `(() => ({
  pp: window.__pp2 || null,
  focus: document.hasFocus(),
  actif: document.activeElement ? document.activeElement.id || document.activeElement.tagName : null,
  statut_texte: (document.querySelector('#status') || {}).textContent || null,
  statut_cache: document.querySelector('#status') ? document.querySelector('#status').dataset.hidden : null,
}))()`;

const EXPR_PERMS = `Promise.all([
  navigator.permissions.query({name:'clipboard-read'}).then(p=>p.state,e=>'ERREUR:'+e.name),
  navigator.clipboard.readText().then(t=>'OK:'+String(t).slice(0,40),e=>'THROW:'+e.name),
]).then(([perm, lecture]) => ({ perm, lecture }))`;

/// Le lecteur, en session 1. L'attente porte sur le FAIT — le numéro d'ordre
/// relu dans `pp2-fait.txt` —, jamais sur une durée.
let numeroOrdre = 0;
async function lire(mode) {
    numeroOrdre += 1;
    const n = numeroOrdre;
    writeFileSync('/media/vm/dev/pp2-ordre.txt', `${n}|${mode}`, 'utf8');
    // 🔴 LA LECTURE EXIGE DEUX RELEVÉS IDENTIQUES, ET CE N'EST PAS DE LA
    // PRUDENCE : la première exécution a lu « notepad longue » — un
    // `longueur=…` COUPÉ EN DEUX. Le partage est un montage CIFS, et
    // `Set-Content` côté Windows n'écrit pas atomiquement : un relevé pris
    // pendant l'écriture rend une ligne tronquée, qui commence bien par le bon
    // numéro d'ordre et passe donc le test de correspondance. Le témoin de
    // mesurabilité l'a attrapé (`longueur=0` introuvable) — mais un critère
    // aurait pu être jugé sur une valeur amputée, ce qui est pire qu'une
    // mesure absente. On exige donc que DEUX relevés à 300 ms d'écart soient
    // identiques avant de rendre quoi que ce soit.
    const relever = () => {
        try { return readFileSync('/media/vm/dev/pp2-fait.txt', 'utf8').replace(/^\uFEFF/, '').trim(); }
        catch { return null; }
    };
    for (let i = 0; i < 60; i += 1) {
        await dodo(500);
        const a = relever();
        if (a === null || !a.startsWith(`${n}|`)) continue;
        await dodo(300);
        const b = relever();
        if (b !== a) continue;
        {
            const rendu = a.slice(String(n).length + 1);
            log(`  lecture ${n} (${mode}) :`, rendu.slice(0, 160));
            return rendu;
        }
    }
    log(`  !! lecture ${n} (${mode}) : aucun compte rendu du lecteur`);
    return null;
}

function lireIdentite(chemin) {
    if (!chemin) throw new Error('--identite=<fichier> est obligatoire : sans identité, '
        + "la plateforme refuse l'agent et aucune session ne s'établit");
    const o = {};
    for (const l of readFileSync(chemin, 'utf8').split('\n')) {
        const i = l.indexOf('=');
        if (i > 0) o[l.slice(0, i).trim()] = l.slice(i + 1).trim();
    }
    for (const c of ['AGENT_VM', 'AGENT_SECRET', 'PREFIXE_VM', 'RECETTE_EMAIL', 'RECETTE_MOTDEPASSE']) {
        if (!o[c]) throw new Error(`${c} manque dans ${chemin}`);
    }
    return o;
}

/// Le jeton vient de la PLATEFORME, jamais forgé ici.
async function obtenirPaire(id) {
    const r = await fetch(`${PLATEFORME_URL}/auth/connexion`, {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email: id.RECETTE_EMAIL, motdepasse: id.RECETTE_MOTDEPASSE }),
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`/auth/connexion a rendu ${r.status} (${corps?.refus ?? 'sans motif'})`);
    if (String(corps.acces).split('.').length !== 3) {
        throw new Error(`jeton d'accès sans forme de JWT : ${String(corps.acces).slice(0, 30)}`);
    }
    return corps;
}

const identite = lireIdentite(IDENTITE);
const paire = await obtenirPaire(identite);
// ⚠️ Ni le secret d'enrôlement ni le mot de passe ne sont journalisés, ni
// écrits dans le relevé JSON : ils vivent HORS du dépôt.
const SEMENCE = `(() => { try {
  localStorage.setItem('guac.jeton.acces', ${JSON.stringify(paire.acces)});
  localStorage.setItem('guac.jeton.rafraichissement', ${JSON.stringify(paire.rafraichissement)});
  localStorage.setItem('guac.prefixe', ${JSON.stringify(identite.PREFIXE_VM)});
} catch (e) { } })();`;
const URL_SHELL = `${CLIENT_URL}/shell.html?signaling=${encodeURIComponent(SIGNALING_RELAIS)}`
    + `&prefixe=${encodeURIComponent(identite.PREFIXE_VM)}`;

// 🔴 Substitution VÉRIFIÉE : un marqueur survivant produirait une URL de
// signaling littérale, la page ne se connecterait à rien, et le seul symptôme
// serait « l'agent n'a pas répondu » — indiscernable d'une panne du produit.
const AMORCE_FINALE = AMORCE.replaceAll('__SIGNALING__', SIGNALING_RELAIS);
if (AMORCE_FINALE.includes('__SIGNALING__')) throw new Error('marqueur __SIGNALING__ non substitué');
if (!AMORCE_FINALE.includes(SIGNALING_RELAIS)) throw new Error('substitution du signaling sans effet');

const dir = await mkdtemp(join(tmpdir(), `pp2-${ETIQUETTE}-`));
const port = 9440 + (DESARME ? 2 : 0) + (ETIQUETTE.endsWith('2') ? 1 : 0);
const releve = { etiquette: ETIQUETTE, desarme: DESARME, nonce: NONCE, phases: [], collages: [] };
let chrome = null;
let cdp = null;
let sidApp = null;
let sidAmorce = null;

try {
    // --- Préparation VM : aucun agent vivant, aucune fenêtre éligible. ------
    log('préparation VM');
    releve.prep = winrm(
        'Get-Process agent,chrome,notepad,mspaint -ErrorAction SilentlyContinue | Stop-Process -Force; '
        + 'Start-Sleep -Seconds 3; '
        + '$a = @(Get-Process agent -ErrorAction SilentlyContinue).Count; '
        + '$n = @(Get-Process notepad -ErrorAction SilentlyContinue).Count; '
        + 'Write-Output "PREP agent=$a notepad=$n"').trim().split('\n').pop();
    log('prep :', releve.prep);

    // --- Le BLOC-NOTES, la fenêtre capturée, AVANT le superviseur. ----------
    // C'est lui qui recevra le `Ctrl+V` que l'agent injecte : `InputInjector`
    // pose `SetForegroundWindow(hwnd)` sur la fenêtre CAPTURÉE avant toute
    // touche, et c'est donc elle qui doit être le Bloc-notes.
    // 🔴 L'OUVERTURE EST VÉRIFIÉE, PAS SUPPOSÉE, ET C'EST MESURÉ. À la
    // deuxième exécution du bras armé, `schtasks /run` n'a rien ouvert — aucune
    // ligne « fenêtre » dans `agent.log`, `enfant lancé` = 0 — et le symptôme
    // remonté était « aucune page d'application attachée », c'est-à-dire un
    // défaut d'INSTRUMENT qui se lit exactement comme une panne du produit.
    // C'est la classe que P1 avait déjà nommée sans la fermer.
    log('ouverture du Bloc-notes (la fenêtre capturée)');
    releve.notepad_ouvert = false;
    for (let essai = 1; essai <= 3 && !releve.notepad_ouvert; essai += 1) {
        vmIt(`pp2-notepad${essai}`, 'Start-Process notepad; Start-Sleep -Seconds 3');
        for (let i = 0; i < 12; i += 1) {
            await dodo(2000);
            const n = winrm('@(Get-Process notepad -ErrorAction SilentlyContinue).Count').trim().split('\n').pop();
            if (/^[1-9]/.test(n.replace(/^\uFEFF/, ''))) { releve.notepad_ouvert = true; break; }
        }
        log(`  Bloc-notes ouvert (essai ${essai}) :`, releve.notepad_ouvert);
    }
    if (!releve.notepad_ouvert) throw new Error("le Bloc-notes ne s'est pas ouvert : rien à capturer");
    await dodo(5000);

    // --- Le LECTEUR à demeure, en session 1, AVANT le superviseur. ----------
    spawnSync('cp', [`${P2}/instrument/lecteur-pp.ps1`, '/media/vm/dev/lecteur-pp.ps1'], { encoding: 'utf8' });
    spawnSync('bash', ['-c', 'rm -f /media/vm/dev/pp2-ordre.txt /media/vm/dev/pp2-fait.txt /media/vm/dev/pp2-lecteur.log'],
        { encoding: 'utf8' });
    vmIt('pp2-lecteur', 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\lecteur-pp.ps1');
    await dodo(6000);
    releve.lecteur_pret = (() => {
        try { return readFileSync('/media/vm/dev/pp2-lecteur.log', 'utf8').includes('lecteur pret'); }
        catch { return false; }
    })();
    log('lecteur prêt :', releve.lecteur_pret);
    if (!releve.lecteur_pret) throw new Error('le lecteur de session 1 ne répond pas');

    // 🔴 LE TÉMOIN DE MESURABILITÉ, joué AVANT tout critère. Il vide le
    // Bloc-notes, le relit, et EXIGE qu'il soit vide : si la lecture ne
    // fonctionne pas, tout ce qui suit serait indiscernable d'un produit muet.
    releve.temoin_vider = await lire('vider-notepad');
    releve.temoin_notepad_vide = await lire('notepad');
    releve.temoin_mesurable = /longueur=0/.test(releve.temoin_notepad_vide ?? '');
    log('témoin de mesurabilité (Bloc-notes lisible et vidé) :', releve.temoin_mesurable);

    // --- Le navigateur pilote, sur l'HÔTE. ---------------------------------
    chrome = lancerChrome(port, dir);
    cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
    const pages = new Map();
    cdp.on(async (m) => {
        if (m.method === 'Target.targetInfoChanged') {
            const p = pages.get(m.params.targetInfo.targetId);
            if (p) p.url = m.params.targetInfo.url;
            return;
        }
        if (m.method !== 'Target.attachedToTarget') return;
        const sid = m.params.sessionId;
        if (m.params.targetInfo.type !== 'page') {
            await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
            return;
        }
        const urlCible = m.params.targetInfo.url ?? '';
        pages.set(m.params.targetInfo.targetId, { sid, url: urlCible });
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
        await cdp.send('Page.enable', {}, sid).catch(() => { });
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: SEMENCE + AMORCE_FINALE }, sid).catch(() => { });
        // `addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page déjà
        // ouverte par `window.open` (D5) : on pose l'amorce explicitement.
        await cdp.send('Runtime.evaluate', { expression: SEMENCE + AMORCE_FINALE }, sid).catch(() => { });
        await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
        log('+ page attachée', sid.slice(0, 8), urlCible);
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await cdp.send('Target.setDiscoverTargets', { discover: true });
    // 🔴 AUCUNE PERMISSION DE PRESSE-PAPIER N'EST ACCORDÉE, et c'est le
    // critère ② : le produit ne doit en demander aucune. La rouge — accorder
    // `clipboardReadWrite` et voir si cela change quelque chose — est jouée en
    // fin d'exécution, et ne doit RIEN changer.

    // --- La page-shell, AVANT le superviseur. ------------------------------
    await cdp.send('Target.createTarget', { url: URL_SHELL });
    await dodo(3000);
    const sidShell = [...pages.values()].find((p) => /shell/.test(p.url ?? ''))?.sid ?? null;
    log('page-shell :', sidShell ? sidShell.slice(0, 8) : 'ABSENTE');
    releve.shell_attachee = sidShell !== null;

    // --- Le superviseur. ----------------------------------------------------
    log('lancement du superviseur', DESARME ? '(PRESSE_PAPIER_GARDE=0)' : '(armé)');
    const env = DESARME ? 'PRESSE_PAPIER_GARDE=0 ' : '';
    const r = spawnSync('bash', ['-c',
        `cd ${RACINE} && set -a && source .env && set +a && `
        + `export AGENT_VM="$1" AGENT_SECRET="$2" && SUPERVISEUR=1 `
        + `SIGNALING_URL=${SIGNALING_WS} LOCAL_IP=192.168.3.2 RUST_LOG=info ${env}scripts/run-agent.sh`,
        'pp2', identite.AGENT_VM, identite.AGENT_SECRET],
        { encoding: 'utf8', env: process.env, timeout: 180000 });
    log('run-agent.sh :', (r.stdout ?? '').trim().split('\n').pop());
    if ((r.stderr ?? '').trim()) log('run-agent.sh STDERR :', r.stderr.trim().slice(0, 300));

    // --- Attendre la page d'application. On attend le FAIT. -----------------
    for (let i = 0; i < 90; i += 1) {
        await dodo(1000);
        const inventaire = await cdp.send('Target.getTargets', {}).catch(() => ({ targetInfos: [] }));
        for (const t of inventaire.targetInfos ?? []) {
            if (t.type !== 'page' || !/\?session=/.test(t.url ?? '')) continue;
            if (pages.has(t.targetId)) continue;
            const a = await cdp.send('Target.attachToTarget', { targetId: t.targetId, flatten: true }).catch(() => null);
            if (!a) continue;
            pages.set(t.targetId, { sid: a.sessionId, url: t.url });
            await cdp.send('Runtime.enable', {}, a.sessionId).catch(() => { });
            await cdp.send('Runtime.evaluate', { expression: SEMENCE + AMORCE_FINALE }, a.sessionId).catch(() => { });
            log('+ page rattrapée par balayage', a.sessionId.slice(0, 8), t.url);
        }
        const p = [...pages.values()].find((x) => /\?session=/.test(x.url ?? ''));
        if (!p) {
            // 🔴 REPLI hérité de P1, mesuré et non supposé : la fenêtre de la
            // page-shell peut s'ouvrir PUIS se refermer, et aucune cible
            // d'application n'existe alors. La cause n'est pas établie ; ce
            // repli l'encaisse, il ne l'explique pas.
            if (i === 25) {
                let session = null;
                try {
                    // ⚠️ METTRE À PLAT D'ABORD : `tracing` sépare le nom du champ
                    // de sa valeur par des séquences ANSI, et « session= » ne
                    // matche jamais sur le journal brut (piège de D8).
                    const brut = readFileSync('/media/vm/dev/agent.log', 'utf8').replace(/\x1b\[[0-9;]*m/g, '');
                    const m = [...brut.matchAll(/enfant lanc\S* session=(\S+)/g)].pop();
                    if (m) session = m[1];
                } catch { /* journal pas encore là */ }
                if (session) {
                    const u = `${CLIENT_URL}/?session=${encodeURIComponent(session)}`
                        + `&signaling=${encodeURIComponent(SIGNALING_RELAIS)}`;
                    log("repli : ouverture directe de la page d'application", u);
                    releve.repli_page_directe = u;
                    await cdp.send('Target.createTarget', { url: u }).catch(() => { });
                }
            }
            continue;
        }
        const e = await cdp.evalBorne(p.sid, EXPR_ETAT, 8000, false);
        if (e && e.pp) { sidApp = p.sid; releve.url_app = p.url; break; }
    }
    if (!sidApp) {
        releve.statut_shell = await cdp.evalBorne(sidShell,
            "document.querySelector('#statut') && document.querySelector('#statut').textContent", 8000, false);
        releve.cibles = (await cdp.send('Target.getTargets', {}).catch(() => ({ targetInfos: [] })))
            .targetInfos?.map((t) => ({ type: t.type, url: (t.url ?? '').slice(0, 120) }));
        throw new Error("aucune page d'application attachée");
    }
    log("page d'application :", releve.url_app);

    // --- L'onglet d'AMORCE du presse-papier, avec son `<textarea>`. ---------
    // Il vit dans un onglet SÉPARÉ : `input.ts` empêcherait un `Ctrl+C` frappé
    // dans la page de session.
    const cible = await cdp.send('Target.createTarget', { url: `${CLIENT_URL}/` });
    await dodo(2500);
    sidAmorce = [...pages.entries()].find(([id]) => id === cible.targetId)?.[1]?.sid ?? null;
    if (!sidAmorce) {
        const a = await cdp.send('Target.attachToTarget', { targetId: cible.targetId, flatten: true });
        sidAmorce = a.sessionId;
        await cdp.send('Runtime.enable', {}, sidAmorce).catch(() => { });
    }
    await cdp.evalBorne(sidAmorce, `(() => {
      document.body.innerHTML = '<textarea id="amorce" style="width:600px;height:200px"></textarea>';
      return 'pret';
    })()`, 8000, false);
    log('onglet d\'amorce prêt', sidAmorce.slice(0, 8));

    // Frappes de confiance, par CDP.
    const bas = (sid, t, mods) => cdp.send('Input.dispatchKeyEvent',
        { type: 'rawKeyDown', modifiers: mods, key: t.key, code: t.code, windowsVirtualKeyCode: t.vk, nativeVirtualKeyCode: t.vk }, sid);
    const haut = (sid, t, mods) => cdp.send('Input.dispatchKeyEvent',
        { type: 'keyUp', modifiers: mods, key: t.key, code: t.code, windowsVirtualKeyCode: t.vk, nativeVirtualKeyCode: t.vk }, sid);
    const CTRL = { key: 'Control', code: 'ControlLeft', vk: 17 };
    const TOUCHE = (k, c, vk) => ({ key: k, code: c, vk });

    /// Copie `texte` dans le presse-papier du NAVIGATEUR, par une VRAIE copie.
    async function copierSurHote(texte) {
        await cdp.send('Page.bringToFront', {}, sidAmorce).catch(() => { });
        await dodo(400);
        await cdp.evalBorne(sidAmorce, `(() => {
          const t = document.getElementById('amorce');
          t.value = ${JSON.stringify(texte)};
          t.focus(); t.select();
          return t.value.length;
        })()`, 8000, false);
        await dodo(300);
        await bas(sidAmorce, CTRL, 2);
        await bas(sidAmorce, TOUCHE('c', 'KeyC', 67), 2);
        await haut(sidAmorce, TOUCHE('c', 'KeyC', 67), 2);
        await haut(sidAmorce, CTRL, 0);
        await dodo(600);
    }

    /// Colle dans la page de session, par un `Ctrl+V` DE CONFIANCE.
    async function collerDansLaSession() {
        await cdp.send('Page.bringToFront', {}, sidApp).catch(() => { });
        await dodo(600);
        await cdp.evalBorne(sidApp, `(() => {
          const v = document.getElementById('remote');
          if (v) v.focus();
          return document.activeElement ? (document.activeElement.id || document.activeElement.tagName) : null;
        })()`, 8000, false);
        await dodo(300);
        await bas(sidApp, CTRL, 2);
        await bas(sidApp, TOUCHE('v', 'KeyV', 86), 2);
        await haut(sidApp, TOUCHE('v', 'KeyV', 86), 2);
        await haut(sidApp, CTRL, 0);
    }

    const phase = async (nom) => {
        const etat = await cdp.evalBorne(sidApp, EXPR_ETAT, 12000, false);
        const p = {
            nom, t: new Date().toISOString(), etat,
            vm_clipboard: await lire('clipboard'),
            vm_notepad: await lire('notepad'),
        };
        releve.phases.push(p);
        log(`[${nom}] messages=${etat?.pp?.messages?.length} focus=${etat?.focus} actif=${etat?.actif}`);
        return p;
    };

    // Les permissions, relevées AVANT tout collage : le critère ② exige que le
    // chemin ne dépende d'AUCUNE d'elles.
    releve.permissions_avant = await cdp.evalBorne(sidApp, EXPR_PERMS, 12000, true);
    log('permissions :', JSON.stringify(releve.permissions_avant));

    await phase('0-avant-tout-collage');

    // V1, V2, V3 — trois collages de textes DIFFÉRENTS. Le troisième donne
    // k = 3 pour le critère ④.
    const textes = [`alpha-${NONCE}`, `beta-${NONCE}`, `gamma-${NONCE}`];
    for (let k = 0; k < textes.length; k += 1) {
        log(`V${k + 1} : copie sur l'hôte puis collage dans la session`);
        await copierSurHote(textes[k]);
        await collerDansLaSession();
        await dodo(6000);
        const p = await phase(`${k + 1}-apres-collage`);
        releve.collages.push({ rang: k + 1, texte: textes[k], notepad: p.vm_notepad, clipboard: p.vm_clipboard });
    }

    // 🔴 LA ROUGE DU CRITÈRE ② : accorder la permission `clipboard-read` et
    // recommencer. **Cela ne doit RIEN changer.** Si cela change quelque chose,
    // le chemin employé n'est pas celui qu'on croit, et il faudrait chercher un
    // `readText` qui n'a rien à faire là.
    log('rouge ② : on ACCORDE clipboardReadWrite, et on recolle');
    await cdp.send('Browser.grantPermissions',
        { origin: CLIENT_URL, permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'] })
        .catch((e) => { releve.grant_erreur = String(e).slice(0, 200); });
    releve.permissions_apres_grant = await cdp.evalBorne(sidApp, EXPR_PERMS, 12000, true);
    const texteGrant = `delta-${NONCE}`;
    await copierSurHote(texteGrant);
    await collerDansLaSession();
    await dodo(6000);
    const pg = await phase('4-apres-collage-avec-permission');
    releve.collage_avec_permission = { texte: texteGrant, notepad: pg.vm_notepad, clipboard: pg.vm_clipboard };
} catch (e) {
    releve.erreur = String(e).slice(0, 500);
    log('!! erreur', releve.erreur);
} finally {
    try { writeFileSync('/media/vm/dev/pp2-ordre.txt', '999|stop', 'utf8'); } catch { /* VM partie */ }
    try { releve.lecteur_journal = readFileSync('/media/vm/dev/pp2-lecteur.log', 'utf8').slice(0, 6000); }
    catch { releve.lecteur_journal = null; }
    await writeFile(SORTIE, JSON.stringify(releve, null, 2));
    log('relevé écrit dans', SORTIE);
    if (chrome) { chrome.kill('SIGKILL'); await dodo(500); }
    await rm(dir, { recursive: true, force: true });
    // Le journal se copie APRÈS la fin réelle : les enfants meurent quand le
    // navigateur se ferme, donc APRÈS la copie si on la fait trop tôt (D4).
    await dodo(4000);
    spawnSync('bash', ['-c',
        `cp /media/vm/dev/agent.log ${P2}/agent-${ETIQUETTE}.log 2>/dev/null && `
        + `sed 's/\\x1b\\[[0-9;]*m//g' ${P2}/agent-${ETIQUETTE}.log > ${P2}/agent-${ETIQUETTE}-plat.log`],
        { encoding: 'utf8' });
    log('journal copié : agent-' + ETIQUETTE + '.log (+ -plat)');
    log('virsh domstate :', (spawnSync('virsh', ['domstate', 'Windows'], { encoding: 'utf8' }).stdout ?? '?').trim());
    // 🔴 SORTIE EXPLICITE : le `WebSocket` de `Cdp` garde la boucle d'événements
    // vivante après la mort de Chrome, et le symptôme se lit comme une mesure
    // interminable alors que tout est fini (P1).
    process.exit(releve.erreur ? 1 : 0);
}
