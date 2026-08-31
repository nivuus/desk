#!/usr/bin/env node
// Lot 32Q : vise TROIS points connus de l'image, pour que la sonde de session 1
// dise ou le curseur atterrit vraiment.
//
// 🔴 POURQUOI TROIS ET PAS DEUX, ET POURQUOI CELUI-LA. Un curseur qui tombe
// juste sur deux points peut tomber juste PAR HASARD si les deux sont mal
// choisis. La regle vient du test d'hote `entrees.rs` : l'erreur a un terme
// d'ORIGINE et un terme d'ECHELLE, et ils ne se distinguent qu'A DISTANCE.
//   - A, pres du coin haut-gauche : isole le terme d'ORIGINE ;
//   - C, pres du coin bas-droite  : la somme des deux ;
//   - B, au centre                : departage (une erreur affine passe par B).
//
// Il DISPATCHE un `pointermove` sur l'element video : c'est exactement ce que
// le client ecoute (`client/src/input.ts:171`), donc le chemin mesure est
// celui du produit, jamais un raccourci.
//
// ⚠️ Le client normalise sur `contentRect(video)` — l'image BANDES NOIRES
// EXCLUES, calculee depuis videoWidth/videoHeight — et non sur le rectangle
// brut de l'element. Le pilote vise donc dans CE rectangle-la, sans quoi il
// mesurerait sa propre erreur de cadrage.
//
// Usage : node pilote-curseur.mjs --etiquette=avant
import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { attendreDevtools } from '../../../../../client/recette/devtools.mjs';

const arg = (n, d) =>
    (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const PLATEFORME = process.env.PLATEFORME ?? 'http://192.168.3.1:3445';
const COURRIEL = process.env.COURRIEL ?? 'maxime.g.allanic@gmail.com';
const ETIQUETTE = arg('etiquette', 'avant');
const SORTIE = arg('sortie', `/var/tmp/curseur-${ETIQUETTE}.json`);
const PORT = 9336;
const dormir = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString(), ...a);

/// Les trois fractions visees, et l'instant (s) ou chacune est envoyee.
const POINTS = [
    { nom: 'A_coin_haut_gauche', fx: 0.02, fy: 0.02, a_t: 6 },
    { nom: 'B_centre', fx: 0.5, fy: 0.5, a_t: 16 },
    { nom: 'C_coin_bas_droite', fx: 0.98, fy: 0.98, a_t: 26 },
];

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.n = 1;
        this.attente = new Map();
        this.pret = new Promise((r) => this.ws.addEventListener('open', () => r(), { once: true }));
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.attente.has(m.id)) {
                const { resolve, reject } = this.attente.get(m.id);
                this.attente.delete(m.id);
                m.error ? reject(new Error(JSON.stringify(m.error))) : resolve(m.result);
            }
        });
    }
    async envoyer(method, params = {}, sessionId) {
        await this.pret;
        const id = this.n++;
        return new Promise((resolve, reject) => {
            this.attente.set(id, { resolve, reject });
            this.ws.send(JSON.stringify(sessionId ? { id, method, params, sessionId } : { id, method, params }));
        });
    }
    async evaluer(expr, sessionId) {
        const r = await this.envoyer('Runtime.evaluate',
            { expression: expr, awaitPromise: true, returnByValue: true }, sessionId);
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
        return r.result.value;
    }
}
const cibles = async () => await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();

async function jeton() {
    const r = await fetch(`${PLATEFORME}/auth/moi`, { headers: { 'X-Pomerium-Claim-Email': COURRIEL } });
    const c = await r.json();
    if (!r.ok) throw new Error(`/auth/moi a refuse : ${r.status}`);
    return c.acces ?? c.jeton;
}
/// 🔴 LANCE UNE APPLICATION PAR DESK LUI-MEME, ET IL LE FAUT.
/// Depuis la regle d'appartenance (lot 32I), desk n'adopte que les fenetres
/// des applications qu'il a lancees : apres un redemarrage de l'agent, AUCUNE
/// fenetre preexistante n'est adoptee, et la shell n'en recoit donc aucune.
/// Le pilote doit donc en ouvrir une lui-meme.
/// ⚠️ APRES que la shell est connectee : une fenetre ouverte plus de 30 s
/// avant la connexion du navigateur est perdue et jamais reproposee (legs du
/// lot 10D).
async function lancerUneApplication(j, vmId, motif) {
    // ⚠️ `?vm=` est OBLIGATOIRE : sans lui la route rend 400 { refus: 'vm-absente' }.
    const r = await fetch(`${PLATEFORME}/applications?vm=${encodeURIComponent(vmId)}`, {
        headers: { Authorization: `Bearer ${j}` },
    });
    const c = await r.json();
    const liste = c.applications ?? c;
    if (!Array.isArray(liste)) throw new Error(`/applications a rendu ${JSON.stringify(c).slice(0, 200)}`);
    const choisie = liste.find((a) => new RegExp(motif, 'i').test(a.nom ?? a.cle ?? ''));
    if (!choisie) {
        throw new Error(
            `aucune application ne correspond a /${motif}/ parmi ${liste.length} : ` +
                liste.slice(0, 25).map((a) => a.nom).join(' | '),
        );
    }
    const l = await fetch(`${PLATEFORME}/application/${choisie.id}/lancer`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${j}`, 'Content-Type': 'application/json' },
    });
    return { nom: choisie.nom ?? choisie.cle, id: choisie.id, statut: l.status, reponse: await l.text() };
}

async function laVm(j) {
    const v = await (await fetch(`${PLATEFORME}/vm`, { headers: { Authorization: `Bearer ${j}` } })).json();
    return (v.vms ?? v)[0];
}
async function prefixe(j, vm) {
    const r = await fetch(`${PLATEFORME}/session`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${j}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ vm: vm.id }),
    });
    const c = await r.json();
    return c.prefixe ?? c.session?.split(':')[0];
}

/// Vise une fraction de `contentRect(video)` — le MEME rectangle que celui sur
/// lequel le client normalise.
const VISER = (fx, fy) => `(() => {
  const v = document.querySelector('video');
  if (!v) return { echec: 'aucun <video>' };
  const e = v.getBoundingClientRect();
  const sw = v.videoWidth, sh = v.videoHeight;
  let r = e;
  if (sw && sh) {
    const s = Math.min(e.width / sw, e.height / sh);
    r = new DOMRect(e.x + (e.width - s * sw) / 2, e.y + (e.height - s * sh) / 2, s * sw, s * sh);
  }
  const cx = r.x + ${fx} * r.width, cy = r.y + ${fy} * r.height;
  v.dispatchEvent(new PointerEvent('pointermove', {
    clientX: cx, clientY: cy, bubbles: true, pointerId: 1, pointerType: 'mouse' }));
  return { cx: Math.round(cx), cy: Math.round(cy),
           image: { x: Math.round(r.x), y: Math.round(r.y),
                    l: Math.round(r.width), h: Math.round(r.height) },
           source: { l: sw, h: sh } };
})()`;

const releve = { etiquette: ETIQUETTE, instant: new Date().toISOString(), points: [] };
const profil = await mkdtemp(join(tmpdir(), 'lot32q-'));
const chrome = spawn('google-chrome', [
    '--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profil}`,
    '--no-sandbox', '--no-first-run', '--disable-popup-blocking',
    '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
    '--disable-renderer-backgrounding', 'about:blank',
], { stdio: 'ignore' });

try {
    const j = await jeton();
    const vm = await laVm(j);
    releve.vm = vm?.id;
    const p = await prefixe(j, vm);
    releve.prefixe = p;
    await attendreDevtools(PORT);
    const page = (await cibles()).find((c) => c.type === 'page');
    const shell = new Cdp(page.webSocketDebuggerUrl);
    await shell.envoyer('Page.enable');
    await shell.envoyer('Runtime.enable');
    await shell.envoyer('Page.navigate', { url: `${PLATEFORME}/` });
    await dormir(2500);
    await shell.evaluer(
        `localStorage.setItem('guac.jeton.acces', ${JSON.stringify(j)});` +
        `localStorage.setItem('guac.prefixe', ${JSON.stringify(p)});'ok'`);
    await shell.envoyer('Page.navigate', { url: `${PLATEFORME}/shell.html` });
    // La shell doit etre CONNECTEE avant le lancement : voir la note de
    // `lancerUneApplication`.
    await dormir(4000);
    releve.lancement = await lancerUneApplication(j, vm.id, process.env.APP ?? 'bloc-?notes|notepad');
    log(`application lancee : ${JSON.stringify(releve.lancement)}`);

    let session = null, url = null;
    releve.cibles_vues = [];
    for (let i = 0; i < 60 && !session; i += 1) {
        await dormir(1000);
        const vues = await cibles();
        // 🔴 ON JOURNALISE CE QU'ON VOIT. Une boucle qui ne trouve rien et ne
        // dit pas ce qu'elle a vu rend un echec INDISCERNABLE d'un produit en
        // panne -- c'est ce qui a coute deux creneaux au lot 32T.
        const empreinte = vues.map((c) => `${c.type}:${c.url}`).sort().join(' ~ ');
        if (releve.cibles_vues[releve.cibles_vues.length - 1] !== empreinte) {
            releve.cibles_vues.push(empreinte);
            log(`cibles (t=${i}s) : ${empreinte}`);
        }
        for (const c of vues) {
            if (c.type === 'page' && /session=/.test(c.url)) {
                session = new Cdp(c.webSocketDebuggerUrl);
                await session.envoyer('Runtime.enable');
                url = c.url;
            }
        }
    }
    if (!session) {
        // 🔴 ON DIT CE QUE LA SHELL MONTRAIT. Sans cela, << aucune page >> est
        // indiscernable de << la shell n a rien recu >>, de << le popup a ete
        // bloque >> et de << il faut cliquer >>.
        releve.shell = await shell.evaluer(`(() => ({
            texte: document.body.innerText.slice(0, 1200),
            boutons: [...document.querySelectorAll('button,a,[role=button]')].map((e) => e.textContent.trim()).slice(0, 30),
            popup: (() => { try { const w = window.open('about:blank', 'sonde-popup');
                                  if (w) { w.close(); return 'autorise'; } return 'BLOQUE'; }
                            catch (e) { return 'exception ' + e.message; } })(),
        }))()`);
        log('etat de la shell : ' + JSON.stringify(releve.shell));
        throw new Error("aucune page de session : voir releve.shell");
    }
    log(`page de session : ${url}`);
    releve.url_session = url;

    const depart = Date.now();
    for (const pt of POINTS) {
        const attendre = pt.a_t * 1000 - (Date.now() - depart);
        if (attendre > 0) await dormir(attendre);
        const vu = await session.evaluer(VISER(pt.fx, pt.fy));
        const ligne = { ...pt, t_ms: Date.now() - depart, ...vu };
        releve.points.push(ligne);
        log(`vise ${pt.nom} : ${JSON.stringify(ligne)}`);
    }
    await dormir(3000);
} catch (e) {
    releve.erreur = String(e && e.message ? e.message : e);
    log('ERREUR ' + releve.erreur);
} finally {
    chrome.kill('SIGKILL');
    await writeFile(SORTIE, JSON.stringify(releve, null, 1));
    log(`releve ecrit : ${SORTIE}`);
    await rm(profil, { recursive: true, force: true }).catch(() => {});
}
