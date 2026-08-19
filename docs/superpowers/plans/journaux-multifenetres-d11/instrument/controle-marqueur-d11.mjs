#!/usr/bin/env node
// Sous-bloc D11, tâche 7, Step 4 — LE CONTRÔLE DE L'INSTRUMENT PAR LUI-MÊME.
//
// ⚠️ Il se joue AVANT toute recette, et SUR L'HÔTE SEUL : aucun flux WebRTC,
// aucune VM, aucun produit. Deux pages locales `anim-d11.html?n=3` — MÊME `n`
// — doivent faire signaler une COLLISION au prédicat ; `n=3` contre `n=4`
// doivent être distinctes.
//
// Sans ce contrôle, la recette ④ mesurerait avec un prédicat non éprouvé —
// exactement ce que D10 a fait, et c'est pourquoi son leg 8 reste ouvert.
//
// Usage : node controle-marqueur-d11.mjs

import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { Cdp, attendreDevtools, collision, dodo, EXPR_MARQUEUR, lancerChrome } from './commun-d11.mjs';

// `MIRE=<chemin>` pointe le contrôle sur une AUTRE mire. C'est ce qui permet
// de le voir ROUGE sur la mire de D10 (`anim-d4.html`), qui n'a pas de
// marqueur : le prédicat y échantillonne un fond qui dérive, exactement le
// défaut que le leg 8 décrit. Un contrôle qu'on n'a jamais vu refuser n'est
// pas un contrôle.
const MIRE = process.env.MIRE
    ? pathToFileURL(process.env.MIRE).href
    : pathToFileURL(join(import.meta.dirname, 'anim-d11.html')).href;
const log = (...a) => console.log(new Date().toISOString(), ...a);

async function marqueurDe(cdp, n) {
    const { targetId } = await cdp.send('Target.createTarget', { url: `${MIRE}?n=${n}` });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    await cdp.send('Runtime.enable', {}, sessionId);
    // Laisser tourner quelques trames : le fond doit avoir DÉRIVÉ, sinon le
    // contrôle ne prouverait rien sur l'invariance dans le temps.
    await dodo(1200);
    const a = await cdp.eval(sessionId, EXPR_MARQUEUR);
    await dodo(900);
    const b = await cdp.eval(sessionId, EXPR_MARQUEUR);
    return { targetId, sessionId, a, b };
}

const dir = await mkdtemp(join(tmpdir(), 'controle-marqueur-'));
const port = 9500 + Math.floor(Math.random() * 400);
const chrome = lancerChrome(port, dir);
let echecs = 0;
try {
    const cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);

    log('--- tirage 1 : MEME n (3 et 3) -> COLLISION attendue ---');
    const p1 = await marqueurDe(cdp, 3);
    const p2 = await marqueurDe(cdp, 3);
    log('page A n=3 :', JSON.stringify(p1.a), 'puis', JSON.stringify(p1.b));
    log('page B n=3 :', JSON.stringify(p2.a), 'puis', JSON.stringify(p2.b));

    // (a) INVARIANCE DANS LE TEMPS : deux relevés espacés de 900 ms sur la
    //     MÊME page, pendant que le fond dérive, doivent coïncider.
    for (const [nom, p] of [['A', p1], ['B', p2]]) {
        const stable = p.a.marqueur === p.b.marqueur;
        log(`  invariance dans le temps, page ${nom} : ${stable ? 'OUI' : 'NON'}`);
        if (!stable) { echecs += 1; log('  ECHEC : le marqueur DERIVE, il ne vaut rien'); }
    }
    // (b) COLLISION : c'est le rouge que D10 ne pouvait pas obtenir.
    const c = collision(p1.a.marqueur, p2.a.marqueur);
    log(`  collision detectee : ${c ? 'OUI' : 'NON'}`);
    if (!c) { echecs += 1; log('  ECHEC : deux pages de meme identite doivent COLLIDER'); }

    log('--- tirage 2 : n DIFFERENTS (3 et 4) -> DISTINCTION attendue ---');
    const p3 = await marqueurDe(cdp, 4);
    log('page C n=4 :', JSON.stringify(p3.a));
    const d = !collision(p1.a.marqueur, p3.a.marqueur);
    log(`  distinction detectee : ${d ? 'OUI' : 'NON'}`);
    if (!d) { echecs += 1; log('  ECHEC : deux identites distinctes doivent DIFFERER'); }

    log(echecs === 0 ? 'CONTROLE RECU : le predicat discrimine.'
                     : `CONTROLE REFUSE : ${echecs} echec(s).`);
} finally {
    chrome.kill('SIGKILL');
    await dodo(300);
    await rm(dir, { recursive: true, force: true });
}
process.exit(echecs === 0 ? 0 : 1);
