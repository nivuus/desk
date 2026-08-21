#!/usr/bin/env node
// Sous-bloc P3 (presse-papier) — SONDE S1 : la MESURABILITÉ du focus à N
// fenêtres, hors VM, hors agent, hors session WebRTC.
//
// 🔴 CE TÉMOIN PEUT ÉCHOUER SUR TROIS DE SES QUATRE ISSUES, et c'est ce qui en
// fait un témoin. Le témoin de mesurabilité de P1 a échoué sur sa troisième
// branche ; celui-ci a les siennes écrites d'avance dans le plan (tâche 1).
//
// Ce qu'il établit, ou réfute : un Chrome `--headless=new` peut-il faire
// qu'EXACTEMENT UNE fenêtre sur trois rapporte `document.hasFocus() === true`,
// et est-ce bien celle qu'on a amenée au premier plan ? **Sans cela, le
// critère ④ de P3 n'est pas mesurable, et il faut le dire AVANT de le tenter.**
//
// Les trois fenêtres sont ouvertes par `window.open`, comme le fait
// `client/src/shell-page.ts` en production — jamais par `Target.createTarget`,
// qui n'est pas le geste du produit.
//
// 🔴 `Emulation.setFocusEmulationEnabled` N'EST JAMAIS APPELÉE, et le JSON de
// sortie le déclare (E9 du plan). Cette commande CDP existe précisément pour
// faire croire à une page qu'elle a le focus : activée, `document.hasFocus()`
// vaudrait `true` partout et le critère ④ ne pourrait plus échouer — vacueux
// EN SILENCE.
//
// Usage : node sonde-p3-focus.mjs --etiquette=1

import { createServer } from 'node:http';
import { writeFileSync } from 'node:fs';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp, attendreDevtools, dodo, lancerChrome } from
    '../../journaux-multifenetres-d11/instrument/commun-d11.mjs';

const arg = (n, d) => (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const ETIQUETTE = arg('etiquette', '1');
const SORTIE = arg('sortie', `p3-focus-${ETIQUETTE}.json`);
const PORT_HTTP = Number(arg('port-http', '45893'));
const PORT_CDP = Number(arg('port-cdp', '9573'));
const N = 3;

const journal = [];
const dire = (...m) => { const l = m.join(' '); journal.push(l); console.log(l); };

// La page-mère ouvre ses filles par `window.open`, comme `shell-page.ts:117`.
const MERE = `<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>mere</title></head>
<body><h1>mere</h1></body></html>`;
const FILLE = (n) => `<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>${n}</title></head>
<body><h1>fenetre ${n}</h1><video id="remote"></video></body></html>`;

// ⚠️ Les deux armes emploient deux préfixes (`w` et `t`) pour que l'inventaire
// les distingue. Une première rédaction ne reconnaissait que `/f\d+` et servait
// donc la page MÈRE aux six filles : la mesure de `document.hasFocus()` n'en
// était pas faussée — une page est une page —, mais l'instrument disait
// « fenetre w0 » d'une page qui n'en était pas une. Corrigé plutôt que laissé.
const srv = createServer((q, r) => {
    const m = /^\/([wt]\d+)$/.exec(q.url ?? '');
    r.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    r.end(m ? FILLE(m[1]) : MERE);
});
await new Promise((r) => srv.listen(PORT_HTTP, '127.0.0.1', r));

const dir = await mkdtemp(join(tmpdir(), 'p3-focus-'));
const chrome = lancerChrome(PORT_CDP, dir);
const version = await attendreDevtools(PORT_CDP);
const cdp = new Cdp(version.webSocketDebuggerUrl);
dire('navigateur', version.Browser);

const triplet = async (s) => JSON.parse(await cdp.eval(s.sessionId,
    `JSON.stringify({hasFocus:document.hasFocus(),isActive:navigator.userActivation.isActive,hasBeenActive:navigator.userActivation.hasBeenActive,visible:!document.hidden})`));

// -------------------------------------------------------------------------
// DEUX ARMES, ET C'EST LE POINT DE CETTE SONDE.
// -------------------------------------------------------------------------
// 🔴 Une première rédaction n'avait que l'arme `window.open` — le geste du
// PRODUIT (`client/src/shell-page.ts:117`) — et concluait « --headless ne
// distingue pas le focus ». La sonde S2, qui ouvre ses pages par
// `Target.createTarget`, a relevé l'INVERSE dans la même heure : `bringToFront`
// y retire bien le focus. **Les deux ne peuvent pas décrire la même cause**,
// et publier la première conclusion aurait attribué au « --headless » ce qui
// tient peut-être au mode d'ouverture. D'où les deux armes, dans la MÊME
// exécution, avec le même code de mesure.
//
// ⚠️ Ce que cela change pour la recette : le produit ouvre ses fenêtres par
// `window.open`. Si c'est l'arme `window.open` qui échoue, ④ reste non
// mesurable EN CONDITIONS DE PRODUIT, quelle que soit la réponse de l'autre.
async function campagne(arme, ouvrirLesTrois, prefixe) {
    const filles = await ouvrirLesTrois(prefixe);
    dire(`[${arme}] cibles trouvees : ${filles.length} / ${N} — ${filles.map((t) => t.url).join(' ')}`);
    if (filles.length !== N) {
        dire(`🔴 [${arme}] INVENTAIRE INCOMPLET — cette arme ne peut rien conclure.`);
        return { arme, n_trouve: filles.length, basculements: [], issue: 'INDECIDABLE — inventaire incomplet' };
    }
    const sessions = [];
    for (const t of filles) {
        const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: t.targetId, flatten: true });
        await cdp.send('Runtime.enable', {}, sessionId);
        sessions.push({ url: t.url, targetId: t.targetId, sessionId });
    }
    const basculements = [];
    for (let i = 0; i < sessions.length; i += 1) {
        await cdp.send('Page.bringToFront', {}, sessions[i].sessionId);
        await dodo(500);
        const releves = [];
        for (const s of sessions) releves.push({ url: s.url, ...(await triplet(s)) });
        const focalisees = releves.filter((r) => r.hasFocus).map((r) => r.url);
        basculements.push({ amenee: sessions[i].url, releves, focalisees });
        dire(`[${arme}] bringToFront(${sessions[i].url}) →`,
            releves.map((r) => `${r.url}:${r.hasFocus ? 'FOCUS' : '—'}`).join(' '));
    }
    return { arme, n_trouve: filles.length, basculements, issue: issueDe(basculements) };
}

/// Les quatre issues du plan (tâche 1), calculées sur le RELEVÉ.
function issueDe(basculements) {
    if (basculements.every((b) => b.focalisees.length === 1 && b.focalisees[0] === b.amenee)) {
        return 'MESURABLE — exactement un true a chaque basculement, et c\'est bien la fenetre amenee au premier plan';
    }
    if (basculements.every((b) => b.focalisees.length === N)) {
        return 'NON MESURABLE — true PARTOUT';
    }
    if (basculements.every((b) => b.focalisees.length === 0)) {
        return 'NON MESURABLE, ET PIRE — false partout : le produit lui-meme n\'ecrirait jamais rien localement';
    }
    if (basculements.every((b) => b.focalisees.length === 1)) {
        return 'MESURABLE mais TROMPEUR — un seul true, mais pas toujours sur la fenetre amenee au premier plan';
    }
    return 'MIXTE — le compte de fenetres focalisees varie d\'un basculement a l\'autre';
}

/// 🔴 L'INVENTAIRE PASSE PAR `Target.getTargets`, PAS PAR LA SEULE
/// AUTO-ATTACHE : P1 a perdu une exécution entière parce que l'auto-attache ne
/// suffit pas, et une cible ouverte par `window.open` s'attache avec une URL
/// VIDE — l'URL n'arrive qu'au `targetInfoChanged` suivant.
async function inventorier(prefixe) {
    const { targetInfos } = await cdp.send('Target.getTargets');
    return targetInfos
        .filter((t) => t.type === 'page' && new RegExp(`/${prefixe}\\d+$`).test(t.url))
        .sort((a, b) => a.url.localeCompare(b.url));
}

/// Ouvre N filles depuis une page mère. `traits` est la CHAÎNE DE
/// CARACTÉRISTIQUES passée en TROISIÈME argument de `window.open`, ou `null`
/// pour n'en passer AUCUN.
///
/// 🔴 **CE TROISIÈME ARGUMENT EST TOUT LE SUJET, ET LA PREMIÈRE RÉDACTION DE
/// CETTE SONDE L'A PASSÉ ALORS QUE LE PRODUIT NE LE PASSE PAS.** Elle appelait
/// `window.open(url, nom, 'width=800,height=600')` ; `client/src/shell-page.ts`
/// appelle `window.open(url, nom)` — **DEUX arguments**. Avec une chaîne de
/// caractéristiques, Chrome ouvre une **POPUP** ; sans, un **ONGLET**. Et c'est
/// exactement ce qui décide : les popups rapportent toutes le focus, les
/// onglets le discriminent.
///
/// ⚠️ **Le verdict « ④ NON MESURABLE en conditions de produit » que cette sonde
/// a d'abord rendu était donc FAUX, et c'est LA RECETTE SUR LA VM qui l'a
/// réfuté** : le focus y discrimine parfaitement, aux deux exécutions. Une
/// sonde qui croit reproduire le geste du produit doit le RELIRE, pas s'en
/// souvenir.
const ouvrirDepuisLaMere = (traits) => async (prefixe) => {
    const { targetId } = await cdp.send('Target.createTarget', { url: `http://127.0.0.1:${PORT_HTTP}/` });
    const { sessionId: sMere } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    await cdp.send('Runtime.enable', {}, sMere);
    await dodo(600);
    for (let i = 0; i < N; i += 1) {
        const args = traits === null
            ? `'/${prefixe}${i}', '${prefixe}${i}'`
            : `'/${prefixe}${i}', '${prefixe}${i}', ${JSON.stringify(traits)}`;
        await cdp.eval(sMere, `window.open(${args}) ? 'ouverte' : 'refusee'`);
        await dodo(400);
    }
    await dodo(600);
    return inventorier(prefixe);
};

// ARME 1 — `window.open(url, nom)` : **LE GESTE EXACT DU PRODUIT**, relu dans
// `client/src/shell-page.ts:117` plutôt que remémoré.
const campagneOuvre = await campagne('window.open(url, nom) — LE PRODUIT', ouvrirDepuisLaMere(null), 'w');

// ARME 1 bis — le MÊME appel, plus une chaîne de caractéristiques. C'est ce que
// la première rédaction prenait pour le geste du produit, et c'est ce qui fait
// la différence entre un ONGLET et une POPUP.
const campagnePopup = await campagne("window.open(url, nom, 'width=…') — une POPUP", ouvrirDepuisLaMere('width=800,height=600'), 'p');

// ARME 2 — `Target.createTarget` : le geste de l'INSTRUMENT, jamais celui du
// produit. Il n'est là que pour dire si l'échec éventuel de l'arme 1 tient au
// `--headless` ou au mode d'ouverture.
const campagneCible = await campagne('Target.createTarget', async (prefixe) => {
    for (let i = 0; i < N; i += 1) {
        await cdp.send('Target.createTarget', { url: `http://127.0.0.1:${PORT_HTTP}/${prefixe}${i}` });
        await dodo(400);
    }
    await dodo(600);
    return inventorier(prefixe);
}, 't');

const filles = { length: campagneOuvre.n_trouve };
const basculements = campagneOuvre.basculements;

// 🔴 LE VERDICT EST CELUI DE L'ARME DU PRODUIT — `window.open` —, parce que
// c'est celle-là que la recette rencontrera. L'autre arme ne sert qu'à
// ATTRIBUER : elle dit si l'échec tient au `--headless` ou au mode d'ouverture.
const verdict = campagneOuvre.issue;
const attribution = 'TROIS ARMES, ET C\'EST LE TROISIEME ARGUMENT DE window.open QUI DECIDE : '
    + 'window.open(url, nom) — LE GESTE DU PRODUIT — « ' + campagneOuvre.issue + ' » ; '
    + "window.open(url, nom, 'width=...') — une POPUP — « " + campagnePopup.issue + ' » ; '
    + 'Target.createTarget — « ' + campagneCible.issue + ' ». '
    + (campagneOuvre.issue === campagnePopup.issue
        ? "La chaine de caracteristiques ne change RIEN : la cause est ailleurs."
        : "🔴 LA CHAINE DE CARACTERISTIQUES CHANGE TOUT — avec elle Chrome ouvre une POPUP, "
          + "sans elle un ONGLET. La premiere redaction de cette sonde la passait alors que "
          + "client/src/shell-page.ts ne la passe PAS, et elle a donc rendu un verdict FAUX "
          + "sur le produit. C'est la RECETTE SUR LA VM qui l'a refute.");

dire('');
dire('=== VERDICT S1 (arme du PRODUIT : window.open(url, nom), DEUX arguments) ===');
dire(verdict);
dire('attribution : ' + attribution);
if (verdict.startsWith('NON MESURABLE')) {
    dire('⟹ ④ NE PEUT PAS ETRE MESURE en conditions de produit. Xvfb + xdotool est la seule voie nommee');
    dire('   (consentement donne en D8, jamais suivi d\'effet ; releve ce jour : Xvfb et xdotool ABSENTS de l\'hote),');
    dire('   et les mesures qui en sortiraient ne se compareraient a AUCUNE campagne anterieure.');
}
dire('emulation de focus : JAMAIS activee (Emulation.setFocusEmulationEnabled n\'est appelee nulle part dans ce fichier)');

writeFileSync(SORTIE, JSON.stringify({
    sonde: 'S1 — mesurabilite du focus a N fenetres',
    etiquette: ETIQUETTE,
    date: new Date().toISOString(),
    navigateur: version.Browser,
    n_demande: N,
    n_trouve: filles.length,
    emulationDeFocusActivee: false,
    armeDuProduit: campagneOuvre,
    armePopup: campagnePopup,
    armeDeControle: campagneCible,
    basculements,
    verdict,
    attribution,
}, null, 2));
dire('releve ecrit : ' + SORTIE);

chrome.kill('SIGKILL'); srv.close(); await dodo(300); process.exit(0);
