#!/usr/bin/env node
// Sous-bloc D11 — pilote de la recette ⑥ : LE MAILLON FAUTIF DU `Resize`
// (leg 7 de D9, leg 2 de D10), ET LE SEUL PILOTE QUI COLLECTE LA CONSOLE.
//
// D10 a instrumenté `client/src/main.ts` d'une grille de lecture à TROIS
// issues, puis n'a jamais joué le rejeu qui la lirait. Et il ne l'aurait pas pu :
// relevé du 19 août 2026, `grep -n 'Runtime.consoleAPICalled'` sur les TROIS
// pilotes de D10 rend ZÉRO — ils appellent tous `Runtime.enable`
// (`pilote-critere1-d10.mjs:278`, `pilote-critere2-d10.mjs:317`,
// `pilote-ab-d10.mjs:351`) sans jamais s'abonner. Les `console.debug` de la
// page n'étaient collectés NULLE PART.
//
// ⚠️⚠️ POURQUOI LA COLLECTE DOIT ÊTRE ÉPROUVÉE AVANT LA MESURE — la ligne la
// plus importante de ce fichier. L'ISSUE N°1 de la grille de `main.ts` se lit
// sur une ABSENCE DE LOG (« aucun log `declenchement` pour une session qui
// n'émet jamais de `Resize` »). Or une absence de log est EXACTEMENT ce que
// produit aussi un pilote qui ne collecte pas la console. LES DEUX SE LISENT
// PAREIL. Sans le contrôle ci-dessous, D11 rejouerait le naufrage de F1 (D7),
// où un contrôle ne pouvait structurellement pas dénoncer ce qu'il existait
// pour dénoncer.
//
// SI LA BALISE NE RESSORT PAS, LA RECETTE ⑥ EST ANNULÉE — pas interprétée.
// Une absence de log ne devient une information qu'une fois la balise
// ressortie.
//
// Les invariants de montage sont dans `commun-d11.mjs` — les relire.
//
// Usage :
//   node pilote-resize-d11.mjs --controle-collecte     (hôte seul, sans VM)
//   node pilote-resize-d11.mjs --url=<page-shell> --duree=90

import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from './commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const CONTROLE = process.argv.includes('--controle-collecte');
const URL_SHELL = arg('url', 'http://127.0.0.1:5173/shell.html?session=demo');
const DUREE = Number(arg('duree', '90'));
const SORTIE = arg('sortie', 'resize-console.json');
const log = (...a) => console.log(new Date().toISOString(), ...a);

const journalConsole = [];

/** L'abonnement que les trois pilotes de D10 n'avaient pas. */
function brancherConsole(cdp) {
    cdp.on((m) => {
        if (m.method !== 'Runtime.consoleAPICalled') return;
        const p = m.params;
        journalConsole.push({
            t: Date.now(),
            sessionId: m.sessionId ?? null,
            type: p.type,
            args: (p.args ?? []).map((a) => a.value ?? a.description ?? a.preview ?? null),
        });
    });
}

const dir = await mkdtemp(join(tmpdir(), 'resize-d11-'));
const port = 9470;
const chrome = lancerChrome(port, dir);
let code = 0;
try {
    const cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
    // `SANS_ABONNEMENT=1` reproduit EXACTEMENT le montage des trois pilotes de
    // D10 : `Runtime.enable` sans abonnement. C'est ce qui permet de voir ce
    // contrôle ROUGE, et donc d'établir qu'il n'est pas vacueux.
    if (process.env.SANS_ABONNEMENT === '1') {
        log('ABONNEMENT DESACTIVE (SANS_ABONNEMENT=1) : montage de D10 reproduit.');
    } else {
        brancherConsole(cdp);
    }
    const pages = new Map();
    cdp.on(async (m) => {
        if (m.method !== 'Target.attachedToTarget') return;
        const sid = m.params.sessionId;
        pages.set(sid, m.params.targetInfo.url);
        // `Runtime.enable` est ce que D10 faisait déjà ; c'est l'abonnement
        // ci-dessus qui manquait, pas lui.
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });

    if (CONTROLE) {
        // ------------------------------------------------------------------
        // LE CONTRÔLE D'ATTEIGNABILITÉ DE LA COLLECTE — hôte seul, sans VM.
        // ------------------------------------------------------------------
        const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
        const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
        await cdp.send('Runtime.enable', {}, sessionId);
        await dodo(300);
        await cdp.send('Runtime.evaluate',
            { expression: "console.debug('[sonde collecte] balise', {a:1})" }, sessionId);
        await dodo(800);
        const vue = journalConsole.some((e) =>
            (e.args ?? []).some((a) => typeof a === 'string' && a.includes('[sonde collecte] balise')));
        log('entrees de console collectees :', journalConsole.length);
        log('journal :', JSON.stringify(journalConsole));
        if (vue) {
            log('CONTROLE DE COLLECTE RECU : la balise ressort, une absence de log devient une information.');
        } else {
            log('CONTROLE DE COLLECTE REFUSE : la balise NE ressort PAS.');
            log('=> LA RECETTE 6 EST ANNULEE, PAS INTERPRETEE.');
            code = 1;
        }
    } else {
        await cdp.send('Target.createTarget', { url: URL_SHELL });
        await dodo(DUREE * 1000);
        const instr = journalConsole.filter((e) =>
            (e.args ?? []).some((a) => typeof a === 'string' && a.includes('[instrumentation resize]')));
        const differes = journalConsole.filter((e) =>
            (e.args ?? []).some((a) => typeof a === 'string' && a.includes('Resize différé')));
        log(`console : ${journalConsole.length} entrees, `
            + `${instr.length} d'instrumentation resize, ${differes.length} « Resize différé »`);
        // ⚠️ LECTURE, dans CET ORDRE, et aucune conclusion au-delà du relevé :
        //   1. AUCUN « declenchement » pour une session qui n'émet jamais de
        //      `Resize` => l'observateur ne s'arme jamais ou n'est jamais
        //      rappelé : le maillon est EN AMONT de la mise en page.
        //   2. Présent, et `clientWidth/Height` SUIT `innerWidth/Height`
        //      => ni l'observateur ni la mise en page ne sont en cause.
        //   3. Présent, et il NE SUIT PAS => la mise en page CSS du `<video>`
        //      est en cause.
        // Le canal de contrôle reste une HYPOTHÈSE À PART ENTIÈRE : la pièce
        // de D8 qui prétendait le disculper est réfutée (C3), et rien ne le
        // remplace. Ne PAS désigner le client sur la seule foi de cette grille.
    }
} finally {
    await writeFile(SORTIE, JSON.stringify({ controle: CONTROLE, journalConsole }, null, 2));
    log('journal de console ecrit dans', SORTIE);
    chrome.kill('SIGKILL');
    await dodo(300);
    await rm(dir, { recursive: true, force: true });
}
process.exit(code);
