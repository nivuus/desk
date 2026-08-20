#!/usr/bin/env node
// Sous-bloc P2 (presse-papier) — SONDE ANNEXE, hors question principale.
//
// ⚠️ ELLE NE MESURE PAS LE PRÉALABLE DE P2. Elle existe pour une seule raison :
// la sonde principale (`sonde-p2-paste-video.mjs`) relève
// `writeText() THROW:NotAllowedError` là où le §3.1 de la spec relève
// `writeText : OK`. Publier ce THROW sans l'expliquer laisserait un fait
// apparemment contradictoire avec R1, et donc avec tout le sens VM →
// navigateur que P1 a déjà recetté.
//
// L'hypothèse à éprouver est écrite dans la spec elle-même (§3, portée) : le
// relevé R1 a été pris « dans un contexte où `navigator.userActivation.isActive`
// valait DÉJÀ `true` ». La sonde principale, elle, lit AVANT tout geste.
//
// Ce qui est mesuré ici, et rien d'autre : `writeText` AVANT un geste de
// confiance, puis APRÈS. Une exécution qui rendrait THROW/THROW réfuterait
// l'hypothèse ; une exécution qui rend THROW/OK la corrobore SANS l'établir —
// le geste n'est pas la seule variable qui a changé entre les deux appels.
//
// Usage : node sonde-p2-annexe-writetext.mjs --etiquette=1

import { createServer } from 'node:http';
import { writeFileSync } from 'node:fs';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-multifenetres-d11/instrument/commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const ETIQUETTE = arg('etiquette', '1');
const SORTIE = arg('sortie', `p2-annexe-writetext-${ETIQUETTE}.json`);
const PORT_HTTP = Number(arg('port-http', '45890'));
const PORT_CDP = Number(arg('port-cdp', '9570'));

const journal = [];
const dire = (...m) => { const l = m.join(' '); journal.push(l); console.log(l); };

const PAGE = `<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>annexe</title></head>
<body><textarea id="amorce" style="position:absolute;left:-9999px"></textarea>
<video id="remote" tabindex="0"></video></body></html>`;

const srv = createServer((q, r) => { r.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); r.end(PAGE); });
await new Promise((r) => srv.listen(PORT_HTTP, '127.0.0.1', r));
const dir = await mkdtemp(join(tmpdir(), 'p2-annexe-'));
const chrome = lancerChrome(PORT_CDP, dir);
const version = await attendreDevtools(PORT_CDP);
const cdp = new Cdp(version.webSocketDebuggerUrl);
const { targetId } = await cdp.send('Target.createTarget', { url: `http://127.0.0.1:${PORT_HTTP}/` });
const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
await cdp.send('Page.enable', {}, sessionId);
await cdp.send('Runtime.enable', {}, sessionId);
await dodo(900);
await cdp.send('Page.bringToFront', {}, sessionId);

const etat = async () => JSON.parse(await cdp.eval(sessionId,
    `JSON.stringify({isActive:navigator.userActivation.isActive,hasBeenActive:navigator.userActivation.hasBeenActive,hasFocus:document.hasFocus()})`));
const ecrire = (t) => cdp.eval(sessionId, `navigator.clipboard.writeText(${JSON.stringify(t)}).then(()=>'OK',e=>'THROW:'+e.name)`, true);

dire('navigateur', version.Browser);
const avant = await etat();
const rAvant = await ecrire(`annexe-${ETIQUETTE}-avant`);
dire('AVANT tout geste :', JSON.stringify(avant), '→ writeText', rAvant);

// Un geste de confiance : une VRAIE frappe. Ctrl+C sur un textarea sélectionné.
await cdp.eval(sessionId, `(() => { const a=document.querySelector('#amorce'); a.value='geste'; a.focus(); a.select(); return 'ok'; })()`);
for (const [type, mods] of [['rawKeyDown', 2], ['keyUp', 0]]) {
    if (type === 'rawKeyDown') {
        await cdp.send('Input.dispatchKeyEvent', { type, modifiers: 2, key: 'Control', code: 'ControlLeft', windowsVirtualKeyCode: 17, nativeVirtualKeyCode: 17 }, sessionId);
        await cdp.send('Input.dispatchKeyEvent', { type, modifiers: 2, key: 'c', code: 'KeyC', windowsVirtualKeyCode: 67, nativeVirtualKeyCode: 67 }, sessionId);
    } else {
        await cdp.send('Input.dispatchKeyEvent', { type, modifiers: 2, key: 'c', code: 'KeyC', windowsVirtualKeyCode: 67, nativeVirtualKeyCode: 67 }, sessionId);
        await cdp.send('Input.dispatchKeyEvent', { type, modifiers: mods, key: 'Control', code: 'ControlLeft', windowsVirtualKeyCode: 17, nativeVirtualKeyCode: 17 }, sessionId);
    }
}
await dodo(300);
const apres = await etat();
const rApres = await ecrire(`annexe-${ETIQUETTE}-apres`);
dire('APRES un Ctrl+C de confiance :', JSON.stringify(apres), '→ writeText', rApres);

const hypothese = rAvant.startsWith('THROW') && rApres === 'OK'
    ? 'CORROBOREE — writeText refuse sans activation, accepte apres. NON ETABLIE : le geste n\'est pas la seule variable.'
    : (rAvant === rApres
        ? `NON DISCRIMINANTE — les deux appels rendent ${rAvant}`
        : `INATTENDUE — avant=${rAvant} apres=${rApres}`);
dire('');
dire('=== ANNEXE ===');
dire('hypothese (R1 mesure sous activation) : ' + hypothese);
dire('⚠️ Cette sonde n\'etablit RIEN du prealable de P2, et rien du sens VM → navigateur,');
dire('   que P1 a recette par ailleurs (4 executions).');

writeFileSync(SORTIE, JSON.stringify({ etiquette: ETIQUETTE, date: new Date().toISOString(),
    navigateur: version.Browser, avant, writeTextAvant: rAvant, apres, writeTextApres: rApres, hypothese }, null, 2));
dire('releve ecrit : ' + SORTIE);
chrome.kill('SIGKILL'); srv.close(); await dodo(300); process.exit(0);
