// Pilote du lot 22 : « un pair se connecte, lance une application, et voit sa
// fenêtre » — le PARCOURS, pas une moitié.
//
// 🔴 POURQUOI UN PILOTE DE PLUS PLUTÔT QUE `journaux-lot17/instrument/
// pilote-pair-tardif.mjs`. Celui-là est LU ET RÉUTILISÉ dans sa structure
// (jeton par `X-Pomerium-Claim-Email`, `POST /session` pour le préfixe,
// `WebSocket` natif de Node), et trois différences seulement justifient un
// fichier distinct :
//   ① il ne répond un `viewport` QU'À LA PREMIÈRE annonce (`!relevé
//      .viewport_envoye`), or le superviseur en redit plusieurs d'un coup sur
//      `pair-present` : la fenêtre qu'on vient de lancer n'est alors PAS celle
//      qui reçoit la réponse, et la mesure porte sur la mauvaise ;
//   ② il ouvre son témoin par `schtasks`, jamais par la ROUTE DE LANCEMENT de
//      la plateforme — or c'est exactement le geste de l'utilisateur qu'il
//      s'agit ici de rejouer (`POST /application/<id>/lancer`) ;
//   ③ il ne relève pas le MOTIF des refus, qui est ce qui distingue « aucune
//      page-shell n'écoutait » de « aucune sortie d'affichage ne peut servir
//      cette fenêtre ».
//
// 🔴 CE QU'IL N'ÉTABLIT PAS, ET IL FAUT LE DIRE : qu'un média traverse. Il n'a
// aucune pile WebRTC et n'émet aucune offre SDP. Il éprouve la chaîne
// hub → plateforme → agent → superviseur → sortie virtuelle, et s'arrête là où
// le navigateur commencerait. Le navigateur réel reste hors d'atteinte (OAuth
// Google exige un humain — blocage ② du critère ⑦ d'`auth-pomerium`).
//
// 🔴 LE VERDICT EST « OUVERTE ET NON REFUSÉE ». Recevoir `fenetre-ouverte`
// puis un `refus` sur la MÊME session n'est PAS un vert : c'est le mur mesuré
// le 30 août 2026 (« aucune sortie d'affichage ne peut servir cette fenêtre »).
//
// Usage :
//   node pilote-parcours.mjs --etiquette=rouge --viewport=1860x1080 \
//        --lancer=<uuid|nom> --secondes=60
//
// ⚠️ Le jeton obtenu N'EST JAMAIS JOURNALISÉ NI ÉCRIT DANS LE RELEVÉ.
// ⚠️ Il doit tourner SUR l'hôte de la plateforme : `X-Pomerium-Claim-Email`
//    n'est cru que d'un pair listé dans `PLATEFORME_PROXY_DE_CONFIANCE`.
import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const arg = (n, d) =>
    (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const PLATEFORME = process.env.PLATEFORME ?? 'http://192.168.3.1:3445';
const COURRIEL = process.env.COURRIEL ?? 'maxime.g.allanic@gmail.com';
const ETIQUETTE = arg('etiquette', '1');
const SECONDES = Number(arg('secondes', '60'));
/// La taille que la page de session annoncerait. C'est le paramètre du bras :
/// `1860x1080` est ce que le navigateur du propriétaire a réellement demandé
/// le 30 août 2026, `1280x720` le témoin.
const VIEWPORT = arg('viewport', '1280x720');
/// L'application à lancer, par identifiant OU par sous-chaîne de son nom.
/// Vide = ne rien lancer (bras « je regarde seulement »).
const LANCER = arg('lancer', '');
const LANCER_APRES = Number(arg('lancer-apres', '6'));
const ICI = dirname(fileURLToPath(import.meta.url));
const SORTIE = arg('sortie', join(ICI, '..', `parcours-${ETIQUETTE}.json`));
const log = (...a) => console.log(new Date().toISOString(), ...a);

async function jetonUtilisateur() {
    const r = await fetch(`${PLATEFORME}/auth/moi`, {
        headers: { 'X-Pomerium-Claim-Email': COURRIEL },
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`/auth/moi a refusé : ${r.status} ${JSON.stringify(corps)}`);
    const jeton = corps.acces ?? corps.jeton;
    if (!jeton) throw new Error(`/auth/moi n'a rendu aucun jeton`);
    return jeton;
}

async function laVm(jeton) {
    const r = await fetch(`${PLATEFORME}/session`, {
        method: 'POST',
        headers: { authorization: `Bearer ${jeton}` },
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`POST /session a refusé : ${r.status} ${JSON.stringify(corps)}`);
    return corps;
}

/// Le geste de l'utilisateur, par la ROUTE que le bouton « Lancer » du hub
/// frappe — jamais par `schtasks`. C'est ce qui rend ce pilote un parcours et
/// non une injection.
async function lancer(jeton, vm, motif) {
    const liste = await fetch(`${PLATEFORME}/applications?vm=${encodeURIComponent(vm)}`, {
        headers: { authorization: `Bearer ${jeton}` },
    });
    const corps = await liste.json().catch(() => undefined);
    if (!liste.ok) throw new Error(`catalogue refusé : ${liste.status} ${JSON.stringify(corps)}`);
    const apps = corps.applications ?? corps;
    const cible =
        apps.find((a) => a.id === motif) ??
        apps.find((a) => String(a.nom).toLowerCase().includes(motif.toLowerCase()));
    if (cible === undefined) {
        throw new Error(`aucune application ne correspond à « ${motif} » parmi ${apps.length}`);
    }
    const r = await fetch(`${PLATEFORME}/application/${encodeURIComponent(cible.id)}/lancer`, {
        method: 'POST',
        headers: { authorization: `Bearer ${jeton}` },
    });
    const issue = await r.json().catch(() => undefined);
    return { nom: cible.nom, id: cible.id, statut: r.status, reponse: issue };
}

const relevé = { etiquette: ETIQUETTE, instant: new Date().toISOString(), viewport: VIEWPORT, messages: [] };
const jeton = await jetonUtilisateur();
const vm = await laVm(jeton);
relevé.vm = vm.vm;
relevé.etat_vm = vm.etat;
relevé.session = vm.prefixe ? `${vm.prefixe}:bureau` : 'bureau';
log('session de contrôle visée :', relevé.session, '| état de la VM :', vm.etat);

const ws = new WebSocket(`${PLATEFORME.replace(/^http/, 'ws')}/signal`);
const debut = Date.now();
const [largeur, hauteur] = VIEWPORT.split('x').map(Number);
/// Les sessions de fenêtre annoncées APRÈS le lancement — les seules qui
/// jugent. Celles d'avant sont l'arriéré que `pair-present` fait redire.
const apresLancement = new Set();
let lancementFait = false;

ws.addEventListener('open', () => {
    log('socket ouvert, déclaration du rôle client');
    ws.send(JSON.stringify({ role: 'client', session: relevé.session, jeton }));
});
ws.addEventListener('message', (evenement) => {
    const message = JSON.parse(String(evenement.data));
    const trace = message.type === 'ice-config' ? { type: 'ice-config' } : message;
    relevé.messages.push({ ms: Date.now() - debut, ...trace });
    log('reçu', JSON.stringify(trace));
    if (message.type === 'fenetre-ouverte') {
        // 🔴 RÉPONDRE À CHACUNE, jamais à la première seule : le superviseur
        // redit tout son arriéré d'un coup sur `pair-present`, et la fenêtre
        // qu'on vient de lancer serait alors la seule sans viewport.
        if (lancementFait) apresLancement.add(message.session);
        ws.send(JSON.stringify({ type: 'viewport', session: message.session, largeur, hauteur }));
    }
});
ws.addEventListener('error', (e) => { relevé.erreur = String(e.message ?? e.type ?? e); log('ERREUR', relevé.erreur); });
ws.addEventListener('close', (e) => {
    relevé.fermeture = { code: e.code, motif: String(e.reason ?? '') };
    log('socket fermé', JSON.stringify(relevé.fermeture));
});

if (LANCER) {
    await new Promise((r) => setTimeout(r, LANCER_APRES * 1000));
    relevé.lancement = await lancer(jeton, relevé.vm, LANCER);
    lancementFait = true;
    relevé.lancement.ms = Date.now() - debut;
    log('lancement :', JSON.stringify(relevé.lancement));
}
await new Promise((r) => setTimeout(r, Math.max(0, SECONDES - (LANCER ? LANCER_APRES : 0)) * 1000));
ws.close();

const ouvertures = relevé.messages.filter((m) => m.type === 'fenetre-ouverte');
const refus = relevé.messages.filter((m) => m.type === 'refus');
relevé.compte_fenetre_ouverte = ouvertures.length;
relevé.compte_refus = refus.length;
relevé.motifs_de_refus = [...new Set(refus.map((m) => m.motif))];
relevé.titres = ouvertures.map((m) => m.titre);
relevé.sessions_apres_lancement = [...apresLancement];
// 🔴 LE JUGE. Une fenêtre annoncée APRÈS le lancement, et JAMAIS refusée
// pendant la fenêtre d'observation. « Ouverte » seul ne suffit pas : c'est ce
// que rendait déjà le produit du 30 août 2026 au matin.
const refusees = new Set(
    relevé.messages.filter((m) => m.type === 'refus').map((m) => m.titre),
);
relevé.tenues = ouvertures
    .filter((m) => apresLancement.has(m.session) && !refusees.has(m.titre))
    .map((m) => ({ session: m.session, titre: m.titre }));
relevé.verdict = relevé.tenues.length > 0 ? 'VERT' : 'ROUGE';
mkdirSync(dirname(SORTIE), { recursive: true });
writeFileSync(SORTIE, JSON.stringify(relevé, null, 2));
log('---');
log(`VERDICT ${relevé.verdict} : ouvertes=${ouvertures.length} refus=${refus.length} tenues=${relevé.tenues.length}`);
log('motifs de refus :', JSON.stringify(relevé.motifs_de_refus));
log('relevé écrit :', SORTIE);
process.exit(0);
