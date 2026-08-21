// SONDE S2 — `move()`, et la sensibilité à la casse de l'INSTRUMENT.
//
// ⚠️ **CE N'EST PAS UNE PORTE ÉLIMINATOIRE, et cet instrument ne la déguise pas
// en porte.** Les deux branches du renommage sont livrées et testées sur l'hôte
// (`client/src/fichiers/mutation.test.ts`, deux faux : l'un qui expose `move()`,
// l'autre non). **Quelle que soit la réponse, F3 fonctionne.** S2 dit ce que la
// RECETTE démontre, pas ce que le produit sait faire.
//
// Les trois questions :
//   ① `typeof handle.move === 'function'` sur un FICHIER ?
//   ② idem sur un RÉPERTOIRE, et accepte-t-elle un parent différent ?
//   ③ OPFS est-il SENSIBLE à la casse ?
//      ⚠️ C'est ce qui dit si l'instrument PEUT reproduire le défaut de casse.
//      S'il est sensible, IL NE LE PEUT PAS.
//
// ⚠️ **LA QUESTION QUI NE PEUT PAS ÊTRE POSÉE À CET INSTRUMENT** :
// `handle.name` reflète-t-il le nom STOCKÉ après une résolution insensible à la
// casse ? Elle exige un système de fichiers insensible à la casse, donc un vrai
// `showDirectoryPicker()`, que F1 a mesuré INATTEIGNABLE sur cet hôte (aucune
// commande CDP pour accepter un sélecteur, ni `DISPLAY`, ni `Xvfb`, ni
// `xdotool`). **C'est pour cela que le canonicaliseur n'exploite pas
// `handle.name` : la seule optimisation évidente repose sur un fait que ce
// montage ne peut pas établir.** Léguée, pas implémentée à moitié.
//
// ⛔ **N'EMPLOIE NI LA VM, NI LE PRODUIT.** OPFS, Chrome sans interface, un
// serveur statique sur la boucle locale. Rien de ce que cette sonde fait ne
// touche l'agent.

import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import WebSocket from 'ws';

const PAGE = `<!doctype html><meta charset="utf-8"><title>s2</title>`;

/** Une origine SÛRE est obligatoire : OPFS n'existe pas sur `about:blank`. */
function servir() {
    const serveur = createServer((_, r) => {
        r.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
        r.end(PAGE);
    });
    return new Promise((ok) => serveur.listen(0, '127.0.0.1', () => ok(serveur)));
}

async function attendreDevtools(port, limiteMs = 15000) {
    const fin = Date.now() + limiteMs;
    for (;;) {
        try {
            const r = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (r.ok) return (await r.json()).webSocketDebuggerUrl;
        } catch {
            /* pas encore là */
        }
        if (Date.now() > fin) throw new Error('CDP injoignable');
        await new Promise((r) => setTimeout(r, 200));
    }
}

const LE_SCRIPT = `(async () => {
  const releve = {};
  const racine = await navigator.storage.getDirectory();

  // ── ③ LA SENSIBILITÉ À LA CASSE — posée EN PREMIER, parce qu'elle décide
  //    de ce que tout le reste démontre.
  const a = await racine.getFileHandle('S2-Casse.txt', { create: true });
  let minuscule = null;
  try {
    const h = await racine.getFileHandle('s2-casse.txt');
    minuscule = h.name;
  } catch (e) { minuscule = 'ABSENT:' + e.name; }
  releve.casse = {
    cree: a.name,
    demande_en_minuscules: minuscule,
    opfs_sensible_a_la_casse: minuscule.startsWith('ABSENT:'),
  };

  // ── ① move() SUR UN FICHIER
  releve.move_fichier = { present: typeof a.move === 'function' };
  if (releve.move_fichier.present) {
    try {
      await a.move(racine, 'S2-Deplace.txt');
      releve.move_fichier.meme_parent = 'ok';
      releve.move_fichier.nom_apres = a.name;
    } catch (e) { releve.move_fichier.meme_parent = 'ECHEC:' + e.name + ':' + e.message; }
    const autre = await racine.getDirectoryHandle('S2-Ailleurs', { create: true });
    try {
      await a.move(autre, 'S2-Deplace.txt');
      releve.move_fichier.autre_parent = 'ok';
    } catch (e) { releve.move_fichier.autre_parent = 'ECHEC:' + e.name + ':' + e.message; }
    // 🔴 move() ÉCRASE-T-ELLE ? C'est ce qui rend la résolution préalable
    // OBLIGATOIRE, et ce n'est écrit nulle part dans la spec du produit.
    const victime = await racine.getFileHandle('S2-Victime.txt', { create: true });
    const f = await victime.createWritable();
    await f.write('AVANT'); await f.close();
    const agresseur = await racine.getFileHandle('S2-Agresseur.txt', { create: true });
    const g = await agresseur.createWritable();
    await g.write('APRES'); await g.close();
    try {
      await agresseur.move(racine, 'S2-Victime.txt');
      const relu = await (await racine.getFileHandle('S2-Victime.txt')).getFile();
      releve.move_ecrase = { issue: 'ok', contenu: await relu.text() };
    } catch (e) { releve.move_ecrase = { issue: 'ECHEC:' + e.name }; }
  }

  // ── ② move() SUR UN RÉPERTOIRE
  const d = await racine.getDirectoryHandle('S2-Dossier', { create: true });
  releve.move_repertoire = { present: typeof d.move === 'function' };
  if (releve.move_repertoire.present) {
    try {
      await d.move(racine, 'S2-Dossier-Bis');
      releve.move_repertoire.meme_parent = 'ok';
    } catch (e) { releve.move_repertoire.meme_parent = 'ECHEC:' + e.name + ':' + e.message; }
  }

  // ── Le renommage de CASSE PURE, directement.
  const c = await racine.getFileHandle('S2-Pure.txt', { create: true });
  if (typeof c.move === 'function') {
    try {
      await c.move(racine, 'S2-PURE.TXT');
      releve.casse_pure_directe = 'ok';
    } catch (e) { releve.casse_pure_directe = 'ECHEC:' + e.name + ':' + e.message; }
  } else {
    releve.casse_pure_directe = 'SANS OBJET (move absente)';
  }

  // ── removeEntry SANS recursive sur un répertoire NON VIDE.
  const plein = await racine.getDirectoryHandle('S2-Plein', { create: true });
  await plein.getFileHandle('dedans.txt', { create: true });
  try {
    await racine.removeEntry('S2-Plein');
    releve.remove_non_vide = 'ACCEPTE (le sous-arbre a disparu)';
  } catch (e) { releve.remove_non_vide = 'REFUSE:' + e.name; }

  releve.chrome = navigator.userAgent;
  return JSON.stringify(releve, null, 2);
})()`;

const serveur = await servir();
const portPage = serveur.address().port;
const profil = mkdtempSync(join(tmpdir(), 's2-'));
const portCdp = 9333;
const chrome = spawn('google-chrome', [
    '--headless=new',
    `--remote-debugging-port=${portCdp}`,
    `--user-data-dir=${profil}`,
    '--no-sandbox',
    '--disable-gpu',
    `http://127.0.0.1:${portPage}/`,
], { stdio: 'ignore' });

let code = 1;
try {
    await attendreDevtools(portCdp);
    // La cible de la page, pas celle du navigateur.
    const cibles = await (await fetch(`http://127.0.0.1:${portCdp}/json/list`)).json();
    const page = cibles.find((c) => c.type === 'page' && c.url.startsWith('http://127.0.0.1'));
    if (!page) throw new Error('aucune page servie');
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((ok, ko) => { ws.once('open', ok); ws.once('error', ko); });
    const reponse = await new Promise((ok, ko) => {
        ws.on('message', (m) => {
            const msg = JSON.parse(m.toString());
            if (msg.id === 1) ok(msg);
        });
        ws.send(JSON.stringify({
            id: 1, method: 'Runtime.evaluate',
            params: { expression: LE_SCRIPT, awaitPromise: true, returnByValue: true },
        }));
        setTimeout(() => ko(new Error('évaluation expirée')), 30000);
    });
    if (reponse.result?.exceptionDetails) {
        console.log('EXCEPTION :', JSON.stringify(reponse.result.exceptionDetails, null, 2));
    } else {
        console.log(reponse.result.result.value);
        code = 0;
    }
    ws.close();
} catch (e) {
    console.log('ÉCHEC DE LA SONDE :', e.message);
} finally {
    chrome.kill('SIGKILL');
    serveur.close();
    rmSync(profil, { recursive: true, force: true });
}
process.exit(code);
