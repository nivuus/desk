#!/usr/bin/env node
// Sous-bloc P3 (presse-papier) — le pilote de recette MULTI-FENÊTRES, tâche 9.
//
// Dérivé de `journaux-presse-papier-p2/instrument/pilote-pp-p2.mjs`, RÉEMPLOYÉ
// et non réécrit (D-P3-10). Ce qui change tient en quatre points :
//
//   1. N Bloc-notes, N sessions, N fenêtres navigateur — et une ATTRIBUTION
//      entre les deux, qui n'existait pas avant la tâche 8 ;
//   2. un écouteur de canal de contrôle PAR FENÊTRE ;
//   3. un relevé de focus PAR FENÊTRE ET PAR PHASE ;
//   4. le nombre de fenêtres se RELÈVE, il ne s'exige pas.
//
// ---------------------------------------------------------------------------
// 🔴 CE QUE LA SONDE S1 A DÉJÀ ÉTABLI, ET QUI COMMANDE CE PILOTE
// ---------------------------------------------------------------------------
//
// Le critère ④ N'EST PAS MESURABLE en conditions de produit, et ce n'est PAS
// une conjecture : la sonde S1 (`p3-focus-{1,2}.json`, deux exécutions) relève
// que trois fenêtres ouvertes par `window.open` — LE GESTE DU PRODUIT,
// `client/src/shell-page.ts` — rapportent TOUTES `document.hasFocus() === true`,
// aux trois basculements. Sa seconde arme montre que `Target.createTarget`
// discrimine parfaitement : la cause est le MODE D'OUVERTURE, pas le
// `--headless`.
//
// ⚠️ CE PILOTE RELÈVE QUAND MÊME LE FOCUS DE CHAQUE FENÊTRE À CHAQUE PHASE, et
// il le fait pour deux raisons :
//   - le rejeu de S1 EN CONDITIONS DE PRODUIT est le Step 2 de la tâche 10, et
//     il PEUT contredire la sonde — les fenêtres du produit portent un flux
//     WebRTC, ce que celles de S1 n'avaient pas ;
//   - si le focus est vrai PARTOUT, alors TOUTES les fenêtres écrivent leur
//     presse-papier local, c'est-à-dire le régime de N ÉCRIVAINS CONCURRENTS
//     que D-P3-4 nomme et que rien ne mesure. La recette le rencontrera par
//     accident ; le relevé doit permettre de le DIRE.
//
// ---------------------------------------------------------------------------
// CE QUI EST OBSERVÉ, ET POURQUOI CE POINT D'OBSERVATION ET PAS UN AUTRE
// ---------------------------------------------------------------------------
//
// 🔴 Les messages `clipboard` AGENT → CLIENT sont comptés SUR LE CANAL DE
// CONTRÔLE, par un écouteur INDÉPENDANT du produit, POSÉ PAR FENÊTRE — jamais
// sur les appels à `writeText`. `PressePapierLocal` dédoublonne LUI AUSSI, si
// bien que compter les écritures mesurerait le garde du CLIENT et non celui de
// l'AGENT. C'est la leçon la plus réutilisable de la recette de P1, reprise
// telle quelle par P2, et P3 l'étend à N.
//
// 🔴 L'ATTRIBUTION session ↔ fenêtre Windows passe par le `pid` de la trace
// `fenêtre attachée au capteur` (tâche 8), JAMAIS par un nonce collé dont on
// regarderait quel Bloc-notes a grandi : cette voie-là est CIRCULAIRE — elle
// établirait l'attribution par le mécanisme même que ② et ③ mesurent. C'est le
// défaut que D8 a payé sur `resoudreIdentite`.
//
// ⚠️ Les trois Bloc-notes sont PRÉ-SEMÉS de marqueurs distincts (D-P3-9), sans
// quoi ils s'intitulent tous « Sans titre - Bloc-notes ». Ce marqueur rend le
// CONTENU attribuable ; il ne dit PAS quelle session pilote quelle fenêtre, et
// les confondre referait la circularité ci-dessus.
//
// ---------------------------------------------------------------------------
// INVARIANTS DE MONTAGE, hérités et NON renégociés
// ---------------------------------------------------------------------------
//
//   - navigateur pilote sur l'HÔTE, jamais sur la VM (sur la VM, la fenêtre de
//     la page-shell est elle-même capturée, ce qui boucle en cascade) ;
//   - les N Bloc-notes ouverts AVANT le superviseur ;
//   - LECTEUR à demeure, un seul processus, né AVANT le superviseur : une tâche
//     planifiée par lecture ouvrirait une console ÉLIGIBLE à la capture (D11),
//     et l'instrument détruirait ce qu'il mesure (payé en P1) ;
//   - page-shell ouverte AVANT le superviseur (le signaling ne mémorise pas les
//     annonces `fenetre-ouverte` — D1) ;
//   - `Target.getTargets` EN PLUS de l'auto-attache, et une cible ouverte par
//     `window.open` s'attache avec une URL VIDE : un instrument qui teste l'URL
//     à `Target.attachedToTarget` ne peut JAMAIS réussir, en silence (P1) ;
//   - aucune capture d'écran CDP, toute évaluation BORNÉE (D1, D2) ;
//   - deux relevés identiques à 300 ms d'écart avant de croire une lecture :
//     CIFS et `Set-Content` ne sont pas atomiques, et P2 a lu une ligne
//     TRONQUÉE qui passait son test de correspondance ;
//   - identité obligatoire depuis P3 de la plateforme : sans
//     `AGENT_VM`/`AGENT_SECRET` l'agent est refusé, et le symptôme se lit
//     exactement comme une panne du produit ;
//   - 🔴 `Emulation.setFocusEmulationEnabled` N'EST JAMAIS APPELÉE (E9) : elle
//     rendrait ④ vacueux EN SILENCE, et le relevé le déclare ;
//   - SORTIE EXPLICITE : le `WebSocket` de `Cdp` garde la boucle d'événements
//     vivante après la mort de Chrome, et le symptôme se lit comme une mesure
//     interminable alors que tout est fini (P1).
//
// 🔴 CE PILOTE N'A JAMAIS ÉTÉ EXÉCUTÉ. Il est écrit pendant que la VM est tenue
// par un chantier voisin (RP3-4), et la tâche 10 est un préalable EXTERNE. Tout
// ce qu'il contient est du raisonnement porté par les instruments de P1 et P2,
// eux mesurés ; rien n'y est un relevé.
//
// Usage :
//   node pilote-pp-p3.mjs --etiquette=1 --identite=<fichier> [--fenetres=3]

import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-multifenetres-d11/instrument/commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const ETIQUETTE = arg('etiquette', '1');
const SORTIE = arg('sortie', `pp3-${ETIQUETTE}.json`);
const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
const P3 = `${RACINE}/docs/superpowers/plans/journaux-presse-papier-p3`;
const VMIT = `${RACINE}/docs/superpowers/plans/journaux-multifenetres-d10/instrument/vm-it.sh`;
const IDENTITE = arg('identite', '');
const PLATEFORME_URL = arg('plateforme', 'http://127.0.0.1:8090');
const CLIENT_URL = arg('client', 'http://127.0.0.1:5174');
const SIGNALING_WS = arg('signaling', `ws://${process.env.HOTE ?? '192.168.3.1'}:8090`);
// ⚠️ Le nombre DEMANDÉ. Le nombre OBTENU est relevé, et c'est lui qui compte
// (D-P3-8). Moins de deux ⟹ P3 n'est pas livrable (RP3-1).
const N_DEMANDE = Number(arg('fenetres', '3'));
const NONCE = `${ETIQUETTE}-${Math.random().toString(36).slice(2, 8)}`;
const MARQUEURS = ['AAA', 'BBB', 'CCC', 'DDD', 'EEE', 'FFF'];
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

// L'amorce. Elle pose UN observateur indépendant PAR PAGE, et ne touche à rien
// d'autre. Reprise verbatim de P2, à ceci près qu'elle nomme `__pp3`.
const AMORCE = `(() => {
  if (window.__pp3) return;
  window.__pp3 = { messages: [], erreurs: [], ecritures: [] };
  // 🔴 L'OBSERVATION DE ④ : les APPELS a writeText, par page, releves en
  // ENVELOPPANT l'API — jamais en lisant le presse-papier local, qui est
  // PARTAGE entre les pages d'un meme navigateur et ne dirait donc pas
  // LAQUELLE a ecrit.
  //
  // ⚠️ Le plan met en garde contre « compter les appels a writeText » : cette
  // garde vise le comptage des messages de l'AGENT, que le dedoublonnage du
  // client masquerait. Ici on veut precisement observer la DECISION DU CLIENT,
  // et c'est le seul observable qui la porte.
  try {
    const c = navigator.clipboard;
    if (c && c.writeText) {
      const w = c.writeText.bind(c);
      c.writeText = function (t) {
        try { window.__pp3.ecritures.push({ t: Date.now(), debut: String(t).slice(0, 60) }); }
        catch (e) { window.__pp3.erreurs.push(String(e).slice(0, 120)); }
        return w(t);
      };
    } else { window.__pp3.erreurs.push('navigator.clipboard.writeText absent'); }
  } catch (e) { window.__pp3.erreurs.push('hameconnage writeText : ' + String(e).slice(0, 120)); }
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
                  window.__pp3.messages.push({
                    t: Date.now(),
                    octets: m.bytes,
                    nul: m.text === null,
                    debut: m.text === null ? null : String(m.text).slice(0, 60),
                  });
                }
              } catch (err) { window.__pp3.erreurs.push(String(err).slice(0, 120)); }
            });
          }
        } catch (err) { window.__pp3.erreurs.push(String(err).slice(0, 120)); }
        return ch;
      };
    } else { window.__pp3.erreurs.push('RTCPeerConnection absent'); }
  } catch (e) { window.__pp3.erreurs.push('hameconnage createDataChannel : ' + String(e).slice(0, 120)); }
  // La page-shell ouvre '/?session=...' SANS parametre 'signaling' : main.ts
  // retombe alors sur ws://<hote>:8080 en dur, or cette recette parle a une
  // SECONDE instance. Geste d'INSTRUMENT, declare.
  try {
    const o = window.open.bind(window);
    window.open = function (url, ...r) {
      try {
        if (typeof url === 'string' && url.includes('?session=') && !url.includes('signaling=')) {
          url += (url.includes('?') ? '&' : '?') + 'signaling=' + encodeURIComponent('__SIGNALING__');
        }
      } catch (e) { window.__pp3.erreurs.push(String(e).slice(0, 120)); }
      return o(url, ...r);
    };
  } catch (e) { window.__pp3.erreurs.push('hameconnage window.open : ' + String(e).slice(0, 120)); }
})();`;

// Le triplet de focus ET les messages, DANS LE MÊME aller-retour : `isActive`
// est transitoire, et le relever à part mesurerait un autre instant.
const EXPR_ETAT = `(() => ({
  pp: window.__pp3 || null,
  ecritures: window.__pp3 ? window.__pp3.ecritures : null,
  focus: document.hasFocus(),
  actif: navigator.userActivation ? navigator.userActivation.isActive : null,
  dejaActif: navigator.userActivation ? navigator.userActivation.hasBeenActive : null,
  visible: !document.hidden,
  elementActif: document.activeElement ? (document.activeElement.id || document.activeElement.tagName) : null,
  statut_texte: (document.querySelector('#status') || {}).textContent || null,
  statut_cache: document.querySelector('#status') ? document.querySelector('#status').dataset.hidden : null,
}))()`;

/// Le lecteur, en session 1. L'attente porte sur le FAIT — le numéro d'ordre
/// relu dans `pp3-fait.txt` —, jamais sur une durée.
let numeroOrdre = 0;
async function lire(ordre) {
    numeroOrdre += 1;
    const n = numeroOrdre;
    writeFileSync('/media/vm/dev/pp3-ordre.txt', `${n}|${ordre}`, 'utf8');
    // 🔴 DEUX RELEVÉS IDENTIQUES À 300 ms D'ÉCART, ET CE N'EST PAS DE LA
    // PRUDENCE : le partage est un montage CIFS, `Set-Content` n'écrit pas
    // atomiquement, et P2 a lu une ligne TRONQUÉE qui commençait bien par le
    // bon numéro d'ordre et passait donc son test de correspondance.
    const relever = () => {
        try { return readFileSync('/media/vm/dev/pp3-fait.txt', 'utf8').replace(/^\uFEFF/, '').trim(); }
        catch { return null; }
    };
    for (let i = 0; i < 60; i += 1) {
        await dodo(500);
        const a = relever();
        if (a === null || !a.startsWith(`${n}|`)) continue;
        await dodo(300);
        const b = relever();
        if (b !== a) continue;
        const rendu = a.slice(String(n).length + 1);
        log(`  lecture ${n} (${ordre}) :`, rendu.slice(0, 160));
        return rendu;
    }
    log(`  !! lecture ${n} (${ordre}) : aucun compte rendu du lecteur`);
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

/// Le jeton vient de la PLATEFORME, jamais forgé ici : le forger mettrait
/// `PLATEFORME_SECRET_JETON` dans un fichier versionné ou dans un `argv`, et le
/// trou serait DÉPLACÉ, pas fermé (P2).
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

/// Lit le texte de CHAQUE fenêtre attribuée, **SÉQUENTIELLEMENT**.
///
/// 🔴 **JAMAIS EN PARALLÈLE, ET C'EST MESURÉ.** Le protocole du lecteur tient
/// dans UN SEUL couple de fichiers (`pp3-ordre.txt` / `pp3-fait.txt`) : deux
/// `lire()` concurrents s'écrasent l'ordre l'un de l'autre, et les perdants
/// EXPIRENT. La première rédaction lisait les trois fenêtres d'un coup : deux
/// sur trois rendaient `null`, ce qui se lit EXACTEMENT comme « le Bloc-notes
/// est introuvable », c'est-à-dire comme un défaut du produit. Le pré-semage,
/// lui, réussissait sur les trois (`ecrit longueur=13` × 3) — c'est ce qui a
/// permis de trancher.
async function lireLesFenetres(attributions) {
    const sortie = [];
    for (const a of attributions) {
        sortie.push({ session: a.session, pid: a.pid, texte: await lire(`fenetre|${a.pid}`) });
    }
    return sortie;
}

/// Les couples (session, pid) que le CAPTEUR a inscrits, lus dans le journal
/// mis À PLAT.
///
/// 🔴 METTRE À PLAT D'ABORD : `tracing` sépare le nom du champ de sa valeur par
/// des séquences ANSI, et `grep 'pid='` ne matche JAMAIS sur un journal brut
/// (piège de la recette d'entrée de D8).
function attributions() {
    let brut;
    try { brut = readFileSync('/media/vm/dev/agent.log', 'utf8'); } catch { return []; }
    const plat = brut.replace(/\x1b\[[0-9;]*m/g, '');
    // 🔴 L'ANCRE EST `session=… pid=…` ADJACENTS, ET C'EST MESURÉ, PAS
    // PRUDENTIEL. La première rédaction cherchait `/session=(\S+)/` sur la
    // ligne, et attrapait le SPAN `tracing` qui la précède —
    // `fenetre{session=F4z…:w-3}:` — d'où des noms de session portant « }: »
    // en trop, et une attribution INUTILISABLE. C'est EXACTEMENT le piège que
    // D8 a payé sur `resoudreIdentite` : `\S+` avale l'accolade et les
    // deux-points, la valeur est fausse mais TRUTHY, et rien ne le dit.
    //
    // Le span ne porte JAMAIS de `pid` : exiger les deux champs adjacents
    // l'exclut par construction, sans avoir à connaître le format de `tracing`.
    const vues = new Map();
    for (const l of plat.split('\n')) {
        if (!/fen\S*tre attach\S*e au capteur/.test(l)) continue;
        const m = /session=(\S+)\s+pid=(\d+)/.exec(l);
        if (m) vues.set(m[1], Number(m[2]));
    }
    return [...vues.entries()].map(([session, pid]) => ({ session, pid }));
}

const identite = lireIdentite(IDENTITE);
const paire = await obtenirPaire(identite);
// ⚠️ Ni le secret d'enrôlement ni le mot de passe ne sont journalisés, ni écrits
// dans le relevé JSON : ils vivent HORS du dépôt.
const SEMENCE = `(() => { try {
  localStorage.setItem('guac.jeton.acces', ${JSON.stringify(paire.acces)});
  localStorage.setItem('guac.jeton.rafraichissement', ${JSON.stringify(paire.rafraichissement)});
  localStorage.setItem('guac.prefixe', ${JSON.stringify(identite.PREFIXE_VM)});
} catch (e) { } })();`;
const URL_SHELL = `${CLIENT_URL}/shell.html?signaling=${encodeURIComponent(SIGNALING_WS)}`
    + `&prefixe=${encodeURIComponent(identite.PREFIXE_VM)}`;

// 🔴 Substitution VÉRIFIÉE : un marqueur survivant produirait une URL de
// signaling littérale, la page ne se connecterait à rien, et le seul symptôme
// serait « l'agent n'a pas répondu » — indiscernable d'une panne du produit.
const AMORCE_FINALE = AMORCE.replaceAll('__SIGNALING__', SIGNALING_WS);
if (AMORCE_FINALE.includes('__SIGNALING__')) throw new Error('marqueur __SIGNALING__ non substitué');
if (!AMORCE_FINALE.includes(SIGNALING_WS)) throw new Error('substitution du signaling sans effet');

const dir = await mkdtemp(join(tmpdir(), `pp3-${ETIQUETTE}-`));
const port = 9460 + (Number(ETIQUETTE) || 1);
const releve = {
    etiquette: ETIQUETTE, nonce: NONCE, n_demande: N_DEMANDE,
    emulationDeFocusActivee: false,
    phases: [], criteres: {},
};
let chrome = null;
let cdp = null;

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

    // 🔴 LA PURGE DU VIVIER DE SORTIES VIRTUELLES, DANS UN LANCEMENT SÉPARÉ.
    // L'aiguillage de `diagnostics::multifenetre` RETOURNE APRÈS LA PREMIÈRE
    // SONDE RECONNUE : deux variables dans le même lancement n'en enchaînent
    // pas deux, et rien ne le signale (D8). Sans purge, une exécution démarre
    // avec un vivier déjà entamé — dix sorties orphelines survivent à un
    // `Stop-Process` (D5, D11).
    log('purge du vivier de sorties virtuelles (lancement SÉPARÉ)');
    spawnSync('bash', ['-c',
        `cd ${RACINE} && set -a && source .env && set +a && `
        + 'MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh'],
        { encoding: 'utf8', env: process.env, timeout: 180000 });
    await dodo(5000);

    // --- Les N Bloc-notes, AVANT le superviseur. ---------------------------
    // 🔴 L'OUVERTURE EST VÉRIFIÉE, PAS SUPPOSÉE : à la deuxième exécution du
    // bras armé de P2, `schtasks /run` n'a rien ouvert, et le symptôme remonté
    // était « aucune page d'application attachée » — un défaut d'INSTRUMENT
    // qui se lit exactement comme une panne du produit.
    log(`ouverture de ${N_DEMANDE} Bloc-notes`);
    releve.notepads_ouverts = 0;
    for (let essai = 1; essai <= 3 && releve.notepads_ouverts < N_DEMANDE; essai += 1) {
        vmIt(`pp3-notepads${essai}`,
            `1..${N_DEMANDE} | ForEach-Object { Start-Process notepad; Start-Sleep -Seconds 2 }; Start-Sleep -Seconds 3`);
        for (let i = 0; i < 12; i += 1) {
            await dodo(2000);
            const n = winrm('@(Get-Process notepad -ErrorAction SilentlyContinue).Count')
                .trim().split('\n').pop().replace(/^\uFEFF/, '');
            releve.notepads_ouverts = Number(n) || 0;
            if (releve.notepads_ouverts >= N_DEMANDE) break;
        }
        log(`  Bloc-notes ouverts (essai ${essai}) : ${releve.notepads_ouverts}`);
    }
    if (releve.notepads_ouverts < 2) {
        throw new Error(`seulement ${releve.notepads_ouverts} Bloc-notes : à une fenêtre, P3 n'est pas livrable (RP3-1)`);
    }
    await dodo(5000);

    // --- Le LECTEUR à demeure, en session 1, AVANT le superviseur. ----------
    spawnSync('cp', [`${P3}/instrument/lecteur-pp-n.ps1`, '/media/vm/dev/lecteur-pp-n.ps1'], { encoding: 'utf8' });
    spawnSync('bash', ['-c', 'rm -f /media/vm/dev/pp3-ordre.txt /media/vm/dev/pp3-fait.txt /media/vm/dev/pp3-lecteur.log'],
        { encoding: 'utf8' });
    vmIt('pp3-lecteur', 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\lecteur-pp-n.ps1');
    await dodo(6000);
    releve.lecteur_pret = (() => {
        try { return readFileSync('/media/vm/dev/pp3-lecteur.log', 'utf8').includes('lecteur pret'); }
        catch { return false; }
    })();
    log('lecteur prêt :', releve.lecteur_pret);
    if (!releve.lecteur_pret) throw new Error('le lecteur de session 1 ne répond pas');

    releve.inventaire_avant = await lire('inventaire');

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

    // --- La page-shell, AVANT le superviseur. ------------------------------
    await cdp.send('Target.createTarget', { url: URL_SHELL });
    await dodo(3000);
    releve.shell_attachee = [...pages.values()].some((p) => /shell/.test(p.url ?? ''));
    log('page-shell attachée :', releve.shell_attachee);

    // --- Le superviseur. ----------------------------------------------------
    log('lancement du superviseur');
    const r = spawnSync('bash', ['-c',
        `cd ${RACINE} && set -a && source .env && set +a && `
        + 'export AGENT_VM="$1" AGENT_SECRET="$2" && SUPERVISEUR=1 '
        + `SIGNALING_URL=${SIGNALING_WS} LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`,
        'pp3', identite.AGENT_VM, identite.AGENT_SECRET],
        { encoding: 'utf8', env: process.env, timeout: 180000 });
    log('run-agent.sh :', (r.stdout ?? '').trim().split('\n').pop());
    if ((r.stderr ?? '').trim()) log('run-agent.sh STDERR :', r.stderr.trim().slice(0, 300));

    // --- Attendre les pages d'application. On attend le FAIT. ---------------
    const appsParSession = new Map();
    for (let i = 0; i < 120; i += 1) {
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
        for (const p of pages.values()) {
            const m = /[?&]session=([^&]+)/.exec(p.url ?? '');
            if (!m) continue;
            const session = decodeURIComponent(m[1]);
            if (appsParSession.has(session)) continue;
            const e = await cdp.evalBorne(p.sid, EXPR_ETAT, 8000, false);
            if (e && e.pp) appsParSession.set(session, { sid: p.sid, url: p.url });
        }
        if (appsParSession.size >= releve.notepads_ouverts) break;
    }
    releve.sessions_diffusant = [...appsParSession.keys()];
    log('pages d\'application :', releve.sessions_diffusant.join(' '));
    if (appsParSession.size < 2) throw new Error(`seulement ${appsParSession.size} page(s) d'application : ①②③ non mesurables`);

    // 🔴 L'ATTRIBUTION, sans laquelle ② et ③ ne sont PAS JUGEABLES (RP3-3).
    //
    // ⚠️ L'ATTENTE PORTE SUR LE FAIT, ET ELLE EST OBLIGATOIRE : `agent.log` est
    // lu à travers un montage CIFS, et la première exécution l'a trouvé VIDE de
    // ces lignes cinq secondes après qu'elles y aient été écrites — l'attribution
    // rendait `[]`, et ② comme ③ cessaient d'être jugeables sans que rien ne le
    // dise. On relit jusqu'à en trouver autant que de sessions, borné.
    for (let i = 0; i < 40; i += 1) {
        releve.attributions = attributions();
        if (releve.attributions.length >= appsParSession.size) break;
        await dodo(1000);
    }
    log('attributions session ↔ pid :', JSON.stringify(releve.attributions));
    const pidsDistincts = new Set(releve.attributions.map((a) => a.pid));
    releve.attribution_utilisable = releve.attributions.length >= 2
        && pidsDistincts.size === releve.attributions.length;
    if (!releve.attribution_utilisable) {
        log('!! ATTRIBUTION INUTILISABLE : ② et ③ ne seront pas des verdicts');
    }

    // --- PRÉ-SEMER les marqueurs distincts (D-P3-9). ------------------------
    // Trois Bloc-notes non enregistrés s'intitulent tous « Sans titre -
    // Bloc-notes ». Ce marqueur rend le CONTENU attribuable ; il ne remplace
    // PAS l'attribution ci-dessus, et les confondre referait la circularité.
    releve.semis = [];
    for (const [k, a] of releve.attributions.entries()) {
        releve.semis.push(await lire(`semer|${a.pid}|${MARQUEURS[k]}-${NONCE}`));
    }

    // --- Frappes de confiance. ----------------------------------------------
    const bas = (sid, t, mods) => cdp.send('Input.dispatchKeyEvent',
        { type: 'rawKeyDown', modifiers: mods, key: t.key, code: t.code, windowsVirtualKeyCode: t.vk, nativeVirtualKeyCode: t.vk }, sid);
    const haut = (sid, t, mods) => cdp.send('Input.dispatchKeyEvent',
        { type: 'keyUp', modifiers: mods, key: t.key, code: t.code, windowsVirtualKeyCode: t.vk, nativeVirtualKeyCode: t.vk }, sid);
    const CTRL = { key: 'Control', code: 'ControlLeft', vk: 17 };
    const TOUCHE = (k, c, vk) => ({ key: k, code: c, vk });

    // L'onglet d'AMORCE du presse-papier, dans un onglet SÉPARÉ : `input.ts`
    // appelle `preventDefault()` sur chaque `keydown` qui n'est pas un
    // raccourci de collage, donc un `Ctrl+C` frappé dans une page de session
    // serait empêché et rien ne serait copié.
    const cible = await cdp.send('Target.createTarget', { url: `${CLIENT_URL}/` });
    await dodo(2500);
    let sidAmorce = [...pages.entries()].find(([id]) => id === cible.targetId)?.[1]?.sid ?? null;
    if (!sidAmorce) {
        const a = await cdp.send('Target.attachToTarget', { targetId: cible.targetId, flatten: true });
        sidAmorce = a.sessionId;
        await cdp.send('Runtime.enable', {}, sidAmorce).catch(() => { });
    }
    await cdp.evalBorne(sidAmorce, `(() => {
      document.body.innerHTML = '<textarea id="amorce" style="width:600px;height:200px"></textarea>';
      return 'pret';
    })()`, 8000, false);

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

    async function collerDans(sid) {
        await cdp.send('Page.bringToFront', {}, sid).catch(() => { });
        await dodo(400);
        await cdp.evalBorne(sid, `(() => {
          const v = document.getElementById('remote');
          if (v) v.focus();
          return document.activeElement ? (document.activeElement.id || document.activeElement.tagName) : null;
        })()`, 8000, false);
        await dodo(200);
        await bas(sid, CTRL, 2);
        await bas(sid, TOUCHE('v', 'KeyV', 86), 2);
        await haut(sid, TOUCHE('v', 'KeyV', 86), 2);
        await haut(sid, CTRL, 0);
    }

    /// Une phase : l'état de CHAQUE fenêtre, plus ce que la VM porte.
    const phase = async (nom) => {
        const fenetres = [];
        for (const [session, p] of appsParSession) {
            const etat = await cdp.evalBorne(p.sid, EXPR_ETAT, 12000, false);
            const pid = releve.attributions.find((a) => a.session === session)?.pid ?? null;
            fenetres.push({
                session, pid,
                messages: etat?.pp?.messages ?? null,
                focus: etat?.focus ?? null,
                actif: etat?.actif ?? null,
                dejaActif: etat?.dejaActif ?? null,
                visible: etat?.visible ?? null,
                ecritures: etat?.ecritures ?? null,
                statut: etat?.statut_texte ?? null,
                statut_cache: etat?.statut_cache ?? null,
                texte_fenetre: pid === null ? null : await lire(`fenetre|${pid}`),
            });
        }
        const p = { nom, t: new Date().toISOString(), fenetres, vm_clipboard: await lire('clipboard') };
        releve.phases.push(p);
        log(`[${nom}] focus=${fenetres.map((f) => f.focus).join(',')} `
            + `messages=${fenetres.map((f) => f.messages?.length ?? '?').join(',')}`);
        return p;
    };

    // 🔴 LE TÉMOIN DE FOCUS, JOUÉ AVANT TOUT CRITÈRE, sur les fenêtres RÉELLES
    // du produit. C'est le rejeu de la sonde S1 en conditions de produit, et il
    // PEUT contredire la sonde — ces fenêtres-ci portent un flux WebRTC.
    releve.temoin_focus = [];
    for (const [session, p] of appsParSession) {
        await cdp.send('Page.bringToFront', {}, p.sid).catch(() => { });
        await dodo(600);
        const vus = [];
        for (const [s2, p2] of appsParSession) {
            const e = await cdp.evalBorne(p2.sid, EXPR_ETAT, 8000, false);
            vus.push({ session: s2, focus: e?.focus ?? null });
        }
        releve.temoin_focus.push({ amenee: session, vus });
        log(`témoin focus : bringToFront(${session}) → ${vus.map((v) => `${v.session}:${v.focus}`).join(' ')}`);
    }
    releve.focus_discriminant = releve.temoin_focus.every(
        (b) => b.vus.filter((v) => v.focus).length === 1 && b.vus.find((v) => v.focus)?.session === b.amenee);
    log('④ mesurable (le focus discrimine) :', releve.focus_discriminant);

    await phase('0-avant-toute-copie');

    // ── CRITÈRE ① — une copie dans la VM parvient aux N fenêtres ───────────
    // Le copieur à demeure de P1 écrit dans le presse-papier de la VM ; le
    // sondeur de l'agent doit pousser à TOUTES les fenêtres.
    const texte1 = `un-${NONCE}`;
    log(`① copie dans la VM : ${texte1}`);
    vmIt('pp3-copie1', `Set-Clipboard -Value '${texte1}'`);
    await dodo(8000);
    const p1 = await phase('1-apres-copie-dans-la-vm');
    releve.criteres.un = {
        texte: texte1,
        recu_par: p1.fenetres.map((f) => ({
            session: f.session,
            recu: (f.messages ?? []).some((m) => (m.debut ?? '').includes(texte1)),
        })),
    };

    // ⚠️ LA MOITIÉ QUI MANQUAIT AU LEGS N°3 : une fenêtre ATTACHÉE APRÈS la
    // copie doit recevoir le contenu courant. Si la VM n'en donne pas une de
    // plus, on ferme une fenêtre et on la rouvre — et on l'ÉCRIT.
    log('① bis : une fenêtre de plus, attachée APRÈS la copie');
    releve.criteres.un_bis = { methode: 'ouverture d\'un Bloc-notes supplémentaire' };
    const connues = new Set(releve.attributions.map((a) => a.session));
    const ciblesConnues = new Set([...pages.keys()]);

    // 🔴 L'AMORCE DOIT ÊTRE POSÉE AVANT QUE LA SESSION NE S'ÉTABLISSE, ET LA
    // PREMIÈRE RÉDACTION NE LE FAISAIT PAS. Elle enveloppe
    // `RTCPeerConnection.prototype.createDataChannel` : si la page a DÉJÀ créé
    // son canal de contrôle, l'enveloppe le manque, et l'observateur reste
    // vide — ce qui se lit EXACTEMENT comme « la fenêtre n'a rien reçu »,
    // c'est-à-dire comme un défaut du produit.
    //
    // ⚠️ MESURÉ, PAS SUPPOSÉ : à l'exécution 2, `① bis` a rendu `false` alors
    // que l'agent avait bel et bien émis — sa trace
    // `etat courant du presse-papier emis a l'inscription` porte la session
    // tardive, une fois, avec ses onze octets. C'est l'instrument qui était en
    // retard, pas le produit.
    //
    // Le remède : guetter `Target.getTargets` DÈS le lancement du Bloc-notes,
    // et attacher l'instant où la cible paraît — la cible existe dès le
    // `window.open` de la page-shell, la SESSION met des secondes de plus
    // (signaling, SDP, ICE).
    const guetter = async (ms) => {
        const fin = Date.now() + ms;
        while (Date.now() < fin) {
            const inv = await cdp.send('Target.getTargets', {}).catch(() => ({ targetInfos: [] }));
            for (const t of inv.targetInfos ?? []) {
                if (t.type !== 'page' || ciblesConnues.has(t.targetId)) continue;
                if (!/\?session=/.test(t.url ?? '')) continue;
                ciblesConnues.add(t.targetId);
                const at = await cdp.send('Target.attachToTarget', { targetId: t.targetId, flatten: true }).catch(() => null);
                if (!at) continue;
                pages.set(t.targetId, { sid: at.sessionId, url: t.url });
                await cdp.send('Runtime.enable', {}, at.sessionId).catch(() => { });
                await cdp.send('Runtime.evaluate', { expression: SEMENCE + AMORCE_FINALE }, at.sessionId).catch(() => { });
                log('+ cible TARDIVE amorcée', at.sessionId.slice(0, 8), t.url);
                return { sid: at.sessionId, url: t.url };
            }
            await dodo(120);
        }
        return null;
    };
    vmIt('pp3-notepad-tardif', 'Start-Process notepad');
    const cibleTardive = await guetter(90000);
    releve.criteres.un_bis.cible_amorcee = cibleTardive !== null;
    // ⚠️ ATTENTE SUR LE FAIT, jamais sur une durée : le journal CIFS est en
    // retard, et la première exécution a calculé les sessions neuves contre une
    // liste `attributions` restée VIDE — toutes les sessions ont donc paru
    // neuves, y compris les trois qui ne l'étaient pas.
    let tardives = [];
    for (let i = 0; i < 60; i += 1) {
        await dodo(1000);
        tardives = attributions().filter((a) => !connues.has(a.session));
        if (tardives.length > 0) break;
    }
    releve.criteres.un_bis.sessions_neuves = tardives;
    releve.criteres.un_bis.connues_avant = [...connues];
    // ⚠️ L'ATTENTE PORTE SUR LE FAIT — le message reçu —, jamais sur une durée :
    // la cible est amorcée, mais la session met encore des secondes à
    // s'établir, et un relevé pris trop tôt rendrait `false` sur un produit
    // correct.
    for (let i = 0; i < 60 && tardives.length > 0; i += 1) {
        const inv = await cdp.send('Target.getTargets', {}).catch(() => ({ targetInfos: [] }));
        for (const t of inv.targetInfos ?? []) {
            const m = /[?&]session=([^&]+)/.exec(t.url ?? '');
            if (!m) continue;
            const session = decodeURIComponent(m[1]);
            if (!tardives.some((a) => a.session === session)) continue;
            let sid = pages.get(t.targetId)?.sid;
            if (!sid) {
                const a = await cdp.send('Target.attachToTarget', { targetId: t.targetId, flatten: true }).catch(() => null);
                if (!a) continue;
                sid = a.sessionId;
                pages.set(t.targetId, { sid, url: t.url });
                await cdp.send('Runtime.enable', {}, sid).catch(() => { });
                await cdp.send('Runtime.evaluate', { expression: SEMENCE + AMORCE_FINALE }, sid).catch(() => { });
            }
            const e = await cdp.evalBorne(sid, EXPR_ETAT, 8000, false);
            if (e && e.pp) {
                releve.criteres.un_bis.session = session;
                releve.criteres.un_bis.messages = e.pp.messages;
                releve.criteres.un_bis.ecritures = e.ecritures;
                releve.criteres.un_bis.recu = (e.pp.messages ?? []).some((x) => (x.debut ?? '').includes(texte1));
                appsParSession.set(session, { sid, url: t.url });
            }
        }
        if (releve.criteres.un_bis.recu) break;
        await dodo(1000);
    }
    // 🔵 LA SECONDE ARME, INDÉPENDANTE DU NAVIGATEUR : la trace de l'agent.
    // Elle dit si la moitié AGENT a émis, et elle permet de départager un
    // défaut du PRODUIT d'un retard de l'INSTRUMENT — c'est ce qui a tranché
    // à l'exécution 2, où l'agent avait émis et le client n'avait rien vu.
    try {
        const plat = readFileSync('/media/vm/dev/agent.log', 'utf8').replace(/\x1b\[[0-9;]*m/g, '');
        const em = [...plat.matchAll(/etat courant du presse-papier emis a l'inscription session=(\S+) octets=(\d+)/g)];
        releve.criteres.un_bis.emissions_agent = em.map((m) => ({ session: m[1], octets: Number(m[2]) }));
    } catch { releve.criteres.un_bis.emissions_agent = null; }
    log('① bis — la fenêtre tardive a reçu le contenu courant :', releve.criteres.un_bis.recu);

    // ── CRITÈRE ② — un collage depuis B met le texte de B dans la VM ───────
    const sessions = [...appsParSession.keys()];
    releve.criteres.deux = [];
    for (let k = 0; k < Math.min(2, sessions.length); k += 1) {
        const session = sessions[k];
        const texte = `deux-${['A', 'B'][k]}-${NONCE}`;
        log(`② collage depuis ${session} : ${texte}`);
        await copierSurHote(texte);
        await collerDans(appsParSession.get(session).sid);
        await dodo(7000);
        const pid = releve.attributions.find((a) => a.session === session)?.pid ?? null;
        releve.criteres.deux.push({
            session, pid, texte,
            vm_clipboard: await lire('clipboard'),
            fenetre_cible: pid === null ? null : await lire(`fenetre|${pid}`),
            fenetres_toutes: await lireLesFenetres(releve.attributions),
        });
    }
    await phase('2-apres-les-collages-attribues');

    // ── CRITÈRE ③ — deux collages QUASI SIMULTANÉS ────────────────────────
    // ⚠️ CE QUE ③ VA PROBABLEMENT EXHIBER, ET QU'IL NE FAUT PAS LIRE COMME UN
    // CONTENU MÊLÉ : la course de D-P3-6 se déclenche exactement sur « deux
    // collages quasi simultanés ». La tâche 5 l'ayant fermée, le compte de
    // messages en RETOUR doit rester à ZÉRO ; s'il ne l'est pas, c'est que le
    // résidu a mordu — et c'est un RELEVÉ, pas une réfutation de ③.
    if (sessions.length >= 2) {
        const t1 = `trois-T1-${NONCE}`;
        const t2 = `trois-T2-${NONCE}`;
        log('③ deux collages à moins de 250 ms');
        const avant = await phase('3-avant-simultane');
        releve.criteres.trois = { t1, t2, messages_avant: avant.fenetres.map((f) => f.messages?.length ?? 0) };
        await copierSurHote(t1);
        await collerDans(appsParSession.get(sessions[0]).sid);
        // Le second collage part AVANT que le premier n'ait pu être servi : la
        // fenêtre visée est `PERIODE_PRESSE_PAPIER` (250 ms).
        await copierSurHote(t2);
        await collerDans(appsParSession.get(sessions[1]).sid);
        await dodo(9000);
        const apres = await phase('3-apres-simultane');
        releve.criteres.trois.messages_apres = apres.fenetres.map((f) => f.messages?.length ?? 0);
        releve.criteres.trois.vm_clipboard = apres.vm_clipboard;
        releve.criteres.trois.fenetres = await lireLesFenetres(releve.attributions);
        // (a) les deux commandes ont-elles reçu leur réponse ?
        //
        // 🔴 LES MOTIFS ONT ÉTÉ VÉRIFIÉS CONTRE LE CODE, ET LES PREMIERS
        // ÉTAIENT FAUX. Le plan prescrivait `aucune réponse du capteur` et
        // `commande expirée` : la première N'EXISTE NULLE PART dans le dépôt,
        // et la seconde n'existe que dans `agent/src/pont/` — LE PONT
        // FICHIERS, pas le presse-papier. Leur zéro était VACUEUX : il serait
        // resté zéro alors même que la borne de 12 s aurait mordu. C'est la
        // règle 10 du §2.2 du plan, payée sur le plan lui-même.
        //
        // Les motifs qui EXISTENT, relevés par `grep` dans `agent/src/` :
        //   - côté ENFANT, et c'est le décisif — toute commande qui échoue,
        //     borne comprise, passe par là :
        //         `collage NON écrit : la touche V est perdue, pas reportée`
        //     (`transport/collage.rs`) ;
        //   - côté CAPTEUR, la borne `DELAI_REPONSE_FENETRE` (12 s,
        //     `capteur/serveur.rs`) :
        //         `aucune réponse du fil de fenêtre en …`
        //         `le fil de fenêtre n'a pas répondu …`
        //
        // ⚠️ ET LE ZÉRO SE QUALIFIE : un CONTRE-CONTRÔLE vérifie que chaque
        // motif matche sur une ligne fabriquée. Un motif qui ne matcherait
        // rien rendrait zéro pour une raison étrangère au produit.
        try {
            const plat = readFileSync('/media/vm/dev/agent.log', 'utf8').replace(/\x1b\[[0-9;]*m/g, '');
            const motifs = {
                collage_non_ecrit: /collage NON \S*crit/g,
                sans_reponse_du_fil: /aucune r\S*ponse du fil de fen\S*tre/g,
                fil_sans_reponse: /le fil de fen\S*tre n'a pas r\S*pondu/g,
                collage_refuse: /collage refus\S*/g,
            };
            const temoin = {
                collage_non_ecrit: "collage NON écrit : la touche V est perdue, pas reportée",
                sans_reponse_du_fil: "aucune réponse du fil de fenêtre en 12s",
                fil_sans_reponse: "le fil de fenêtre n'a pas répondu, canal clos",
                collage_refuse: "collage refusé : au-dessus de la borne",
            };
            releve.criteres.trois.echecs = {};
            releve.criteres.trois.motifs_discriminants = {};
            for (const [nom, re] of Object.entries(motifs)) {
                releve.criteres.trois.echecs[nom] = (plat.match(re) ?? []).length;
                // Le contre-contrôle : le motif matche-t-il sa propre ligne ?
                releve.criteres.trois.motifs_discriminants[nom] =
                    new RegExp(re.source).test(temoin[nom]);
            }
            // Le témoin POSITIF : la trace que le produit émet à chaque annonce.
            // Sans lui, un zéro d'échecs serait aussi ce que rendrait un journal
            // vide ou mal lu.
            releve.criteres.trois.annonces_dans_le_journal =
                (plat.match(/presse-papier de la VM/g) ?? []).length;
        } catch (e) { releve.criteres.trois.echecs = { erreur: String(e).slice(0, 120) }; }
    }

    // ── CRITÈRE ④ — une fenêtre sans focus n'écrit pas localement ──────────
    // 🔴 IL N'EST JOUÉ QUE SI LE TÉMOIN DE FOCUS DISCRIMINE. Sinon il est
    // DÉCLARÉ NON MESURABLE, et les trois autres se jouent quand même.
    if (!releve.focus_discriminant) {
        releve.criteres.quatre = {
            verdict: 'NON MESURABLE',
            raison: 'le témoin de focus ne discrimine pas : toutes les fenêtres rapportent le même '
                + 'hasFocus. C\'est ce que la sonde S1 avait relevé hors VM, sur le geste du produit '
                + '(window.open). Xvfb + xdotool est la seule voie nommée, et les mesures qui en '
                + 'sortiraient ne se compareraient à aucune campagne antérieure.',
            temoin: releve.temoin_focus,
            // 🔵 ET C'EST UN RELEVÉ EN SOI : si toutes les fenêtres ont le
            // focus, TOUTES écrivent leur presse-papier local — c'est le régime
            // de N ÉCRIVAINS CONCURRENTS que D-P3-4 nomme et que rien ne
            // mesure. La recette le rencontre par accident.
            n_ecrivains_concurrents: releve.temoin_focus[0]?.vus.filter((v) => v.focus).length ?? null,
        };
    } else {
        const sansFocus = sessions[sessions.length - 1];
        const avecFocus = sessions[0];
        await cdp.send('Page.bringToFront', {}, appsParSession.get(avecFocus).sid).catch(() => { });
        await dodo(600);
        const texte4 = `quatre-${NONCE}`;
        vmIt('pp3-copie4', `Set-Clipboard -Value '${texte4}'`);
        await dodo(8000);
        const p4a = await phase('4-copie-pendant-que-B-est-sans-focus');
        await cdp.send('Page.bringToFront', {}, appsParSession.get(sansFocus).sid).catch(() => { });
        await dodo(3000);
        const p4b = await phase('4-apres-reprise-du-focus-par-B');

        // 🔴 LE VERDICT SE CALCULE SUR LES ÉCRITURES OBSERVÉES, pas sur les
        // phases brutes. Deux propriétés, séparées :
        //   (a) la fenêtre SANS focus a bien REÇU le message — sans quoi on
        //       mesurerait une fenêtre qu'on n'a simplement pas servie ;
        //   (b) elle ne l'a PAS écrit sans focus, et l'a écrit APRÈS.
        const av = p4a.fenetres.find((f) => f.session === sansFocus);
        const ap = p4b.fenetres.find((f) => f.session === sansFocus);
        const porte = (f) => (f?.ecritures ?? []).some((e) => (e.debut ?? '').includes(texte4));
        const recu = (f) => (f?.messages ?? []).some((m) => (m.debut ?? '').includes(texte4));
        const a_recu = recu(av);
        const ecrit_avant = porte(av);
        const ecrit_apres = porte(ap);
        releve.criteres.quatre = {
            texte: texte4, sansFocus, avecFocus,
            a_recu_le_message_sans_focus: a_recu,
            focus_de_B_avant: av?.focus ?? null,
            focus_de_B_apres: ap?.focus ?? null,
            a_ecrit_sans_focus: ecrit_avant,
            a_ecrit_apres_reprise: ecrit_apres,
            verdict: !a_recu
                ? 'NON JUGEABLE — la fenêtre sans focus n\'a pas reçu le message : on mesurerait une fenêtre non servie'
                : (av?.focus !== false
                    ? 'NON JUGEABLE — la fenêtre censée être sans focus en avait'
                    : (!ecrit_avant && ecrit_apres
                        ? 'TENU — sans focus elle n\'écrit pas, et elle écrit à la reprise'
                        : (ecrit_avant
                            ? 'NON TENU — elle a écrit SANS focus'
                            : 'NON TENU — elle n\'a pas écrit même après la reprise du focus'))),
            avant: p4a.fenetres, apres: p4b.fenetres,
        };
        log('④ verdict :', releve.criteres.quatre.verdict);
    }
} catch (e) {
    releve.erreur = String(e).slice(0, 500);
    log('!! erreur', releve.erreur);
} finally {
    try { writeFileSync('/media/vm/dev/pp3-ordre.txt', '999|stop', 'utf8'); } catch { /* VM partie */ }
    try { releve.lecteur_journal = readFileSync('/media/vm/dev/pp3-lecteur.log', 'utf8').slice(0, 8000); }
    catch { releve.lecteur_journal = null; }
    await writeFile(SORTIE, JSON.stringify(releve, null, 2));
    log('relevé écrit dans', SORTIE);
    if (chrome) { chrome.kill('SIGKILL'); await dodo(500); }
    await rm(dir, { recursive: true, force: true });
    // Le journal se copie APRÈS la fin réelle : les enfants meurent quand le
    // navigateur se ferme, donc APRÈS la copie si on la fait trop tôt (D4). Et
    // JAMAIS sous un nom déjà pris : P2 a écrasé le journal qui portait sa
    // fuite de presse-papier.
    await dodo(4000);
    spawnSync('bash', ['-c',
        `cp /media/vm/dev/agent.log ${P3}/agent-${ETIQUETTE}.log 2>/dev/null && `
        + `sed 's/\\x1b\\[[0-9;]*m//g' ${P3}/agent-${ETIQUETTE}.log > ${P3}/agent-${ETIQUETTE}-plat.log`],
        { encoding: 'utf8' });
    log('journal copié : agent-' + ETIQUETTE + '.log (+ -plat)');
    log('virsh domstate :', (spawnSync('virsh', ['domstate', 'Windows'], { encoding: 'utf8' }).stdout ?? '?').trim());
    process.exit(releve.erreur ? 1 : 0);
}
