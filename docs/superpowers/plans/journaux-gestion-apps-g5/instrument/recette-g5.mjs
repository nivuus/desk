#!/usr/bin/env node
// LE PILOTE DE RECETTE DE G5 — un seul, qui joue les trois critères et le WCO.
//
// 🔴 IL NE MONTE NI AGENT NI VM (décision D7). Les trois critères mesurent un
// NAVIGATEUR face à un MANIFESTE et à un DÉPÔT DE FICHIER ; rien n'y exige
// qu'un agent réel ait produit le catalogue. La base est ensemencée par les
// MODULES DU DÉPÔT (`ensemencer.mts`), c'est-à-dire par le code que le chemin
// de l'agent emprunte. ⚠️ CE QUE CELA N'ÉTABLIT PAS, et qui est écrit dans le
// verdict : aucun agent réel n'a produit le catalogue de ces exécutions.
//
// Usage : node recette-g5.mjs <urlClient> <urlPlateforme> <email> <motdepasse> <sortie.json>

import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [urlClient, urlPlateforme, email, motdepasse, sortie] = process.argv.slice(2);
const chromeBin = process.env.CHROME_BIN ?? 'google-chrome';
if (!urlClient || !urlPlateforme || !email || !motdepasse) {
    console.error('usage : recette-g5.mjs <urlClient> <urlPlateforme> <email> <motdepasse> [sortie.json]');
    process.exit(2);
}

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.ready = new Promise((r) => this.ws.addEventListener('open', () => r(), { once: true }));
        this.ws.addEventListener('message', (e) => this.onMessage(e));
        this.consoleLines = [];
        this.pageErrors = [];
    }
    onMessage(event) {
        const m = JSON.parse(String(event.data));
        if (m.id !== undefined && this.pending.has(m.id)) {
            const { resolve, reject } = this.pending.get(m.id);
            this.pending.delete(m.id);
            if (m.error) reject(new Error(JSON.stringify(m.error)));
            else resolve(m.result);
            return;
        }
        if (m.method === 'Runtime.consoleAPICalled') {
            const t = (m.params.args ?? []).map((a) => a.value ?? a.description ?? '').join(' ');
            this.consoleLines.push(`[${m.params.type}] ${t}`);
        }
        if (m.method === 'Runtime.exceptionThrown') this.pageErrors.push(m.params.exceptionDetails.text);
    }
    async send(method, params = {}) {
        await this.ready;
        const id = this.nextId++;
        return new Promise((resolve, reject) => {
            this.pending.set(id, { resolve, reject });
            this.ws.send(JSON.stringify({ id, method, params }));
        });
    }
    async eval(expression, awaitPromise = false) {
        const r = await this.send('Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
        if (r.exceptionDetails) {
            throw new Error(r.exceptionDetails.exception?.description ?? JSON.stringify(r.exceptionDetails));
        }
        return r.result.value;
    }
    close() { this.ws.close(); }
}

async function attendreDevtools(port, tentatives = 60) {
    for (let i = 0; i < tentatives; i += 1) {
        try { if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) return; } catch { /* pas prêt */ }
        await new Promise((r) => setTimeout(r, 200));
    }
    throw new Error('Chrome DevTools ne répond pas après le délai imparti');
}

/// Attend un FAIT, jamais une durée seule.
async function jusqua(predicat, ms, quoi) {
    const fin = Date.now() + ms;
    while (Date.now() < fin) {
        if (await predicat()) return true;
        await new Promise((r) => setTimeout(r, 150));
    }
    throw new Error(`délai dépassé en attendant : ${quoi}`);
}

/// 🔴 LE JETON EST OBTENU, JAMAIS FORGÉ — la leçon de P2. Un script qui
/// signerait lui-même porterait `PLATEFORME_SECRET_JETON` dans un fichier ou
/// dans l'`argv` d'un processus : le trou serait DÉPLACÉ, pas fermé.
async function obtenirJeton() {
    const r = await fetch(`${urlPlateforme}/auth/connexion`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        // ⚠️ `motdepasse`, EN UN MOT — pas `motDePasse`. Un `400 {"refus":"forme"}`
        //    sans plus de détail est la seule indication (piège relevé par G1).
        body: JSON.stringify({ email, motdepasse }),
    });
    if (!r.ok) throw new Error(`connexion refusée : ${r.status} ${await r.text()}`);
    const corps = await r.json();
    return corps.acces ?? corps.jeton ?? corps.access;
}

// ⚠️ `?plateforme=` EST OBLIGATOIRE ICI, ET C'EST UNE PROPRIÉTÉ DU MONTAGE,
//    PAS DU PRODUIT : `adressePlateforme` retombe sur L'ORIGINE DE LA PAGE
//    quand le paramètre est absent, et c'est le cas NOMINAL derrière le proxy
//    inverse de P5, où la page et l'API partagent une origine. La recette, elle,
//    sert le client et la plateforme sur DEUX ports — donc en origine croisée,
//    ce qui met en outre la politique CORS du navigateur dans le chemin.
const PAGE_HUB = `${urlClient}/hub.html?plateforme=${encodeURIComponent(urlPlateforme)}`;

const releve = { horodatage: new Date().toISOString(), urlClient, urlPlateforme, pageHub: PAGE_HUB };

async function jouer() {
    const jeton = await obtenirJeton();
    releve.jetonObtenu = typeof jeton === 'string' && jeton.length > 0;

    const port = 9600 + Math.floor(Math.random() * 300);
    const profil = await mkdtemp(join(tmpdir(), 'recette-g5-'));
    const chrome = spawn(chromeBin, [
        '--headless=new',
        `--remote-debugging-port=${port}`,
        '--remote-allow-origins=*',
        `--user-data-dir=${profil}`,
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--disable-gpu',
        'about:blank',
    ], { stdio: 'ignore' });

    let cdp;
    try {
        await attendreDevtools(port);
        releve.navigateur = (await (await fetch(`http://127.0.0.1:${port}/json/version`)).json()).Browser;
        const cible = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
        cdp = new Cdp(cible.webSocketDebuggerUrl);
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');

        // Le jeton est SEMÉ dans `localStorage` avant le chargement : c'est le
        // seul stockage partagé entre une page et celles qu'elle ouvre, et
        // `Page.addScriptToEvaluateOnNewDocument` ne court pas sur les fenêtres
        // ouvertes par `window.open` (piège mesuré en D5).
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
            source: `try { localStorage.setItem('guac.jeton.acces', ${JSON.stringify(jeton)}); } catch (e) {}`,
        });

        await cdp.send('Page.navigate', { url: PAGE_HUB });
        await jusqua(
            () => cdp.eval("document.querySelectorAll('#applications li').length > 0").catch(() => false),
            20_000,
            'le hub peuple sa liste',
        );

        releve.applications = await cdp.eval(
            "JSON.stringify([...document.querySelectorAll('#applications li .hub__nom')].map(e => e.textContent))",
        );
        releve.message = await cdp.eval("document.getElementById('message').textContent");
        // Les icônes sont lues par `fetch` authentifié puis publiées en objet :
        // une `blob:` dans `src` prouve que le chemin a abouti.
        releve.iconesChargees = await cdp.eval(
            "[...document.querySelectorAll('#applications img')].filter(i => i.src.startsWith('blob:')).length",
        );

        // ── LE PRÉSENT DE `launchQueue` ────────────────────────────────────
        releve.launchQueuePresent = await cdp.eval("'launchQueue' in window");

        // ── CRITÈRE ① : l'installabilité, par application ──────────────────
        releve.installabilite = [];
        for (const nom of ['Bloc-notes', 'Temoin-128', 'Sans-icone']) {
            const pose = await cdp.eval(
                `(() => {
                    const titres = [...document.querySelectorAll('#applications li')];
                    const li = titres.find(e => e.querySelector('.hub__nom').textContent === ${JSON.stringify(nom)});
                    if (!li) return 'absente';
                    const b = [...li.querySelectorAll('button')].find(x => x.textContent === 'Installer');
                    b.click();
                    return 'clic';
                })()`,
            );
            // On attend que le lien change, puis on laisse Chromium aller le
            // chercher. Le manifeste est `blob:` : c'est la voie V1.
            await jusqua(
                () => cdp.eval("(document.querySelector('link[rel=\"manifest\"]')?.href ?? '').startsWith('blob:')").catch(() => false),
                10_000,
                `le manifeste blob: de ${nom}`,
            ).catch(() => {});
            await new Promise((r) => setTimeout(r, 1500));
            const m = await cdp.send('Page.getAppManifest');
            let erreursInstall;
            try {
                erreursInstall = (await cdp.send('Page.getInstallabilityErrors')).installabilityErrors;
            } catch (e) {
                erreursInstall = `INDISPONIBLE : ${e.message}`;
            }
            releve.installabilite.push({
                application: nom,
                pose,
                // ⚠️ L'URL D'ABORD : `getAppManifest` rend un objet MÊME quand
                //    la page n'a pas de manifeste. C'est elle qui tranche.
                url: m.url ?? null,
                estBlob: String(m.url ?? '').startsWith('blob:'),
                errors: m.errors ?? [],
                installabilityErrors: erreursInstall,
                displayOverride: m.manifest?.displayOverride ?? m.parsed?.displayOverride ?? null,
                manifestBrut: m.data ? JSON.parse(m.data) : null,
            });
        }

        // ── LE WCO ─────────────────────────────────────────────────────────
        // Ce qu'on peut mesurer : que la DÉCLARATION est posée et acceptée.
        // Ce qu'on ne peut PAS : qu'une barre superposée existe — il faudrait
        // une PWA installée, et un Chromium sans interface n'en installe aucune.
        releve.wco = {
            declareParLeHub: null,
            declareParApplication: null,
            titlebarAreaHeight: await cdp.eval(
                "getComputedStyle(document.documentElement).getPropertyValue('--x') || (CSS.supports('top', 'env(titlebar-area-height, 0px)') ? 'env() supporte' : 'env() non supporte')",
            ),
            fenetreEnBarreSuperposee: await cdp.eval(
                "matchMedia('(display-mode: window-controls-overlay)').matches",
            ),
        };

        // ── CRITÈRE ③ : le manifeste du HUB et ses file_handlers ───────────
        await cdp.send('Page.navigate', { url: PAGE_HUB });
        await jusqua(() => cdp.eval("document.readyState === 'complete'").catch(() => false), 15_000, 'le hub recharge');
        await new Promise((r) => setTimeout(r, 1500));
        const mh = await cdp.send('Page.getAppManifest');
        releve.manifesteHub = {
            url: mh.url ?? null,
            errors: mh.errors ?? [],
            data: mh.data ? JSON.parse(mh.data) : null,
            fileHandlers: mh.manifest?.fileHandlers ?? mh.parsed?.fileHandlers ?? null,
            manifestComplet: mh.manifest ?? null,
        };
        releve.wco.declareParLeHub = releve.manifesteHub.data?.display_override ?? null;
        try {
            releve.manifesteHub.installabilityErrors = (await cdp.send('Page.getInstallabilityErrors')).installabilityErrors;
        } catch (e) {
            releve.manifesteHub.installabilityErrors = `INDISPONIBLE : ${e.message}`;
        }

        // ── CRITÈRE ② : le glisser-déposer, SANS aucun file handler ────────
        // ⚠️ UN `DataTransfer` CONSTRUIT DANS LA PAGE N'EST PAS UN GESTE
        //    HUMAIN, et c'est écrit dans le verdict. Ce qu'il exerce, en
        //    revanche, est LE VRAI ÉCOUTEUR `drop` du produit, avec de vrais
        //    `dataTransfer.files` — pas une fonction exposée pour l'occasion.
        await jusqua(
            () => cdp.eval("document.querySelectorAll('#applications li').length > 0").catch(() => false),
            20_000,
            'le hub repeuple',
        );
        const avant = await cdp.eval("document.getElementById('message').textContent");
        await cdp.eval(`(() => {
            const dt = new DataTransfer();
            dt.items.add(new File([new Uint8Array([71, 53, 33, 33])], 'temoin-g5.msi', { type: 'application/x-msi' }));
            document.getElementById('depot').dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
            return true;
        })()`);
        await jusqua(
            async () => (await cdp.eval("document.getElementById('message').textContent")) !== avant,
            20_000,
            'le bandeau change après le dépôt',
        ).catch(() => {});
        // On laisse la séquence complète se dérouler : créer, déposer, sceller.
        await new Promise((r) => setTimeout(r, 4000));
        releve.depot = {
            avant,
            apres: await cdp.eval("document.getElementById('message').textContent"),
            classeDuBandeau: await cdp.eval("document.getElementById('message').className"),
        };

        releve.console = cdp.consoleLines;
        releve.erreursPage = cdp.pageErrors;
    } catch (e) {
        releve.echec = e.message;
    } finally {
        cdp?.close();
        chrome.kill('SIGKILL');
        await new Promise((r) => setTimeout(r, 300));
        await rm(profil, { recursive: true, force: true }).catch(() => {});
    }

    const texte = JSON.stringify(releve, null, 2);
    if (sortie) await writeFile(sortie, texte + '\n');
    console.log(texte);
}

jouer().then(() => process.exit(0), (e) => { console.error(e); process.exit(1); });
