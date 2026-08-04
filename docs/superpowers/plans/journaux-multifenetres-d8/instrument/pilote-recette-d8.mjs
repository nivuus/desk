#!/usr/bin/env node
// Sous-bloc D8, tâche 10 — le PILOTE de la recette du plein écran.
//
// Dérivé de `docs/superpowers/plans/journaux-multifenetres-d6/instrument/pilote-recette-d6.mjs`
// (copié, pas modifié en place — les journaux de D6 y renvoient) : structure CDP,
// conventions de journal, `imposerScenario`, `deltas`. L'instrumentation audio par
// fréquence dominante (§ étape 5 du brief) est PORTÉE depuis
// `docs/superpowers/plans/journaux-multifenetres-d7/instrument/pilote-recette-d7.mjs`
// (`expressionReleveFrequence`) — copiée, pas importée, pour que ce fichier reste
// autonome. La page source `ton.html` (D7) est RÉUTILISÉE telle quelle depuis son
// propre répertoire (`TON_HTML_D7` ci-dessous) : c'est un ASSET HTML, pas un pilote,
// et elle porte déjà les deux propriétés dont D8 a besoin — une animation (Desktop
// Duplication n'émet qu'au changement du bureau) et un ton pur à fréquence connue.
//
// CETTE TÂCHE N'EXÉCUTE RIEN SUR LA VM. Elle écrit l'instrument ; la tâche 11 le
// fait tourner et verse les journaux (`critere-{1..5}-*.log`, `temoin-*.log`).
//
// ============================================================================
// DÉCISION ACTÉE EN TÊTE DE TÂCHE — LE CRITÈRE ③ N'EST PAS INSTRUMENTÉ ICI
// ============================================================================
//
// La sonde P2 (tâche 1, `p2-instrument.log`) a établi que Chrome `--headless=new`
// n'entre PAS réellement en plein écran (`document.fullscreenElement` reste `null`
// 800 ms après un `requestFullscreen()` par ailleurs bien invoqué) et n'expose PAS
// `navigator.keyboard` du tout. Le propriétaire du dépôt a tranché : le critère ③
// (« Échap ne casse pas le plein écran et atteint le jeu ») est déclaré NON ÉPROUVÉ,
// sans installation de `Xvfb`. L'étape 4 du cahier des charges de cette tâche
// (réécriture de titre sur `keydown` + relecture `WM_GETTEXT`) est donc ANNULÉE :
// aucune fonction de ce fichier ne s'en approche. Ce pilote joue QUATRE critères
// (①②④⑤) plus le témoin de non-régression.
//
// ============================================================================
// CE QUE LA SUBSTITUTION DU CRITÈRE ① MESURE, ET CE QU'ELLE NE MESURE PAS
// ============================================================================
//
// L'énoncé du critère ① (spec §8) est : « une application Windows passe en plein
// écran, sa fenêtre navigateur y entre au geste suivant, et elle seule ». La
// SECONDE moitié — la fenêtre navigateur qui entre effectivement en plein écran —
// N'EST PAS OBSERVABLE ici, pour la même raison que P2 : `armerPleinEcranAuDOM`
// (`client/src/fullscreen.ts`) attend un geste utilisateur réel (`pointerdown`/
// `keydown`) pour appeler `requestFullscreen()`, et même un tel geste simulé par
// CDP ne ferait qu'échouer silencieusement comme P2 l'a mesuré — aucune information
// nouvelle n'en sortirait.
//
// CE QUI RESTE ENTIÈREMENT MESURABLE, ET QUE `phaseCritere1Et2` MESURE RÉELLEMENT :
//   - côté WINDOWS : `togglerStyleFenetre` force RÉELLEMENT le style de la fenêtre
//     cible (retire `WS_CAPTION`/`WS_THICKFRAME` par `SetWindowLongPtrW`, comme le
//     ferait un vrai jeu passant en plein écran sans bordure) et relit le style
//     obtenu — jamais le code de retour de l'API, doctrine déjà établie par
//     `mode_sortie.rs` (« le verdict est ce que Windows relit, jamais ce que
//     l'appel annonce ») ;
//   - côté AGENT : `pleinEcranAgent` confirme que le fil de fenêtre du capteur a
//     bien LU ce changement de style et ANNONCÉ `PleinEcran { actif: true }`, avec
//     la bonne `session` (extraite du span `fenetre{session=…}` que D7 a posé — il
//     couvre toute la boucle de capture, y compris les traces de `mode_sortie.rs`
//     et `capture.rs` qui n'ont PAS de champ `session` explicite) ;
//   - côté NAVIGATEUR : l'AMORCE intercepte CHAQUE `RTCDataChannel` de label
//     `control` et empile tout message `{"type":"fullscreen",...}` reçu dans
//     `window.__pleinEcran`. On vérifie que SEULE la page correspondant à la
//     fenêtre cible reçoit ce message, et qu'aucune des voisines ne le reçoit —
//     c'est la partie du critère ① (« et elle seule ») qui reste mesurable sans
//     jamais avoir besoin que `requestFullscreen()` aboutisse réellement.
//
// Ce que cette substitution NE mesure PAS, et qu'aucune ligne de ce fichier ne
// prétend mesurer : que la page navigateur passe RÉELLEMENT en plein écran, que
// Keyboard Lock s'arme, et tout ce qui dépend de `document.fullscreenElement`
// devenant non nul. Si `Xvfb` est un jour installé, ces mesures ne se
// compareraient à aucune campagne antérieure (§10 de la spec) — ni à celle-ci ni
// aux précédentes.
//
// ============================================================================
// LE CRITÈRE ② EST INDÉPENDANT DE ①, ET C'EST DÉLIBÉRÉ
// ============================================================================
//
// Le § 6 de la spec est clair : la résolution qui suit un plein écran est câblée
// par le `ResizeObserver` du CLIENT sur l'élément `<video>` (`client/src/main.ts`),
// qui envoie un message `Resize` dès que `video.clientWidth`/`clientHeight`
// changent — QUELLE QUE SOIT LA CAUSE de ce changement, `fullscreenElement` compris
// ou non. Le style Windows (critère ①) et le redimensionnement du viewport
// (critère ②) sont donc deux mécanismes DISJOINTS dans le produit lui-même, reliés
// seulement par l'intention (une vraie fenêtre plein écran ferait les deux à la
// fois). Ce pilote peut donc éprouver ② en forçant directement la taille de
// viewport rapportée par CDP (`Emulation.setDeviceMetricsOverride`, le MÊME
// mécanisme que `VIEWPORT_FORCE` emploie déjà à l'attache de chaque page) sur la
// page cible, SANS dépendre du tout de ce qui bloque ① et ③. C'est une mesure
// honnête du mécanisme du §6, pas un contournement de ce que P2 a réfuté.
//
// ============================================================================
// LES SEPT GARDE-FOUS
// ============================================================================
//
// Les six premiers sont ceux que les recettes précédentes ont payés (étape 2 du
// brief) ; le septième est ajouté par cette tâche.
//
//   1. `--user-data-dir` PAR FENÊTRE (fonction `ouvrirFenetre`) — sinon Chrome
//      rejoint son instance existante et l'on compte des lancements, pas des
//      fenêtres (D4).
//   2. VISIBILITÉ IMPOSÉE PAGE PAR PAGE (`imposerScenario`, `marquerCachee`) — un
//      Chrome sans interface rapporte `document.hidden = true` pour toute fenêtre
//      d'arrière-plan (D5), et `Page.addScriptToEvaluateOnNewDocument` ne court
//      pas sur une page ouverte par `window.open` : l'override est reposé
//      explicitement à chaque page (voir le handler `Target.attachedToTarget`).
//   3. LA CIBLE DE FOCUS PASSE PAR `blur` PUIS `focus` (`imposerScenario`) — la
//      déduplication de `client/src/visibilite.ts` peut sinon faire disparaître
//      le focus entièrement (D6).
//   4. AUCUNE CAPTURE D'ÉCRAN CDP PENDANT UNE MESURE, et toute évaluation CDP sur
//      une page portant un flux WebRTC actif est BORNÉE (`Cdp.evalBorne`,
//      `Promise.race`) — une capture d'écran provoque un `Resize`, donc un
//      `SHOW`, donc une session et une sortie de plus (D1), et `Page.
//      captureScreenshot` peut ne jamais rendre (D2). Ce fichier n'appelle JAMAIS
//      `Page.captureScreenshot`.
//   5. AUCUN PALIER SOUS 60 S (`PALIER_S`, valeur par défaut 60) — `DELAI_
//      REMONTEE` vaut 20 s (`agent/src/congestion/hysteresis.rs`), consignation
//      n°3 de D6, trois mesures y ont été perdues pour un palier trop court.
//   6. ATTENDRE LE FAIT, JAMAIS UNE DURÉE (`attendreBarreauxStables`,
//      `attendreTraceAgent`) — douze secondes sans changement observé avant de
//      mesurer, jamais un `sleep` arbitraire.
//   7. [NEUF, D8] LA SORTIE GARDE SON NOM `\\.\DISPLAYn` À TRAVERS LE CHANGEMENT
//      DE MODE (`sortieDeSession`, comparée avant/après dans `phaseCritere1Et2`)
//      — c'est ce qui prouve un CHANGEMENT de mode et non une RECRÉATION de la
//      sortie (qui, elle, abandonnerait le mutex de TOUTES les voisines, pas
//      seulement le sien).
//
// ============================================================================
// L'INCONNUE QUE LA RECETTE DEVRA TRANCHER, ET QUE CE PILOTE REND OBSERVABLE
// ============================================================================
//
// Le pilote SudoVDA accepte-t-il un changement de mode sur une sortie DONT LA
// DUPLICATION DXGI EST OUVERTE ? La sonde P1 (tâche 3) ne l'a JAMAIS éprouvé :
// elle créait la sortie, changeait le mode, relisait — sans jamais ouvrir de
// duplication dessus. En production la duplication est ouverte pendant toute la
// tentative. `phaseCritere1Et2` VERSE, sans hypothèse préalable :
//   - les lignes `changement de mode de la sortie virtuelle demandé` (issue,
//     code, la taille demandée) — `modeSortieAgent` ;
//   - leur verdict : acceptées (`sortie retaillée : chaîne d'encodage
//     reconstruite…`), refusées (`la sortie n'a pas pris le mode demandé`) ou
//     illisibles (`topologie illisible après le changement de mode`) ;
//   - les pertes d'accès `0x887a0026` sur les sorties VOISINES (pas la cible)
//     PENDANT la fenêtre de la tentative — `pertesAcces`, filtrée par le nom de
//     sortie extrait du champ `cible` (format `Debug` de l'enum Rust,
//     échappement de barres obliques inverses compris).
//
// Ce fichier ne suppose PAS le résultat : si le pilote refuse, la ligne `la
// sortie n'a pas pris le mode demandé` le dira, et `frameWidth`/`frameHeight` de
// `getStats()` resteront à la taille précédente — ce que le JSON de sortie
// enregistrera fidèlement.
//
// ============================================================================
// AUTRES ÉCARTS AU CAHIER DES CHARGES, AVEC LEUR RAISON
// ============================================================================
//
//   - Étape 3 du brief (« lire le bandeau au bon endroit ») : DÉJÀ correcte dans
//     le pilote de D6 dont ce fichier part (`STATS` lit `#status`, jamais
//     `#statut`). Aucune correction à faire ; gardé tel quel, avec le même
//     commentaire d'avertissement (`#status` garde son texte une fois masqué :
//     prouve qu'un message est arrivé, pas qu'il était affiché).
//   - N = 3 (spec §8 : « pour laisser des voisines à observer »), configurable
//     par `N_FENETRES`.
//   - Chaque fenêtre est une instance de `ton.html?hz=…` (D7), pas
//     `anim-d4.html` : ce fichier sert à la fois de source vidéo animée (requis
//     par TOUTES les phases) et de source audio à fréquence connue (requis par
//     le critère ⑤), ce qui évite de gérer deux familles de pages pour cinq
//     critères qui se recouvrent dans le temps.
//   - Le critère ④ (sommeil des voisines) n'attend PAS que `N_FENETRES` dépasse
//     `vivier::PLAFOND_EVEIL` (8, LRU par éviction) : `marquerCachee` déclenche
//     directement la SECONDE raison de sommeil du vivier (`Raison::Masquee`,
//     `agent/src/capteur/vivier.rs`), qui ne dépend d'aucun nombre de fenêtres.
//     Les deux raisons empruntent le même chemin de code (« sans code neuf »,
//     spec §8) ; choisir `Masquee` évite d'ouvrir huit fenêtres pour une seule
//     mesure.
//
// Contraintes de protocole héritées (§8 de la spec), reprises sans modification :
// aucune capture d'écran pendant une mesure ; `Get-Process agent` avant chaque
// exécution (tâche 11) ; purge des sorties orphelines entre deux exécutions ;
// copier `agent.log` APRÈS la fin réelle, pas à la fin du pilote ; toute variable
// d'environnement neuve doit être ajoutée à `scripts/run-agent.sh` (déjà fait
// pour `PLEIN_ECRAN`, tâche 9, étape 4) ; vérifier la survie de la VM après
// chaque rang.

import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const RACINE = process.env.RACINE ?? '/home/mallanic/Projects/Guacamole';
// `ton.html` n'est PAS recopié dans ce répertoire : c'est un ASSET du sous-bloc
// D7, réutilisé en lecture seule depuis son propre répertoire.
const TON_HTML_D7 = join(RACINE, 'docs/superpowers/plans/journaux-multifenetres-d7/instrument/ton.html');
const HOTE = process.env.HOTE ?? '192.168.3.1';
const URL_SHELL = `http://${HOTE}:5173/shell.html`;
const ETIQUETTE = process.env.ETIQUETTE ?? 'sans-etiquette';
const COPIE_LOG = process.env.COPIE_LOG ?? `/tmp/agent-recette-d8-${ETIQUETTE}.log`;
const SORTIE_JSON = process.env.SORTIE_JSON
    ?? join(RACINE, `docs/superpowers/plans/journaux-multifenetres-d8/recette-${ETIQUETTE}.json`);
const VIEWPORT_FORCE = process.env.VIEWPORT_FORCE ?? '1280x720';
const BITRATE = process.env.BITRATE ?? '8000000';
const BUDGET_BPS = process.env.BUDGET_BPS ?? '12000000';
const PLEIN_ECRAN = process.env.PLEIN_ECRAN ?? '1';
const AUDIO = process.env.AUDIO ?? '1';
const N_FENETRES = Number(process.env.N_FENETRES ?? 3);
// Garde-fou 5 : jamais sous 60 s.
const PALIER_S = Number(process.env.PALIER_S ?? Math.max(60, 60));
// Garde-fou 6 : le fait, douze secondes sans mouvement, jamais un sleep plat.
const CALME_S = Number(process.env.CALME_S ?? 12);
const STABILISATION_MAX_S = Number(process.env.STABILISATION_MAX_S ?? 150);
// Taille visée pour simuler un viewport « plein écran » côté client (§6 de la
// spec). Choisie nettement au-dessus de 1280×720 pour dépasser la tolérance de
// 4 px de `taille_compatible`, et nettement EN DESSOUS de `TAILLE_MAX_SORTIE`
// (1920×1080, `agent/src/windows_source/sortie.rs`) pour que ② mesure un
// changement de mode ACCEPTÉ plutôt que borné.
const VIEWPORT_PLEIN_ECRAN = [1920, 1080];
// Volontairement AU-DESSUS de `TAILLE_MAX_SORTIE`, pour vérifier que le plafond
// borne réellement — probe annexe de ②, pas le critère lui-même.
const VIEWPORT_SURDIMENSIONNE = [3840, 2160];

const t0 = Date.now();
function log(...a) {
    const dt = ((Date.now() - t0) / 1000).toFixed(1).padStart(7);
    console.log(`[${dt}s ${new Date().toISOString()}] ${a.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join(' ')}`);
}
const dodo = (ms) => new Promise((r) => setTimeout(r, ms));
const maintenantIso = () => new Date().toISOString();

// ---------------------------------------------------------------- VM (session interactive)
//
// WinRM tourne en session 0 : une fenêtre créée ou manipulée depuis là ne serait
// pas visible de la session interactive, et ni le superviseur ni un vrai
// utilisateur ne la verraient (même raison que `scripts/run-agent.sh` et le
// `vm-it.sh` de D1/D6/D7). Réimplémenté ici en JS pur, pas en shell externe : le
// brief ne liste qu'UN fichier à créer pour cette tâche, et externaliser la
// mécanique de tâche planifiée dans un `.sh` compagnon aurait rompu cette
// contrainte pour un gain nul.
function vmIt(nom, ps) {
    const userName = process.env.WINDOWS_ADMIN_USERNAME ?? 'Administrateur';
    const password = process.env.WINDOWS_ADMIN_PASSWORD ?? '';
    spawnSync('bash', ['-c', `cat > /media/vm/dev/it-${nom}.ps1`], { input: ps, encoding: 'utf8' });
    const commande = [
        `schtasks /delete /tn it-${nom} /f 2>$null;`,
        `schtasks /create /tn it-${nom} /f /it /ru '${userName}' /rp '${password}'`,
        `/sc once /st 00:00 /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\it-${nom}.ps1';`,
        `schtasks /run /tn it-${nom}`,
    ].join(' ');
    const r = spawnSync('node', [join(RACINE, 'scripts/winrm.js'), commande], { encoding: 'utf8', env: process.env });
    if (r.status !== 0) log(`!! vm-it ${nom} a échoué`, (r.stderr ?? '').slice(0, 400));
    return r;
}
function vmVivante(etiquette) {
    const virsh = spawnSync('virsh', ['domstate', 'Windows'], { encoding: 'utf8' }).stdout?.trim() ?? '?';
    const acces = spawnSync('bash', ['-c', 'ls /media/vm/dev/ton.html >/dev/null 2>&1 && echo OUI || echo NON'],
        { encoding: 'utf8' }).stdout?.trim() ?? '?';
    log(`SURVIE VM (${etiquette}) virsh="${virsh}" acces_partage=${acces}`);
    return { virsh, acces };
}
function copierLog() {
    spawnSync('bash', ['-c', `cp /media/vm/dev/agent.log ${COPIE_LOG} 2>/dev/null`]);
}
async function journalPlat() {
    copierLog();
    let texte = '';
    try { texte = await readFile(COPIE_LOG, 'utf8'); } catch { }
    return texte.replace(/\x1b\[[0-9;]*m/g, '');
}
const horodate = (l) => l.match(/(\d{4}-\d\d-\d\dT[\d:.]+Z)/)?.[1];
/// Le span `fenetre{session=…}` (posé par D7, `agent/src/capteur/fenetre.rs:241`)
/// enveloppe TOUTE la boucle de capture d'une fenêtre — y compris les traces de
/// `mode_sortie.rs` et `capture.rs`, qui n'ont PAS de champ `session` explicite
/// sur leur propre ligne. C'est la source d'attribution la plus fiable, et elle
/// couvre plus de lignes que le champ `session=` explicite (réservé aux traces
/// qui l'ont ajouté au fil des sous-blocs, voir `CLAUDE.md`).
const sessionDeLigne = (l) => l.match(/fenetre\{session=([^}]+)\}/)?.[1]
    ?? l.match(/\bsession=(\S+)/)?.[1] ?? null;
const champ = (l, nom) => l.match(new RegExp(`\\b${nom}=(\\S+)`))?.[1] ?? null;

// ---------------------------------------------------------------- parseurs de journal
/// Les lignes `plein ecran de la fenetre Windows` (`agent/src/capteur/fenetre.rs`),
/// bornées à une fenêtre temporelle si fournie.
async function pleinEcranAgent(debutIso, finIso) {
    const plat = await journalPlat();
    const out = [];
    for (const l of plat.split('\n')) {
        if (!l.includes('plein ecran de la fenetre Windows')) continue;
        const t = horodate(l);
        if (!t || (debutIso && t < debutIso) || (finIso && t > finIso)) continue;
        out.push({ t, session: sessionDeLigne(l), actif: champ(l, 'actif') === 'true' });
    }
    return out;
}

/// Les quatre issues du changement de mode de sortie (`mode_sortie.rs`), bornées
/// à une fenêtre temporelle. C'est la pièce centrale de l'inconnue non tranchée
/// (§ tête de fichier) : ce parseur ne SUPPOSE aucune issue, il les distingue
/// toutes.
async function modeSortieAgent(debutIso, finIso) {
    const plat = await journalPlat();
    const demandees = [], reussies = [], refusees = [], illisibles = [], impossibles = [];
    for (const l of plat.split('\n')) {
        const t = horodate(l);
        if (!t || t < debutIso || t > finIso) continue;
        const session = sessionDeLigne(l);
        if (l.includes('changement de mode de la sortie virtuelle demandé')) {
            demandees.push({
                t, session, sortie: champ(l, 'sortie'),
                largeur: champ(l, 'largeur'), hauteur: champ(l, 'hauteur'),
                code: champ(l, 'code'), api_annonce_succes: champ(l, 'api_annonce_succes'),
            });
        } else if (l.includes("sortie retaillée : chaîne d'encodage reconstruite à la taille obtenue")) {
            // Champs bruts `self.width`/`self.height` (raccourci tracing) : le
            // NOM de champ produit est `width`/`height`, pas `largeur`/`hauteur`.
            reussies.push({ t, session, sortie: champ(l, 'sortie'), largeur: champ(l, 'width'), hauteur: champ(l, 'height') });
        } else if (l.includes("la sortie n'a pas pris le mode demandé")) {
            refusees.push({
                t, session, sortie: champ(l, 'sortie'),
                largeur_relue: champ(l, 'largeur_relue'), hauteur_relue: champ(l, 'hauteur_relue'),
            });
        } else if (l.includes('topologie illisible apres le changement de mode')) {
            illisibles.push({ t, session, sortie: champ(l, 'sortie') });
        } else if (l.includes('changement de mode impossible')) {
            impossibles.push({ t, session });
        }
    }
    return { demandees, reussies, refusees, illisibles, impossibles };
}

/// Les pertes d'accès `0x887a0026` (abandon de mutex DXGI, `capture.rs`), bornées
/// à une fenêtre temporelle, avec le NOM de sortie visé — extrait du format
/// `Debug` de `CibleCapture::Sortie(String)` (`cible=Sortie("\\\\.\\DISPLAY5")`),
/// où chaque barre oblique inverse réelle est doublée par l'échappement Debug.
async function pertesAcces(debutIso, finIso) {
    const plat = await journalPlat();
    const out = [];
    for (const l of plat.split('\n')) {
        if (!l.includes('accès à la duplication perdu, réouverture')) continue;
        if (!l.includes('887a0026')) continue; // le seul HRESULT qui nous intéresse ici
        const t = horodate(l);
        if (!t || t < debutIso || t > finIso) continue;
        const m = l.match(/cible=Sortie\("((?:\\\\|[^"])*)"\)/);
        out.push({ t, session: sessionDeLigne(l), sortie: m ? m[1].replace(/\\\\/g, '\\') : null });
    }
    return out;
}

/// Le nom de sortie `\\.\DISPLAYn` attaché à une session, tel qu'annoncé UNE FOIS
/// à l'attache (`fenêtre attachée au capteur`). C'est le « AVANT » du garde-fou 7.
async function sortieDeSession(session) {
    const plat = await journalPlat();
    const re = new RegExp(`fenêtre attachée au capteur session=${session} sortie=(\\S+)`);
    return plat.match(re)?.[1] ?? null;
}

/// `cadence du capteur` (capteur) et `cadence de la piste vidéo (côté enfant)`
/// (enfant), plus la liste des passages `endormie=true` — c'est par ce dernier
/// champ que le critère ④ (sommeil des voisines, chemin existant) se lit, sans
/// dépendre d'un texte de trace `fenêtre endormie` qui n'existe plus tel quel
/// dans le code actuel (vérifié par `grep` avant d'écrire cette fonction).
async function cadencesAgent(debutIso, finIso) {
    const plat = await journalPlat();
    const out = { capteur: {}, enfant: {}, endormies: [], eveillees: [] };
    for (const l of plat.split('\n')) {
        const t = horodate(l);
        if (!t || t < debutIso || t > finIso) continue;
        const s = champ(l, 'session') ?? sessionDeLigne(l);
        const c = champ(l, 'cadence')?.replace(/"/g, '');
        if (!s || !c) continue;
        if (l.includes('cadence du capteur')) {
            (out.capteur[s] ??= []).push(Number(c));
            // `eveillees` : le pendant symétrique d'`endormies`, une ligne
            // `endormie=false` DANS la fenêtre bornée fournie par l'appelant.
            // Indispensable pour un contrôle de RÉVEIL qui ne se contente pas
            // de la seule PRÉSENCE de la session dans `capteur` (laquelle est
            // vraie dès la première ligne du run, avant même tout sommeil —
            // voir la revue de la tâche 10, Critique 2) : l'appelant doit
            // borner `debutIso` à l'instant de l'ordre de réveil, pas au
            // début de la phase, pour que cette liste ne puisse être non vide
            // QUE si un réveil a réellement eu lieu depuis.
            if (l.includes('endormie=true')) out.endormies.push(`${s}@${t}`);
            else if (l.includes('endormie=false')) out.eveillees.push(`${s}@${t}`);
        } else if (l.includes('cadence de la piste vidéo')) {
            (out.enfant[s] ??= []).push(Number(c));
        }
    }
    const moy = (o) => Object.fromEntries(Object.entries(o).map(([k, v]) =>
        [k, Number((v.reduce((a, b) => a + b, 0) / v.length).toFixed(2))]));
    return { capteur: moy(out.capteur), enfant: moy(out.enfant), endormies: out.endormies, eveillees: out.eveillees };
}

/// Les parts de budget appliquées, avec leur champ `endormie` — second témoin
/// (indépendant de `cadence du capteur`) du sommeil d'une session, et de la
/// doctrine D6 selon laquelle une part dormante ne doit JAMAIS atteindre le
/// contrôleur d'une fenêtre éveillée (sans objet direct ici, mais le champ est
/// gratuit à relever).
async function partsAgent(debutIso, finIso) {
    const plat = await journalPlat();
    const out = [];
    for (const l of plat.split('\n')) {
        if (!l.includes('part de budget appliquee')) continue;
        const t = horodate(l);
        if (!t || t < debutIso || t > finIso) continue;
        out.push({ t, session: champ(l, 'session'), part_bps: Number(champ(l, 'part_bps')), endormie: champ(l, 'endormie') === 'true' });
    }
    return out;
}

/// Les changements de barreau de l'ADAPTATION RÉSEAU (`transport/adaptation.rs`) —
/// mécanisme SANS RAPPORT avec le changement de mode de sortie du §6 (celui-ci
/// suit la congestion, celui-là suit le viewport). Reprise du D6 pour le
/// garde-fou 6 (attendre le fait, pas une durée) avant la mesure du témoin.
async function barreaux() {
    const plat = await journalPlat();
    const ok = [], ko = [];
    for (const l of plat.split('\n')) {
        const t = horodate(l) ?? '?';
        const L = champ(l, 'largeur'), H = champ(l, 'hauteur');
        if (l.includes("taille d'encodage changée")) ok.push({ t, taille: `${L}x${H}` });
        else if (l.includes("changement de taille d'encodage refusé")) ko.push({ t, taille: `${L}x${H}` });
    }
    return { changees: ok, refusees: ko };
}
async function attendreBarreauxStables(calmeS, maxS) {
    const debut = Date.now();
    let dernierCompte = -1, dernierChangement = Date.now();
    while ((Date.now() - debut) / 1000 < maxS) {
        const b = await barreaux();
        if (b.changees.length !== dernierCompte) {
            dernierCompte = b.changees.length;
            dernierChangement = Date.now();
        } else if ((Date.now() - dernierChangement) / 1000 >= calmeS) {
            const attendu = Number(((Date.now() - debut) / 1000).toFixed(1));
            log(`ÉCHELLE POSÉE après ${attendu} s (${dernierCompte} changements cumulés)`);
            return { stable: true, attente_s: attendu, changements: dernierCompte };
        }
        await dodo(2000);
    }
    const attendu = Number(((Date.now() - debut) / 1000).toFixed(1));
    log(`!! ÉCHELLE NON POSÉE après ${attendu} s (${dernierCompte} changements cumulés)`);
    return { stable: false, attente_s: attendu, changements: dernierCompte };
}
/// Garde-fou 6, forme générale : interroge `test()` toutes les 2 s jusqu'à ce
/// qu'il rende vrai, borné à `maxS`. Utilisée pour attendre une TRACE précise
/// plutôt qu'une durée fixe (l'annonce `PleinEcran`, le sommeil d'une fenêtre…).
async function attendreLeFait(etiquette, test, maxS) {
    const debut = Date.now();
    while ((Date.now() - debut) / 1000 < maxS) {
        const r = await test();
        if (r) {
            log(`FAIT ATTEINT (${etiquette}) après ${((Date.now() - debut) / 1000).toFixed(1)} s`);
            return r;
        }
        await dodo(2000);
    }
    log(`!! FAIT NON ATTEINT (${etiquette}) après ${maxS} s`);
    return null;
}

async function marqueurs(etiquette) {
    const plat = await journalPlat();
    const compte = (motif) => (plat.match(new RegExp(motif, 'g')) ?? []).length;
    const m = {
        lignes: plat.split('\n').length,
        enfant_lance: compte('enfant lancé'),
        attachee_capteur: compte('fenêtre attachée au capteur'),
        cloture: compte('clôture de session amorcée'),
        // Le champ `actif` est émis APRÈS le message, pas juxtaposé à lui
        // (`… plein ecran de la fenetre Windows session=w-2 actif=true`) : le
        // motif doit couvrir toute la ligne, pas seulement sa fin.
        plein_ecran_total: compte('plein ecran de la fenetre Windows'),
        plein_ecran_actif: compte('plein ecran de la fenetre Windows.*actif=true'),
        mode_sortie_demande: compte('changement de mode de la sortie virtuelle demandé'),
        mode_sortie_reussi: compte("sortie retaillée : chaîne d'encodage reconstruite"),
        mode_sortie_refuse: compte("la sortie n'a pas pris le mode demandé"),
        mutex_abandonne: compte('887a0026'),
        erreurs: compte('ERROR'),
    };
    log(`MARQUEURS (${etiquette}) ` + JSON.stringify(m));
    return m;
}

// ---------------------------------------------------------------- CDP (repris de D6)
class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl);
        this.nextId = 1;
        this.pending = new Map();
        this.handlers = [];
        this.ready = new Promise((res) => this.ws.addEventListener('open', () => res(), { once: true }));
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.pending.has(m.id)) {
                const { resolve, reject } = this.pending.get(m.id);
                this.pending.delete(m.id);
                if (m.error) reject(new Error(JSON.stringify(m.error)));
                else resolve(m.result);
            } else if (m.method) { for (const h of this.handlers) h(m); }
        });
    }
    on(h) { this.handlers.push(h); }
    async send(method, params = {}, sessionId) {
        await this.ready;
        const id = this.nextId++;
        const msg = { id, method, params };
        if (sessionId) msg.sessionId = sessionId;
        return new Promise((resolve, reject) => {
            this.pending.set(id, { resolve, reject });
            this.ws.send(JSON.stringify(msg));
        });
    }
    async eval(sessionId, expression, awaitPromise = false) {
        const r = await this.send('Runtime.evaluate',
            { expression, awaitPromise, returnByValue: true }, sessionId);
        if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 600));
        return r.result.value;
    }
    // Garde-fou 4 : toute évaluation sur une page WebRTC active est BORNÉE.
    async evalBorne(sessionId, expression, ms = 8000, awaitPromise = true) {
        return Promise.race([
            this.eval(sessionId, expression, awaitPromise),
            new Promise((r) => setTimeout(() => r({ __timeout: ms }), ms)),
        ]).catch((e) => ({ __erreur: String(e).slice(0, 200) }));
    }
}
async function attendreDevtools(port) {
    for (let i = 0; i < 80; i += 1) {
        try {
            const r = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (r.ok) return await r.json();
        } catch { }
        await dodo(250);
    }
    throw new Error('devtools timeout');
}

// L'amorce pose visibilité + focus (garde-fous 2 et 3, comme D6) ET intercepte
// désormais DEUX types de message sur le canal `control`, pas un seul : `link`
// (hérité, non exploité par ce pilote mais gratuit à garder) et surtout
// `fullscreen` — c'est l'instrument du critère ①, partie « et elle seule ».
const AMORCE = `
(() => {
  if (window.__amorceD8) return;
  window.__amorceD8 = true;
  window.__pc = null;
  window.__pleinEcran = [];
  const N = window.RTCPeerConnection;
  const creer = N.prototype.createDataChannel;
  N.prototype.createDataChannel = function (label, ...r) {
    const c = creer.call(this, label, ...r);
    if (label === 'control') {
      c.addEventListener('message', (e) => {
        try {
          const m = JSON.parse(e.data);
          if (m && m.type === 'fullscreen') {
            window.__pleinEcran.push({ t: Date.now(), active: m.active });
            if (window.__pleinEcran.length > 50) window.__pleinEcran.shift();
          }
        } catch { }
      });
    }
    return c;
  };
  window.RTCPeerConnection = function (...a) { const p = new N(...a); window.__pc = p; return p; };
  window.RTCPeerConnection.prototype = N.prototype;

  window.__cachee = false;
  window.__focalisee = false;
  Object.defineProperty(document, 'hidden', {
    configurable: true, get: () => window.__cachee });
  Object.defineProperty(document, 'visibilityState', {
    configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
  document.hasFocus = () => window.__focalisee;
})();
`;

const pages = new Map();
const appPages = () => [...pages].filter(([, p]) =>
    p.url.includes('?session=') && !p.url.includes('shell.html'));
const nomDe = (url) => url.replace(/^.*\?session=/, 'w:');
const rang = (n) => Number(String(n).match(/(\d+)\s*$/)?.[1] ?? 0);
const nomsTries = () => appPages().map(([, p]) => nomDe(p.url)).sort((a, b) => rang(a) - rang(b));
const sidDe = (nom) => appPages().find(([, p]) => nomDe(p.url) === nom)?.[0];

// Garde-fou 3 : la cible passe PAR `blur` PUIS `focus`, toutes les pages passent
// par `blur` d'abord (déduplication de `client/src/visibilite.ts`).
async function imposerScenario(cdp, cible, etiquette) {
    const ordre = appPages();
    for (const passe of ['blur', 'focus']) {
        for (const [sid, p] of ordre) {
            const veutFocus = passe === 'focus' && nomDe(p.url) === cible;
            if (passe === 'focus' && !veutFocus) continue;
            await cdp.evalBorne(sid, `(() => {
                if (!Object.getOwnPropertyDescriptor(document, 'hidden')) {
                    Object.defineProperty(document, 'hidden', {
                        configurable: true, get: () => window.__cachee });
                    Object.defineProperty(document, 'visibilityState', {
                        configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
                }
                document.hasFocus = () => window.__focalisee;
                window.__cachee = false;
                window.__focalisee = ${veutFocus};
                document.dispatchEvent(new Event('visibilitychange'));
                window.dispatchEvent(new Event(${veutFocus ? "'focus'" : "'blur'"}));
                return document.hidden + '/' + document.hasFocus();
            })()`, 6000, false);
        }
    }
    const etats = [];
    for (const [sid, p] of ordre) {
        const r = await cdp.evalBorne(sid, `document.hidden + '/' + document.hasFocus()`, 6000, false);
        etats.push([nomDe(p.url), r]);
    }
    const focalisees = etats.filter(([, v]) => String(v).endsWith('/true')).map(([k]) => k);
    log(`SCÉNARIO IMPOSÉ (${etiquette}) cible=${cible} hidden/hasFocus=` + JSON.stringify(Object.fromEntries(etats)));
    log(`  → focalisées effectives : ${JSON.stringify(focalisees)} (une seule attendue)`);
    return { etats: Object.fromEntries(etats), focalisees };
}

/// Marque UNE page cachée ou visible, SANS toucher au focus des autres. C'est le
/// levier direct de la raison de sommeil `Masquee` (`agent/src/capteur/vivier.rs`),
/// choisie plutôt que `Evincee` pour éprouver le critère ④ sans avoir à ouvrir
/// `vivier::PLAFOND_EVEIL` (8) fenêtres pour une seule mesure.
async function marquerCachee(cdp, cible, cachee, etiquette) {
    const sid = sidDe(cible);
    if (!sid) throw new Error(`page introuvable pour ${cible}`);
    const r = await cdp.evalBorne(sid, `(() => {
        window.__cachee = ${cachee};
        document.dispatchEvent(new Event('visibilitychange'));
        return document.hidden;
    })()`, 6000, false);
    log(`VISIBILITÉ FORCÉE (${etiquette}) cible=${cible} cachee=${cachee} relu=${r}`);
    return r;
}

/// Force le viewport rapporté par CDP — LE MÊME mécanisme que `VIEWPORT_FORCE`
/// emploie déjà à l'attache de chaque page. C'est l'instrument du critère ②,
/// indépendant de la question du plein écran réel (voir l'en-tête de fichier).
async function forcerViewport(cdp, cible, largeur, hauteur, etiquette) {
    const sid = sidDe(cible);
    if (!sid) throw new Error(`page introuvable pour ${cible}`);
    await cdp.send('Emulation.setDeviceMetricsOverride',
        { width: largeur, height: hauteur, deviceScaleFactor: 1, mobile: false }, sid);
    log(`VIEWPORT FORCÉ (${etiquette}) cible=${cible} ${largeur}x${hauteur}`);
}

const STATS = `(async () => {
  const pc = window.__pc;
  if (!pc) return { pc: null };
  const r = await pc.getStats(); const t = [...r.values()];
  const v = t.find(x => x.type === 'inbound-rtp' && x.kind === 'video');
  const a = t.find(x => x.type === 'inbound-rtp' && x.kind === 'audio');
  return {
    etat: pc.iceConnectionState, horloge: Date.now(),
    images_decodees: v?.framesDecoded ?? null,
    images_recues: v?.framesReceived ?? null,
    images_perdues: v?.framesDropped ?? null,
    octets_recus: v?.bytesReceived ?? null,
    octets_audio: a?.bytesReceived ?? null,
    l: v?.frameWidth ?? null, h: v?.frameHeight ?? null,
    // Relevé pour mémoire (P2 REFUSE) : ne sert PAS de signal de critère ①. Voir
    // l'en-tête de fichier — le signal réel est window.__pleinEcran.
    fullscreen_element_non_nul: !!document.fullscreenElement,
    cachee: document.hidden, focalisee: document.hasFocus(),
    // #status, PAS #statut (celui-ci est réservé à la page-shell) — garde-fou
    // 3 du brief, déjà correct dans le pilote de D6 dont ce fichier part. Le
    // texte survit une fois le bandeau masqué : prouve qu'un message est arrivé,
    // pas qu'il était affiché.
    bandeau: document.querySelector('#status')?.textContent ?? null,
    plein_ecran_messages: window.__pleinEcran.slice(),
  };
})()`;
async function statsToutes(cdp, etiquette) {
    const entrees = appPages();
    const resultats = await Promise.all(entrees.map(async ([sid, p]) =>
        [nomDe(p.url), await cdp.evalBorne(sid, STATS)]));
    const out = Object.fromEntries(resultats);
    log(`STATS (${etiquette}) ` + JSON.stringify(out));
    return out;
}

// ---------------------------------------------------------------- audio (porté de D7)
//
// Copié depuis `pilote-recette-d7.mjs::expressionReleveFrequence`, tel quel :
// même doctrine (`niveaux` par fréquence assignée, jamais le seul argmax ;
// `ctx.resume()` + vérification `ctx.state === 'running'` ; repli par
// `<video>.srcObject` qui ne dépend d'aucune course avec l'AMORCE ;
// `ctx.close()` en `finally`). Voir le fichier source pour la justification
// complète de chaque choix — non recopiée ici pour ne pas la faire dériver de
// l'original en la reformulant.
const SENTINEL_DB = -1000;
function expressionReleveFrequence(hzsAssignes) {
    return `(async () => {
  const CIBLES = ${JSON.stringify(hzsAssignes)};
  const SENTINEL = ${SENTINEL_DB};
  const pc = window.__pc;
  let piste = pc ? pc.getReceivers().map(r => r.track).find(t => t && t.kind === 'audio') : null;
  if (!piste) {
    const v = document.querySelector('video');
    piste = v?.srcObject?.getAudioTracks?.()[0] ?? null;
  }
  if (!piste) return { erreur: pc ? 'aucune piste audio' : 'aucune PeerConnection exposee' };
  const ctx = new AudioContext();
  try {
    await ctx.resume();
    if (ctx.state !== 'running') {
      return { erreur: 'AudioContext non demarre etat=' + ctx.state };
    }
    const analyseur = ctx.createAnalyser();
    analyseur.fftSize = 8192;
    ctx.createMediaStreamSource(new MediaStream([piste])).connect(analyseur);
    await new Promise(r => setTimeout(r, 1500));
    const bins = new Float32Array(analyseur.frequencyBinCount);
    analyseur.getFloatFrequencyData(bins);
    const db = (x) => (Number.isFinite(x) ? Math.round(x) : SENTINEL);
    const binDe = (f) => Math.max(0, Math.min(bins.length - 1,
      Math.round(f * analyseur.fftSize / ctx.sampleRate)));
    let meilleur = 0;
    for (let i = 1; i < bins.length; i++) if (bins[i] > bins[meilleur]) meilleur = i;
    const finis = [...bins].filter(Number.isFinite);
    const plancher_db = finis.length ? Math.round(finis.reduce((a, b) => a + b, 0) / finis.length) : SENTINEL;
    let statsAudio = null;
    if (pc) {
      try {
        const rapport = await pc.getStats();
        const entree = [...rapport.values()].find((x) => x.type === 'inbound-rtp' && x.kind === 'audio');
        if (entree) statsAudio = { bytes_recus: entree.bytesReceived ?? null, paquets_recus: entree.packetsReceived ?? null };
      } catch { }
    }
    return {
      hz: Math.round(meilleur * ctx.sampleRate / analyseur.fftSize),
      db: db(bins[meilleur]), plancher_db, sample_rate: ctx.sampleRate,
      niveaux: CIBLES.map((f) => ({ f, db: db(bins[binDe(f)]) })),
      piste_muted: piste.muted, piste_ready_state: piste.readyState,
      stats_audio: statsAudio,
    };
  } finally {
    await ctx.close();
  }
})()`;
}

// ---------------------------------------------------------------- Windows : style réel
//
// Retire/restaure RÉELLEMENT `WS_CAPTION`/`WS_THICKFRAME` sur la fenêtre Chrome
// `--app` cible, par `SetWindowLongPtrW` — exactement l'instruction du cadrage de
// tâche. Le VERDICT est ce que `GetWindowLongPtrW` RELIT après coup, jamais ce
// que l'appel prétend avoir fait : même doctrine que `mode_sortie.rs`
// (« le verdict est ce que DXGI relit, jamais ce que l'API retourne »), reprise
// ici côté Win32 fenêtre plutôt que côté DXGI sortie.
//
// La fenêtre cible est retrouvée par le `CommandLine` de son processus (le
// `--user-data-dir` unique posé par `ouvrirFenetre`), PAS par un titre ou un
// index de fenêtre — les fenêtres Chrome `--app` d'un même hôte n'ont pas
// d'ordre stable, et un titre peut être partagé (`ton.html` fixe le sien à
// `${hz} Hz`, mais deux fenêtres pourraient partager un `hz` par erreur de
// configuration). Écrite sur le PARTAGE et invoquée par `-File`
// (`vmIt`) : `nodejs-winrm` enveloppe toute commande inline dans
// `powershell -Command "& { … }"`, et un script portant des guillemets doubles y
// entrerait en collision — piège déjà payé (voir `CLAUDE.md`, sous-bloc D8). Le
// résultat est écrit dans un FICHIER JSON sur le partage plutôt qu'imprimé sur
// la sortie standard de la commande : cela évite AUSSI le défaut d'encodage à
// deux réglages de `build-agent.sh`/`run-agent.sh` (`[Console]::OutputEncoding`
// non posé), puisque rien d'accentué ne transite par le flux que `winrm.js`
// capture — seul PowerShell lit et écrit son propre fichier.
function psStyleFenetre(marqueur, action) {
    // GWL_STYLE = -16 ; WS_CAPTION = 0x00C00000 ; WS_THICKFRAME = 0x00040000 ;
    // SWP_FRAMECHANGED|NOMOVE|NOSIZE|NOZORDER|NOACTIVATE = 0x0037. Valeurs
    // reprises TEXTUELLEMENT du plan (`docs/superpowers/plans/2026-08-04-multifenetres-plein-ecran.md`,
    // §« le prédicat de plein écran »).
    return [
        'Add-Type @"',
        'using System;',
        'using System.Runtime.InteropServices;',
        'public class D8Fenetre {',
        '  [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr GetWindowLongPtrW(IntPtr hWnd, int nIndex);',
        '  [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SetWindowLongPtrW(IntPtr hWnd, int nIndex, IntPtr dwNewLong);',
        '  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);',
        '}',
        '"@',
        '$GWL_STYLE = -16',
        '$WS_CAPTION = 0x00C00000',
        '$WS_THICKFRAME = 0x00040000',
        '$SWP_FLAGS = 0x0037',
        `$procs = Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" | Where-Object { $_.CommandLine -like '*${marqueur}*' }`,
        '$hwnd = [IntPtr]::Zero',
        'foreach ($p in $procs) {',
        '  $proc = Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue',
        '  if ($proc -and $proc.MainWindowHandle -ne [IntPtr]::Zero) { $hwnd = $proc.MainWindowHandle; break }',
        '}',
        'if ($hwnd -eq [IntPtr]::Zero) {',
        `  '{"erreur":"fenetre introuvable pour ${marqueur}"}' | Out-File -Encoding ascii C:\\dev\\${marqueur}-style.json`,
        '  exit',
        '}',
        '$avant = [D8Fenetre]::GetWindowLongPtrW($hwnd, $GWL_STYLE).ToInt64()',
        action === 'lire'
            ? '$nouveau = $avant'
            : action === 'sansBordure'
                ? '$nouveau = $avant -band (-bnot ($WS_CAPTION -bor $WS_THICKFRAME))'
                : '$nouveau = $avant -bor ($WS_CAPTION -bor $WS_THICKFRAME)',
        action === 'lire' ? '' : '[void][D8Fenetre]::SetWindowLongPtrW($hwnd, $GWL_STYLE, [IntPtr]$nouveau)',
        action === 'lire' ? '' : '[void][D8Fenetre]::SetWindowPos($hwnd, [IntPtr]::Zero, 0, 0, 0, 0, $SWP_FLAGS)',
        '$apres = [D8Fenetre]::GetWindowLongPtrW($hwnd, $GWL_STYLE).ToInt64()',
        '$sansBordureAvant = (($avant -band ($WS_CAPTION -bor $WS_THICKFRAME)) -eq 0)',
        '$sansBordureApres = (($apres -band ($WS_CAPTION -bor $WS_THICKFRAME)) -eq 0)',
        '$resultat = @{ hwnd = $hwnd.ToString("x"); avant = $avant; apres = $apres; sans_bordure_avant = $sansBordureAvant; sans_bordure_apres = $sansBordureApres }',
        `$resultat | ConvertTo-Json -Compress | Out-File -Encoding ascii C:\\dev\\${marqueur}-style.json`,
    ].filter((l) => l !== '').join('\n');
}
/// Exécute `psStyleFenetre` et relit le résultat depuis le partage. `action` :
/// `'lire'` (aucun changement), `'sansBordure'` (retire les deux bits, ce que
/// ferait un vrai jeu plein écran) ou `'restaurer'` (les remet).
async function togglerStyleFenetre(marqueur, action) {
    vmIt(`style-${marqueur}`, psStyleFenetre(marqueur, action));
    await dodo(4000);
    let brut = '';
    try { brut = await readFile(`/media/vm/dev/${marqueur}-style.json`, 'utf8'); } catch { }
    let resultat = null;
    try { resultat = JSON.parse(brut); } catch { }
    log(`STYLE FENÊTRE (${marqueur}, action=${action}) ` + (brut || '(fichier absent)'));
    return resultat;
}

// ---------------------------------------------------------------- ouverture des fenêtres VM
function marqueurFenetre(n) { return `chrome-d8-${ETIQUETTE}-${n}`; }
function hzDe(n) { return 300 + 110 * n; }

/// Copie `ton.html` (D7) vers une copie PROPRE À CHAQUE FRÉQUENCE. DEUX choses
/// distinctes en dépendent, et il ne faut pas les confondre :
///   - le NOM DE FICHIER (`ton-${hz}.html`) donne à chaque fenêtre une identité
///     stable pour la corrélation (même correctif que D7, round 3 :
///     `document.title`, donc la légende de fenêtre Windows, est la seule
///     identité disponible pour une fenêtre `--app` sur `file://` — le CHEMIN
///     de fichier y est visible, pas la chaîne de requête) ;
///   - la FRÉQUENCE RÉELLEMENT JOUÉE, elle, vient exclusivement de
///     `?hz=…` dans l'URL ouverte par `ouvrirFenetre` (`ton.html` lit
///     `URLSearchParams(location.search).get('hz')`, `null` sans requête, et
///     retombe alors sur son défaut 440). Copier le fichier ne suffit PAS à
///     donner sa fréquence à une fenêtre : sans le paramètre de requête,
///     TOUTES les fenêtres joueraient 440 Hz, quel que soit le nom du fichier
///     ouvert — bug trouvé en revue, corrigé dans `ouvrirFenetre` ci-dessous.
function assurerTonHtml(hz) {
    spawnSync('bash', ['-c', `cp ${TON_HTML_D7} /media/vm/dev/ton-${hz}.html`]);
}

async function ouvrirFenetre(n) {
    const marqueur = marqueurFenetre(n);
    const hz = hzDe(n);
    assurerTonHtml(hz);
    const avant = appPages().length;
    log(`  · ouverture fenêtre ${n} (hz=${hz}, marqueur=${marqueur})`);
    vmIt(`ouvrird8-${n}`, [
        '$a = @(',
        // `?hz=${hz}` est INDISPENSABLE : sans lui `ton.html` retombe sur son
        // défaut (440 Hz) pour TOUTES les fenêtres, quel que soit le fichier
        // ouvert (voir le commentaire de `assurerTonHtml`).
        `  "--app=file:///C:/dev/ton-${hz}.html?hz=${hz}",`,
        `  "--user-data-dir=C:\\dev\\${marqueur}",`,
        "  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',",
        `  '--window-size=1280,720','--window-position=${30 + n * 12},${30 + n * 12}',`,
        "  '--autoplay-policy=no-user-gesture-required',",
        "  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',",
        "  '--disable-renderer-backgrounding')",
        "Start-Process 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe' -ArgumentList $a",
        'Start-Sleep -Seconds 2',
    ].join('\n'));
    for (let i = 0; i < 30; i += 1) {
        await dodo(2000);
        if (appPages().length > avant) return { ok: true, hz, marqueur };
    }
    log(`  !! fenêtre ${n} : aucune page de plus après 60 s`);
    return { ok: false, hz, marqueur };
}

function cpuHote(etiquette) {
    const cpu = spawnSync('bash', ['-c', "top -b -n 3 -d 2 | grep -E '^%Cpu' | sed 's/^/    /'"], { encoding: 'utf8' }).stdout ?? '';
    const charge = spawnSync('bash', ['-c', 'cat /proc/loadavg'], { encoding: 'utf8' }).stdout?.trim() ?? '';
    log(`CPU HÔTE (${etiquette})\n${cpu.trimEnd()}\n    loadavg=${charge}`);
    return { cpu: cpu.trim(), loadavg: charge };
}

/// Deltas par fenêtre entre deux relevés (repris de D6, sans les champs propres
/// au budget de session — D8 ne le mesure pas).
function deltas(a, b) {
    const parFenetre = {};
    for (const k of Object.keys(b)) {
        const av = a[k], ap = b[k];
        if (!av || !ap || typeof av.images_decodees !== 'number' || typeof ap.images_decodees !== 'number') {
            parFenetre[k] = null; continue;
        }
        const dt = (ap.horloge - av.horloge) / 1000;
        const rec = ap.images_recues - av.images_recues;
        const dec = ap.images_decodees - av.images_decodees;
        const jet = (ap.images_perdues ?? 0) - (av.images_perdues ?? 0);
        parFenetre[k] = {
            secondes: Number(dt.toFixed(3)),
            images_recues: rec, images_decodees: dec, images_jetees: jet,
            i_par_s: Number((dec / dt).toFixed(2)),
            pc_jetees: rec > 0 ? Number((100 * jet / rec).toFixed(2)) : null,
            mbps: Number(((ap.octets_recus - av.octets_recus) * 8 / dt / 1e6).toFixed(3)),
            kbps_audio: Number((((ap.octets_audio ?? 0) - (av.octets_audio ?? 0)) * 8 / dt / 1e3).toFixed(1)),
            taille: `${ap.l}x${ap.h}`, cachee: ap.cachee, focalisee: ap.focalisee, bandeau: ap.bandeau,
        };
    }
    return parFenetre;
}

// ---------------------------------------------------------------- phases
/// Le TÉMOIN de non-régression (spec §8) : un palier à N fenêtres sans AUCUN
/// plein écran, qui doit se comporter comme D7 — même mécanisme de mesure
/// (`statsToutes`/`deltas`) que les phases de critère, pour rester comparable.
async function phaseTemoin(cdp) {
    log('>>> PHASE TÉMOIN — aucun plein écran, palier de non-régression');
    const cible = nomsTries()[0];
    const scenario = await imposerScenario(cdp, cible, 'témoin');
    await dodo(8000);
    const pose = await attendreBarreauxStables(CALME_S, STABILISATION_MAX_S);
    const debut = maintenantIso();
    const a = await statsToutes(cdp, 'témoin — début');
    await dodo(PALIER_S * 1000);
    const b = await statsToutes(cdp, 'témoin — fin');
    const fin = maintenantIso();
    const d = deltas(a, b);
    log('TÉMOIN — DELTAS ' + JSON.stringify(d, null, 1));
    return { cible, scenario, debut, fin, pose, deltas: d, survie: vmVivante('après le témoin') };
}

/// Critères ① et ②, dans la MÊME phase : ils partagent la même fenêtre cible et
/// la même fenêtre temporelle, ce qui permet de compter les pertes d'accès des
/// VOISINES une seule fois pour les deux mécanismes qu'ils déclenchent (le style
/// Windows d'un côté, le viewport CDP de l'autre).
///
/// ⚠️ CES DEUX CRITÈRES NE SONT PAS ISOLÉS : ② est mesuré sur la fenêtre cible
/// APRÈS que ① l'a déjà basculée `sansBordure`, sans restauration
/// intermédiaire. C'est défendable (① et ② sont deux mécanismes indépendants,
/// voir l'en-tête de fichier), mais le relevé JSON le dit explicitement
/// (`critere2.style_de_la_fenetre_pendant_cette_mesure`) pour qu'un lecteur ne
/// croie pas ② mesuré à l'état nominal sans avoir à relire ce fichier.
async function phaseCritere1Et2(cdp, cible, marqueurCible) {
    log(`>>> PHASE CRITÈRE ①+② — cible=${cible} marqueur=${marqueurCible}`);
    const sessionCible = cible.replace(/^w:/, '');
    const sortieAvant = await sortieDeSession(sessionCible);
    log(`  sortie AVANT (garde-fou 7) : ${sortieAvant}`);

    await imposerScenario(cdp, cible, 'critère ①+② (visibilité de référence)');
    await dodo(4000);
    const debut = maintenantIso();

    // --- ① : bascule RÉELLE du style Windows ---
    const styleAvant = await togglerStyleFenetre(marqueurCible, 'lire');
    const styleBascule = await togglerStyleFenetre(marqueurCible, 'sansBordure');

    const detectionAgent = await attendreLeFait('annonce PleinEcran actif=true côté agent', async () => {
        const evts = await pleinEcranAgent(debut, maintenantIso());
        return evts.find((e) => e.session === sessionCible && e.actif === true) ?? null;
    }, 30);

    const messagesRecus = await attendreLeFait('message fullscreen reçu côté navigateur (cible)', async () => {
        const r = await cdp.evalBorne(sidDe(cible), 'window.__pleinEcran.slice()', 4000, false);
        return Array.isArray(r) && r.some((m) => m.active === true) ? r : null;
    }, 30);
    // « et elle seule » : les VOISINES ne doivent RIEN avoir reçu.
    const messagesVoisines = {};
    for (const nom of nomsTries()) {
        if (nom === cible) continue;
        messagesVoisines[nom] = await cdp.evalBorne(sidDe(nom), 'window.__pleinEcran.slice()', 4000, false);
    }
    const fuiteVersVoisine = Object.entries(messagesVoisines)
        .filter(([, msgs]) => Array.isArray(msgs) && msgs.some((m) => m.active === true))
        .map(([nom]) => nom);

    // --- ② : viewport forcé, indépendant de ① (voir en-tête de fichier) ---
    await forcerViewport(cdp, cible, VIEWPORT_PLEIN_ECRAN[0], VIEWPORT_PLEIN_ECRAN[1], 'critère ② — cible plein écran');
    const modeApresCible = await attendreLeFait('changement de mode côté agent (cible)', async () => {
        const m = await modeSortieAgent(debut, maintenantIso());
        return (m.reussies.find((r) => r.session === sessionCible)
            || m.refusees.find((r) => r.session === sessionCible)
            || m.illisibles.find((r) => r.session === sessionCible)) ?? null;
    }, 30);
    const statsApresRedim = await cdp.evalBorne(sidDe(cible), STATS, 6000, true);

    // Probe annexe : au-dessus de TAILLE_MAX_SORTIE, le flux doit rester borné.
    await forcerViewport(cdp, cible, VIEWPORT_SURDIMENSIONNE[0], VIEWPORT_SURDIMENSIONNE[1], 'critère ② — probe TAILLE_MAX_SORTIE');
    await dodo(6000);
    const statsApresSurdimensionne = await cdp.evalBorne(sidDe(cible), STATS, 6000, true);

    const fin = maintenantIso();

    // L'inconnue non tranchée (§ en-tête) : les pertes d'accès des VOISINES
    // pendant TOUTE la fenêtre de la tentative — comptées, pas supposées.
    const pertes = await pertesAcces(debut, fin);
    const pertesVoisines = pertes.filter((p) => p.sortie !== sortieAvant);
    const modeSortieComplet = await modeSortieAgent(debut, fin);
    // Garde-fou 7, le VRAI relevé « APRÈS » : le nom que la trace de
    // changement de mode elle-même rapporte pour CETTE session — PAS une
    // seconde lecture de `sortieDeSession`, qui ne relit que la ligne
    // statique d'attache (immuable, donc une comparaison contre elle serait
    // triviale et ne prouverait rien). C'est `demandees`/`reussies` qui disent
    // ce que le pilote SudoVDA a effectivement visé.
    const sortieApres = modeSortieComplet.demandees.find((d) => d.session === sessionCible)?.sortie
        ?? modeApresCible?.sortie ?? null;

    // Restauration : bordure Windows remise, viewport ramené à la taille de
    // référence — pour ne pas polluer les phases suivantes.
    const styleRestaure = await togglerStyleFenetre(marqueurCible, 'restaurer');
    const [vpL, vpH] = VIEWPORT_FORCE.split('x').map(Number);
    await forcerViewport(cdp, cible, vpL, vpH, 'critère ①+② — restauration');
    await dodo(4000);
    const detectionSortieAgent = await attendreLeFait('annonce PleinEcran actif=false (restauration)', async () => {
        const evts = await pleinEcranAgent(fin, maintenantIso());
        return evts.find((e) => e.session === sessionCible && e.actif === false) ?? null;
    }, 20);

    const releve = {
        cible, session_cible: sessionCible, marqueur_cible: marqueurCible, debut, fin,
        critere1: {
            style_avant: styleAvant, style_bascule: styleBascule,
            detection_agent: detectionAgent,
            messages_navigateur_cible: messagesRecus,
            messages_navigateur_voisines: messagesVoisines,
            fuite_vers_voisine: fuiteVersVoisine,
            verdict_partie_mesurable: !!detectionAgent && !!messagesRecus && fuiteVersVoisine.length === 0,
        },
        critere2: {
            // Revue de la tâche 10, Important : ② est mesuré SANS restaurer le
            // style Windows entre ① et ②, sur la MÊME fenêtre déjà sans
            // bordure (défendable — les deux mécanismes sont indépendants,
            // voir l'en-tête de fichier — mais un lecteur du seul JSON ne
            // doit pas croire ② mesuré à l'état nominal). L'état de style au
            // moment de ② est donc porté explicitement ici plutôt que laissé
            // implicite dans l'ordre du code.
            style_de_la_fenetre_pendant_cette_mesure: 'sans_bordure (bascule du critère 1, non restaurée avant 2)',
            viewport_demande: VIEWPORT_PLEIN_ECRAN, mode_apres_cible: modeApresCible,
            stats_apres_redimensionnement: statsApresRedim,
            viewport_surdimensionne_demande: VIEWPORT_SURDIMENSIONNE,
            stats_apres_surdimensionne: statsApresSurdimensionne,
            surdimensionne_borne: !!(statsApresSurdimensionne?.l <= 1920 && statsApresSurdimensionne?.h <= 1080),
        },
        garde_fou_7_nom_sortie: { avant: sortieAvant, apres: sortieApres, conserve: !!sortieAvant && sortieAvant === sortieApres },
        mode_sortie_complet: modeSortieComplet,
        pertes_acces_toutes: pertes, pertes_acces_voisines: pertesVoisines,
        detection_agent_sortie: detectionSortieAgent,
        style_restaure: styleRestaure,
    };
    log('CRITÈRE ①+② — RELEVÉ ' + JSON.stringify(releve, null, 1));
    return releve;
}

/// Critères ④ (sommeil de la voisine par le chemin existant) et ⑤ (son audio
/// survit) dans la MÊME phase : ⑤ n'a de sens qu'une fois ④ établi.
async function phaseCritere4Et5(cdp, voisine, hzVoisine) {
    log(`>>> PHASE CRITÈRE ④+⑤ — voisine=${voisine} hz=${hzVoisine}`);
    const sessionVoisine = voisine.replace(/^w:/, '');
    const debut = maintenantIso();

    await marquerCachee(cdp, voisine, true, 'critère ④ — masquage');
    const sommeilConfirme = await attendreLeFait('sommeil de la voisine (endormie=true)', async () => {
        const cad = await cadencesAgent(debut, maintenantIso());
        if (cad.endormies.some((e) => e.startsWith(`${sessionVoisine}@`))) return cad;
        const parts = await partsAgent(debut, maintenantIso());
        return parts.some((p) => p.session === sessionVoisine && p.endormie) ? { parts } : null;
    }, 30);

    // ⑤ : la voisine dort (plus de capture ni d'encodage vidéo) — son audio
    // doit pourtant continuer d'arriver. Jamais un compte d'octets (D7 : un
    // `bytesReceived` peut croître sur un spectre à −1000 dB) : on lit la
    // fréquence DOMINANTE, comparée au ton assigné à CETTE fenêtre.
    const audioPendantSommeil = await cdp.evalBorne(
        sidDe(voisine), expressionReleveFrequence([hzVoisine]), 8000, true);

    // Revue de la tâche 10, Critique 2 : la voisine émettait DÉJÀ des lignes
    // `cadence du capteur` avant d'être masquée (elle diffusait normalement
    // depuis l'ouverture), donc `hasOwnProperty(cad.capteur, sessionVoisine)`
    // sur une fenêtre partant de `debut` (avant le masquage) serait vrai dès
    // le premier sondage, QUE le réveil ait fonctionné ou non — un contrôle
    // qui ne peut pas échouer n'en est pas un. Remède : borner la fenêtre
    // d'observation à `debutReveil`, pris APRÈS l'ordre de réveil, et exiger
    // un FAIT qui n'existait pas avant ce geste — une ligne `endormie=false`
    // POSTÉRIEURE à l'ordre (`cad.eveillees`), pas la seule présence d'une clé.
    const debutReveil = maintenantIso();
    await marquerCachee(cdp, voisine, false, 'critère ④ — réveil');
    const reveilConfirme = await attendreLeFait('réveil de la voisine (endormie=false, postérieur à l\'ordre)', async () => {
        const cad = await cadencesAgent(debutReveil, maintenantIso());
        return cad.eveillees.some((e) => e.startsWith(`${sessionVoisine}@`)) ? cad : null;
    }, 30);
    const fin = maintenantIso();

    const releve = {
        voisine, session_voisine: sessionVoisine, hz_assigne: hzVoisine, debut, fin,
        sommeil_confirme: sommeilConfirme,
        audio_pendant_sommeil: audioPendantSommeil,
        // Jugement : le niveau à SA fréquence doit dominer un plancher net, la
        // même marge que D7 (`MARGE_SIGNAL_DB`, ~30 dB) — sans reprendre son
        // arbitrage multi-fréquence (D8 ne teste pas l'isolation, seulement la
        // survie), un simple écart au plancher suffit ici.
        audio_survit: !!(audioPendantSommeil && !audioPendantSommeil.erreur
            && (audioPendantSommeil.niveaux?.[0]?.db ?? -1000) - (audioPendantSommeil.plancher_db ?? 0) >= 20),
        reveil_confirme: reveilConfirme,
        survie: vmVivante('après critère ④+⑤'),
    };
    log('CRITÈRE ④+⑤ — RELEVÉ ' + JSON.stringify(releve, null, 1));
    return releve;
}

// ---------------------------------------------------------------- main
async function main() {
    const port = Number(process.env.PORT_CDP ?? 9994);
    const userDataDir = await mkdtemp(join(tmpdir(), 'chrome-d8-'));
    const chrome = spawn(process.env.CHROME_BIN ?? 'google-chrome', [
        '--headless=new', `--remote-debugging-port=${port}`, '--remote-allow-origins=*',
        `--user-data-dir=${userDataDir}`, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
        '--window-size=1280,720', '--ozone-override-screen-size=1600,1000',
        '--autoplay-policy=no-user-gesture-required',
        '--disable-features=WebRtcHideLocalIpsWithMdns', '--disable-popup-blocking',
        // Garde-fou 5/6 (paliers longs) : ces trois drapeaux anti-gel de Chrome
        // sont nécessaires dès qu'une mesure dépasse ~5 min (piège TURN/D2).
        '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding', 'about:blank',
    ], { stdio: 'ignore' });
    log(`ÉTIQUETTE=${ETIQUETTE} N_FENETRES=${N_FENETRES} PALIER_S=${PALIER_S} BITRATE=${BITRATE} `
        + `BUDGET_BPS=${BUDGET_BPS} PLEIN_ECRAN=${PLEIN_ECRAN} AUDIO=${AUDIO} chrome pid=${chrome.pid}`);

    const releve = { etiquette: ETIQUETTE, n_fenetres: N_FENETRES, palier_s: PALIER_S, phases: {} };
    try {
        const version = await attendreDevtools(port);
        const qui = spawnSync('bash', ['-c',
            `ss -ltnp 2>/dev/null | grep ':${port} ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1`],
            { encoding: 'utf8' }).stdout.trim();
        log(`identité du navigateur : pid écoutant=${qui} pid lancé=${chrome.pid} version=${version.Browser}`);
        if (String(qui) !== String(chrome.pid)) throw new Error(`port ${port} tenu par ${qui}, pas ${chrome.pid}`);
        releve.navigateur = version.Browser;

        const cdp = new Cdp(version.webSocketDebuggerUrl);
        cdp.on(async (m) => {
            if (m.method === 'Target.attachedToTarget') {
                const { sessionId, targetInfo } = m.params;
                if (targetInfo.type !== 'page') {
                    await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
                    return;
                }
                pages.set(sessionId, { targetId: targetInfo.targetId, url: targetInfo.url });
                log(`+ page attachée  session=${sessionId.slice(0, 8)} url=${targetInfo.url}`);
                await cdp.send('Page.enable', {}, sessionId).catch(() => { });
                await cdp.send('Runtime.enable', {}, sessionId).catch(() => { });
                // Garde-fou 2 : reposée EXPLICITEMENT, `Page.addScriptToEvaluateOnNewDocument`
                // ne courant pas sur une page ouverte par `window.open`.
                await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: AMORCE }, sessionId).catch(() => { });
                await cdp.send('Runtime.evaluate', { expression: AMORCE, returnByValue: true }, sessionId).catch(() => { });
                if (VIEWPORT_FORCE) {
                    const [L, H] = VIEWPORT_FORCE.split('x').map(Number);
                    await cdp.send('Emulation.setDeviceMetricsOverride',
                        { width: L, height: H, deviceScaleFactor: 1, mobile: false }, sessionId).catch(() => { });
                }
                await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
            } else if (m.method === 'Target.detachedFromTarget') {
                pages.delete(m.params.sessionId);
            } else if (m.method === 'Target.targetInfoChanged') {
                for (const [, p] of pages) {
                    if (p.targetId === m.params.targetInfo.targetId) p.url = m.params.targetInfo.url;
                }
            }
        });
        await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
        await cdp.send('Target.setDiscoverTargets', { discover: true });

        log('>>> ÉTAPE 0 : préparation — VM sans fenêtre éligible');
        releve.cpu_repos = cpuHote('au repos, avant toute session');

        await cdp.send('Target.createTarget', { url: URL_SHELL });
        await dodo(3000);
        const sidShell = [...pages].find(([, p]) => p.url.includes('shell.html'))?.[0];
        if (!sidShell) throw new Error("la page-shell ne s'est pas attachée");
        log('statut shell :', await cdp.eval(sidShell, `document.querySelector('#statut').textContent`));

        log('>>> lancement du superviseur');
        const sup = spawnSync('bash', ['-c',
            `cd ${RACINE} && SUPERVISEUR=1 BITRATE=${BITRATE} BUDGET_BPS=${BUDGET_BPS} ` +
            `PLEIN_ECRAN=${PLEIN_ECRAN} AUDIO=${AUDIO} ` +
            `SIGNALING_URL=ws://${HOTE}:8080 LOCAL_IP=192.168.3.2 RUST_LOG=info scripts/run-agent.sh`],
            { encoding: 'utf8', env: process.env });
        log('run-agent.sh :', (sup.stdout ?? '').trim().replace(/\n/g, ' / '));
        await dodo(6000);

        // ---- ouverture des N fenêtres (garde-fou 1 : un --user-data-dir chacune) ----
        const fenetres = [];
        for (let n = 1; n <= N_FENETRES; n += 1) {
            const r = await ouvrirFenetre(n);
            fenetres.push(r);
            if (!r.ok) break;
            await imposerScenario(cdp, nomsTries()[0], `après ouverture ${n}`);
        }
        log(`  ${fenetres.filter((f) => f.ok).length}/${N_FENETRES} fenêtres ouvertes`);
        await dodo(6000);
        const noms = nomsTries();
        if (noms.length < 2) throw new Error(`au moins deux fenêtres sont requises pour observer des voisines (${noms.length} ouvertes)`);

        releve.phases.temoin = await phaseTemoin(cdp);

        const cible = noms[0];
        const marqueurCible = marqueurFenetre(1);
        releve.phases.critere1et2 = await phaseCritere1Et2(cdp, cible, marqueurCible);

        const voisine = noms[1];
        const hzVoisine = hzDe(2);
        releve.phases.critere4et5 = await phaseCritere4Et5(cdp, voisine, hzVoisine);

        releve.marqueurs = await marqueurs('FIN');
        await writeFile(SORTIE_JSON, JSON.stringify(releve, null, 1));
        log('relevé écrit dans ' + SORTIE_JSON);
    } finally {
        chrome.kill('SIGKILL');
        await rm(userDataDir, { recursive: true, force: true }).catch(() => { });
        // Les enfants meurent quand le navigateur se ferme : copier APRÈS ce
        // délai, pas à la fin du pilote (leçon D4 — une pièce perdue ainsi).
        await dodo(8000);
        copierLog();
        log('journal copié dans ' + COPIE_LOG + ' (après la fermeture du navigateur)');
    }
}

main().catch((e) => { console.error('ERREUR FATALE', e); process.exit(1); });
