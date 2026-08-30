#!/usr/bin/env node
// Pilote du JUGE, lot 32C : une image de la VM arrive-t-elle au navigateur ?
//
// DERIVE de `journaux-lot31/instrument/pilote-lot31.mjs` — meme classe CDP,
// meme RELEVE, meme juge. UNE SEULE difference, et elle est la raison d'etre
// de ce fichier : le lot 31 s'authentifiait PAR LE FORMULAIRE, ce qui exige
// une plateforme en `motdepasse`. Celui-ci vise la plateforme de PRODUCTION,
// qui est en `pomerium` — il obtient donc le jeton par
// `X-Pomerium-Claim-Email`, exactement comme `journaux-lot22/instrument/
// pilote-parcours.mjs`, et le SEME dans le stockage du navigateur.
//
// LE JUGE EST `framesDecoded`, jamais un compteur d'octets : ce depot a
// mesure que `bytesReceived` croit sur un flux vide. `bytesVideo_delta` est
// releve A COTE, comme TEMOIN, jamais comme verdict.
//
// LA MIRE DOIT BOUGER : Desktop Duplication n'emet qu'au CHANGEMENT du
// bureau. `mire.ps1` anime a 10 Hz et PEINT sa cadence dans l'image.
//
// Il doit tourner SUR l'hote de la plateforme : `X-Pomerium-Claim-Email`
// n'est cru que d'un pair liste dans `PLATEFORME_PROXY_DE_CONFIANCE`.
// Le jeton obtenu n'est JAMAIS journalise.
import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { attendreDevtools } from '../../../../../client/recette/devtools.mjs';

const arg = (n, d) =>
    (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const PLATEFORME = process.env.PLATEFORME ?? 'http://192.168.3.1:3445';
const COURRIEL = process.env.COURRIEL ?? 'maxime.g.allanic@gmail.com';
const PALIER = Number(arg('palier', '20000'));
const ETIQUETTE = arg('etiquette', '1');
const SORTIE = arg('sortie', `/var/tmp/juge-${ETIQUETTE}.json`);
const PORT = 9334;
const dormir = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString(), ...a);

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.n = 1;
        this.attente = new Map();
        this.pret = new Promise((r) => this.ws.addEventListener('open', () => r(), { once: true }));
        this.surEvenement = null;
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.attente.has(m.id)) {
                const { resolve, reject } = this.attente.get(m.id);
                this.attente.delete(m.id);
                m.error ? reject(new Error(JSON.stringify(m.error))) : resolve(m.result);
            } else if (m.method && this.surEvenement) {
                this.surEvenement(m);
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
        const r = await this.envoyer('Runtime.evaluate', {
            expression: expr, awaitPromise: true, returnByValue: true,
        }, sessionId);
        if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
        return r.result.value;
    }
    fermer() { try { this.ws.close(); } catch {} }
}

async function cibles() {
    return await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();
}

/// Le releve `inbound-rtp` video : ce qui juge, et ce qui NE juge pas.
const RELEVE = `(async () => {
  const p = (window.__pcs || [])[0];
  if (!p) return null;
  const s = await p.getStats();
  let v = null, a = null;
  s.forEach((r) => {
    if (r.type === 'inbound-rtp' && r.kind === 'video') v = r;
    if (r.type === 'inbound-rtp' && r.kind === 'audio') a = r;
  });
  return {
    ice: p.iceConnectionState,
    framesDecoded: v ? (v.framesDecoded ?? 0) : null,
    framesReceived: v ? (v.framesReceived ?? 0) : null,
    bytesVideo: v ? (v.bytesReceived ?? 0) : null,
    bytesAudio: a ? (a.bytesReceived ?? 0) : null,
    t: Date.now(),
  };
})()`;

async function jeton() {
    const r = await fetch(`${PLATEFORME}/auth/moi`, {
        headers: { 'X-Pomerium-Claim-Email': COURRIEL },
    });
    const c = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`/auth/moi a refuse : ${r.status} ${JSON.stringify(c)}`);
    const j = c.acces ?? c.jeton;
    if (!j) throw new Error("/auth/moi n'a rendu aucun jeton");
    return j;
}

async function prefixe(j) {
    const v = await (await fetch(`${PLATEFORME}/vm`, { headers: { Authorization: `Bearer ${j}` } })).json();
    const vm = (v.vms ?? v)[0];
    if (!vm) throw new Error('aucune VM attribuee a ce compte');
    const r = await fetch(`${PLATEFORME}/session`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${j}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ vm: vm.id }),
    });
    const c = await r.json();
    if (!r.ok) throw new Error(`POST /session a refuse : ${r.status} ${JSON.stringify(c)}`);
    return { prefixe: c.prefixe ?? c.session?.split(':')[0], vm: vm.id, etat: c.etat };
}

const releve = { etiquette: ETIQUETTE, instant: new Date().toISOString(), palier_ms: PALIER };
const profil = await mkdtemp(join(tmpdir(), 'lot32c-'));
const chrome = spawn('google-chrome', [
    '--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profil}`,
    '--no-sandbox', '--no-first-run',
    // Sans ces trois-la, Chrome gele une page jamais mise au premier plan.
    '--disable-background-timer-throttling',
    '--disable-backgrounding-occluded-windows',
    '--disable-renderer-backgrounding',
    // La shell ouvre chaque fenetre par `window.open`, SANS geste humain : le
    // bloqueur de pop-ups l'annule en silence, et le symptome se lit « la
    // shell n'a recu aucune fenetre ».
    '--disable-popup-blocking',
    'about:blank',
], { stdio: 'ignore' });

try {
    const j = await jeton();
    const p = await prefixe(j);
    releve.vm = p.vm; releve.etat_vm = p.etat; releve.prefixe = p.prefixe;
    log(`VM ${p.vm} etat=${p.etat} prefixe=${p.prefixe}`);

    await attendreDevtools(PORT);

    // 🔴 POSER L'AMORCE AVANT QUE LA PAGE NE S'EXECUTE, SANS RIEN FERMER.
    // `Page.addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page
    // ouverte par `window.open` — mais `Target.setAutoAttach` avec
    // `waitForDebuggerOnStart` fait PAUSER chaque cible neuve avant son
    // premier script : on y pose le crochet, puis on la relache.
    //
    // ⚠️ La premiere version FERMAIT la pop-up et rouvrait l'URL. C'etait
    // faux : l'agent voit alors partir son pair, demonte la session, et la
    // page rouverte reste a `ice=new` — mesure, pas supposition.
    const vers = await (await fetch(`http://127.0.0.1:${PORT}/json/version`)).json();
    const nav = new Cdp(vers.webSocketDebuggerUrl);
    const AMORCE = `(() => { const O = window.RTCPeerConnection; window.__pcs = [];
        window.RTCPeerConnection = function (...a) { const p = new O(...a); window.__pcs.push(p); return p; };
        window.RTCPeerConnection.prototype = O.prototype; })();`;
    const sessionsParCible = new Map();
    nav.surEvenement = async (m) => {
        if (m.method !== 'Target.attachedToTarget') return;
        const { sessionId, targetInfo } = m.params;
        sessionsParCible.set(targetInfo.targetId, { sessionId, url: targetInfo.url });
        try {
            await nav.envoyer('Page.addScriptToEvaluateOnNewDocument', { source: AMORCE }, sessionId);
            await nav.envoyer('Runtime.enable', {}, sessionId);
        } catch {}
        await nav.envoyer('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => {});
    };
    await nav.envoyer('Target.setAutoAttach', {
        autoAttach: true, waitForDebuggerOnStart: true, flatten: true,
    });

    const page = (await cibles()).find((c) => c.type === 'page');
    const shell = new Cdp(page.webSocketDebuggerUrl);
    await shell.envoyer('Page.enable');
    await shell.envoyer('Runtime.enable');

    // Il faut etre SUR l'origine pour ecrire son localStorage : `about:blank`
    // n'en a pas. On y va d'abord, on seme, puis on ouvre la shell.
    await shell.envoyer('Page.navigate', { url: `${PLATEFORME}/` });
    await dormir(2500);
    await shell.evaluer(
        `localStorage.setItem('guac.jeton.acces', ${JSON.stringify(j)});` +
        `localStorage.setItem('guac.prefixe', ${JSON.stringify(p.prefixe)});'ok'`);
    await shell.envoyer('Page.navigate', { url: `${PLATEFORME}/shell.html` });
    await dormir(4000);
    log(`page courante : ${await shell.evaluer('document.location.href')}`);
    log(`prefixe relu  : ${await shell.evaluer("localStorage.getItem('guac.prefixe')")}`);

    // La page de session est ouverte par la shell : on la CHERCHE parmi les
    // cibles — `Page.addScriptToEvaluateOnNewDocument` NE COURT PAS sur une
    // page ouverte par `window.open`, ce depot l'a mesure.
    // 🔴 `window.__pc` N'EXISTE PAS. Le pilote du lot 31 s'appuyait dessus ;
    // ni `client/src/` ni le bundle servi par la production ne le posent
    // (verifie par grep sur les deux). Une detection fondee dessus rend
    // « aucune page de session » alors que les pages SONT la — c'est ce que
    // le dump des cibles CDP a montre.
    //
    // On pose donc NOTRE crochet. Mais `Page.addScriptToEvaluateOnNewDocument`
    // NE COURT PAS sur une page ouverte par `window.open` (mesure de ce
    // depot) : on ferme donc la pop-up de la shell et on rouvre la MEME URL
    // dans une cible qu'on cree, ou l'amorce court. Fermer d'abord est
    // necessaire : le role `client` est EXCLUSIF par session.
    let urlSession = null;
    for (let i = 0; i < 90 && !urlSession; i += 1) {
        await dormir(1000);
        for (const c of await cibles()) {
            if (c.type === 'page' && /index\.html\?session=/.test(c.url)) { urlSession = c.url; break; }
        }
    }
    if (!urlSession) {
        // Ne JAMAIS conclure d'un silence sans avoir verifie qu'il y aurait eu
        // du bruit (lecon du lot 31). On demande a la shell ce qu'elle a vu,
        // et on dumpe la liste BRUTE des cibles.
        const etat = await shell.evaluer(`(() => ({
            url: document.location.href,
            cartes: document.querySelectorAll('#fenetres > *').length,
            corps: document.body.innerText.slice(0, 400),
        }))()`).catch((e) => ({ echec: String(e) }));
        releve.diagnostic_shell = etat;
        releve.cibles_cdp = (await cibles()).map((c) => `${c.type} ${c.url}`);
        log('DIAGNOSTIC shell : ' + JSON.stringify(etat));
        log('CIBLES CDP : ' + JSON.stringify(releve.cibles_cdp));
        throw new Error("aucune page de session n'est apparue : la shell a-t-elle recu une fenetre ?");
    }
    log(`page de session reperee : ${urlSession}`);
    releve.url_session = urlSession;

    // ⚠️ L'URL portee par `Target.attachedToTarget` est celle d'AVANT la
    // navigation (`about:blank`) : l'apparier a l'URL de session ne peut pas
    // marcher, et la premiere version echouait la-dessus. On DEMANDE son
    // adresse a chaque session attachee, au lieu de la supposer.
    let sid = null;
    for (let k = 0; k < 60 && !sid; k += 1) {
        for (const [, v] of sessionsParCible) {
            const ici = await nav.evaluer('document.location.href', v.sessionId).catch(() => null);
            if (ici && ici.includes('index.html?session=')) { sid = v.sessionId; urlSession = ici; break; }
        }
        if (!sid) await dormir(1000);
    }
    if (!sid) throw new Error("la page de session n'a jamais ete attachee : l'amorce n'a pas pu etre posee");
    const session = { evaluer: (e) => nav.evaluer(e, sid) };
    for (let k = 0; k < 60; k += 1) {
        const n = await session.evaluer('(window.__pcs || []).length').catch(() => 0);
        if (n > 0) { log(`PeerConnection capturee (${n})`); break; }
        await dormir(1000);
    }

    // On attend que la connexion ICE tienne avant de demarrer le palier :
    // mesurer pendant l'etablissement melangerait deux regimes.
    for (let i = 0; i < 40; i += 1) {
        const e = await session.evaluer('(window.__pcs||[])[0] && window.__pcs[0].iceConnectionState');
        if (e === 'connected' || e === 'completed') { log(`ice=${e}`); break; }
        await dormir(1000);
    }

    const a = await session.evaluer(RELEVE);
    log('releve A ' + JSON.stringify(a));
    await dormir(PALIER);
    const b = await session.evaluer(RELEVE);
    log('releve B ' + JSON.stringify(b));
    releve.a = a; releve.b = b;

    const d = (b?.framesDecoded ?? 0) - (a?.framesDecoded ?? 0);
    const s = ((b?.t ?? 0) - (a?.t ?? 0)) / 1000;
    releve.framesDecoded_delta = d;
    releve.palier_s = s;
    releve.cadence_ips = s > 0 ? d / s : null;
    releve.bytesVideo_delta = (b?.bytesVideo ?? 0) - (a?.bytesVideo ?? 0);
    releve.ice = b?.ice;
    releve.verdict = d > 0 ? 'VERT' : 'ROUGE';
    log(`\nCHIFFRE-JUGE framesDecoded_delta=${d} palier_s=${s.toFixed(1)} cadence=${s > 0 ? (d / s).toFixed(2) : 'n/a'} i/s`);
    log(`TEMOIN ice=${b?.ice} bytesVideo_delta=${releve.bytesVideo_delta}`);
    log(`VERDICT ${releve.verdict}`);
} catch (e) {
    releve.erreur = String(e && e.message ? e.message : e);
    releve.verdict = 'ROUGE';
    log('ERREUR ' + releve.erreur);
} finally {
    chrome.kill('SIGKILL');
    await writeFile(SORTIE, JSON.stringify(releve, null, 1));
    log(`releve ecrit : ${SORTIE}`);
    // Le menage ne doit JAMAIS masquer l'erreur du corps.
    await rm(profil, { recursive: true, force: true }).catch(() => {});
}
