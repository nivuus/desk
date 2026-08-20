#!/usr/bin/env node
// Sous-bloc P2 (presse-papier) — LE PRÉALABLE ÉLIMINATOIRE, mesuré.
//
// Question, telle que la spec la pose
// (`docs/superpowers/specs/2026-08-19-presse-papier-design.md:838`, critère ①
// de P2) : **l'événement `paste` de confiance parvient-il quand le focus est
// sur le `<video>` ?** Le relevé R2 de la spec (§3.2) a été pris avec le focus
// sur un `body` ; le §3.3 le dit lui-même, et range la question en tête de P2.
//
// ---------------------------------------------------------------------------
// CE QUI REND UN VERDICT NÉGATIF FONDÉ, ET POURQUOI IL FALLAIT L'ÉCRIRE
// ---------------------------------------------------------------------------
//
// 🔴 La sonde P0 de P1 a rendu un FAUX verdict éliminatoire : trois zéros lus
// sur une machine saine, parce que rien n'avait encore eu lieu
// (`journaux-presse-papier-p1/p0-sonde-0-instrument-defectueux.log`). **Un
// verdict négatif exige que la chose mesurée soit ABSENTE, pas seulement
// nulle.**
//
// D'où la forme de cette sonde : elle ne mesure pas UNE cellule, elle mesure
// une MATRICE {cible de focus} × {régime de `preventDefault`} × {raccourci}
// dans UNE SEULE session de navigateur, et le verdict de la cellule
// `video × etroit` ne se lit QUE relativement aux autres :
//
//   - si `body × etroit` rend un `paste` et `video × etroit` n'en rend aucun,
//     le verdict négatif est FONDÉ — le montage sait produire un `paste`, et
//     le `<video>` le refuse ;
//   - si AUCUNE cellule ne rend de `paste`, la sonde ne conclut RIEN : elle
//     mesure son instrument. Le champ `verdict` vaut alors `NON MESURABLE`.
//
// Et le régime `produit` — `preventDefault()` inconditionnel, la copie exacte
// de `client/src/input.ts:94-98` — est la ROUGE DE L'INSTRUMENT : il doit
// rendre ZÉRO `paste` partout. S'il en rendait un, l'instrument ne
// reproduirait pas le client et rien de ce qui suit ne vaudrait.
//
// ---------------------------------------------------------------------------
// CE QUE LA SONDE NE MESURE PAS, ET IL FAUT LE DIRE
// ---------------------------------------------------------------------------
//
//   - **Rien d'un Chromium AVEC interface**, rien de Firefox, rien de Safari.
//     Même portée que le §3 de la spec, qui la déclare déjà.
//   - **Rien de `Ctrl+W`/`Ctrl+T`/`Ctrl+N`** (critère ⑤ de P2) : `--headless`
//     n'a ni onglets ni fenêtres au sens de l'utilisateur, et
//     `Input.dispatchKeyEvent` n'emprunte pas le chemin des raccourcis du
//     NAVIGATEUR. Ce critère se joue sur un navigateur réel, à la recette.
//   - **Rien de la VM Windows** : un chantier concurrent la tient.
//   - **Aucun taux** : deux exécutions, comme tout ce dépôt.
//
// ---------------------------------------------------------------------------
// PIÈGES D'INSTRUMENT ÉVITÉS, NOMMÉS
// ---------------------------------------------------------------------------
//
//   - **`--virtual-time-budget` n'attend pas une E/S réelle** : il n'est pas
//     employé. Toutes les attentes sont des `dodo()` de temps mural.
//   - **Le presse-papier est amorcé par une VRAIE copie** (`Ctrl+C` de
//     confiance sur un `<textarea>`), jamais par `navigator.clipboard.write*`
//     — qui rejette en `NotAllowedError` sur ce montage (relevé, champ
//     `writeTextAmorce`), et qui aurait de toute façon demandé une permission
//     que le chemin sous test n'a pas.
//   - **Un texte d'amorce DIFFÉRENT par cellule** : sans lui, un `paste`
//     rendant le texte de la cellule précédente serait indiscernable d'un
//     `paste` de la cellule courante.
//
// Usage : node sonde-p2-paste-video.mjs --etiquette=1 [--sortie=x.json]

import { createServer } from 'node:http';
import { writeFileSync } from 'node:fs';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-multifenetres-d11/instrument/commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const ETIQUETTE = arg('etiquette', '1');
const SORTIE = arg('sortie', `p2-paste-video-${ETIQUETTE}.json`);
const PORT_HTTP = Number(arg('port-http', '45880'));
const PORT_CDP = Number(arg('port-cdp', '9560'));

const journal = [];
const dire = (...m) => { const l = m.join(' '); journal.push(l); console.log(l); };

// La page reproduit `client/index.html` AU POINT QUI COMPTE : un
// `<video id="remote" … tabindex="0">` (l. 15 de ce fichier), plus le conteneur
// et l'élément racine, pour éprouver les trois porteurs de focus possibles.
// Les écouteurs `keydown`/`keyup` sont sur `window`, comme `input.ts:111-112`.
const PAGE = `<!doctype html>
<html lang="fr"><head><meta charset="utf-8"><title>sonde P2</title></head>
<body>
<textarea id="amorce" style="position:absolute;left:-9999px"></textarea>
<div id="conteneur" tabindex="0"><video id="remote" autoplay playsinline muted tabindex="0"></video></div>
<script>
// Trois régimes de preventDefault :
//   'produit' — inconditionnel, la copie exacte de client/src/input.ts:94-98 ;
//   'etroit'  — l'exception de D6, et ELLE SEULE : Ctrl+V et Shift+Insert
//               passent, TOUT le reste (y compris ControlLeft et ShiftLeft
//               eux-mêmes) garde son preventDefault ;
//   'aucun'   — témoin large.
window.__regime = 'produit';
window.__k = []; window.__p = [];
function estCollage(e) {
  return (e.ctrlKey && e.code === 'KeyV') || (e.shiftKey && e.code === 'Insert');
}
window.addEventListener('keydown', (e) => {
  if (window.__regime === 'produit') e.preventDefault();
  else if (window.__regime === 'etroit' && !estCollage(e)) e.preventDefault();
  window.__k.push({ code: e.code, ctrl: e.ctrlKey, shift: e.shiftKey, dp: e.defaultPrevented, trusted: e.isTrusted });
});
window.addEventListener('paste', (e) => {
  window.__p.push({
    trusted: e.isTrusted,
    texte: e.clipboardData ? e.clipboardData.getData('text/plain') : null,
    types: e.clipboardData ? Array.from(e.clipboardData.types) : [],
    cible: e.target && e.target.tagName ? e.target.tagName + '#' + (e.target.id || '') : String(e.target),
  });
});
<\/script></body></html>`;

const srv = createServer((q, r) => { r.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); r.end(PAGE); });
await new Promise((r) => srv.listen(PORT_HTTP, '127.0.0.1', r));

const dir = await mkdtemp(join(tmpdir(), 'p2-sonde-'));
const chrome = lancerChrome(PORT_CDP, dir);
const version = await attendreDevtools(PORT_CDP);
dire('navigateur', version.Browser, '| sans interface (--headless=new)');

const cdp = new Cdp(version.webSocketDebuggerUrl);
const { targetId } = await cdp.send('Target.createTarget', { url: `http://127.0.0.1:${PORT_HTTP}/` });
const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
await cdp.send('Page.enable', {}, sessionId);
await cdp.send('Runtime.enable', {}, sessionId);
await dodo(900);
await cdp.send('Page.bringToFront', {}, sessionId);

const ctrl = { key: 'Control', code: 'ControlLeft', vk: 17 };
const maj = { key: 'Shift', code: 'ShiftLeft', vk: 16 };
const bas = (t, mods) => cdp.send('Input.dispatchKeyEvent',
    { type: 'rawKeyDown', modifiers: mods, key: t.key, code: t.code, windowsVirtualKeyCode: t.vk, nativeVirtualKeyCode: t.vk }, sessionId);
const haut = (t, mods) => cdp.send('Input.dispatchKeyEvent',
    { type: 'keyUp', modifiers: mods, key: t.key, code: t.code, windowsVirtualKeyCode: t.vk, nativeVirtualKeyCode: t.vk }, sessionId);

/** Un raccourci COMPLET, modificateur compris, en touches de confiance. */
async function raccourci(nom) {
    if (nom === 'ctrl+v') {
        await bas(ctrl, 2); await bas({ key: 'v', code: 'KeyV', vk: 86 }, 2);
        await haut({ key: 'v', code: 'KeyV', vk: 86 }, 2); await haut(ctrl, 0);
    } else if (nom === 'ctrl+c') {
        await bas(ctrl, 2); await bas({ key: 'c', code: 'KeyC', vk: 67 }, 2);
        await haut({ key: 'c', code: 'KeyC', vk: 67 }, 2); await haut(ctrl, 0);
    } else if (nom === 'shift+insert') {
        await bas(maj, 8); await bas({ key: 'Insert', code: 'Insert', vk: 45 }, 8);
        await haut({ key: 'Insert', code: 'Insert', vk: 45 }, 8); await haut(maj, 0);
    } else throw new Error('raccourci inconnu ' + nom);
}

const CIBLES = {
    body: `document.activeElement && document.activeElement.blur && document.activeElement.blur(); document.body.focus(); 'ok'`,
    video: `document.querySelector('#remote').focus(); 'ok'`,
    conteneur: `document.querySelector('#conteneur').focus(); 'ok'`,
    html: `document.activeElement && document.activeElement.blur && document.activeElement.blur(); document.documentElement.focus(); 'ok'`,
};
const REGIMES = ['produit', 'etroit', 'aucun'];
const RACCOURCIS = ['ctrl+v', 'shift+insert'];

// L'état des permissions, relevé une fois : le chemin sous test ne doit
// dépendre d'aucune d'elles (R1/R2 de la spec, §3.1 et §3.2).
const permLecture = await cdp.eval(sessionId, `navigator.permissions.query({name:'clipboard-read'}).then(p=>p.state,e=>'ERREUR:'+e.name)`, true);
const permEcriture = await cdp.eval(sessionId, `navigator.permissions.query({name:'clipboard-write'}).then(p=>p.state,e=>'ERREUR:'+e.name)`, true);
const readText = await cdp.eval(sessionId, `navigator.clipboard.readText().then(t=>'OK:'+t,e=>'THROW:'+e.name)`, true);
const writeTextAmorce = await cdp.eval(sessionId, `navigator.clipboard.writeText('x').then(()=>'OK',e=>'THROW:'+e.name)`, true);
// ⚠️ Relevé pour que le résultat de `writeText` ci-dessus soit LISIBLE et non
// mal imputé : le §3.1 de la spec a mesuré `writeText: OK` dans un contexte où
// `userActivation.isActive` valait DÉJÀ `true`. Ici la lecture est prise AVANT
// tout geste. Ce que cette sonde établit du sens VM → navigateur : RIEN — ce
// n'est pas sa question, et P1 l'a déjà recetté.
const activation = await cdp.eval(sessionId, `JSON.stringify({isActive:navigator.userActivation.isActive,hasBeenActive:navigator.userActivation.hasBeenActive})`);
dire('permissions   clipboard-read=' + permLecture, 'clipboard-write=' + permEcriture);
dire('readText()    ' + readText);
dire('writeText()   ' + writeTextAmorce, '| userActivation=' + activation);
dire('isSecureContext=' + await cdp.eval(sessionId, 'isSecureContext'),
     'hasFocus=' + await cdp.eval(sessionId, 'document.hasFocus()'));
dire('');

const cellules = [];
let n = 0;
for (const cible of Object.keys(CIBLES)) {
    for (const regime of REGIMES) {
        for (const rac of RACCOURCIS) {
            n += 1;
            const attendu = `P2-${ETIQUETTE}-${String(n).padStart(2, '0')}`;
            // AMORCE — une vraie copie, régime 'aucun' le temps du Ctrl+C.
            // IIFE OBLIGATOIRE : `Runtime.evaluate` évalue dans la portée
            // globale, et un `const` y persiste d'un appel à l'autre — la
            // deuxième cellule échouerait en « Identifier has already been
            // declared ». Rencontré à la première exécution de cette sonde.
            await cdp.eval(sessionId, `(() => { window.__regime='aucun'; window.__k=[]; window.__p=[];
                const a=document.querySelector('#amorce'); a.value=${JSON.stringify(attendu)}; a.focus(); a.select(); return 'ok'; })()`);
            await raccourci('ctrl+c');
            await dodo(250);
            // La cellule elle-même.
            await cdp.eval(sessionId, `window.__k=[]; window.__p=[]; window.__regime=${JSON.stringify(regime)}; ${CIBLES[cible]}`);
            const actif = await cdp.eval(sessionId, `document.activeElement.tagName+'#'+(document.activeElement.id||'')`);
            const focusDoc = await cdp.eval(sessionId, 'document.hasFocus()');
            await raccourci(rac);
            await dodo(350);
            const k = await cdp.eval(sessionId, 'window.__k');
            const p = await cdp.eval(sessionId, 'window.__p');
            const juste = p.length === 1 && p[0].trusted === true && p[0].texte === attendu;
            cellules.push({ cible, regime, raccourci: rac, activeElement: actif, hasFocus: focusDoc, attendu,
                keydowns: k, pastes: p, pasteJuste: juste });
            dire(`cellule ${String(n).padStart(2, '0')}  cible=${cible.padEnd(10)} regime=${regime.padEnd(8)} ${rac.padEnd(13)}`
                + ` active=${actif.padEnd(16)} pastes=${p.length}`
                + ` texte=${p.length ? JSON.stringify(p[0].texte) : '—'}`
                + ` cibleEvt=${p.length ? p[0].cible : '—'}`
                + ` juste=${juste}`);
        }
    }
}

// ---------------------------------------------------------------------------
// LE VERDICT, et sa condition de fondement
// ---------------------------------------------------------------------------
const cell = (c, r, k) => cellules.find((x) => x.cible === c && x.regime === r && x.raccourci === k);
const videoEtroitV = cell('video', 'etroit', 'ctrl+v');
const bodyEtroitV = cell('body', 'etroit', 'ctrl+v');
const produitZero = cellules.filter((x) => x.regime === 'produit').every((x) => x.pastes.length === 0);
const auMoinsUn = cellules.some((x) => x.pastes.length > 0);

let verdict;
let motif;
if (!auMoinsUn) {
    verdict = 'NON MESURABLE';
    motif = 'aucune cellule ne rend de paste : la sonde mesure son instrument, pas le produit';
} else if (videoEtroitV.pasteJuste) {
    verdict = 'FAVORABLE';
    motif = 'paste de confiance recu avec le focus sur le <video>, portant le dernier texte copie';
} else {
    verdict = 'DEFAVORABLE';
    motif = `aucun paste juste sur le <video> alors que body=${bodyEtroitV.pastes.length} paste(s)`
        + ' dans la MEME session : le montage sait en produire, le <video> le refuse';
}

dire('');
dire('=== VERDICT ===');
dire('verdict                       : ' + verdict);
dire('motif                         : ' + motif);
dire('temoin de mesurabilite        : body/etroit/ctrl+v pastes=' + bodyEtroitV.pastes.length
     + ' juste=' + bodyEtroitV.pasteJuste);
dire('rouge de l\'instrument         : regime "produit" (input.ts:94-98) rend zero paste partout : ' + produitZero);
dire('cellules avec paste            : ' + cellules.filter((x) => x.pastes.length > 0)
     .map((x) => `${x.cible}/${x.regime}/${x.raccourci}`).join(', '));

const releve = { etiquette: ETIQUETTE, date: new Date().toISOString(), navigateur: version.Browser,
    permissions: { lecture: permLecture, ecriture: permEcriture }, readText, writeTextAmorce, activation,
    verdict, motif, temoinMesurabilite: { pastes: bodyEtroitV.pastes.length, juste: bodyEtroitV.pasteJuste },
    rougeInstrumentZeroPartout: produitZero, cellules };
writeFileSync(SORTIE, JSON.stringify(releve, null, 2));
dire('releve ecrit                  : ' + SORTIE);

chrome.kill('SIGKILL');
srv.close();
await dodo(300);
process.exit(0);
