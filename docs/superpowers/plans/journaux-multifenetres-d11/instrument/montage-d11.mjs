// Sous-bloc D11 — LA MOITIÉ « VM » des recettes ④, ⑤ et ⑥, extraite plutôt
// que recopiée trois fois.
//
// ⚠️ POURQUOI CE FICHIER EXISTE, ET IL FAUT LE DIRE. Les trois pilotes écrits
// par la tâche 8 (`pilote-flux-d11.mjs`, `pilote-cout-d11.mjs`,
// `pilote-resize-d11.mjs`) portaient la PLOMBERIE CDP mais PAS le montage :
// ils ouvraient la page-shell et attendaient que des fenêtres apparaissent
// toutes seules. Or aucune fenêtre n'apparaît si personne ne lance le
// superviseur ni n'ouvre les applications sur la VM. La tâche 9 a découvert
// ce manque en jouant la recette ① et l'a comblé DANS `pilote-audio-d11.mjs`
// seul ; les trois autres sont restés dans leur état d'avant la première
// mesure. Ce fichier leur porte le montage ÉPROUVÉ, celui de la recette ① de
// D10 (`journaux-multifenetres-d10/instrument/pilote-critere1-d10.mjs`), qui
// a tenu DIX fenêtres.
//
// INVARIANTS D'ORDRE, NON NÉGOCIABLES ET HÉRITÉS :
//   1. préparation VM (aucune fenêtre éligible, aucun agent vivant) ;
//   2. page-shell OUVERTE ET ATTACHÉE — le signaling ne mémorise que les
//      offres SDP, une annonce `fenetre-ouverte` émise avant qu'elle ne soit
//      connectée est PERDUE SANS TRACE (D1) ;
//   3. superviseur ;
//   4. fenêtres, une par une, espacées — et depuis D3 une entrée en attente
//      de viewport plus de 30 s est ABANDONNÉE sans jamais être reproposée.
//
// ⚠️ LE REGISTRE N'EST JAMAIS TOUCHÉ ICI. C'est la variable de la recette ⑤,
// et elle se pose par un lancement SÉPARÉ de `MULTIFENETRE_MODE_SORTIE`
// (l'aiguillage de `diagnostics/multifenetre.rs` retourne après la première
// sonde reconnue : une seconde variable dans le même lancement serait ignorée
// EN SILENCE).

import { spawnSync } from 'node:child_process';

export const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
export const D11 = `${RACINE}/docs/superpowers/plans/journaux-multifenetres-d11`;
export const INSTRUMENT = `${D11}/instrument`;
export const VMIT = `${RACINE}/docs/superpowers/plans/journaux-multifenetres-d10/instrument/vm-it.sh`;
export const HOTE = process.env.HOTE ?? '192.168.3.1';

export const journal = (...a) => console.log(new Date().toISOString(), ...a);

export function winrm(commande) {
    const r = spawnSync('node', [`${RACINE}/scripts/winrm.js`, commande],
        { encoding: 'utf8', env: process.env, timeout: 120000 });
    return (r.stdout ?? '') + (r.stderr ? `\n[stderr] ${r.stderr}` : '');
}

/** Un script PowerShell dans la SESSION INTERACTIVE (session 1), par tâche /IT. */
export function vmIt(nom, ps) {
    const r = spawnSync('bash', [VMIT, nom, ps], { encoding: 'utf8', env: process.env, timeout: 120000 });
    if (r.status !== 0) journal(`!! vm-it ${nom} a échoué`, (r.stderr ?? '').slice(0, 400));
    return r.status;
}

/** `loadavg` de l'hôte : D6 a établi qu'il covarie avec les performances. */
export function loadavg() {
    return (spawnSync('cat', ['/proc/loadavg'], { encoding: 'utf8' }).stdout ?? '').trim();
}

export function virshEtat() {
    return (spawnSync('virsh', ['domstate', 'Windows'], { encoding: 'utf8' }).stdout ?? '?').trim();
}

/**
 * Préparation : aucun agent vivant, aucune fenêtre éligible, la mire déployée.
 *
 * ⚠️ `Get-Process agent` se revérifie APRÈS, y compris sur une tentative
 * échouée : un superviseur resté vivant empêche le nouveau StreamWriter
 * d'ouvrir `agent.log`, et la copie relue est celle, PÉRIMÉE, de la tentative
 * précédente. Rencontré trois fois sur trois en D8.
 */
export function preparerVm(etiquette) {
    spawnSync('cp', [`${INSTRUMENT}/anim-d11.html`, '/media/vm/dev/anim-d11.html'], { encoding: 'utf8' });
    const avant = winrm(
        "Get-Process agent,chrome,notepad,mspaint -ErrorAction SilentlyContinue | Stop-Process -Force; "
        + "Start-Sleep -Seconds 3; "
        + "Get-ChildItem 'C:\\dev' -Directory -Filter 'chrome-d11r-*' -ErrorAction SilentlyContinue | "
        + "Remove-Item -Recurse -Force -ErrorAction SilentlyContinue; "
        + "$a = @(Get-Process agent -ErrorAction SilentlyContinue).Count; "
        + "$c = @(Get-Process chrome -ErrorAction SilentlyContinue).Count; "
        + "$m = Test-Path 'C:\\dev\\anim-d11.html'; "
        + "Write-Output \"PREP agent=$a chrome=$c mire=$m\"");
    journal(`préparation (${etiquette}) :`, avant.replace(/\s+/g, ' ').trim().slice(-120));
    return avant;
}

export function tuerAgent() {
    return winrm('Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; '
        + 'Start-Sleep -Seconds 3; '
        + '$a = @(Get-Process agent -ErrorAction SilentlyContinue).Count; Write-Output "AGENT-RESTANT=$a"');
}

/** Le superviseur, APRÈS la page-shell. `extra` : variables d'environnement. */
export function lancerSuperviseur(extra = {}) {
    const env = Object.entries(extra).map(([k, v]) => `${k}=${v}`).join(' ');
    const r = spawnSync('bash', ['-c',
        `cd ${RACINE} && set -a && source .env && set +a && SUPERVISEUR=1 `
        + `SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info ${env} scripts/run-agent.sh`],
        { encoding: 'utf8', env: process.env, timeout: 180000 });
    journal('run-agent.sh :', (r.stdout ?? '').trim().replace(/\n/g, ' / '));
    if ((r.stderr ?? '').trim()) journal('run-agent.sh STDERR :', r.stderr.trim().replace(/\n/g, ' / ').slice(0, 400));
    return r.status;
}

/**
 * Une sonde de banc dans un lancement À ELLE SEULE, agent tué ensuite.
 * C'est ainsi que se pose le registre pour le bras PROPRE de la recette ⑤.
 */
export function sonde(nom, extra) {
    const env = Object.entries(extra).map(([k, v]) => `${k}=${v}`).join(' ');
    const r = spawnSync('bash', ['-c',
        `cd ${RACINE} && set -a && source .env && set +a && ${env} scripts/run-agent.sh`],
        { encoding: 'utf8', env: process.env, timeout: 180000 });
    journal(`sonde ${nom} :`, (r.stdout ?? '').trim().replace(/\n/g, ' / '));
    return r.status;
}

/** Une fenêtre Chrome `--app` sur la mire, dans la session interactive. */
export function ouvrirFenetre(etiquette, rang, n) {
    return vmIt(`ouvrird11-${etiquette}-${rang}`, [
        '$a = @(',
        `  "--app=file:///C:/dev/anim-d11.html?n=${n}",`,
        `  "--user-data-dir=C:\\dev\\chrome-d11r-${etiquette}-${rang}",`,
        "  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',",
        `  '--window-size=1280,720','--window-position=${40 + rang * 10},${40 + rang * 10}',`,
        "  '--disable-features=CalculateNativeWinOcclusion',",
        "  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',",
        "  '--disable-renderer-backgrounding')",
        "Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a",
        'Start-Sleep -Seconds 2',
    ].join('\n'));
}

/** Copie du journal — APRÈS la fin réelle de l'exécution, avec son jumeau `-plat`. */
export function copierLog(etiquette) {
    spawnSync('bash', ['-c',
        `cp /media/vm/dev/agent.log ${D11}/agent-${etiquette}.log 2>/dev/null && `
        + `sed 's/\\x1b\\[[0-9;]*m//g' ${D11}/agent-${etiquette}.log > ${D11}/agent-${etiquette}-plat.log`],
        { encoding: 'utf8' });
    journal(`journal copié : agent-${etiquette}.log (+ -plat)`);
}

/**
 * Les marqueurs que le journal de l'agent porte de lui-même. `grep -a` :
 * un journal peut porter une queue d'octets NUL (lecture CIFS pendant que
 * Windows écrit encore), et `grep` sans `-a` rend alors une sortie VIDE — pas
 * zéro — indiscernable d'un compte nul (D10).
 */
export function marqueurs(etiquette) {
    const c = (motif) => {
        const r = spawnSync('bash', ['-c',
            `sed 's/\\x1b\\[[0-9;]*m//g' /media/vm/dev/agent.log 2>/dev/null | grep -ac ${JSON.stringify(motif)}`],
            { encoding: 'utf8' });
        return Number((r.stdout ?? '0').trim()) || 0;
    };
    const m = {
        attachee_capteur: c('fenêtre attachée au capteur'),
        enfant_lance: c('enfant lancé'),
        introuvable_topologie: c('introuvable dans la topologie DXGI'),
        sortie_creee: c('sortie virtuelle créée'),
        cloture: c('clôture de session amorcée'),
        erreurs: c('ERROR'),
    };
    journal(`marqueurs [${etiquette}]`, JSON.stringify(m));
    return m;
}
