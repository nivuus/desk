#!/usr/bin/env node
// Sous-bloc P1 (presse-papier) — le pilote de recette, tâche 16.
//
// Il mesure les QUATRE critères dans une seule exécution armée, et le seul
// critère ④ dans une exécution désarmée. Deux exécutions de chaque bras.
//
// ---------------------------------------------------------------------------
// CE QUI EST OBSERVÉ, ET POURQUOI CE POINT D'OBSERVATION ET PAS UN AUTRE
// ---------------------------------------------------------------------------
//
// 🔴 Les messages sont comptés SUR LE CANAL DE CONTRÔLE, par un écouteur
// INDÉPENDANT du produit, jamais sur les appels à `writeText`. La raison est
// que le critère ② serait sinon INCAPABLE D'ÉCHOUER : `PressePapierLocal`
// dédoublonne LUI AUSSI (`aEcrire` rend `undefined` quand `enAttente ===
// ecrit`), donc deux messages identiques ne produiraient qu'une écriture même
// si le garde de l'AGENT était retiré. Compter les écritures mesurerait le
// garde du client, pas celui qu'on veut juger. C'est le patron que ce dépôt
// paie depuis D6 (« un contrôle satisfait par un appel intercalé qui détecte
// la même chose »), et il a encore été payé en tâche 13 de ce sous-bloc.
//
// Les écritures SONT relevées elles aussi, mais pour le critère ① au NIVEAU 1 :
// le texte réellement passé à `writeText` et la RÉSOLUTION de sa promesse.
//
// ⚠️ NIVEAU 2 — « l'hôte relit ce que la page a écrit » — déclaré NON MESURABLE
// sur ce montage par le témoin de l'étape 0 (`temoin-mesurabilite-pp.mjs`,
// joué le 20 août 2026) : aucun serveur X n'est joignable, `xclip` et
// `wl-paste` sont absents, `xsel` échoue en « Can't open display ». Le pilote
// relève À LA PLACE une relecture DANS LA PAGE (`navigator.clipboard.readText`),
// qui prouve que le presse-papier du NAVIGATEUR porte bien le texte — et qui
// n'est PAS le niveau 2, parce qu'un presse-papier interne au processus
// Chromium suffirait à la satisfaire.
//
// ---------------------------------------------------------------------------
// INVARIANTS DE MONTAGE, hérités et non renégociés (voir `commun-d11.mjs`)
// ---------------------------------------------------------------------------
//
//   - navigateur pilote sur l'HÔTE, jamais sur la VM ;
//   - page-shell ouverte AVANT le superviseur (le signaling ne mémorise pas
//     les annonces `fenetre-ouverte` — D1) ;
//   - fenêtre VM ouverte AVANT le superviseur (les consoles PowerShell des
//     tâches planifiées sont éligibles à la capture — D11) ;
//   - `waitForDebuggerOnStart` + `setDiscoverTargets` : sans eux, aucune page
//     ouverte par `window.open` n'est attachée (D5, D10) ;
//   - aucune capture d'écran CDP, toute évaluation BORNÉE (D1, D2) ;
//   - copies faites dans la SESSION 1 : `Get-Clipboard` par WinRM (session 0)
//     rend `-1`, le presse-papier étant par station de fenêtres (sonde P0) ;
//   - 🔴 L'IDENTITÉ EST OBLIGATOIRE DEPUIS LE SOUS-BLOC P3 DE LA PLATEFORME.
//     Sans `AGENT_VM`/`AGENT_SECRET`, l'agent est refusé par le signaling
//     (« session de contrôle REFUSÉE … motif=jeton-absent ») et AUCUNE session
//     ne s'établit : la première exécution de cette recette l'a rencontré, et
//     le symptôme — « aucune page d'application attachée » — se lit exactement
//     comme une panne du produit. Le pilote lit donc son identité dans un
//     fichier HORS DÉPÔT (`--identite=<fichier>`), obtient un jeton de la
//     plateforme et le sème dans `localStorage` AVANT toute navigation.
//     ⚠️ Ni le secret d'enrôlement ni le mot de passe ne sont versés, ni
//     journalisés, ni écrits dans le relevé JSON.
//
// Usage :
//   node pilote-pp-p1.mjs --etiquette=arme-1 [--desarme=1] [--sortie=x.json]

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
const SORTIE = arg('sortie', `pp-${ETIQUETTE}.json`);
const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
const P1 = `${RACINE}/docs/superpowers/plans/journaux-presse-papier-p1`;
const VMIT = `${RACINE}/docs/superpowers/plans/journaux-multifenetres-d10/instrument/vm-it.sh`;
const HOTE = process.env.HOTE ?? '192.168.3.1';
const IDENTITE = arg('identite', '');
const PLATEFORME_URL = arg('plateforme', 'http://127.0.0.1:8090');
const CLIENT_URL = arg('client', 'http://127.0.0.1:5173');
const SIGNALING_WS = arg('signaling', `ws://${process.env.HOTE ?? '192.168.3.1'}:8090`);
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

// L'amorce. Elle pose DEUX observateurs indépendants et ne touche à rien
// d'autre. Aucun accent grave : c'est un littéral de gabarit.
const AMORCE = `
(() => {
  if (window.__pp) return;
  window.__pp = { messages: [], ecritures: [], erreurs: [] };
  const SIGNALING_DE_RECETTE = '__SIGNALING__';
  // 1) Le canal de controle, hameconne a la CREATION : c'est le point
  //    d'observation qui compte des MESSAGES et non des ecritures.
  const N = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) {
    const p = new N(...a);
    window.__pc = p;
    const c = p.createDataChannel.bind(p);
    p.createDataChannel = function (label, opts) {
      const ch = c(label, opts);
      if (label === 'control') {
        ch.addEventListener('message', (e) => {
          try {
            const m = JSON.parse(e.data);
            if (m && m.type === 'clipboard') {
              window.__pp.messages.push({
                t: Date.now(),
                texte_present: m.text !== null && m.text !== undefined,
                longueur: m.text == null ? 0 : m.text.length,
                // Les nonces de recette sont choisis par le pilote : les
                // verser est sans risque. Un contenu long n'est JAMAIS versé
                // (D-P1-7) -- seuls ses 60 premiers caracteres.
                debut: m.text == null ? null : String(m.text).slice(0, 60),
                bytes: m.bytes,
                v: m.v,
              });
            }
          } catch (err) { window.__pp.erreurs.push(String(err).slice(0, 120)); }
        });
      }
      return ch;
    };
    return p;
  };
  window.RTCPeerConnection.prototype = N.prototype;
  // 1bis) 🔴 LE SIGNALING DE LA PAGE D'APPLICATION.
  //   shell-page.ts ouvre /?session=<id> SANS parametre signaling, et main.ts
  //   retombe alors sur ws://<hote>:8080 en dur. Cette recette parle a une
  //   SECONDE instance de plateforme (voir l'en-tete du pilote) : on ajoute
  //   donc le parametre A L'OUVERTURE, dans la page-shell.
  //
  //   ⚠️ Une premiere version renavigait la cible depuis CDP, sur
  //   Target.attachedToTarget. Elle ne pouvait pas marcher : une cible ouverte
  //   par window.open s'attache avec une URL VIDE, l'URL n'arrivant qu'au
  //   targetInfoChanged suivant -- le test d'URL etait donc toujours faux, EN
  //   SILENCE. Releve a la premiere execution.
  try {
    const O = window.open;
    window.open = function (url, ...reste) {
      let u = url;
      if (typeof u === 'string' && u.indexOf('?session=') !== -1 && u.indexOf('signaling=') === -1) {
        u = u + '&signaling=' + encodeURIComponent(SIGNALING_DE_RECETTE);
      }
      return O.call(window, u, ...reste);
    };
  } catch (e) { window.__pp.erreurs.push('hameconnage window.open : ' + String(e).slice(0, 120)); }
  // 2) L'ecriture reelle -- le NIVEAU 1 du critere ①.
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      const orig = navigator.clipboard.writeText.bind(navigator.clipboard);
      Object.defineProperty(navigator.clipboard, 'writeText', {
        configurable: true,
        value: (t) => {
          const rec = { t: Date.now(), longueur: String(t).length, debut: String(t).slice(0, 60) };
          window.__pp.ecritures.push(rec);
          return orig(t).then(
            () => { rec.resolue = true; },
            (e) => { rec.resolue = false; rec.erreur = String(e).slice(0, 120); throw e; });
        },
      });
    } else { window.__pp.erreurs.push('navigator.clipboard absent'); }
  } catch (e) { window.__pp.erreurs.push('hameconnage writeText : ' + String(e).slice(0, 120)); }
})();
`;

/** Relève l'état complet d'une page d'application. */
const EXPR_ETAT = `(() => {
  const s = document.querySelector('#status');
  return {
    pp: window.__pp || null,
    focus: document.hasFocus(),
    statut_texte: s ? s.textContent : null,
    statut_cache: s ? s.dataset.hidden : null,
    video_pret: !!(document.querySelector('#remote') && document.querySelector('#remote').videoWidth > 0),
  };
})()`;

/** Relecture du presse-papier DU NAVIGATEUR. Ce n'est PAS le niveau 2. */
const EXPR_RELIRE = `(async () => {
  try { return { lu: await navigator.clipboard.readText() }; }
  catch (e) { return { erreur: String(e).slice(0, 160) }; }
})()`;

/// Une copie dans la SESSION 1, par le COPIEUR à demeure (`copieur-pp.ps1`).
///
/// 🔴 Ce n'est PAS un `vm-it.sh` par geste, et la raison est mesurée : la
/// première exécution de cette recette copiait ainsi, et **une page navigateur
/// de plus s'ouvrait 0,5 s après CHAQUE appel** — la console PowerShell d'une
/// tâche planifiée est éligible à la capture (piège de D11), le superviseur lui
/// donnait une session, la page-shell lui ouvrait une fenêtre, la fenêtre de
/// session perdait le focus et sa page était renavigée. **L'instrument
/// détruisait ce qu'il mesurait.** Ici, un seul processus, né avant le
/// superviseur, et plus rien ne s'ouvre pendant la mesure.
///
/// L'attente porte sur le FAIT — le numéro d'ordre relu dans `pp-fait.txt` —
/// jamais sur une durée.
let numeroOrdre = 0;
async function copier(mode, texte = '') {
    numeroOrdre += 1;
    const n = numeroOrdre;
    writeFileSync('/media/vm/dev/pp-ordre.txt', `${n}|${mode}|${texte}`, 'utf8');
    for (let i = 0; i < 120; i += 1) {
        await dodo(500);
        let brut = '';
        try { brut = readFileSync('/media/vm/dev/pp-fait.txt', 'utf8').trim(); } catch { continue; }
        if (brut.startsWith(`${n}|`)) {
            const rendu = brut.slice(String(n).length + 1);
            log(`  copie ${n} (${mode}) :`, rendu.slice(0, 120));
            return rendu;
        }
    }
    log(`  !! copie ${n} (${mode}) : aucun compte rendu du copieur`);
    return null;
}

/// Lit le fichier d'identité (hors dépôt). Rend un objet de clés simples.
///
/// 🔴 L'IDENTITÉ EST OBLIGATOIRE DEPUIS LE SOUS-BLOC P3 DE LA PLATEFORME.
/// Sans elle, l'agent est refusé par le signaling (« session de contrôle
/// REFUSÉE … motif=jeton-absent ») et AUCUNE session ne s'établit : la
/// première exécution de cette recette l'a rencontré, et le symptôme —
/// « aucune page d'application attachée » — se lit exactement comme une panne
/// du produit.
function lireIdentite(chemin) {
    if (!chemin) throw new Error('--identite=<fichier> est obligatoire : sans identité, '
        + 'la plateforme refuse l\'agent et aucune session ne s\'établit');
    const brut = readFileSync(chemin, 'utf8');
    const o = {};
    for (const l of brut.split('\n')) {
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
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email: id.RECETTE_EMAIL, motdepasse: id.RECETTE_MOTDEPASSE }),
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`/auth/connexion a rendu ${r.status} (${corps?.refus ?? 'sans motif'})`);
    // 🔴 GARDE : un jeton sans forme de JWT serait semé quand même, la page ne
    // redirigerait pas (chaîne non vide) et le seul symptôme serait un
    // « jeton refusé (forme) » côté service. Piège relevé et payé en F1.
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
const URL_SHELL = `${CLIENT_URL}/shell.html?signaling=${encodeURIComponent(SIGNALING_WS)}`
    + `&prefixe=${encodeURIComponent(identite.PREFIXE_VM)}`;

// 🔴 Substitution VERIFIEE : un marqueur survivant produirait une URL de
// signaling litterale, la page ne se connecterait a rien, et le seul symptome
// serait « l'agent n'a pas repondu » -- indiscernable d'une panne du produit.
// C'est le controle que F1 a du ajouter apres l'avoir paye.
const AMORCE_FINALE = AMORCE.replaceAll('__SIGNALING__', SIGNALING_WS);
if (AMORCE_FINALE.includes('__SIGNALING__')) throw new Error('marqueur __SIGNALING__ non substitue');
if (!AMORCE_FINALE.includes(SIGNALING_WS)) throw new Error('substitution du signaling sans effet');

const dir = await mkdtemp(join(tmpdir(), `pp-p1-${ETIQUETTE}-`));
const port = 9420 + (DESARME ? 2 : 0) + (ETIQUETTE.endsWith('2') ? 1 : 0);
const releve = { etiquette: ETIQUETTE, desarme: DESARME, nonce: NONCE, phases: [] };
let chrome = null;
try {
    // --- Préparation VM : aucun agent vivant, aucune fenêtre éligible. ------
    log('préparation VM');
    releve.prep = winrm(
        "Get-Process agent,chrome,notepad,mspaint -ErrorAction SilentlyContinue | Stop-Process -Force; "
        + 'Start-Sleep -Seconds 3; '
        + '$a = @(Get-Process agent -ErrorAction SilentlyContinue).Count; '
        + '$c = @(Get-Process chrome -ErrorAction SilentlyContinue).Count; '
        + 'Write-Output "PREP agent=$a chrome=$c"').trim().split('\n').pop();
    log('prep :', releve.prep);
    spawnSync('cp', [`${RACINE}/docs/superpowers/plans/journaux-multifenetres-d11/instrument/anim-d11.html`,
        '/media/vm/dev/anim-d11.html'], { encoding: 'utf8' });

    // --- Le navigateur pilote, sur l'HÔTE. ---------------------------------
    chrome = lancerChrome(port, dir);
    const cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
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
        // 🔴 LA PAGE D'APPLICATION EST OUVERTE PAR `window.open('/?session=…')`
        // (`shell-page.ts`), SANS paramètre `signaling` : `main.ts` retombe
        // alors sur `ws://<hôte>:8080` en dur. Or cette recette parle à une
        // SECONDE instance de plateforme, sur 8090 — voir l'en-tête. On
        // renavigue donc la page vers la MÊME URL augmentée du paramètre,
        // AVANT que son premier script ne coure (`waitForDebuggerOnStart`).
        // C'est un geste d'INSTRUMENT, déclaré : il ne change rien au produit,
        // et `main.ts:34-35` documente lui-même ce paramètre comme prévu « pour
        // faciliter les essais ».
        // `addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page déjà
        // ouverte par `window.open` (D5) : on pose l'amorce explicitement.
        await cdp.send('Runtime.evaluate', { expression: SEMENCE + AMORCE_FINALE }, sid).catch(() => { });
        await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
        log('+ page attachée', sid.slice(0, 8), urlCible);
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await cdp.send('Target.setDiscoverTargets', { discover: true });
    await cdp.send('Browser.grantPermissions',
        { origin: 'http://127.0.0.1:5173', permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'] })
        .catch((e) => { releve.grant_erreur = String(e).slice(0, 200); });

    // --- La fenêtre de la VM, AVANT le superviseur. -------------------------
    log('ouverture de la fenêtre VM (mire animée)');
    vmIt('pp-fenetre', [
        '$a = @(',
        '  "--app=file:///C:/dev/anim-d11.html?n=1",',
        '  "--user-data-dir=C:\\dev\\chrome-pp-p1",',
        "  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',",
        "  '--window-size=1280,720','--window-position=40,40',",
        "  '--disable-features=CalculateNativeWinOcclusion',",
        "  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',",
        "  '--disable-renderer-backgrounding')",
        "Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a",
        'Start-Sleep -Seconds 3',
    ].join('\n'));
    await dodo(9000);

    // --- Le COPIEUR à demeure, en session 1, AVANT le superviseur. ---------
    spawnSync('cp', [`${P1}/instrument/copieur-pp.ps1`, '/media/vm/dev/copieur-pp.ps1'], { encoding: 'utf8' });
    spawnSync('bash', ['-c', 'rm -f /media/vm/dev/pp-ordre.txt /media/vm/dev/pp-fait.txt /media/vm/dev/pp-copieur.log'],
        { encoding: 'utf8' });
    vmIt('pp-copieur', 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\copieur-pp.ps1');
    await dodo(6000);
    releve.copieur_pret = (() => {
        try { return readFileSync('/media/vm/dev/pp-copieur.log', 'utf8').includes('copieur pret'); }
        catch { return false; }
    })();
    log('copieur prêt :', releve.copieur_pret);

    // --- La page-shell, AVANT le superviseur. ------------------------------
    await cdp.send('Target.createTarget', { url: URL_SHELL });
    await dodo(3000);
    const sidShell = [...pages.values()].find((p) => /shell/.test(p.url ?? ''))?.sid ?? null;
    log('page-shell :', sidShell ? sidShell.slice(0, 8) : 'ABSENTE');
    releve.shell_attachee = sidShell !== null;

    // --- Le superviseur. ----------------------------------------------------
    log('lancement du superviseur', DESARME ? '(PRESSE_PAPIER=0)' : '(armé)');
    const env = DESARME ? 'PRESSE_PAPIER=0 ' : '';
    const r = spawnSync('bash', ['-c',
        `cd ${RACINE} && set -a && source .env && set +a && `
        + `export AGENT_VM="$1" AGENT_SECRET="$2" && SUPERVISEUR=1 `
        + `SIGNALING_URL=${SIGNALING_WS} LOCAL_IP=192.168.3.2 RUST_LOG=info ${env}scripts/run-agent.sh`,
        'pp', identite.AGENT_VM, identite.AGENT_SECRET],
        { encoding: 'utf8', env: process.env, timeout: 180000 });
    log('run-agent.sh :', (r.stdout ?? '').trim().split('\n').pop());
    if ((r.stderr ?? '').trim()) log('run-agent.sh STDERR :', r.stderr.trim().slice(0, 300));

    // --- Attendre qu'une page d'application soit attachée ET que sa session
    //     porte un canal de contrôle. On attend le FAIT, jamais une durée.
    //
    // 🔴 L'AUTO-ATTACHE NE SUFFIT PAS, ET C'EST MESURÉ. À la deuxième exécution,
    // la page-shell a bien ouvert sa fenêtre — le superviseur a reçu le
    // viewport et lancé l'enfant, ce qui l'établit — mais `Target.attachedToTarget`
    // n'a JAMAIS été émis pour elle : deux cibles attachées en tout sur toute
    // l'exécution. On balaie donc aussi `Target.getTargets` et on s'attache
    // explicitement à ce qu'on y trouve. Sans ce balayage, l'exécution rendait
    // « aucune page d'application attachée », c'est-à-dire un défaut
    // d'INSTRUMENT qui se lit comme une panne du produit.
    let sidApp = null;
    for (let i = 0; i < 90; i += 1) {
        await dodo(1000);
        const inventaire = await cdp.send('Target.getTargets', {}).catch(() => ({ targetInfos: [] }));
        for (const t of inventaire.targetInfos ?? []) {
            if (t.type !== 'page' || !/\?session=/.test(t.url ?? '')) continue;
            if (pages.has(t.targetId)) continue;
            const r = await cdp.send('Target.attachToTarget', { targetId: t.targetId, flatten: true })
                .catch(() => null);
            if (!r) continue;
            pages.set(t.targetId, { sid: r.sessionId, url: t.url });
            await cdp.send('Runtime.enable', {}, r.sessionId).catch(() => { });
            await cdp.send('Runtime.evaluate', { expression: SEMENCE + AMORCE_FINALE }, r.sessionId).catch(() => { });
            log('+ page rattrapée par balayage', r.sessionId.slice(0, 8), t.url);
        }
        const p = [...pages.values()].find((x) => /\?session=/.test(x.url ?? ''));
        if (!p) {
            // 🔴 REPLI : ouvrir la page d'application NOUS-MÊMES, sur la session
            // que l'agent a nommée dans son journal.
            //
            // La fenêtre de la page-shell a rempli son seul rôle indispensable
            // dès qu'elle a existé : porter le viewport à l'opener par
            // `postMessage` (`shell.ts::viewportRecu`) — sans quoi le
            // superviseur ne lancerait aucun enfant. Une fois `enfant lancé`
            // au journal, ce rôle est TENU, et la session attend simplement une
            // offre sur le signaling.
            //
            // ⚠️ POURQUOI CE REPLI EXISTE, mesuré et non supposé : à la deuxième
            // exécution, le viewport est bien arrivé (`enfant lancé … largeur=780
            // hauteur=492`) mais AUCUNE cible d'application n'existait plus
            // 90 secondes durant — ni par auto-attache, ni au balayage de
            // `Target.getTargets`, qui ne listait que l'onglet neuf et la
            // page-shell. La fenêtre s'était donc ouverte PUIS refermée. La
            // CAUSE n'est pas établie, et ce repli ne l'explique pas : il
            // l'encaisse. Déclaré comme tel.
            if (i === 25) {
                let session = null;
                try {
                    // ⚠️ METTRE À PLAT D'ABORD. `tracing` sépare le nom du champ
                    // de sa valeur par des séquences ANSI (`\x1b[3msession\x1b[0m
                    // \x1b[2m=\x1b[0m…`) : une expression cherchant « session= »
                    // littéralement ne matche JAMAIS sur le journal brut. C'est
                    // le piège du `grep` de la recette d'entrée de D8, rejoué —
                    // et il a coûté une exécution ici avant d'être vu.
                    const brut = readFileSync('/media/vm/dev/agent.log', 'utf8')
                        .replace(/\x1b\[[0-9;]*m/g, '');
                    const m = [...brut.matchAll(/enfant lanc\S* session=(\S+)/g)].pop();
                    if (m) session = m[1];
                } catch { /* journal pas encore là */ }
                if (session) {
                    const u = `${CLIENT_URL}/?session=${encodeURIComponent(session)}`
                        + `&signaling=${encodeURIComponent(SIGNALING_WS)}`;
                    log('repli : ouverture directe de la page d\'application', u);
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
        throw new Error('aucune page d\'application attachée');
    }
    log('page d\'application :', releve.url_app);
    // Le focus : `writeText` échoue sur un document qui ne l'a pas.
    await cdp.send('Page.bringToFront', {}, sidApp).catch(() => { });
    await dodo(4000);

    const phase = async (nom) => {
        const etat = await cdp.evalBorne(sidApp, EXPR_ETAT, 12000, false);
        const relu = await cdp.evalBorne(sidApp, EXPR_RELIRE, 12000, true);
        const p = { nom, t: new Date().toISOString(), etat, relecture_navigateur: relu };
        releve.phases.push(p);
        log(`[${nom}]`, JSON.stringify({
            messages: etat?.pp?.messages?.length, ecritures: etat?.pp?.ecritures?.length,
            focus: etat?.focus, statut: etat?.statut_texte, cache: etat?.statut_cache,
            relu: relu?.lu ? String(relu.lu).slice(0, 40) : relu,
        }));
        return p;
    };

    await phase('0-avant-toute-copie');

    // C1 — une copie neuve. Critère ①.
    log('C1 : copie neuve');
    await copier('texte', `alpha-${NONCE}`);
    await dodo(6000);
    await phase('1-apres-copie-neuve');

    // C2 — LA MÊME copie, à l'identique. Critère ②.
    log('C2 : recopie IDENTIQUE');
    await copier('texte', `alpha-${NONCE}`);
    await dodo(6000);
    await phase('2-apres-recopie-identique');

    // C3 — une copie différente : c'est ce qui rend ② DISCRIMINANT. Sans elle,
    // « aucun message » serait aussi le relevé d'un mécanisme mort.
    log('C3 : copie différente');
    await copier('texte', `beta-${NONCE}`);
    await dodo(6000);
    await phase('3-apres-copie-differente');

    // C4 — 100 KiB. Critère ③.
    log('C4 : copie de 100 KiB');
    await copier('gros');
    await dodo(8000);
    await phase('4-apres-copie-100kio');

    // C5 — la VRAIE copie Bloc-notes. Critère ①, geste authentique.
    log('C5 : copie réelle dans le Bloc-notes');
    await copier('notepad', `gamma-${NONCE}`);
    await dodo(10000);
    await phase('5-apres-copie-bloc-notes');
} catch (e) {
    releve.erreur = String(e).slice(0, 500);
    log('!! erreur', releve.erreur);
} finally {
    try { writeFileSync('/media/vm/dev/pp-ordre.txt', '999|stop|', 'utf8'); } catch { /* VM partie */ }
    try { releve.copieur_journal = readFileSync('/media/vm/dev/pp-copieur.log', 'utf8').slice(0, 4000); }
    catch { releve.copieur_journal = null; }
    await writeFile(SORTIE, JSON.stringify(releve, null, 2));
    log('relevé écrit dans', SORTIE);
    if (chrome) { chrome.kill('SIGKILL'); await dodo(500); }
    await rm(dir, { recursive: true, force: true });
    // Le journal se copie APRÈS la fin réelle : les enfants meurent quand le
    // navigateur se ferme, donc APRÈS la copie si on la fait trop tôt (D4).
    await dodo(4000);
    spawnSync('bash', ['-c',
        `cp /media/vm/dev/agent.log ${P1}/agent-${ETIQUETTE}.log 2>/dev/null && `
        + `sed 's/\\x1b\\[[0-9;]*m//g' ${P1}/agent-${ETIQUETTE}.log > ${P1}/agent-${ETIQUETTE}-plat.log`],
        { encoding: 'utf8' });
    log('journal copié : agent-' + ETIQUETTE + '.log (+ -plat)');
    log('virsh domstate :', (spawnSync('virsh', ['domstate', 'Windows'], { encoding: 'utf8' }).stdout ?? '?').trim());
    // 🔴 SORTIE EXPLICITE. Le `WebSocket` de `Cdp` garde la boucle d'événements
    // vivante après la mort de Chrome : la première exécution a écrit son
    // relevé, copié son journal, puis N'EST JAMAIS SORTIE — le harnais l'a
    // basculée en arrière-plan au bout de dix minutes, et le symptôme se lit
    // comme une mesure interminable alors que tout était fini depuis deux
    // minutes. Tout ce qui compte est déjà écrit ici.
    process.exit(releve.erreur ? 1 : 0);
}
