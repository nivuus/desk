#!/usr/bin/env node
// Lot 3, item 1 (3.7) — « TEMPORAIRE + RENOMMAGE » SUR UN ÉDITEUR RÉEL,
// DANS LA RACINE ProjFS.
//
// 🔴 LE SEUL ITEM DU LOT DONT L'ÉCHEC PERDRAIT DES DONNÉES DE L'UTILISATEUR.
//
// Ce que ce pilote établit, et rien de plus : monter le pont avec une racine
// locale RÉELLE (OPFS), faire enregistrer un éditeur RÉEL de l'invité DANS la
// racine ProjFS, et relire le poste local pour dire si la sauvegarde est
// arrivée ENTIÈRE et si un résidu a été laissé.
//
// ⚠️ CE QU'IL N'ÉTABLIT PAS : ce que ferait VS Code, Word ou LibreOffice.
// Aucun des trois n'est installé sur cette appliance (relevé, voir le verdict).
// La sonde d'idiome du 21 août 2026 a déjà tranché la moitié NTFS de la
// question pour cinq outils ; celui-ci ajoute la racine ProjFS, et l'ajoute
// pour le SEUL éditeur présent.
//
// Usage :
//   node pilote-item1.mjs --etiquette=1 --sortie=…/sequence-1.json
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { attendreDevtools } from '../../../../../client/recette/devtools.mjs';

const ICI = dirname(fileURLToPath(import.meta.url));
const arg = (n, d) =>
    (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const PLATEFORME = process.env.PLATEFORME ?? 'http://192.168.3.1:3445';
const COURRIEL = process.env.COURRIEL ?? 'maxime.g.allanic@gmail.com';
const ETIQUETTE = arg('etiquette', '1');
const SORTIE = arg('sortie', `/var/tmp/item1-${ETIQUETTE}.json`);
const MAINTIEN_S = Number(arg('maintien', '150'));
const PORT = 9351;
// 🔴 CE PILOTE EST PARTAGÉ PAR LES ITEMS DU PONT, ET IL SE RÉUTILISE PAR
// PARAMÈTRE — jamais par copie : « une copie éprouverait la copie, pas
// l'instrument » (F3). L'item 1 garde les défauts ci-dessous, donc son
// invocation d'origine est inchangée ; l'item 3 passe les siens.
const INJECTION_NOM = arg('injection', 'injection-item1.js');
const PREPARER = arg('prepare', '__item1Preparer');
const RELIRE = arg('relire', '__item1Relire');

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString(), ...a);

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.n = 1;
        this.attente = new Map();
        this.evenements = [];
        this.pret = new Promise((r) => this.ws.addEventListener('open', () => r(), { once: true }));
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.attente.has(m.id)) {
                const { resolve, reject } = this.attente.get(m.id);
                this.attente.delete(m.id);
                m.error ? reject(new Error(JSON.stringify(m.error))) : resolve(m.result);
                return;
            }
            if (m.method === 'Target.attachedToTarget' && this.equiper) this.equiper(m.params).catch(() => {});
        });
    }
    async envoyer(method, params = {}, sessionId) {
        await this.pret;
        const id = this.n++;
        return new Promise((resolve, reject) => {
            // ⚠️ BORNÉE : une évaluation qui ne rend jamais fige le pilote au
            // milieu de la mesure, et le relevé serait perdu.
            const t = setTimeout(() => {
                this.attente.delete(id);
                reject(new Error(`CDP ${method} sans réponse après 25 s`));
            }, 25000);
            this.attente.set(id, {
                resolve: (v) => { clearTimeout(t); resolve(v); },
                reject: (e) => { clearTimeout(t); reject(e); },
            });
            this.ws.send(JSON.stringify(sessionId ? { id, method, params, sessionId } : { id, method, params }));
        });
    }
    async evaluer(expr, sessionId) {
        const r = await this.envoyer('Runtime.evaluate',
            { expression: expr, awaitPromise: true, returnByValue: true }, sessionId);
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.text ?? JSON.stringify(r.exceptionDetails));
        return r.result.value;
    }
}

const cibles = async () => await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();

async function jeton() {
    const r = await fetch(`${PLATEFORME}/auth/moi`, { headers: { 'X-Pomerium-Claim-Email': COURRIEL } });
    const c = await r.json();
    if (!r.ok) throw new Error(`/auth/moi a refusé : ${r.status}`);
    return c.acces ?? c.jeton;
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

const releve = { etiquette: ETIQUETTE, instant: new Date().toISOString(), etapes: [] };
const noter = (m, d) => { releve.etapes.push({ t: new Date().toISOString(), m, ...d }); log(m, d ? JSON.stringify(d).slice(0, 300) : ''); };

const INJECTION = await readFile(join(ICI, INJECTION_NOM), 'utf8');
const profil = await mkdtemp(join(tmpdir(), 'lot3-item1-'));
const chrome = spawn('google-chrome', [
    '--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profil}`,
    '--no-sandbox', '--no-first-run', '--disable-popup-blocking',
    '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
    '--disable-renderer-backgrounding',
    // 🔴 SANS CE DRAPEAU, `navigator.storage` EST `undefined` ET RIEN NE LE DIT
    // AUTREMENT QUE PAR « Cannot read properties of undefined ». La plateforme
    // est servie en HTTP CLAIR sur 192.168.3.1:3445 ; OPFS
    // (`navigator.storage.getDirectory`) exige un CONTEXTE SÛR, et
    // 192.168.3.1 n'est pas `localhost`. Mesuré le 5 septembre 2026.
    // ⚠️ C'EST UNE SUBSTITUTION D'INSTRUMENT, ET ELLE EST DÉCLARÉE : en usage
    // réel la page vit derrière Pomerium, donc en HTTPS, donc en contexte sûr
    // sans aucun drapeau. Ce drapeau rétablit la condition du produit réel, il
    // ne la contourne pas — mais il n'ÉPROUVE pas non plus qu'elle tienne.
    `--unsafely-treat-insecure-origin-as-secure=${PLATEFORME}`,
    'about:blank',
], { stdio: 'ignore' });

try {
    await attendreDevtools(PORT);
    // 🔴 L'INJECTION SE POSE AVANT LE PREMIER SCRIPT DE CHAQUE CIBLE.
    // `Page.addScriptToEvaluateOnNewDocument` seul ne court pas sur une page
    // ouverte par `window.open` : c'est `Target.setAutoAttach` avec
    // `waitForDebuggerOnStart`, au niveau NAVIGATEUR, qui l'attrape (lot 32C).
    const v = await (await fetch(`http://127.0.0.1:${PORT}/json/version`)).json();
    const nav = new Cdp(v.webSocketDebuggerUrl);
    nav.equiper = async ({ sessionId, targetInfo }) => {
        try {
            if (targetInfo.type === 'page') {
                await nav.envoyer('Page.enable', {}, sessionId);
                await nav.envoyer('Page.addScriptToEvaluateOnNewDocument', { source: INJECTION }, sessionId);
            }
        } finally {
            await nav.envoyer('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => {});
        }
    };
    await nav.envoyer('Target.setAutoAttach',
        { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    noter('injection armée au niveau navigateur');

    const j = await jeton();
    const vm = await laVm(j);
    const p = await prefixe(j, vm);
    releve.vm = vm?.id; releve.prefixe = p;

    const page = (await cibles()).find((c) => c.type === 'page');
    const hub = new Cdp(page.webSocketDebuggerUrl);
    await hub.envoyer('Page.enable');
    await hub.envoyer('Runtime.enable');
    await hub.envoyer('Page.navigate', { url: `${PLATEFORME}/` });
    await dormir(2500);
    await hub.evaluer(
        `localStorage.setItem('guac.jeton.acces', ${JSON.stringify(j)});`
        + `localStorage.setItem('guac.prefixe', ${JSON.stringify(p)});'ok'`);
    await hub.envoyer('Page.navigate', { url: `${PLATEFORME}/` });
    await dormir(4000);

    releve.injection_vue = await hub.evaluer(`({presente: typeof window.${PREPARER} === "function", chemin: location.pathname, etapes: (window.__item1||{}).etapes})`);
    noter('injection vue dans le hub', releve.injection_vue);

    releve.opfs_prepare = await hub.evaluer(`window.${PREPARER}()`);
    noter('racine locale préparée (OPFS)', releve.opfs_prepare);

    // 🔴 LE CLIC EST LE CHEMIN DU PRODUIT : `installerLePont` n'appelle
    // `choisirDossier()` que depuis ce gestionnaire, jamais depuis un message
    // de canal (l'activation utilisateur transitoire l'exige).
    releve.clic = await hub.evaluer(`(() => {
      const b = document.getElementById('choisir-dossier');
      if (!b) return { echec: 'bouton choisir-dossier absent', corps: document.body.innerText.slice(0,400) };
      b.click();
      return { clique: true, texte: b.textContent.trim() };
    })()`);
    noter('clic sur « Choisir mon dossier »', releve.clic);

    // Attendre que le pont dise, côté navigateur, qu'il est monté.
    for (let i = 0; i < 45; i += 1) {
        await dormir(1000);
        const etat = await hub.evaluer(`(() => {
          const e = document.getElementById('etat-fichiers');
          const s = document.getElementById('section-fichiers');
          return { etat: e ? e.textContent.trim().slice(0,300) : null, ouvert: s ? s.open : null };
        })()`);
        if (etat.etat && etat.etat.length) {
            releve.etat_pont = etat;
            if (/mont|prêt|pret|connect/i.test(etat.etat)) { noter('pont monté côté navigateur', etat); break; }
        }
        if (i === 44) noter('le bandeau du pont n a jamais rien dit', etat);
    }

    noter(`maintien de ${MAINTIEN_S} s — l'observateur de l'invité travaille pendant ce temps`);
    releve.relecture_avant_editeur = await hub.evaluer(`window.${RELIRE}()`);
    noter('relecture AVANT que l éditeur touche quoi que ce soit', releve.relecture_avant_editeur);

    await writeFile(`${SORTIE}.partiel`, JSON.stringify(releve, null, 2));
    await dormir(MAINTIEN_S * 1000);

    // 🔴 LE CRITÈRE. Ce que le POSTE LOCAL a reçu — pas ce que l'invité croit
    // avoir écrit, et pas ce que le journal de l'agent a notifié.
    releve.relecture_apres_editeur = await hub.evaluer(`window.${RELIRE}()`);
    noter('relecture APRÈS l éditeur', releve.relecture_apres_editeur);
    releve.etapes_page = await hub.evaluer('(window.__item1||{}).etapes');
} catch (e) {
    releve.erreur = String(e && e.message ? e.message : e);
    log('ERREUR : ' + releve.erreur);
} finally {
    chrome.kill('SIGKILL');
    // 🔴 LE PROFIL CHROME PART AVEC LE PILOTE. Chaque exécution en laissait un
    // de ~100 Mio sous `tmpdir()` ; vingt-trois d'entre eux ont REMPLI le
    // tmpfs de 10 Gio de cette machine le 5 septembre 2026, et le symptôme
    // n'était pas « disque plein » mais **une commande qui ne rend RIEN** —
    // l'enveloppe du shell n'arrivant plus à écrire la sortie de l'enfant.
    await rm(profil, { recursive: true, force: true }).catch(() => {});
    await writeFile(SORTIE, JSON.stringify(releve, null, 2));
    log(`relevé écrit : ${SORTIE}`);
}
