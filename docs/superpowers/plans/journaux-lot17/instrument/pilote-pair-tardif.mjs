// Pilote de l'épreuve du lot 17 : un pair qui arrive TARD voit-il les fenêtres ?
//
// 🔴 CE QU'IL JOUE, ET POURQUOI IL NE JOUE PAS UN NAVIGATEUR. Le juge du lot
// est « un pair qui se connecte plusieurs minutes après le démarrage de
// l'agent voit les fenêtres ». Le navigateur réel est hors d'atteinte : le
// flux OAuth Google de Pomerium exige un humain, ce que ce dépôt tient pour
// un blocage nommé depuis le chantier `auth-pomerium` (critère ⑦, toujours
// NON JOUÉ). On joue donc le pair AU NIVEAU DU SIGNALING — c'est exactement
// le rôle `client` que la page-shell occupe, avec le même jeton, sur la même
// session de contrôle, par la même poignée de main.
//
// ⚠️ CE QU'IL N'ÉTABLIT DONC PAS : que la page-shell OUVRE les fenêtres. Il
// établit que les annonces lui PARVIENNENT, ce qui est précisément le maillon
// que la capture réseau du 30 août 2026 a montré rompu.
//
// 🔴 IL N'ÉCRIT RIEN SUR LA VM ET NE RELANCE RIEN : il lit. C'est
// l'opérateur qui décide quand relancer l'agent, jamais ce pilote.
//
// Usage :
//   node pilote-pair-tardif.mjs --etiquette=rouge --secondes=25
//
// Variables : PLATEFORME (défaut http://192.168.3.1:3445),
//             COURRIEL (défaut maxime.g.allanic@gmail.com).
//
// ⚠️ Le jeton obtenu N'EST JAMAIS JOURNALISÉ NI ÉCRIT DANS LE RELEVÉ.
// 🔴 LE `WebSocket` NATIF DE NODE, JAMAIS LE PAQUET `ws` — et ce n'est pas
// une préférence. Ce fichier vit sous `docs/`, où aucun `node_modules` ne
// remonte : un `import ... from 'ws'` y échoue en `ERR_MODULE_NOT_FOUND`
// quel que soit le répertoire courant, la résolution ESM partant de
// l'EMPLACEMENT du fichier et non du `cwd`. Mesuré, deux fois, avant de
// basculer. Node 22+ expose `WebSocket` en global.
import { writeFileSync, mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const arg = (n, d) =>
    (process.argv.find((a) => a.startsWith(`--${n}=`)) ?? `--${n}=${d}`).split('=').slice(1).join('=');
const PLATEFORME = process.env.PLATEFORME ?? 'http://192.168.3.1:3445';
const COURRIEL = process.env.COURRIEL ?? 'maxime.g.allanic@gmail.com';
const ETIQUETTE = arg('etiquette', '1');
const SECONDES = Number(arg('secondes', '25'));
/// Secondes d'attente AVANT de se connecter. C'est la condition même de
/// l'épreuve : le pair doit arriver APRÈS que l'agent a annoncé ses fenêtres
/// et APRÈS que `DELAI_ATTENTE_VIEWPORT_MAX` (30 s) les a fait abandonner.
/// Une valeur inférieure à 40 s ne mesurerait rien.
const ATTENTE = Number(arg('attente', '0'));
/// 🔴 LE TÉMOIN POSITIF, ET IL EST INDISPENSABLE. Un `fenetre-ouverte=0`
/// rendu par un agent qui n'a AUCUNE fenêtre à montrer n'est pas une mesure —
/// c'est le piège « un zéro n'est interprétable qu'avec un témoin négatif »
/// que ce dépôt a payé plusieurs fois. Ces deux options ouvrent une fenêtre
/// SUR LA VM pendant que le pilote est déjà connecté : ce qui arrive alors
/// établit que l'agent annonce bien cette classe de fenêtre, et rend le zéro
/// du bras tardif discriminant.
///
/// ⚠️ La fenêtre est ouverte par une TÂCHE PLANIFIÉE `/it` : lancée par WinRM
/// nue, elle naîtrait en SESSION 0, où l'agent ne la verrait jamais.
const OUVRIR = arg('ouvrir', '');
const OUVRIR_APRES = Number(arg('ouvrir-apres', '8'));
/// `--viewport=1280x720` : répondre à la PREMIÈRE annonce comme la page-shell
/// le ferait, pour que l'agent aille jusqu'au bout de sa chaîne — sortie
/// virtuelle, lancement de l'enfant, duplication DXGI, encodeur.
///
/// ⚠️ **CE N'EST PAS UNE SESSION MÉDIA** : ce pilote n'a pas de pile WebRTC
/// et n'envoie aucune offre SDP. Il éprouve la moitié WINDOWS de la chaîne,
/// et s'arrête là où le navigateur commencerait. Le dire, c'est refuser de
/// faire passer une demi-preuve pour une preuve entière.
const VIEWPORT = arg('viewport', '');
const ICI = dirname(fileURLToPath(import.meta.url));
const SORTIE = arg('sortie', join(ICI, '..', `pair-tardif-${ETIQUETTE}.json`));
const log = (...a) => console.log(new Date().toISOString(), ...a);

/// L'identité arrive par l'en-tête que le proxy pose — c'est le mode
/// `pomerium`, celui de la production. La plateforme n'accepte cet en-tête
/// que d'un pair listé dans `PLATEFORME_PROXY_DE_CONFIANCE` : ce pilote doit
/// donc tourner SUR l'hôte de la plateforme, sinon il reçoit
/// `401 pair-non-de-confiance` — et ce refus-là est un résultat, pas une
/// panne du pilote.
async function jetonUtilisateur() {
    const r = await fetch(`${PLATEFORME}/auth/moi`, {
        headers: { 'X-Pomerium-Claim-Email': COURRIEL },
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`/auth/moi a refusé : ${r.status} ${JSON.stringify(corps)}`);
    const jeton = corps.acces ?? corps.jeton;
    if (!jeton) throw new Error(`/auth/moi n'a rendu aucun jeton : ${JSON.stringify(Object.keys(corps))}`);
    return jeton;
}

/// Le préfixe de la VM, tel que la page-shell l'obtient elle-même.
async function sessionDeControle(jeton) {
    const r = await fetch(`${PLATEFORME}/session`, {
        method: 'POST',
        headers: { authorization: `Bearer ${jeton}` },
    });
    const corps = await r.json().catch(() => undefined);
    if (!r.ok) throw new Error(`POST /session a refusé : ${r.status} ${JSON.stringify(corps)}`);
    return { prefixe: corps.prefixe, etat: corps.etat, vm: corps.vm };
}

/// Ouvre une fenêtre dans la SESSION INTERACTIVE de la VM.
///
/// Passe par `installer/console/guest/winrm_exec.py` (NTLM) : `scripts/winrm.js`
/// (transport Basic) rend 401 sur cette appliance depuis le 22 août 2026.
function ouvrirUneFenetreSurLaVm(exe) {
    const tache = `lot17-temoin-${Date.now()}`;
    const ps = [
        `schtasks /Create /TN ${tache} /TR '${exe}' /SC ONCE /ST 00:00 /RU Administrator /IT /F | Out-Null`,
        `schtasks /Run /TN ${tache} | Out-Null`,
        `Start-Sleep -Seconds 4`,
        `'session=' + ((Get-Process -Name ([System.IO.Path]::GetFileNameWithoutExtension('${exe}')) -ErrorAction SilentlyContinue | Select-Object -First 1).SessionId)`,
    ].join('; ');
    const r = spawnSync(
        'python3',
        [`${process.env.WINRM_EXEC ?? '/home/mallanic/Projects/Nivuus/packages/installer/console/guest/winrm_exec.py'}`, 'ps', ps],
        { encoding: 'utf8', timeout: 120000 },
    );
    return `${r.stdout ?? ''}${r.stderr ?? ''}`.split('\n').filter((l) => l.startsWith('session=')).join(' ');
}

const relevé = {
    etiquette: ETIQUETTE,
    instant: new Date().toISOString(),
    secondes: SECONDES,
    messages: [],
};

if (ATTENTE > 0) {
    log(`attente de ${ATTENTE} s avant de se connecter : le pair doit arriver TARD`);
    await new Promise((r) => setTimeout(r, ATTENTE * 1000));
}
relevé.attente_s = ATTENTE;
const jeton = await jetonUtilisateur();
const { prefixe, etat, vm } = await sessionDeControle(jeton);
relevé.vm = vm;
relevé.etat_vm = etat;
relevé.session = prefixe ? `${prefixe}:bureau` : 'bureau';
log('session de contrôle visée :', relevé.session, '| état de la VM :', etat);

const url = `${PLATEFORME.replace(/^http/, 'ws')}/signal`;
const ws = new WebSocket(url);
const debut = Date.now();

ws.addEventListener('open', () => {
    log('socket ouvert, déclaration du rôle client');
    ws.send(JSON.stringify({ role: 'client', session: relevé.session, jeton }));
});
ws.addEventListener('message', (evenement) => {
    const message = JSON.parse(String(evenement.data));
    // ⚠️ `ice-config` porte des identifiants TURN à durée de vie : on retient
    // son TYPE et rien d'autre. Un relevé versionné ne porte aucun secret.
    const trace = message.type === 'ice-config' ? { type: 'ice-config' } : message;
    relevé.messages.push({ ms: Date.now() - debut, ...trace });
    log('reçu', JSON.stringify(trace));
    if (VIEWPORT && message.type === 'fenetre-ouverte' && !relevé.viewport_envoye) {
        const [largeur, hauteur] = VIEWPORT.split('x').map(Number);
        ws.send(JSON.stringify({ type: 'viewport', session: message.session, largeur, hauteur }));
        relevé.viewport_envoye = { session: message.session, largeur, hauteur, ms: Date.now() - debut };
        log('viewport envoyé', JSON.stringify(relevé.viewport_envoye));
    }
});
ws.addEventListener('error', (e) => {
    relevé.erreur = String(e.message ?? e.type ?? e);
    log('ERREUR socket', relevé.erreur);
});
// La fermeture PORTE LE MOTIF quand la plateforme refuse : sans ce bras, un
// refus de poignée de main se lirait comme « aucun message », qui est aussi
// ce que rend le défaut qu'on mesure. Les deux doivent être discernables.
ws.addEventListener('close', (e) => {
    relevé.fermeture = { code: e.code, motif: String(e.reason ?? '') };
    log('socket fermé', JSON.stringify(relevé.fermeture));
});

if (OUVRIR) {
    await new Promise((r) => setTimeout(r, OUVRIR_APRES * 1000));
    log(`témoin positif : ouverture de ${OUVRIR} sur la VM, en session interactive`);
    relevé.temoin_positif = { exe: OUVRIR, ms: Date.now() - debut, session: ouvrirUneFenetreSurLaVm(OUVRIR) };
    log('témoin positif :', JSON.stringify(relevé.temoin_positif));
    await new Promise((r) => setTimeout(r, Math.max(0, SECONDES - OUVRIR_APRES) * 1000));
} else {
    await new Promise((r) => setTimeout(r, SECONDES * 1000));
}
ws.close();

const ouvertures = relevé.messages.filter((m) => m.type === 'fenetre-ouverte');
const refus = relevé.messages.filter((m) => m.type === 'refus');
relevé.compte_fenetre_ouverte = ouvertures.length;
relevé.compte_refus = refus.length;
relevé.titres = ouvertures.map((m) => m.titre);
mkdirSync(dirname(SORTIE), { recursive: true });
writeFileSync(SORTIE, JSON.stringify(relevé, null, 2));
log('---');
log(`VERDICT : fenetre-ouverte=${ouvertures.length} refus=${refus.length}`);
log('titres :', JSON.stringify(relevé.titres));
log('relevé écrit :', SORTIE);
process.exit(0);
