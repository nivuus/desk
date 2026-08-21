// Le banc S1 / S1bis du bloc E3 : « l'AEC de Chrome annule-t-elle ce qu'une
// AUTRE fenêtre restitue ? »
//
//     node banc-aec.mjs <s1|s1bis> <fichier-json>
//
// 🔴 C'EST UN BANC DE NAVIGATEUR : NI VM, NI AGENT, NI CÂBLE VIRTUEL
// (Décision 4). La question ne fait intervenir aucune pièce de ce dépôt — deux
// pages, une sortie, une entrée, un `AnalyserNode`. Y mêler l'agent ajouterait
// quatre maillons entre la question et sa réponse, dont trois ont chacun leur
// mode de panne documenté. ⚠️ **Le prix est nommé : ce banc mesure CHROME, pas
// LE PRODUIT.**
//
// 🔴 ET IL EST UNILATÉRAL (Décision 3, écrite deux fois dans le plan) : une
// salle émulée n'est PAS une pièce — pas de réponse de salle, pas de retard de
// propagation, **pas de distorsion non linéaire de haut-parleur**, et c'est
// précisément la non-linéarité qui met une annulation d'écho en défaut.
// **Issue A ⇒ le défaut est établi. Issue B ⇒ RIEN n'est levé.**
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath } from 'node:url';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { Cdp, attendreDevtools, dodo }
    from '../../journaux-pont-fichiers/instrument/commun-f1.mjs';

const execFileAsync = promisify(execFile);
const ICI = path.dirname(fileURLToPath(import.meta.url));
const MODE = process.argv[2] ?? 's1';
const SORTIE = process.argv[3] ?? `/tmp/e3/banc-${MODE}.json`;
const PORT_HTTP = Number(process.env.PORT_HTTP ?? 5391);
const PORT_CDP = Number(process.env.PORT_CDP ?? 9481);
const AFFICHAGE = process.env.DISPLAY ?? '';

// 🔴 `PULSE_SERVER` EXPLICITE, ET C'EST CE QUI DÉBLOQUE TOUT LE BANC.
//
// Chrome et `pactl` tournent en ROOT ici, et la bibliothèque PulseAudio REFUSE
// de se connecter quand `XDG_RUNTIME_DIR` ne lui appartient pas : « XDG_RUNTIME_DIR
// (/run/user/1000) is not owned by us (uid 0), but by uid 1000! […] Connection
// refused ». **Le symptôme côté Chrome n'y ressemble en rien** : il retombe sur
// son dorsal ALSA, `enumerateDevices()` rend des noms de cartes brutes
// (« HDA NVidia, HDMI 0-Hardware device… »), **`audioinput` est VIDE**, et
// `getUserMedia` échoue en `NotFoundError: Requested device not found` — ce qui
// se lit comme « la salle n'existe pas » alors qu'elle existe.
//
// Nommer la socket directement contourne l'heuristique du répertoire
// d'exécution. **Mesuré** : `pactl info` passe, et la salle apparaît comme sink
// ET comme source PulseAudio.
const ENV = { ...process.env, XDG_RUNTIME_DIR: '/run/user/1000',
    PULSE_SERVER: 'unix:/run/user/1000/pulse/native' };
const TON_A = 440, TON_B = 660;

const journal = [];
const dire = (m) => { const l = `[${new Date().toISOString()}] ${m}`; journal.push(l); console.log(l); };
const r = { mode: MODE, affichage: AFFICHAGE || '<sans interface>', journal, etapes: [], erreurs: [] };

// 🔴 LE JUGE EXTERNE : `pw-record` sur le MONITEUR de la salle, puis la
// fréquence dominante. Il est indépendant de la page — sans lui, « Chrome ne
// rend rien » et « la page ne joue rien » se liraient pareil.
async function enregistrerSalle(etiquette, secondes) {
    const wav = `/tmp/e3/salle-${MODE}-${etiquette}.wav`;
    fs.rmSync(wav, { force: true });
    const p = spawn('pw-record', ['--target', 'salle_e3', '--rate', '48000', '--channels', '1',
        '--format', 's16', wav], { env: ENV, stdio: 'ignore' });
    await dodo(secondes * 1000);
    p.kill('SIGINT');
    await dodo(600);
    try {
        const { stdout } = await execFileAsync('node', [path.join(ICI, 'dominante.mjs'), wav], { encoding: 'utf8' });
        return JSON.parse(stdout);
    } catch (e) { return { erreur: String(e).slice(0, 200) }; }
}

let serveur, chrome;
try {
    // ── Le serveur, sur 127.0.0.1 : `getUserMedia` n'existe pas ailleurs.
    const page = fs.readFileSync(path.join(ICI, 'banc-aec.html'), 'utf8');
    serveur = http.createServer((_, res) => {
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); res.end(page);
    });
    await new Promise((ok) => serveur.listen(PORT_HTTP, '127.0.0.1', ok));
    dire(`page servie sur http://127.0.0.1:${PORT_HTTP}`);

    // ── Chrome. ⚠️ `--no-sandbox` est OBLIGATOIRE (il tourne en root), et son
    // absence ne se voit que sur SON stderr : le pilote, lui, ne verrait que
    // « CDP injoignable ». `XDG_RUNTIME_DIR` le raccorde au PipeWire de
    // l'utilisateur 1000 — sans lui, il n'a aucun serveur audio.
    const drapeaux = [
        `--remote-debugging-port=${PORT_CDP}`, `--user-data-dir=/tmp/e3/udd-banc-${MODE}`,
        '--no-first-run', '--no-default-browser-check', '--no-sandbox',
        '--disable-dev-shm-usage', '--remote-allow-origins=*', '--disable-popup-blocking',
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', '--autoplay-policy=no-user-gesture-required',
        '--use-fake-ui-for-media-stream',
    ];
    // Sans interface, Chrome n'ouvre AUCUN flux de rendu — c'est la première
    // cause d'échec attendue par le plan. Avec `DISPLAY`, c'est un Chrome
    // ordinaire sous Xvfb, qui rend son son comme n'importe lequel.
    if (!AFFICHAGE) drapeaux.unshift('--headless=new');
    fs.rmSync(`/tmp/e3/udd-banc-${MODE}`, { recursive: true, force: true });
    chrome = spawn('/usr/bin/google-chrome', drapeaux,
        { env: ENV, stdio: 'ignore' });
    const ver = await attendreDevtools(PORT_CDP);
    dire(`chrome : ${ver.Browser} (${AFFICHAGE ? 'sous ' + AFFICHAGE : 'sans interface'})`);
    r.chrome = ver.Browser;

    const cdp = new Cdp(ver.webSocketDebuggerUrl);
    const sessions = new Map();
    cdp.on(async (m) => {
        if (m.method !== 'Target.attachedToTarget') return;
        sessions.set(m.params.targetInfo.targetId, m.params.sessionId);
        try { await cdp.send('Runtime.enable', {}, m.params.sessionId); } catch (e) { /* partie */ }
        try { await cdp.send('Runtime.runIfWaitingForDebugger', {}, m.params.sessionId); } catch (e) { /* idem */ }
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });

    const ouvrir = async (nom) => {
        const c = await cdp.send('Target.createTarget', { url: `http://127.0.0.1:${PORT_HTTP}/` });
        let sid;
        for (let i = 0; i < 80; i += 1) { sid = sessions.get(c.targetId); if (sid) break; await dodo(100); }
        if (!sid) throw new Error(`aucune session CDP pour ${nom}`);
        await dodo(1500);
        return { nom, sid };
    };
    const ev = (f, e) => cdp.evalBorne(f.sid, e, 15000, true);

    const A = await ouvrir('A');
    dire('fenêtre A ouverte');

    // ── STEP 2 : Chrome voit-il la salle ?
    const peri = await ev(A, 'window.__peripheriques()');
    dire(`périphériques : ${String(peri).slice(0, 400)}`);
    r.peripheriques = peri;
    r.etapes.push({ step: 2, voit_la_salle: String(peri).includes('Salle E3') || String(peri).includes('audioinput') });

    // ── STEP 3 : Chrome REND-il du son dans la salle ?
    const jA = await ev(A, `window.__jouer(${TON_A})`);
    dire(`A joue : ${jA}`);
    await dodo(1500);
    const rendu = await enregistrerSalle('rendu', 4);
    dire(`juge de rendu : ${JSON.stringify(rendu)}`);
    r.etapes.push({ step: 3, juge: rendu, attendu_hz: TON_A });
    const rendOk = rendu && Math.abs((rendu.dominante_hz ?? 0) - TON_A) < 15 && (rendu.db ?? -1000) > -60;
    r.rend_dans_la_salle = !!rendOk;
    if (!rendOk) {
        dire('🔴 S1 NON MESURABLE : Chrome ne rend aucun son dans la salle. C\'est une ISSUE, pas un échec.');
        r.verdict = 'NON MESURABLE — aucun rendu dans la salle';
        throw new Error('pas-de-rendu');
    }

    // ── STEP 4 : LE TÉMOIN POSITIF, et il passe AVANT toute conclusion.
    // Une SEULE fenêtre : elle joue SA tonalité dans la salle ET capte la
    // salle avec `echoCancellation: true`. Son analyseur doit NE PAS la
    // retrouver. 🔴 Si l'AEC n'annule pas même sa PROPRE restitution, le banc
    // ne mesure rien, et l'issue C est la seule lisible.
    // 🔴 LE BANC EST DIFFÉRENTIEL, et il a fallu une mesure pour le comprendre.
    // Le résidu ne se juge PAS contre le plancher : la salle est du silence
    // NUMÉRIQUE (plancher mesuré à −151,5 dB), et une AEC qui retire 60 dB y
    // laisse encore un résidu 55 dB au-dessus. **Ce qui se juge est l'ÉCART
    // entre `aec:false` et `aec:true` sur LE MÊME son.**
    const mesurer = async (f, aec, attente = 4000) => {
        const c = await ev(f, `window.__capter(${aec})`);
        await dodo(attente);
        const sp = await ev(f, `window.__spectre([${TON_A}, ${TON_B}])`);
        return { aec, capture: c, spectre: sp, s: JSON.parse(String(sp)) };
    };

    const sansAec = await mesurer(A, false);
    dire(`A capte SANS aec  : ${sansAec.spectre}`);
    const avecAec = await mesurer(A, true);
    dire(`A capte AVEC aec  : ${avecAec.spectre}`);
    r.capture_A = { sans_aec: sansAec.capture, avec_aec: avecAec.capture };
    const profondeur = Number((sansAec.s.aux[TON_A] - avecAec.s.aux[TON_A]).toFixed(1));
    dire(`TÉMOIN POSITIF — l'AEC retire ${profondeur} dB de la PROPRE restitution de A ` +
         `(${sansAec.s.aux[TON_A]} → ${avecAec.s.aux[TON_A]} dB)`);
    r.etapes.push({ step: 4, sans_aec: sansAec.spectre, avec_aec: avecAec.spectre, profondeur_db: profondeur });
    r.profondeur_aec_db = profondeur;
    // ⚠️ 20 dB est un SEUIL DE BANC, jamais une constante calibrée : il sépare
    // « l'AEC agit » de « l'AEC ne fait rien », et rien de plus fin.
    r.aec_annule_sa_propre_restitution = profondeur >= 20;
    if (!r.aec_annule_sa_propre_restitution) {
        dire("🔴 ISSUE C : l'AEC n'annule pas même la PROPRE restitution de A. " +
             'Le banc ne mesure rien, et on le déclare.');
        r.issue = 'C';
        r.verdict = "NON MESURABLE — l'AEC n'agit pas du tout sur ce montage";
    }
    r.reference_sans_aec = sansAec.s;

    if (MODE === 's1ter') {
        // 🔴 LA VARIANTE QUI DISCRIMINE, et elle n'était pas au plan.
        //
        // S1bis met les deux fenêtres dans LA MÊME instance de Chrome — ce qui
        // est **exactement le montage du produit**, la page-shell ouvrant ses N
        // fenêtres par `window.open`. Si l'AEC y couvre l'autre fenêtre, reste
        // à savoir POURQUOI : parce que les deux partagent le flux de rendu du
        // MÊME navigateur, ou parce que l'AEC couvre le PÉRIPHÉRIQUE entier ?
        //
        // **Les deux réponses n'ont pas les mêmes conséquences produit.** Si
        // c'est « même navigateur », le son d'une AUTRE application — un
        // lecteur, une autre visioconférence — resterait, lui, non annulé.
        // Cette variante joue B depuis une SECONDE instance de Chrome, donc un
        // autre processus, sur la même salle.
        const PORT2 = PORT_CDP + 1;
        fs.rmSync('/tmp/e3/udd-banc-s1ter-B', { recursive: true, force: true });
        const drapeaux2 = drapeaux.map((d) => d.startsWith('--remote-debugging-port') ? `--remote-debugging-port=${PORT2}`
            : (d.startsWith('--user-data-dir') ? '--user-data-dir=/tmp/e3/udd-banc-s1ter-B' : d));
        const chrome2 = spawn('/usr/bin/google-chrome', drapeaux2, { env: ENV, stdio: 'ignore' });
        try {
            const ver2 = await attendreDevtools(PORT2);
            dire(`chrome n°2 : ${ver2.Browser}`);
            const cdp2 = new Cdp(ver2.webSocketDebuggerUrl);
            const s2 = new Map();
            cdp2.on(async (m) => {
                if (m.method !== 'Target.attachedToTarget') return;
                s2.set(m.params.targetInfo.targetId, m.params.sessionId);
                try { await cdp2.send('Runtime.enable', {}, m.params.sessionId); } catch (e) { /* partie */ }
                try { await cdp2.send('Runtime.runIfWaitingForDebugger', {}, m.params.sessionId); } catch (e) { /* idem */ }
            });
            await cdp2.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
            const c2 = await cdp2.send('Target.createTarget', { url: `http://127.0.0.1:${PORT_HTTP}/` });
            let sid2;
            for (let i = 0; i < 80; i += 1) { sid2 = s2.get(c2.targetId); if (sid2) break; await dodo(100); }
            if (!sid2) throw new Error('aucune session CDP pour B (instance 2)');
            await dodo(1500);
            const jB2 = await cdp2.evalBorne(sid2, `window.__jouer(${TON_B})`, 15000, true);
            dire(`B (instance 2) joue : ${jB2}`);
            await dodo(2000);
            const dansSalle = await enregistrerSalle('deux-instances', 4);
            dire(`juge de la salle, deux instances : ${JSON.stringify(dansSalle)}`);
            r.etapes.push({ step: 'B-instance-2', juge: dansSalle });
            const ref2 = await mesurer(A, false);
            dire(`A, SANS aec, B en instance 2 : ${ref2.spectre}`);
            const aec2 = await mesurer(A, true);
            dire(`A, AVEC aec, B en instance 2 : ${aec2.spectre}`);
            const p2A = Number((ref2.s.aux[TON_A] - aec2.s.aux[TON_A]).toFixed(1));
            const p2B = Number((ref2.s.aux[TON_B] - aec2.s.aux[TON_B]).toFixed(1));
            r.profondeurs_deux_instances = { [TON_A]: p2A, [TON_B]: p2B };
            r.b_presente_sans_aec = (ref2.s.aux[TON_B] - ref2.s.plancher_db) >= 12;
            r.issue = p2A >= 20 && p2B < 20 ? 'A' : (p2A >= 20 && p2B >= 20 ? 'B' : 'C');
            dire(`ISSUE ${r.issue} (DEUX INSTANCES) — A=${p2A} dB, B=${p2B} dB`);
        } finally { try { chrome2.kill(); } catch (e) { /* mort */ } }
    }

    if (MODE === 's1bis') {
        // ── S1bis : B joue SA tonalité, A capte toujours la salle.
        const B = await ouvrir('B');
        dire('fenêtre B ouverte');
        const jB = await ev(B, `window.__jouer(${TON_B})`);
        dire(`B joue : ${jB}`);
        await dodo(1500);
        const deux = await enregistrerSalle('deux-tons', 4);
        dire(`juge de la salle, deux tons : ${JSON.stringify(deux)}`);
        r.etapes.push({ step: 'B-dans-la-salle', juge: deux });
        await dodo(4000);
        // 🔴 LES DEUX BRAS, SUR LA MÊME SCÈNE : `aec:false` donne la
        // RÉFÉRENCE de ce que la salle porte réellement, `aec:true` ce que
        // l'AEC en laisse. **La profondeur d'annulation, ton par ton, EST la
        // mesure** — et elle rend l'issue sans qu'aucun seuil de plancher
        // n'intervienne.
        const refDeux = await mesurer(A, false);
        dire(`A, deux tons, SANS aec : ${refDeux.spectre}`);
        const aecDeux = await mesurer(A, true);
        dire(`A, deux tons, AVEC aec : ${aecDeux.spectre}`);
        r.spectre_A_avec_B = { sans_aec: refDeux.spectre, avec_aec: aecDeux.spectre };
        const pA = Number((refDeux.s.aux[TON_A] - aecDeux.s.aux[TON_A]).toFixed(1));
        const pB = Number((refDeux.s.aux[TON_B] - aecDeux.s.aux[TON_B]).toFixed(1));
        r.profondeurs = { [TON_A]: pA, [TON_B]: pB };
        // A est le son de la fenêtre QUI CAPTE ; B celui de l'AUTRE fenêtre.
        const aAnnule = pA >= 20, bAnnule = pB >= 20;
        r.issue = aAnnule && !bAnnule ? 'A' : (aAnnule && bAnnule ? 'B' : (!aAnnule ? 'C' : 'INATTENDUE'));
        dire(`ISSUE ${r.issue} — profondeur d'annulation : A(sa propre voix)=${pA} dB, ` +
             `B(l'autre fenêtre)=${pB} dB`);
        // ⚠️ Le témoin de PRÉSENCE : B est-elle seulement arrivée dans la salle ?
        r.b_presente_sans_aec = (refDeux.s.aux[TON_B] - refDeux.s.plancher_db) >= 12;

        // ── Le BRAS « AVEC CASQUE » : le témoin NÉGATIF du banc. B cesse de
        // rendre dans la salle ; le spectre capté par A doit perdre 660 Hz.
        // 🔴 Sans lui, « l'AEC annule B » et « B n'est JAMAIS arrivée dans la
        // salle » se lisent pareil.
        await ev(B, 'window.__arreter()');
        await dodo(2000);
        const casque = await mesurer(A, false);
        dire(`BRAS CASQUE (B ne rend plus, SANS aec) : ${casque.spectre}`);
        r.bras_casque = casque.spectre;
        // 🔴 Le témoin NÉGATIF du banc : sans lui, « l'AEC annule B » et « B
        // n'est JAMAIS arrivée dans la salle » se lisent pareil.
        // ⚠️ **DIFFÉRENTIEL ICI AUSSI, et c'est la MÊME leçon deux fois.** Un
        // seuil sur le plancher rendait `casque_b_absent = false` alors que
        // 660 Hz était passé de −42,7 à −156,8 dB : le plancher de cette salle
        // est du silence NUMÉRIQUE, et 16 dB au-dessus de lui reste 114 dB
        // SOUS le signal. **Ce qui juge est la chute, jamais la hauteur.**
        const chute = Number((refDeux.s.aux[TON_B] - casque.s.aux[TON_B]).toFixed(1));
        r.casque_chute_b_db = chute;
        r.casque_b_absent = chute >= 20;
        r.casque_detachement_b = Number((casque.s.aux[TON_B] - casque.s.plancher_db).toFixed(1));
        dire(`BRAS CASQUE — 660 Hz chute de ${chute} dB quand B cesse de rendre ` +
             `(${refDeux.s.aux[TON_B]} → ${casque.s.aux[TON_B]} dB)`);
    }
    dire('mesure terminée');
} catch (e) {
    if (String(e).includes('pas-de-rendu')) { /* déjà déclaré */ }
    else { r.erreurs.push(String(e?.stack ?? e).slice(0, 1500)); dire(`ERREUR : ${String(e).slice(0, 300)}`); }
} finally {
    try { chrome?.kill(); } catch (e) { /* mort */ }
    try { serveur?.close(); } catch (e) { /* fermé */ }
    fs.mkdirSync(path.dirname(SORTIE), { recursive: true });
    fs.writeFileSync(SORTIE, JSON.stringify(r, null, 2));
    dire(`sortie : ${SORTIE}`);
    process.exit(0);
}
