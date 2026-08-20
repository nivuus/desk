#!/usr/bin/env node
// Sous-bloc P1 (presse-papier) — LE TÉMOIN DE MESURABILITÉ, étape 0 de la
// tâche 16, divergence E11 du plan.
//
// Question, et une seule : **ce montage peut-il lire ce que
// `navigator.clipboard.writeText` a écrit ?** Si oui, le critère ① se juge au
// NIVEAU 2 — le vrai, celui où l'on vérifie qu'un humain pourrait coller.
// Sinon, le niveau 2 est NON MESURABLE sur ce montage, et le relevé l'écrit :
// ce n'est pas un échec du produit, c'est une mesure non prise.
//
// 🔴 **CE TÉMOIN PEUT ÉCHOUER, ET C'EST CE QUI EN FAIT UN TÉMOIN.** Il ne
// touche NI la VM, NI l'agent, NI le produit : uniquement un Chromium sans
// interface sur l'hôte, et le presse-papier X11 de l'hôte. Un échec ici ne dit
// rien du produit, et le relevé doit le dire aussi.
//
// Il relève TROIS choses, pas une :
//   1. `writeText` a-t-elle RÉSOLU (et non levé) — c'est le niveau 1 ;
//   2. la page peut-elle se relire elle-même (`readText`) — un presse-papier
//      interne au navigateur suffirait, ce n'est PAS le niveau 2 ;
//   3. l'HÔTE lit-il le nonce (`xclip`, `wl-paste`, `xsel`) — LE NIVEAU 2.
//
// Les trois sont rapportés séparément : confondre 2 et 3 ferait conclure au
// niveau 2 sur un presse-papier qui ne sort jamais du processus Chromium.

import { spawnSync } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-multifenetres-d11/instrument/commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const URL_PAGE = arg('url', 'http://127.0.0.1:5173/');
const SORTIE = arg('sortie', 'temoin-mesurabilite.json');
const NONCE = `temoin-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
const log = (...a) => console.log(new Date().toISOString(), ...a);

/** Tente de lire le presse-papier de l'HÔTE par les trois outils usuels. */
function lireHote() {
    const essais = [
        ['xclip', ['-o', '-selection', 'clipboard']],
        ['wl-paste', ['--no-newline']],
        ['xsel', ['-b', '-o']],
    ];
    const resultats = [];
    for (const [outil, args] of essais) {
        const quel = spawnSync('sh', ['-c', `command -v ${outil}`], { encoding: 'utf8' });
        if (quel.status !== 0) { resultats.push({ outil, present: false }); continue; }
        const r = spawnSync(outil, args, { encoding: 'utf8', timeout: 8000 });
        resultats.push({
            outil, present: true, code: r.status,
            stdout: (r.stdout ?? '').slice(0, 200),
            stderr: (r.stderr ?? '').slice(0, 200),
        });
    }
    return resultats;
}

const dir = await mkdtemp(join(tmpdir(), 'temoin-pp-'));
const port = 9411;
const chrome = lancerChrome(port, dir);
const releve = {
    nonce: NONCE, url: URL_PAGE,
    display: process.env.DISPLAY ?? null,
    wayland: process.env.WAYLAND_DISPLAY ?? null,
};
try {
    const cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
    let sid = null;
    cdp.on(async (m) => {
        if (m.method !== 'Target.attachedToTarget') return;
        if (m.params.targetInfo.type !== 'page') {
            await cdp.send('Runtime.runIfWaitingForDebugger', {}, m.params.sessionId).catch(() => { });
            return;
        }
        sid = m.params.sessionId;
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
        await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await cdp.send('Target.setDiscoverTargets', { discover: true });
    // La permission est accordée EXPLICITEMENT : sans elle un refus de
    // permission serait indiscernable d'un presse-papier qui ne sort pas.
    await cdp.send('Browser.grantPermissions',
        { origin: new URL(URL_PAGE).origin, permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'] })
        .catch((e) => { releve.grant_erreur = String(e).slice(0, 200); });
    await cdp.send('Target.createTarget', { url: URL_PAGE });
    for (let i = 0; i < 40 && !sid; i += 1) await dodo(250);
    if (!sid) throw new Error('aucune page attachée');
    await dodo(1500);
    releve.avant_hote = lireHote();
    releve.ecriture = await cdp.evalBorne(sid, `(async () => {
      try {
        await navigator.clipboard.writeText(${JSON.stringify(NONCE)});
        return { resolue: true };
      } catch (e) { return { resolue: false, erreur: String(e).slice(0, 200) }; }
    })()`, 10000, true);
    log('writeText :', JSON.stringify(releve.ecriture));
    releve.relecture_page = await cdp.evalBorne(sid, `(async () => {
      try { return { lu: await navigator.clipboard.readText() }; }
      catch (e) { return { erreur: String(e).slice(0, 200) }; }
    })()`, 10000, true);
    log('readText (page) :', JSON.stringify(releve.relecture_page));
    await dodo(1000);
    releve.apres_hote = lireHote();
    log('hôte :', JSON.stringify(releve.apres_hote));
} catch (e) {
    releve.erreur = String(e).slice(0, 400);
    log('!! erreur', releve.erreur);
} finally {
    // Le verdict, écrit ici et pas laissé à l'interprétation.
    const trouve = (releve.apres_hote ?? []).some((r) => r.present && (r.stdout ?? '').includes(NONCE));
    releve.niveau_2_mesurable = trouve;
    releve.verdict = trouve
        ? 'NIVEAU 2 MESURABLE — le nonce écrit par writeText est relu par l\'hôte'
        : 'NIVEAU 2 NON MESURABLE sur ce montage — le nonce ne revient pas à l\'hôte';
    log('VERDICT :', releve.verdict);
    await writeFile(SORTIE, JSON.stringify(releve, null, 2));
    log('relevé écrit dans', SORTIE);
    chrome.kill('SIGKILL');
    await dodo(300);
    await rm(dir, { recursive: true, force: true });
}
