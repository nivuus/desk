#!/usr/bin/env node
// Sous-bloc D11 — pilote de la recette ⑤ : LE COÛT DE LA DUPLICATION D'UNE
// SORTIE SURDIMENSIONNÉE (leg 7 de D10).
//
// Depuis D10, le superviseur TOLÈRE une sortie née trop grande (le registre
// laissé sale) et RECADRE le rectangle utile dans la duplication. C'est ce qui
// a fait passer le produit de 3 à 10 fenêtres. Le prix de cette voie —
// dupliquer du 3840×2160 pour n'en recadrer que 1280×720 — n'est mesuré PAR
// RIEN, et D10 le déclare tel quel.
//
// ⚠️ AJOUT DE LA TÂCHE 13, déclaré : ce pilote ne portait, à sa création
// (tâche 8), QUE la plomberie CDP — il ouvrait la page-shell et attendait des
// fenêtres que personne n'ouvrait. Le montage vient de `montage-d11.mjs`.
//
// ⚠️ LA GRANDEUR RELEVÉE EST CELLE DU CAPTEUR, PAS DU NAVIGATEUR. Le prix de
// la duplication se paie côté AGENT (surface dupliquée, recadrage, copie GPU),
// et la cadence du capteur (`cadence du capteur … images=… cadence="…"`) est
// la seule qui le voie. Ce pilote pose donc la BORNE TEMPORELLE de la fenêtre
// utile, et `analyse-cout-d11.mjs` lit `agent.log` entre ces deux bornes. Les
// statistiques navigateur sont relevées en plus, jamais à la place.
//
// ⚠️ LE BRAS SE POSE PAR UN LANCEMENT À LUI SEUL. `MULTIFENETRE_MODE_SORTIE`
// et `MULTIFENETRE_VDD_PURGE` ne peuvent pas cohabiter : l'aiguillage de
// `diagnostics/multifenetre.rs` retourne après la PREMIÈRE sonde reconnue, et
// la seconde variable serait ignorée EN SILENCE.
//
// ⚠️ RÈGLE D'ADMISSION, écrite AVANT la mesure et appliquée par
// `analyse-cout-d11.mjs` : un bras n'est réputé obtenu que si TOUTES les
// lignes `duplication de sortie établie` du rang portent la MÊME valeur de
// `desktop_width`. À défaut la mesure est DÉCLARÉE NON PRISE — pas approchée,
// pas interprétée.
//
// ⚠️ CE N'EST PAS UN CONTRÔLE, C'EST UNE MESURE : elle n'a pas de rouge. Ce
// qui a un rouge, c'est le contrôle du bras ci-dessus, et il PEUT échouer.
//
// Les invariants de montage sont dans `commun-d11.mjs` — les relire.
//
// Usage :
//   ETIQUETTE=cout-propre-1 MODE_SORTIE=1280x720  N=8 node pilote-cout-d11.mjs
//   ETIQUETTE=cout-sale-1   MODE_SORTIE=3840x2160 N=8 node pilote-cout-d11.mjs
//   (MODE_SORTIE vide = registre laissé tel quel)

import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { Cdp, attendreDevtools, dodo, lancerChrome } from './commun-d11.mjs';
import {
    D11, HOTE, copierLog, journal as log, loadavg, lancerSuperviseur, marqueurs,
    ouvrirFenetre, preparerVm, sonde, tuerAgent, virshEtat,
} from './montage-d11.mjs';

const ETIQUETTE = process.env.ETIQUETTE ?? 'cout-sans-etiquette';
const MODE_SORTIE = process.env.MODE_SORTIE ?? '';
const N = Number(process.env.N ?? 8);
const BUDGET_BPS = process.env.BUDGET_BPS ?? '12000000';
const DELAI_ENTRE_FENETRES_MS = Number(process.env.DELAI_ENTRE_FENETRES_MS ?? 8000);
// Palier de 90 s dont les 30 premières sont ÉCARTÉES : 60 s de mesure utile
// pour un `DELAI_REMONTEE` de 20 s (`congestion/hysteresis.rs`, relevé), soit
// un rapport de 3. D6 a perdu trois mesures pour un palier de 25 s face à 20 s.
const PALIER_S = Number(process.env.PALIER_S ?? 90);
const ECART_S = Number(process.env.ECART_S ?? 30);
// Établissement attendu en FAIT et non en durée : douze secondes consécutives
// sans aucun changement de barreau. Une exécution où l'échelle ne se pose pas
// est DISQUALIFIÉE — mélanger deux régimes a rendu quatre exécutions de D6
// incomparables.
const QUIET_S = Number(process.env.QUIET_S ?? 12);
const ETABLISSEMENT_MAX_S = Number(process.env.ETABLISSEMENT_MAX_S ?? 180);
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const SORTIE = process.env.SORTIE ?? `${D11}/${ETIQUETTE}.json`;

/** Combien de changements de barreau le journal porte à cet instant. */
function changementsBarreau() {
    const r = spawnSync('bash', ['-c',
        "sed 's/\\x1b\\[[0-9;]*m//g' /media/vm/dev/agent.log 2>/dev/null | grep -ac \"taille d'encodage changée\""],
        { encoding: 'utf8' });
    return Number((r.stdout ?? '0').trim()) || 0;
}

const AMORCE = `
  window.__pc = null;
  const N = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;
`;
const EXPR_STATS = `(async () => {
  const pc = window.__pc;
  if (!pc) return { erreur: 'aucune RTCPeerConnection' };
  const t = [...(await pc.getStats()).values()];
  const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
  if (!v) return { erreur: 'aucun inbound-rtp video' };
  return {
    framesDecoded: v.framesDecoded, framesDropped: v.framesDropped,
    framesReceived: v.framesReceived, packetsLost: v.packetsLost,
    bytesReceived: v.bytesReceived, frameWidth: v.frameWidth, frameHeight: v.frameHeight,
    totalDecodeTime: v.totalDecodeTime, horodatage_ms: Date.now(),
  };
})()`;

const port = Number(process.env.PORT_CDP ?? 9450);
const dir = await mkdtemp(join(tmpdir(), `cout-d11-${ETIQUETTE}-`));
const chrome = lancerChrome(port, dir, [
    '--remote-allow-origins=*', '--disable-dev-shm-usage', '--disable-gpu',
    '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
    '--disable-features=WebRtcHideLocalIpsWithMdns', 'about:blank',
]);
const releve = {
    etiquette: ETIQUETTE, mode_sortie_pose: MODE_SORTIE || null, n_fenetres: N,
    budget_bps: BUDGET_BPS, palier_s: PALIER_S, ecart_s: ECART_S,
    virsh_debut: virshEtat(), loadavg_debut: loadavg(), echantillons: [],
};
try {
    const cdp = new Cdp((await attendreDevtools(port)).webSocketDebuggerUrl);
    const pages = new Map();
    cdp.on(async (m) => {
        if (m.method === 'Target.targetInfoChanged') {
            for (const [, p] of pages) if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
            return;
        }
        if (m.method === 'Target.detachedFromTarget') { pages.delete(m.params.sessionId); return; }
        if (m.method !== 'Target.attachedToTarget') return;
        const { sessionId: sid, targetInfo: ti } = m.params;
        if (ti.type !== 'page') { await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { }); return; }
        pages.set(sid, { targetId: ti.targetId, url: ti.url });
        await cdp.send('Page.enable', {}, sid).catch(() => { });
        await cdp.send('Runtime.enable', {}, sid).catch(() => { });
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: AMORCE }, sid).catch(() => { });
        await cdp.send('Runtime.evaluate', { expression: AMORCE }, sid).catch(() => { });
        if (!ti.url.includes('shell.html') && ti.url !== 'about:blank') {
            await cdp.send('Emulation.setDeviceMetricsOverride',
                { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false }, sid).catch(() => { });
        }
        await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid).catch(() => { });
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await cdp.send('Target.setDiscoverTargets', { discover: true });

    log(`>>> ÉTAPE 0 : préparation VM — bras « ${ETIQUETTE} », mode posé « ${MODE_SORTIE || 'aucun' } »`);
    releve.preparation = preparerVm(ETIQUETTE);
    // ⚠️ LA PURGE VIENT AVANT LA POSE, et c'est un défaut trouvé PAR
    // L'EXÉCUTION : `preparerVm` tue les processus, mais les sorties
    // virtuelles SURVIVENT à un `Stop-Process -Force` (CLAUDE.md, D5). La
    // première tentative du bras SALE a donc lancé la sonde de pose sur un
    // vivier DÉJÀ PLEIN (dix orphelines de la recette ④) : elle n'a pas pu
    // créer sa sortie, s'est arrêtée en 19 lignes SANS VERDICT, et le bras
    // n'était pas posé — sans qu'aucune erreur ne le dise au pilote.
    log('>>> purge PRÉALABLE des sorties virtuelles (lancement à elle seule)');
    sonde('purge-prealable', { MULTIFENETRE_VDD_PURGE: '1' });
    await dodo(20000);
    tuerAgent();
    if (MODE_SORTIE) {
        log(`>>> pose du registre : MULTIFENETRE_MODE_SORTIE=${MODE_SORTIE} (lancement à elle seule)`);
        sonde('mode-sortie', { MULTIFENETRE_MODE_SORTIE: MODE_SORTIE });
        await dodo(45000);
        spawnSync('bash', ['-c',
            `cp /media/vm/dev/agent.log ${D11}/agent-${ETIQUETTE}-pose.log && `
            + `sed 's/\\x1b\\[[0-9;]*m//g' ${D11}/agent-${ETIQUETTE}-pose.log > ${D11}/agent-${ETIQUETTE}-pose-plat.log`]);
        log('journal de pose copié');
        tuerAgent();
    }
    log('>>> purge des sorties virtuelles (lancement à elle seule)');
    sonde('purge', { MULTIFENETRE_VDD_PURGE: '1' });
    await dodo(20000);
    tuerAgent();

    await cdp.send('Target.createTarget', { url: URL_SHELL });
    await dodo(4000);
    const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
    if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
    log('statut shell :', await cdp.evalBorne(sidShell, "document.querySelector('#statut').textContent", 8000, false));

    log('>>> lancement du superviseur');
    lancerSuperviseur({ BUDGET_BPS });
    await dodo(8000);

    for (let i = 1; i <= N; i += 1) {
        log(`>>> OUVERTURE fenêtre ${i}/${N} (mire n=${i})`);
        ouvrirFenetre(ETIQUETTE, i, i);
        await dodo(DELAI_ENTRE_FENETRES_MS);
    }

    // ---- Établissement : QUIET_S secondes sans aucun changement de barreau.
    log(`>>> ÉTABLISSEMENT : ${QUIET_S} s consécutives sans changement de barreau (max ${ETABLISSEMENT_MAX_S} s)`);
    let dernier = changementsBarreau();
    let calme = 0;
    const t0 = Date.now();
    while (calme < QUIET_S && (Date.now() - t0) / 1000 < ETABLISSEMENT_MAX_S) {
        await dodo(3000);
        const c = changementsBarreau();
        if (c === dernier) { calme += 3; } else { calme = 0; dernier = c; }
    }
    releve.etablissement = {
        calme_s: calme, attente_s: Math.round((Date.now() - t0) / 1000),
        changements_barreau_cumules: dernier,
        atteint: calme >= QUIET_S,
    };
    log('établissement :', JSON.stringify(releve.etablissement));
    if (!releve.etablissement.atteint) log('!! ÉCHELLE NON POSÉE — exécution DISQUALIFIÉE (deux régimes mélangés)');

    // ---- Le palier. Les deux bornes sont ce que l'analyse lira.
    releve.palier_debut = new Date().toISOString();
    releve.loadavg_palier_debut = loadavg();
    log(`>>> PALIER ${PALIER_S} s (les ${ECART_S} premières écartées) — début ${releve.palier_debut}`);
    for (const borne of ['debut', 'fin']) {
        if (borne === 'fin') await dodo(PALIER_S * 1000);
        else await dodo(ECART_S * 1000);
        for (const [sid, p] of pages) {
            if (/shell/.test(p.url ?? '') || p.url === 'about:blank') continue;
            const s = await cdp.evalBorne(sid, EXPR_STATS, 9000, true);
            releve.echantillons.push({ borne, t: new Date().toISOString(), sessionId: sid.slice(0, 8), ...s });
        }
        if (borne === 'debut') {
            releve.utile_debut = new Date().toISOString();
            log(`>>> FENÊTRE UTILE ouverte à ${releve.utile_debut}`);
        }
    }
    releve.utile_fin = new Date().toISOString();
    releve.loadavg_palier_fin = loadavg();
    log(`>>> FENÊTRE UTILE fermée à ${releve.utile_fin}`);
    releve.marqueurs_journal = marqueurs('fin du palier');
} finally {
    releve.loadavg_fin = loadavg();
    releve.virsh_fin = virshEtat();
    await writeFile(SORTIE, JSON.stringify(releve, null, 1));
    log('relevé écrit dans', SORTIE);
    chrome.kill('SIGKILL');
    await dodo(500);
    await rm(dir, { recursive: true, force: true }).catch(() => { });
    await dodo(8000);
    log('arrêt de l’agent :', tuerAgent().replace(/\s+/g, ' ').trim().slice(-40));
    copierLog(ETIQUETTE);
}
