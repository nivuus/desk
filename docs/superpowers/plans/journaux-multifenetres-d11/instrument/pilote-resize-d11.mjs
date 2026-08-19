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
// ressortie. ⚠️ ET LA BALISE SE POSE DANS CHAQUE PAGE D'APPLICATION DU RUN
// RÉEL, pas seulement sur un `about:blank` de l'hôte : c'est la page
// d'application qui doit prouver que SA console remonte, puisque c'est SON
// silence qu'on s'apprête à interpréter.
//
// ⚠️ AJOUT DE LA TÂCHE 14, déclaré : ce pilote ne portait, à sa création
// (tâche 8), QUE la plomberie CDP — il ouvrait la page-shell et attendait des
// fenêtres que personne n'ouvrait. Le montage vient de `montage-d11.mjs`.
//
// ⚠️ LE CANAL DE CONTRÔLE RESTE UNE HYPOTHÈSE À PART ENTIÈRE. La correction C3
// de D8 a établi qu'AUCUNE pièce ne le disculpe — celle qui prétendait le
// faire portait sur tout le run et non sur la phase, laquelle était muette
// 141 s. Cette recette rend le maillon DÉCIDABLE ; elle ne l'identifie pas
// d'avance.
//
// Usage :
//   node pilote-resize-d11.mjs --controle-collecte      (hôte seul, sans VM)
//   ETIQUETTE=resize-1 N=5 node pilote-resize-d11.mjs

import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { Cdp, attendreDevtools, dodo, lancerChrome } from './commun-d11.mjs';
import {
    D11, HOTE, copierLog, journal as log, loadavg, lancerSuperviseur, marqueurs,
    ouvrirFenetre, preparerVm, sonde, tuerAgent, virshEtat,
} from './montage-d11.mjs';

const CONTROLE = process.argv.includes('--controle-collecte');
const ETIQUETTE = process.env.ETIQUETTE ?? 'resize-sans-etiquette';
const N = Number(process.env.N ?? 5);
const SANS_VIEWPORT = process.env.SANS_VIEWPORT === '1';
const DELAI_ENTRE_FENETRES_MS = Number(process.env.DELAI_ENTRE_FENETRES_MS ?? 8000);
const OBSERVATION_S = Number(process.env.OBSERVATION_S ?? 90);
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const SORTIE = process.env.SORTIE ?? `${D11}/${ETIQUETTE}.json`;
const JSONL = process.env.JSONL ?? `${D11}/console-${ETIQUETTE}.jsonl`;

const journalConsole = [];
const urlDe = new Map();

/**
 * ⚠️ DÉFAUT D'INSTRUMENT TROUVÉ PAR L'EXÉCUTION (resize-1, 19 août 2026), et
 * il annulait la moitié de la lecture : `a.value ?? a.description ?? a.preview`
 * rend la CHAÎNE `"Object"` pour tout argument objet — c'est-à-dire pour
 * `{clientWidth, clientHeight, innerWidth, innerHeight}`, les QUATRE nombres
 * dont la grille de `main.ts` a besoin pour départager ses issues 2 et 3.
 * Le premier run a donc rendu dix lignes `[instrumentation resize] … "Object"`,
 * qui tranchent l'issue 1 et RIEN d'autre. `Runtime.consoleAPICalled` livre les
 * champs dans `preview.properties` : c'est de là qu'ils se relèvent.
 */
function aplatir(a) {
    if (a == null) return null;
    if (a.value !== undefined) return a.value;
    if (a.preview?.properties) {
        const o = {};
        for (const q of a.preview.properties) o[q.name] = q.type === 'number' ? Number(q.value) : q.value;
        if (a.preview.overflow) o.__tronque = true;
        return o;
    }
    return a.description ?? null;
}

/** L'abonnement que les trois pilotes de D10 n'avaient pas. */
function brancherConsole(cdp) {
    cdp.on((m) => {
        if (m.method !== 'Runtime.consoleAPICalled') return;
        const p = m.params;
        journalConsole.push({
            t: new Date().toISOString(),
            sessionId: m.sessionId ?? null,
            url: urlDe.get(m.sessionId) ?? null,
            type: p.type,
            args: (p.args ?? []).map(aplatir),
        });
    });
}

const port = Number(process.env.PORT_CDP ?? 9470);
const dir = await mkdtemp(join(tmpdir(), 'resize-d11-'));
const chrome = lancerChrome(port, dir, [
    '--remote-allow-origins=*', '--disable-dev-shm-usage', '--disable-gpu',
    '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
    '--disable-features=WebRtcHideLocalIpsWithMdns', 'about:blank',
]);
let code = 0;
const releve = { etiquette: ETIQUETTE, controle: CONTROLE, n_fenetres: N, viewport_impose: !SANS_VIEWPORT, virsh_debut: virshEtat(), loadavg_debut: loadavg() };
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
        if (m.method === 'Target.targetInfoChanged') {
            for (const [sid, p] of pages) {
                if (p.targetId === m.params.targetInfo.targetId) { p.url = m.params.targetInfo.url; urlDe.set(sid, p.url); }
            }
            return;
        }
        if (m.method === 'Target.detachedFromTarget') { pages.delete(m.params.sessionId); return; }
        if (m.method !== 'Target.attachedToTarget') return;
        const { sessionId: sid, targetInfo: ti } = m.params;
        if (ti.type !== 'page') { await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { }); return; }
        pages.set(sid, { targetId: ti.targetId, url: ti.url });
        urlDe.set(sid, ti.url);
        await cdp.send('Page.enable', {}, sid).catch(() => { });
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
        // ⚠️ `SANS_VIEWPORT=1` retire l'imposition de viewport que les pilotes
        // de D10 posaient sur chaque page d'application. Elle est elle-même un
        // changement de taille : elle peut donc PROVOQUER le `ResizeObserver`
        // qu'on prétend observer. Les deux bras existent pour que la mesure
        // n'ait pas à supposer qu'elle est neutre.
        if (!SANS_VIEWPORT && !ti.url.includes('shell.html') && ti.url !== 'about:blank') {
            await cdp.send('Emulation.setDeviceMetricsOverride',
                { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false }, sid).catch(() => { });
        }
        await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
        log('+ page attachée', sid.slice(0, 8), ti.url);
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await cdp.send('Target.setDiscoverTargets', { discover: true });

    if (CONTROLE) {
        // ---- Le contrôle d'atteignabilité, hôte seul, sans VM. ----
        const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
        const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
        await cdp.send('Runtime.enable', {}, sessionId);
        await dodo(300);
        await cdp.send('Runtime.evaluate',
            { expression: "console.debug('[sonde collecte] balise', {a:1})" }, sessionId);
        await dodo(800);
        const vue = journalConsole.some((e) =>
            (e.args ?? []).some((a) => typeof a === 'string' && a.includes('[sonde collecte] balise')));
        log('entrees de console collectees :', journalConsole.length, JSON.stringify(journalConsole));
        log(vue ? 'CONTROLE DE COLLECTE RECU' : 'CONTROLE DE COLLECTE REFUSE => RECETTE 6 ANNULEE');
        if (!vue) code = 1;
    } else {
        log(`>>> ÉTAPE 0 : préparation VM — ${N} fenêtres`);
        releve.preparation = preparerVm(ETIQUETTE);
        log('>>> purge des sorties virtuelles (lancement à elle seule)');
        sonde('purge', { MULTIFENETRE_VDD_PURGE: '1' });
        await dodo(20000);
        tuerAgent();

        await cdp.send('Target.createTarget', { url: URL_SHELL });
        await dodo(4000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");

        log('>>> lancement du superviseur');
        lancerSuperviseur({});
        await dodo(8000);
        for (let i = 1; i <= N; i += 1) {
            log(`>>> OUVERTURE fenêtre ${i}/${N}`);
            ouvrirFenetre(ETIQUETTE, i, i);
            await dodo(DELAI_ENTRE_FENETRES_MS);
        }
        log(`>>> OBSERVATION ${OBSERVATION_S} s`);
        await dodo(OBSERVATION_S * 1000);

        // ---- LA BALISE, DANS CHAQUE PAGE D'APPLICATION DU RUN RÉEL. ----
        // C'est ici, et pas sur un `about:blank`, que se joue le contrôle :
        // c'est le silence de CES pages qu'on s'apprête à interpréter.
        releve.balises = [];
        for (const [sid, p] of pages) {
            if (/shell/.test(p.url ?? '') || p.url === 'about:blank') continue;
            await cdp.send('Runtime.evaluate',
                { expression: `console.debug('[sonde collecte] balise', ${JSON.stringify(p.url)})` }, sid)
                .catch((e) => log('!! balise refusée', sid.slice(0, 8), String(e).slice(0, 120)));
            releve.balises.push({ sessionId: sid, url: p.url });
        }
        await dodo(2000);
        const balisesVues = new Set(journalConsole
            .filter((e) => (e.args ?? []).some((a) => typeof a === 'string' && a.includes('[sonde collecte] balise')))
            .map((e) => e.sessionId));
        releve.balises_posees = releve.balises.length;
        releve.balises_ressorties = balisesVues.size;
        log(`CONTRÔLE D'ATTEIGNABILITÉ : ${releve.balises_ressorties}/${releve.balises_posees} pages `
            + "d'application dont la console remonte");
        if (releve.balises_posees === 0 || releve.balises_ressorties < releve.balises_posees) {
            log('!! LA RECETTE ⑥ EST ANNULÉE, PAS INTERPRÉTÉE.');
            code = 1;
        }

        releve.pages = [...pages].map(([sid, p]) => ({ sessionId: sid, url: p.url }));
        releve.marqueurs_journal = marqueurs('fin d’observation');
        const instr = journalConsole.filter((e) =>
            (e.args ?? []).some((a) => typeof a === 'string' && a.includes('[instrumentation resize]')));
        const differes = journalConsole.filter((e) =>
            (e.args ?? []).some((a) => typeof a === 'string' && a.includes('Resize différé')));
        log(`console : ${journalConsole.length} entrées, ${instr.length} d'instrumentation resize, `
            + `${differes.length} « Resize différé »`);
        // Côté agent : les `contrôle reçu` PORTENT LEUR SESSION depuis la
        // tâche 5 de D9 — c'est ce qui rend le leg 10 décidable.
        const r = spawnSync('bash', ['-c',
            "sed 's/\\x1b\\[[0-9;]*m//g' /media/vm/dev/agent.log | grep -a 'contrôle reçu' "
            + "| grep -o 'session=[^ ]* [A-Za-z]*' | sort | uniq -c"], { encoding: 'utf8' });
        releve.controles_recus = (r.stdout ?? '').trim();
        log('contrôles reçus par l’agent :\n' + releve.controles_recus);
    }
} finally {
    releve.loadavg_fin = loadavg();
    releve.virsh_fin = virshEtat();
    await writeFile(SORTIE, JSON.stringify(releve, null, 1));
    await writeFile(JSONL, journalConsole.map((e) => JSON.stringify(e)).join('\n') + '\n');
    log('relevé écrit dans', SORTIE, '| console dans', JSONL);
    chrome.kill('SIGKILL');
    await dodo(500);
    await rm(dir, { recursive: true, force: true }).catch(() => { });
    if (!CONTROLE) {
        await dodo(8000);
        log('arrêt de l’agent :', tuerAgent().replace(/\s+/g, ' ').trim().slice(-40));
        copierLog(ETIQUETTE);
    }
}
process.exit(code);
