#!/usr/bin/env node
// Sous-bloc P3 (presse-papier) — SONDE S2 : `writeText` × FOCUS × ACTIVATION,
// QUATRE cellules. C'est la mesure du §3.3 de la spec, déclaré SUPPOSÉ depuis
// le 28 juillet 2026.
//
// 🔴 POURQUOI UN 2×2 ET PAS UNE SIMPLE OBSERVATION « pas de focus ⟹ THROW » :
// P2 a DÉJÀ mesuré un `NotAllowedError` sur `writeText`, et ce n'était PAS le
// focus. Son annexe versée (`journaux-presse-papier-p2/p2-annexe-writetext-{1,2}.json`,
// deux exécutions) relève `hasFocus: true` DES DEUX CÔTÉS :
//
//     avant : { isActive:false, hasBeenActive:false, hasFocus:true } → THROW:NotAllowedError
//     après : { isActive:true,  hasBeenActive:true,  hasFocus:true } → OK
//
// Ce qu'elle mesure est l'ACTIVATION UTILISATEUR TRANSITOIRE, pas le focus, et
// **les deux mécanismes lèvent la même exception**. Une sonde qui se
// contenterait d'observer « fenêtre non focalisée ⟹ THROW » attribuerait au
// focus ce qui pourrait être l'activation — et le verdict de D-P3-4 serait
// faux, DANS LES DEUX SENS POSSIBLES.
//
// La cellule qui TRANCHE est `{hasFocus:false, isActive:true}` :
//   - si le §3.3 est vrai      → THROW
//   - si c'est l'activation seule → OK
//
// ⚠️ Cette cellule peut être INATTEIGNABLE (une fenêtre sans focus ne reçoit
// peut-être pas de geste de confiance ; et un `--headless` peut ne pas savoir
// retirer le focus du tout — c'est ce que la sonde S1 a relevé). **Si elle
// l'est, cette sonde le DIT et le §3.3 reste SUPPOSÉ.** C'est un verdict
// recevable ; en fabriquer un autre ne le serait pas.
//
// ⚠️ `isActive` est TRANSITOIRE : il retombe quelques secondes après le geste.
// Il est donc relevé DANS LE MÊME `Runtime.evaluate` que l'appel à `writeText`,
// jamais avant.
//
// 🔴 `Emulation.setFocusEmulationEnabled` N'EST JAMAIS APPELÉE (E9).
//
// Usage : node sonde-p3-writetext.mjs --etiquette=1

import { createServer } from 'node:http';
import { writeFileSync } from 'node:fs';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-multifenetres-d11/instrument/commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const ETIQUETTE = arg('etiquette', '1');
const SORTIE = arg('sortie', `p3-writetext-${ETIQUETTE}.json`);
const PORT_HTTP = Number(arg('port-http', '45894'));
const PORT_CDP = Number(arg('port-cdp', '9574'));

const journal = [];
const dire = (...m) => { const l = m.join(' '); journal.push(l); console.log(l); };

const PAGE = (t) => `<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>${t}</title></head>
<body><textarea id="amorce" style="position:absolute;left:-9999px"></textarea>
<video id="remote" tabindex="0"></video></body></html>`;

const srv = createServer((q, r) => {
    r.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    r.end(PAGE((q.url ?? '/').slice(1) || 'a'));
});
await new Promise((r) => srv.listen(PORT_HTTP, '127.0.0.1', r));

const dir = await mkdtemp(join(tmpdir(), 'p3-wt-'));
const chrome = lancerChrome(PORT_CDP, dir);
const version = await attendreDevtools(PORT_CDP);
const cdp = new Cdp(version.webSocketDebuggerUrl);
dire('navigateur', version.Browser);

async function ouvrir(url, contexteId) {
    const p = { url };
    if (contexteId) p.browserContextId = contexteId;
    const { targetId } = await cdp.send('Target.createTarget', p);
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    await cdp.send('Runtime.enable', {}, sessionId);
    return { url, targetId, sessionId };
}

const a = await ouvrir(`http://127.0.0.1:${PORT_HTTP}/a`);
const b = await ouvrir(`http://127.0.0.1:${PORT_HTTP}/b`);
await dodo(800);

const etat = async (s) => JSON.parse(await cdp.eval(s.sessionId,
    `JSON.stringify({hasFocus:document.hasFocus(),isActive:navigator.userActivation.isActive,hasBeenActive:navigator.userActivation.hasBeenActive})`));

/// Le triplet ET le résultat de `writeText` dans le MÊME aller-retour :
/// `isActive` est transitoire, et le relever avant mesurerait un autre instant.
/// Le message est rendu VERBATIM — c'est lui, et non le seul nom
/// `NotAllowedError`, qui distingue « Document is not focused » d'un refus
/// d'activation.
async function cellule(s, nonce) {
    const brut = await cdp.eval(s.sessionId, `(async () => {
        const etat = { hasFocus: document.hasFocus(),
                       isActive: navigator.userActivation.isActive,
                       hasBeenActive: navigator.userActivation.hasBeenActive };
        let res;
        try { await navigator.clipboard.writeText(${JSON.stringify(nonce)}); res = 'OK'; }
        catch (e) { res = 'THROW:' + e.name + ': ' + e.message; }
        return JSON.stringify({ etat, res });
    })()`, true);
    return JSON.parse(brut);
}

/// Un geste de CONFIANCE : une vraie frappe Ctrl+C sur un textarea sélectionné.
/// C'est ce qui pose `userActivation.isActive`.
async function geste(s) {
    await cdp.eval(s.sessionId, `(() => { const t=document.querySelector('#amorce'); t.value='geste'; t.focus(); t.select(); return 'ok'; })()`);
    const touche = (type, key, code, vk) => cdp.send('Input.dispatchKeyEvent',
        { type, modifiers: 2, key, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk }, s.sessionId);
    await touche('rawKeyDown', 'Control', 'ControlLeft', 17);
    await touche('rawKeyDown', 'c', 'KeyC', 67);
    await touche('keyUp', 'c', 'KeyC', 67);
    await touche('keyUp', 'Control', 'ControlLeft', 17);
    await dodo(150);
}

/// Le même geste de confiance, mais SANS focaliser aucun élément de la page.
///
/// 🔴 C'est ce qui permet d'espérer la cellule `{focus:faux, activation:vrai}` :
/// `geste` ci-dessus appelle `t.focus()`, ce qui REND le focus à la fenêtre —
/// et la cellule qu'on croyait mesurer devient alors la précédente.
async function gesteSansFocus(s) {
    const touche = (type, key, code, vk) => cdp.send('Input.dispatchKeyEvent',
        { type, modifiers: 0, key, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk }, s.sessionId);
    await touche('rawKeyDown', 'a', 'KeyA', 65);
    await touche('keyUp', 'a', 'KeyA', 65);
    await dodo(150);
}

// -------------------------------------------------------------------------
// D'ABORD : PEUT-ON SEULEMENT RETIRER LE FOCUS ? Trois moyens, chacun relevé.
// -------------------------------------------------------------------------
// Sans un `hasFocus === false` atteignable, DEUX des quatre cellules sont
// hors d'atteinte, et la sonde doit le dire plutôt que de les remplir.
const tentatives = [];
async function tenter(nom, action) {
    await action();
    await dodo(400);
    const e = await etat(b);
    tentatives.push({ moyen: nom, hasFocus: e.hasFocus });
    dire(`  retrait du focus par « ${nom} » → b.hasFocus =`, e.hasFocus);
    return e.hasFocus === false;
}

dire('tentatives de retrait du focus sur la fenetre B :');
let sansFocus = await tenter('Page.bringToFront sur A', () => cdp.send('Page.bringToFront', {}, a.sessionId));
if (!sansFocus) sansFocus = await tenter('window.blur() sur B', () => cdp.eval(b.sessionId, `(window.blur(), 'ok')`));
if (!sansFocus) {
    sansFocus = await tenter('contexte de navigateur SEPARE amene au premier plan', async () => {
        const { browserContextId } = await cdp.send('Target.createBrowserContext', {});
        const c = await ouvrir(`http://127.0.0.1:${PORT_HTTP}/c`, browserContextId);
        await dodo(400);
        await cdp.send('Page.bringToFront', {}, c.sessionId);
    });
}

// -------------------------------------------------------------------------
// LES QUATRE CELLULES
// -------------------------------------------------------------------------
const cellules = {};

// focus ✔, activation ✘ — A vient d'être amenée au premier plan, sans geste.
await cdp.send('Page.bringToFront', {}, a.sessionId);
await dodo(400);
cellules['focus=vrai,activation=faux'] = await cellule(a, `p3-${ETIQUETTE}-f1a0`);

// focus ✔, activation ✔ — geste de confiance immédiatement avant.
await geste(a);
cellules['focus=vrai,activation=vrai'] = await cellule(a, `p3-${ETIQUETTE}-f1a1`);

// focus ✘, × 2 — seulement si le focus a pu être retiré.
if (sansFocus) {
    // La fenêtre B n'a pas le focus (A est au premier plan) et n'a reçu aucun
    // geste.
    await cdp.send('Page.bringToFront', {}, a.sessionId);
    await dodo(400);
    cellules['focus=faux,activation=faux'] = await cellule(b, `p3-${ETIQUETTE}-f0a0`);

    // 🔴 LA CELLULE QUI TRANCHE, ET L'ORDRE EST LE MÉCANISME. Le geste de
    // confiance REND LE FOCUS à la fenêtre qui le reçoit — `t.focus()` puis un
    // `Input.dispatchKeyEvent` dirigé sur elle. Le jouer en dernier
    // produirait `{hasFocus:true, isActive:true}`, c'est-à-dire la cellule
    // PRÉCÉDENTE sous une autre étiquette.
    //
    // ⚠️ **UNE PREMIÈRE RÉDACTION DE CETTE SONDE A FAIT EXACTEMENT CELA, et
    // en a tiré « le §3.3 est RÉFUTÉ ».** Son propre relevé la réfutait —
    // la cellule dite « focus=faux » portait `hasFocus: true`. C'est
    // littéralement l'erreur d'attribution que D-P3-5 existe pour empêcher,
    // commise par l'instrument écrit pour l'empêcher. D'où les deux remèdes
    // ci-dessous : l'ordre geste → retrait du focus → écriture, ET le verdict
    // calculé sur l'ÉTAT OBSERVÉ, jamais sur l'étiquette voulue.
    //
    // `isActive` est transitoire mais persiste quelques secondes après le
    // geste : les ~400 ms du retrait de focus tiennent dans cette fenêtre, et
    // la cellule relève `isActive` au moment de l'appel pour qu'on le voie.
    //
    // DEUX moyens sont essayés, et l'attente porte sur le FAIT (`hasFocus`
    // relu) et jamais sur une durée — piège maison payé en D3 et D6.
    const retirerLeFocusDeB = async () => {
        await cdp.send('Page.bringToFront', {}, a.sessionId);
        for (let i = 0; i < 20; i += 1) {
            await dodo(100);
            if ((await etat(b)).hasFocus === false) return true;
        }
        return false;
    };

    // Moyen 1 : le geste ORDINAIRE (il focalise un `<textarea>` de B), puis on
    // reprend le focus.
    await geste(b);
    let retire = await retirerLeFocusDeB();
    // Moyen 2 : un geste qui NE FOCALISE AUCUN ÉLÉMENT — les seules frappes,
    // dirigées sur la session de B. `userActivation` est une propriété du
    // document, posée par tout événement d'entrée de confiance ; rien n'exige
    // qu'un élément prenne le focus.
    if (!retire) {
        await gesteSansFocus(b);
        retire = await retirerLeFocusDeB();
    }
    tentatives.push({ moyen: 'retrait du focus APRES un geste de confiance', hasFocus: !retire });
    dire('  retrait du focus APRES un geste de confiance →', retire ? 'obtenu' : 'IMPOSSIBLE');
    cellules['focus=faux,activation=vrai'] = await cellule(b, `p3-${ETIQUETTE}-f0a1`);
} else {
    cellules['focus=faux,activation=faux'] = { inatteignable: true };
    cellules['focus=faux,activation=vrai'] = { inatteignable: true };
}

// 🔴 LE VERDICT SE CALCULE SUR L'ÉTAT OBSERVÉ, JAMAIS SUR L'ÉTIQUETTE VOULUE.
// Une cellule dont l'état relevé ne correspond pas à ce qu'elle prétend
// mesurer n'est PAS cette cellule : elle est requalifiée en INATTEIGNABLE,
// et le §3.3 reste supposé.
const ATTENDU = {
    'focus=vrai,activation=faux': { hasFocus: true, isActive: false },
    'focus=vrai,activation=vrai': { hasFocus: true, isActive: true },
    'focus=faux,activation=faux': { hasFocus: false, isActive: false },
    'focus=faux,activation=vrai': { hasFocus: false, isActive: true },
};
for (const [nom, c] of Object.entries(cellules)) {
    if (c.inatteignable) continue;
    const a_ = ATTENDU[nom];
    if (c.etat.hasFocus !== a_.hasFocus || c.etat.isActive !== a_.isActive) {
        cellules[nom] = { inatteignable: true, requalifiee: true, etatObserve: c.etat, res: c.res };
        dire(`⚠️ cellule « ${nom} » REQUALIFIEE INATTEIGNABLE : etat observe ${JSON.stringify(c.etat)} != etiquette`);
    }
}

dire('');
for (const [nom, c] of Object.entries(cellules)) {
    dire(c.inatteignable
        ? `  ${nom.padEnd(28)} : INATTEIGNABLE`
        : `  ${nom.padEnd(28)} : etat=${JSON.stringify(c.etat)} → ${c.res}`);
}

// -------------------------------------------------------------------------
// LE VERDICT
// -------------------------------------------------------------------------
const tranchante = cellules['focus=faux,activation=vrai'];
const sansFocusSansGeste = cellules['focus=faux,activation=faux'];
const avecFocusSansGeste = cellules['focus=vrai,activation=faux'];

// 🔵 CE QUE LES DEUX CELLULES ATTEIGNABLES ÉTABLISSENT MALGRÉ TOUT, et c'est
// beaucoup plus qu'un « non tranché » : les deux refus portent le MÊME NOM
// (`NotAllowedError`) et des MESSAGES DIFFÉRENTS. Le nom seul ne les
// distinguait pas — c'est exactement pourquoi le plan (D-P3-5) exige le
// message verbatim, et c'est ce qui rend l'attribution possible.
const messagesDistincts = !sansFocusSansGeste.inatteignable && !avecFocusSansGeste.inatteignable
    && sansFocusSansGeste.res.startsWith('THROW') && avecFocusSansGeste.res.startsWith('THROW')
    && sansFocusSansGeste.res !== avecFocusSansGeste.res;
const refusNommeLeFocus = !sansFocusSansGeste.inatteignable
    && /not focused/i.test(sansFocusSansGeste.res ?? '');

let verdict;
if (tranchante.inatteignable) {
    verdict = 'NON TRANCHE — la cellule {focus:faux, activation:vrai} est INATTEIGNABLE a ce montage'
        + (tranchante.requalifiee
            ? ' : elle a ete JOUEE mais son etat observe ne correspond pas a son etiquette ('
              + JSON.stringify(tranchante.etatObserve) + '), donc ce n\'est pas cette cellule.'
            : ' (aucun des moyens essayes ne retire le focus a une page).')
        + (refusNommeLeFocus
            ? ' ⚠️ MAIS LE §3.3 EST CORROBORE SANS ETRE ETABLI, et par une piece : sans focus et sans geste, '
              + 'writeText refuse en NOMMANT le focus — « ' + sansFocusSansGeste.res + ' » — la ou le refus '
              + 'd\'activation dit « ' + avecFocusSansGeste.res + ' ». Les deux portent le MEME NOM et des '
              + 'MESSAGES DIFFERENTS : un chemin de refus PROPRE AU FOCUS existe donc et se nomme lui-meme. '
              + 'Ce qui reste non mesure est s\'il survit a une activation.'
            : ' Aucune piece ne corrobore le §3.3 dans cette execution.')
        + ' Le §3.3 reste SUPPOSE au sens strict. C\'est un verdict recevable ; en fabriquer un autre ne le serait pas.';
} else if (tranchante.res.startsWith('THROW')) {
    verdict = 'LE §3.3 EST CONFIRME — sans focus, writeText THROW meme sous activation. '
        + 'Message verbatim : ' + tranchante.res;
} else {
    verdict = 'LE §3.3 EST REFUTE — sans focus MAIS sous activation, writeText REUSSIT. '
        + 'La contrainte mesuree est l\'ACTIVATION, pas le focus.';
}
dire('');
dire('=== VERDICT S2 ===');
dire(verdict);
dire('emulation de focus : JAMAIS activee');

writeFileSync(SORTIE, JSON.stringify({
    sonde: 'S2 — writeText x focus x activation',
    etiquette: ETIQUETTE,
    date: new Date().toISOString(),
    navigateur: version.Browser,
    emulationDeFocusActivee: false,
    tentativesDeRetraitDuFocus: tentatives,
    focusRetirable: sansFocus,
    cellules,
    verdict,
    messagesDeRefusDistincts: messagesDistincts,
    leRefusSansFocusNommeLeFocus: refusNommeLeFocus,
}, null, 2));
dire('releve ecrit : ' + SORTIE);

chrome.kill('SIGKILL'); srv.close(); await dodo(300); process.exit(0);
