// Pilote de la recette du sous-bloc A1 — la couleur d'accent.
//
// 🔴 DÉRIVÉ du pilote de P3 (`journaux-presse-papier-p3/instrument/pilote-pp-p3.mjs`),
// jamais réécrit : son échafaudage — attache CDP, balayage `Target.getTargets`,
// amorce posée DEUX fois (une page ouverte par `window.open` ne reçoit pas
// `addScriptToEvaluateOnNewDocument`, D5), attribution session↔pid lue dans le
// journal MIS À PLAT — est éprouvé. Ce qui change est ce qu'on observe.
//
// Usage :
//   node pilote-accent-a1.mjs --etiquette=1 --identite=/tmp/a1-identite.env
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-multifenetres-d11/instrument/commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const ETIQUETTE = arg('etiquette', '1');
const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
const A1 = `${RACINE}/docs/superpowers/plans/journaux-accent-a1`;
const VMIT = `${RACINE}/docs/superpowers/plans/journaux-multifenetres-d10/instrument/vm-it.sh`;
const IDENTITE = arg('identite', '/tmp/a1-identite.env');
const ACCENT_VAL = arg('accent', '');           // '' = armé (défaut), '0' = désarmé
const PALIER_MS = Number(arg('palier', '75000')); // ≥ 60 s, soit 15× PERIODE_ACCENT
// 🔴 LE THÈME DU NAVIGATEUR, ÉMULÉ ET DÉCLARÉ. Chrome sans interface rend
// `prefers-color-scheme: light`, et l'exécution 1 a MESURÉ que les deux icônes
// de cette VM y sont ILLISIBLES (1,24 à 1,86 contre les trois fonds clairs,
// pour un seuil de 3) — donc REFUSÉES, ce qui est le rempart qui fonctionne,
// mais ce qui rend le critère ② non discriminant : le token porte alors le
// repli, et « il vaut --accent » ne distingue pas une couleur acceptée d'un
// refus. En SOMBRE les mêmes couleurs valent 8,8 à 13,5 : elles sont acceptées,
// et le token porte alors une valeur DIFFÉRENTE de `--accent`.
const THEME = arg('theme', 'light');
const SORTIE = arg('sortie', `${A1}/a1-${ETIQUETTE}.json`);
const log = (...a) => console.log(new Date().toISOString(), ...a);

const winrm = (c) => {
    const r = spawnSync('node', [`${RACINE}/scripts/winrm.js`, c],
        { encoding: 'utf8', env: process.env, timeout: 120000 });
    return `${r.stdout ?? ''}${r.stderr ?? ''}`;
};
const vmIt = (nom, ps) => {
    const r = spawnSync('bash', [VMIT, nom, ps], { encoding: 'utf8', env: process.env, timeout: 120000 });
    return `${r.stdout ?? ''}${r.stderr ?? ''}`;
};

function lireIdentite(chemin) {
    const o = {};
    for (const l of readFileSync(chemin, 'utf8').split('\n')) {
        const i = l.indexOf('=');
        if (i > 0) o[l.slice(0, i).trim()] = l.slice(i + 1).trim();
    }
    for (const c of ['AGENT_VM', 'AGENT_SECRET', 'PREFIXE_VM', 'RECETTE_EMAIL', 'RECETTE_MOTDEPASSE']) {
        if (!o[c]) throw new Error(`identité incomplète : ${c} absent`);
    }
    return o;
}

/// Ni le secret d'enrôlement ni le mot de passe ne sont journalisés ni écrits
/// dans le relevé : ils vivent hors du dépôt.
async function obtenirPaire(id, url) {
    const r = await fetch(`${url}/auth/connexion`, {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email: id.RECETTE_EMAIL, motdepasse: id.RECETTE_MOTDEPASSE }),
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`connexion refusée : ${r.status} ${JSON.stringify(corps)}`);
    return { acces: corps.acces, rafraichissement: corps.rafraichissement };
}

// 🔴 L'OBSERVATION EST INDÉPENDANTE DU PRODUIT : l'écouteur est posé en
// ENVELOPPANT `createDataChannel`, AVANT que le produit n'attache le sien.
// Compter les appels à `setProperty` mesurerait le garde du CLIENT ; ce qu'on
// veut est ce que l'AGENT a émis.
const AMORCE = `(() => {
  if (window.__a1) return;
  window.__a1 = { messages: [], erreurs: [] };
  try {
    const P = window.RTCPeerConnection;
    if (P && P.prototype && P.prototype.createDataChannel) {
      const c = P.prototype.createDataChannel;
      P.prototype.createDataChannel = function (label, opts) {
        const ch = c.call(this, label, opts);
        try {
          if (label === 'control') {
            ch.addEventListener('message', (e) => {
              try {
                const m = JSON.parse(e.data);
                if (m && m.type === 'accent') {
                  window.__a1.messages.push({ t: Date.now(), couleur: m.couleur });
                }
              } catch (err) { window.__a1.erreurs.push(String(err).slice(0, 120)); }
            });
          }
        } catch (err) { window.__a1.erreurs.push(String(err).slice(0, 120)); }
        return ch;
      };
    } else { window.__a1.erreurs.push('RTCPeerConnection absent'); }
  } catch (e) { window.__a1.erreurs.push('hameconnage createDataChannel : ' + String(e).slice(0, 120)); }
  // La page-shell ouvre '/?session=...' SANS parametre 'signaling' : main.ts
  // retombe alors sur ws://<hote>:8080 en dur, or cette recette parle a une
  // SECONDE instance. Geste d'INSTRUMENT, declare.
  try {
    const o = window.open.bind(window);
    window.open = function (url, ...r) {
      try {
        if (typeof url === 'string' && url.includes('?session=') && !url.includes('signaling=')) {
          url += (url.includes('?') ? '&' : '?') + 'signaling=' + encodeURIComponent('__SIGNALING__');
        }
      } catch (e) { window.__a1.erreurs.push(String(e).slice(0, 120)); }
      return o(url, ...r);
    };
  } catch (e) { window.__a1.erreurs.push('hameconnage window.open : ' + String(e).slice(0, 120)); }
})();`;

// 🔴 LA LECTURE QUI JUGE ② : la valeur COURANTE du token sur la RACINE.
// `--accent` est relevé au même aller-retour, parce qu'un accent REFUSÉ pose
// justement `--accent` : sans lui, « le token vaut #7aa2f7 » ne distinguerait
// pas une couleur acceptée d'un repli.
const EXPR_ETAT = `(() => ({
  a1: window.__a1 || null,
  token: getComputedStyle(document.documentElement).getPropertyValue('--accent-fenetre').trim(),
  accentDuTheme: getComputedStyle(document.documentElement).getPropertyValue('--accent').trim(),
  fond0: getComputedStyle(document.documentElement).getPropertyValue('--fond-0').trim(),
  visible: !document.hidden,
}))()`;

/// Les couples (session, pid) que le CAPTEUR a inscrits, lus dans le journal
/// MIS À PLAT — `tracing` sépare le nom du champ de sa valeur par des séquences
/// ANSI, et `grep 'pid='` ne matche JAMAIS sur un journal brut (piège D8).
function attributions() {
    let brut;
    try { brut = readFileSync('/media/vm/dev/agent.log', 'utf8'); } catch { return []; }
    const plat = brut.replace(/\x1b\[[0-9;]*m/g, '');
    const vues = new Map();
    for (const l of plat.split('\n')) {
        if (!/fen\S*tre attach\S*e au capteur/.test(l)) continue;
        // L'ancre exige `session=… pid=…` ADJACENTS : le span `tracing` qui
        // précède la ligne porte bien `session=` mais JAMAIS `pid=`, ce qui
        // l'exclut par construction (piège payé par P3).
        const m = /session=(\S+)\s+pid=(\d+)/.exec(l);
        if (m) vues.set(m[1], Number(m[2]));
    }
    return [...vues.entries()].map(([session, pid]) => ({ session, pid }));
}

/// 🔴 LES ANNONCES D'ACCENT DU CAPTEUR, PAR SESSION — c'est sur CETTE ligne, et
/// sur elle seule, que les critères ① et ④ se comptent.
function annonces() {
    let brut;
    try { brut = readFileSync('/media/vm/dev/agent.log', 'utf8'); } catch { return []; }
    const plat = brut.replace(/\x1b\[[0-9;]*m/g, '');
    const out = [];
    for (const l of plat.split('\n')) {
        if (!l.includes('accent de la fenetre Windows')) continue;
        const m = /session=(\S+)\s+couleur=(\S+)/.exec(l);
        if (m) out.push({ session: m[1], couleur: m[2] });
    }
    return out;
}

const identite = lireIdentite(IDENTITE);
const PLATEFORME_URL = identite.PLATEFORME_URL;
const CLIENT_URL = identite.CLIENT_URL;
const SIGNALING_WS = identite.SIGNALING_WS;
// 🔴 CHANTIER auth-pomerium : le relais a quitté `/` pour `/signal`.
// SIGNALING_WS RESTE LA BASE — c'est elle qui va dans SIGNALING_URL au
// lancement de l'agent (`agent/src/signaling.rs::url_du_relais` y ajoute
// `/signal` lui-même). `?signaling=` côté NAVIGATEUR, lui, est EXPLICITE et
// NE REÇOIT AUCUN AJOUT (client/src/adresse-plateforme.ts::adresseSignaling) :
// c'est ce pilote qui doit fournir l'URL du relais, pas sa seule base.
const SIGNALING_RELAIS = `${SIGNALING_WS.replace(/\/+$/, '')}/signal`;
const paire = await obtenirPaire(identite, PLATEFORME_URL);
const SEMENCE = `(() => { try {
  localStorage.setItem('guac.jeton.acces', ${JSON.stringify(paire.acces)});
  localStorage.setItem('guac.jeton.rafraichissement', ${JSON.stringify(paire.rafraichissement)});
  localStorage.setItem('guac.prefixe', ${JSON.stringify(identite.PREFIXE_VM)});
} catch (e) { } })();`;
const URL_SHELL = `${CLIENT_URL}/shell.html?signaling=${encodeURIComponent(SIGNALING_RELAIS)}`
    + `&prefixe=${encodeURIComponent(identite.PREFIXE_VM)}`;

// 🔴 Substitution VÉRIFIÉE : un marqueur survivant produirait une URL littérale,
// la page ne se connecterait à rien, et le seul symptôme serait « l'agent n'a
// pas répondu » — indiscernable d'une panne du produit.
const AMORCE_FINALE = AMORCE.replaceAll('__SIGNALING__', SIGNALING_RELAIS);
if (AMORCE_FINALE.includes('__SIGNALING__')) throw new Error('marqueur non substitué');
if (!AMORCE_FINALE.includes(SIGNALING_RELAIS)) throw new Error('substitution sans effet');

const dir = await mkdtemp(join(tmpdir(), `a1-${ETIQUETTE}-`));
const port = 9480 + (Number(ETIQUETTE) || 1);
const releve = {
    etiquette: ETIQUETTE, accent_pose: ACCENT_VAL === '' ? '(armé, défaut)' : ACCENT_VAL,
    theme_emule: THEME,
    palier_ms: PALIER_MS, phases: [], criteres: {},
};
let chrome = null, cdp = null;

try {
    log('préparation VM');
    releve.prep = winrm(
        'Get-Process agent,notepad,explorer -ErrorAction SilentlyContinue | Stop-Process -Force; '
        + 'Start-Sleep -Seconds 4; '
        + '$a = @(Get-Process agent -ErrorAction SilentlyContinue).Count; '
        + 'Write-Output "PREP agent=$a"').trim().split('\n').pop();
    log('prep :', releve.prep);

    // 🔴 LA PURGE DU VIVIER, DANS UN LANCEMENT SÉPARÉ : l'aiguillage de
    // `diagnostics::multifenetre` RETOURNE APRÈS LA PREMIÈRE SONDE RECONNUE,
    // et deux variables dans le même lancement n'en enchaînent pas deux (D8).
    log('purge du vivier de sorties virtuelles (lancement SÉPARÉ)');
    spawnSync('bash', ['-c',
        `cd ${RACINE} && set -a && source .env && set +a && MULTIFENETRE_VDD_PURGE=1 scripts/run-agent.sh`],
        { encoding: 'utf8', env: process.env, timeout: 180000 });
    await dodo(5000);

    // --- DEUX applications DIFFÉRENTES, aux icônes VISIBLEMENT contrastées. --
    // 🔴 Le critère ① est NON MESURABLE si les deux icônes ont la même
    // dominante. Bloc-notes (bleu/blanc) et Explorateur (dossier JAUNE) sont le
    // couple le plus contrasté que cette VM offre — `mspaint` est écarté parce
    // qu'il ouvre DEUX fenêtres éligibles (piège de D2).
    log('ouverture de Bloc-notes et de l’Explorateur');
    vmIt('a1-apps', 'Start-Process notepad; Start-Sleep -Seconds 3; '
        + 'Start-Process explorer -ArgumentList "C:\\dev"; Start-Sleep -Seconds 5');
    for (let i = 0; i < 15; i += 1) {
        await dodo(2000);
        const n = winrm('@(Get-Process notepad -ErrorAction SilentlyContinue).Count').trim().split('\n').pop().replace(/^\uFEFF/, '');
        releve.notepad = Number(n) || 0;
        if (releve.notepad >= 1) break;
    }
    log('Bloc-notes ouverts :', releve.notepad);
    await dodo(4000);

    chrome = lancerChrome(port, dir);
    cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
    const pages = new Map();
    cdp.on(async (m) => {
        if (m.method === 'Target.targetInfoChanged') {
            const p = pages.get(m.params.targetInfo.targetId);
            if (p) p.url = m.params.targetInfo.url;
            return;
        }
        if (m.method !== 'Target.attachedToTarget') return;
        const sid = m.params.sessionId;
        if (m.params.targetInfo.type !== 'page') {
            await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
            return;
        }
        pages.set(m.params.targetInfo.targetId, { sid, url: m.params.targetInfo.url ?? '' });
        await cdp.send('Emulation.setEmulatedMedia',
            { features: [{ name: 'prefers-color-scheme', value: THEME }] }, sid).catch(() => { });
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
        await cdp.send('Page.enable', {}, sid).catch(() => { });
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: SEMENCE + AMORCE_FINALE }, sid).catch(() => { });
        await cdp.send('Runtime.evaluate', { expression: SEMENCE + AMORCE_FINALE }, sid).catch(() => { });
        await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
        log('+ page attachée', sid.slice(0, 8), m.params.targetInfo.url ?? '');
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await cdp.send('Target.setDiscoverTargets', { discover: true });

    await cdp.send('Target.createTarget', { url: URL_SHELL });
    await dodo(4000);
    releve.shell_attachee = [...pages.values()].some((p) => /shell/.test(p.url ?? ''));
    log('page-shell attachée :', releve.shell_attachee);

    log('lancement du superviseur', ACCENT_VAL === '' ? '(ACCENT armé)' : `(ACCENT=${ACCENT_VAL})`);
    const envAccent = ACCENT_VAL === '' ? '' : `ACCENT=${ACCENT_VAL} `;
    const r = spawnSync('bash', ['-c',
        `cd ${RACINE} && set -a && source .env && set +a && `
        + 'export AGENT_VM="$1" AGENT_SECRET="$2" && '
        + `${envAccent}SUPERVISEUR=1 SIGNALING_URL=${SIGNALING_WS} LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`,
        'a1', identite.AGENT_VM, identite.AGENT_SECRET],
        { encoding: 'utf8', env: process.env, timeout: 180000 });
    log('run-agent.sh :', (r.stdout ?? '').trim().split('\n').pop());

    // 🔴 LA DETTE DE VÉRIFICATION DE LA TÂCHE 12 : la ligne lue dans le
    // `run-agent.ps1` GÉNÉRÉ sur la VM, jamais dans le script hôte. Ce dépôt a
    // payé CINQ fois qu'une variable n'atteigne pas le processus.
    try {
        const ps1 = readFileSync('/media/vm/dev/run-agent.ps1', 'utf8');
        releve.ps1_accent = ps1.split('\n').filter((l) => l.includes('ACCENT'));
        releve.ps1_temoin_negatif = ps1.split('\n').filter((l) => l.includes('ACCENT_INEXISTANT'));
    } catch (e) { releve.ps1_accent = [`illisible : ${String(e).slice(0, 80)}`]; }
    log('run-agent.ps1 — lignes ACCENT :', JSON.stringify(releve.ps1_accent));

    const appsParSession = new Map();
    for (let i = 0; i < 150; i += 1) {
        await dodo(1000);
        const inv = await cdp.send('Target.getTargets', {}).catch(() => ({ targetInfos: [] }));
        for (const t of inv.targetInfos ?? []) {
            if (t.type !== 'page' || !/\?session=/.test(t.url ?? '')) continue;
            if (pages.has(t.targetId)) continue;
            const a = await cdp.send('Target.attachToTarget', { targetId: t.targetId, flatten: true }).catch(() => null);
            if (!a) continue;
            pages.set(t.targetId, { sid: a.sessionId, url: t.url });
            await cdp.send('Emulation.setEmulatedMedia',
                { features: [{ name: 'prefers-color-scheme', value: THEME }] }, a.sessionId).catch(() => { });
            await cdp.send('Runtime.enable', {}, a.sessionId).catch(() => { });
            await cdp.send('Runtime.evaluate', { expression: SEMENCE + AMORCE_FINALE }, a.sessionId).catch(() => { });
            log('+ page rattrapée par balayage', a.sessionId.slice(0, 8), t.url);
        }
        for (const p of pages.values()) {
            const m = /[?&]session=([^&]+)/.exec(p.url ?? '');
            if (!m) continue;
            const session = decodeURIComponent(m[1]);
            if (appsParSession.has(session)) continue;
            const e = await cdp.evalBorne(p.sid, EXPR_ETAT, 8000, false);
            if (e && e.a1) appsParSession.set(session, { sid: p.sid, url: p.url });
        }
        if (appsParSession.size >= 2) break;
    }
    releve.sessions = [...appsParSession.keys()];
    log('pages d’application :', releve.sessions.join(' '));

    releve.attributions = attributions();
    log('attributions session ↔ pid :', JSON.stringify(releve.attributions));

    // --- PHASE 1 : le premier relevé, après que les sessions se sont établies.
    // PERIODE_ACCENT vaut 5 s et le minuteur part à `Instant::now()` : on laisse
    // largement le temps de la PREMIÈRE annonce, qui est attendue (D-A1-7).
    await dodo(20000);
    releve.phases.push(await releverPhase('apres-etablissement', appsParSession));

    // --- PHASE 2 : le PALIER de ④. Rien n'est touché ; l'icône ne change pas.
    // ⚠️ Le palier est PLUSIEURS FOIS plus long que PERIODE_ACCENT (5 s) : 75 s
    // donne un facteur 15. D6 a vu trois promotions arriver APRÈS un palier qui
    // n'excédait la temporisation observée que de 25 %.
    log(`palier de ${PALIER_MS} ms — rien n'est touché`);
    await dodo(PALIER_MS);
    releve.phases.push(await releverPhase('apres-palier', appsParSession));

    async function releverPhase(nom, apps) {
        const par = {};
        for (const [session, p] of apps) {
            const e = await cdp.evalBorne(p.sid, EXPR_ETAT, 10000, false);
            par[session] = e ? {
                token: e.token, accentDuTheme: e.accentDuTheme, fond0: e.fond0,
                messages: (e.a1?.messages ?? []).map((m) => m.couleur),
                erreurs: e.a1?.erreurs ?? [],
            } : { erreur: 'évaluation non rendue' };
        }
        const a = annonces();
        const parSession = {};
        for (const x of a) parSession[x.session] = (parSession[x.session] ?? []).concat(x.couleur);
        const phase = { nom, t: new Date().toISOString(), pages: par, annonces_agent: parSession };
        log(`phase ${nom} :`, JSON.stringify(parSession));
        return phase;
    }
} catch (e) {
    releve.erreur_fatale = String(e).slice(0, 400);
    log('!! ', releve.erreur_fatale);
} finally {
    try { chrome?.kill(); } catch { }
    try { cdp?.close(); } catch { }
    writeFileSync(SORTIE, JSON.stringify(releve, null, 2));
    log('relevé écrit :', SORTIE);
    // 🔴 SORTIE EXPLICITE : le WebSocket CDP tient la boucle d'événements après
    // la mort de Chrome, et sans cela le harnais bascule le pilote en
    // arrière-plan alors que tout est fini (piège de P1).
    process.exit(releve.erreur_fatale ? 1 : 0);
}
