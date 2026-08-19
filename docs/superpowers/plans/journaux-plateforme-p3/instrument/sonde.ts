// Les sondes de la recette du sous-bloc P3. Un sous-commande par critère.
//
//     tsx sonde.ts <critere-1|critere-2|critere-3|critere-4|e2-ferme> <sqlite|postgres> <commit>
//
// 🔴 CHAQUE SONDE ÉCRIT SON RELEVÉ SUR LA SORTIE STANDARD, et rend un code de
// sortie NON NUL si son critère n'est pas tenu. Un journal qui se contenterait
// d'imprimer sans juger ne pourrait pas rougir — c'est le patron que ce dépôt
// a payé quatre fois au sous-bloc D10.
//
// ⚠️ AUCUN SECRET N'EST ÉCRIT EN CLAIR. Les secrets d'enrôlement sont tirés au
// sort par `enrolerLaVm` et n'apparaissent dans aucun journal versé : seules
// leur LONGUEUR et leur égalité/différence sont relevées. Les jetons signés ne
// sont montrés que par leur préfixe d'en-tête et leur longueur.

import { enrolerLaVm } from '../../../../../plateforme/src/admin/enroler-agent';
import { lireParVm } from '../../../../../plateforme/src/depot/agent';
import { etatDe, SEUIL_INJOIGNABLE_MS } from '../../../../../plateforme/src/agents/fraicheur';
import {
    encodeBattement,
    encodeEnroler,
    parseDepuisLaPlateforme,
    parseVersLaPlateforme,
    PLATEFORME_VERSION,
} from '../../../../../proto/ts/plateforme';
import { demarrerService, entete, ligne, Pair, poignee, type Moteur } from './socle';

/// Le titre que porte l'en-tête de chaque journal.
const TITRES: Record<string, string> = {
    'critere-1': 'CRITÈRE ① — deux agents simulés, deux VMs, aucun conflit ; et chacun ne voit que sa session',
    'critere-2': 'CRITÈRE ② — un agent au mauvais secret est refusé, sans distinguer « VM inconnue » de « secret faux »',
    'critere-3': 'CRITÈRE ③ — une version de protocole incompatible est refusée des deux côtés',
    'critere-4': 'CRITÈRE ④ — un agent muet est vu comme tel',
    'e2-ferme': 'E2 FERMÉE — le pair qui se déclare agent sans rien présenter',
    'enrolements-concurrents':
        'HORS CRITÈRE — N enrôlements CONCURRENTS pour la MÊME VM (le superviseur et ses N-1 enfants)',
};

const [critere, moteurBrut, commit] = process.argv.slice(2);
const moteur = moteurBrut as Moteur;
if (!['sqlite', 'postgres'].includes(moteur)) {
    throw new Error(`moteur attendu sqlite|postgres, reçu : ${moteurBrut}`);
}

const sortie: string[] = [];
function dire(texte = ''): void {
    sortie.push(texte);
}
let echecs = 0;
function juger(nom: string, tenu: boolean): void {
    dire(ligne(nom, tenu ? 'TENU' : '🔴 NON TENU'));
    if (!tenu) echecs += 1;
}

/// Rend les revendications d'un jeton — elles ne portent aucun secret, et ce
/// sont elles qui montrent le claim de type `sty` et le sujet = préfixe.
function decoderCharge(jeton: string): unknown {
    const charge = jeton.split('.')[1];
    if (charge === undefined) return '<illisible>';
    try {
        return JSON.parse(Buffer.from(charge, 'base64url').toString('utf8'));
    } catch {
        return '<illisible>';
    }
}

/// Cache le corps d'un jeton : sa présence et sa forme suffisent au relevé.
function jetonAbrege(jeton: string): string {
    const parts = jeton.split('.');
    return `${parts[0]}.<charge:${parts[1]?.length ?? 0}>.<sig:${parts[2]?.length ?? 0}>`;
}

/// Enrôle une VM et ouvre son canal `/agent`. Rend l'identité délivrée.
async function enrolerEtOuvrir(
    port: number,
    base: Parameters<typeof lireParVm>[0],
    nom: string,
): Promise<{ vmId: string; secret: string; prefixe: string; jeton: string; canal: Pair }> {
    const { vmId, secret, prefixe } = await enrolerLaVm(base, nom, '192.168.3.2', Date.now());
    const canal = await Pair.ouvrir(`ws://127.0.0.1:${port}/agent`);
    canal.envoyer(encodeEnroler(vmId, secret));
    await canal.attendre(1);
    const message = parseDepuisLaPlateforme(canal.recues[0].brut);
    if (message.type !== 'enrole') throw new Error(`enrôlement refusé : ${canal.recues[0].brut}`);
    if (message.prefixe !== prefixe) {
        throw new Error(`préfixe divergent : base ${prefixe}, canal ${message.prefixe}`);
    }
    return { vmId, secret, prefixe, jeton: message.jeton, canal };
}

/// Ouvre une poignée de main de rôle `agent` sur le relais et relève ce
/// qu'elle obtient. ⚠️ ON RELÈVE `ice-config` SÉPARÉMENT du refus : un service
/// qui refuserait APRÈS avoir envoyé la configuration TURN aurait déjà tout
/// donné (c'est ce que `garde-fil.test.ts` tient de son côté).
async function tenterSession(
    port: number,
    session: string,
    jeton?: string,
    role = 'agent',
): Promise<{ acceptee: boolean; iceConfig: boolean; refus?: string; brut: string[] }> {
    const pair = await Pair.ouvrir(`ws://127.0.0.1:${port}/`);
    pair.envoyer(poignee(role, session, jeton));
    await pair.attendre(1);
    const brut = pair.recues.map((t) => t.brut);
    const erreurs = brut
        .map((b) => JSON.parse(b) as Record<string, unknown>)
        .filter((m) => m.type === 'error');
    const iceConfig = brut.some((b) => (JSON.parse(b) as { type?: string }).type === 'ice-config');
    // ⚠️ « ACCEPTÉE » NE PEUT PAS SE LIRE SUR LA SEULE PRÉSENCE D'`ice-config` :
    // sans TURN_URL, le relais n'en envoie AUCUNE et le pair accepté ne reçoit
    // rien du tout. L'acceptation se lit donc à l'ABSENCE de refus, socket
    // toujours ouvert — et `ice-config` est relevé à part.
    const acceptee = erreurs.length === 0 && pair.fermeture === undefined;
    return {
        acceptee,
        iceConfig,
        refus: erreurs.length > 0 ? String(erreurs[0].reason) : undefined,
        brut,
    };
}

const service = await demarrerService(moteur, critere);
const port = service.port;
dire(entete(TITRES[critere] ?? critere, moteur, commit ?? 'inconnu'));

try {
    if (critere === 'critere-1') await critere1();
    else if (critere === 'critere-2') await critere2();
    else if (critere === 'critere-3') await critere3();
    else if (critere === 'critere-4') await critere4();
    else if (critere === 'e2-ferme') await e2Ferme();
    else if (critere === 'enrolements-concurrents') await enrolementsConcurrents();
    else throw new Error(`sonde inconnue : ${critere}`);
} finally {
    await service.arreter();
}

dire();
dire(ligne('VERDICT', echecs === 0 ? 'TOUT TENU' : `🔴 ${echecs} assertion(s) NON TENUE(S)`));
process.stdout.write(`${sortie.join('\n')}\n`);
process.exit(echecs === 0 ? 0 : 1);

// ---------------------------------------------------------------------------

async function critere1(): Promise<void> {
    const p = await enrolerEtOuvrir(port, service.base, 'vm-p');
    const q = await enrolerEtOuvrir(port, service.base, 'vm-q');

    dire(ligne('VM P — préfixe délivré', p.prefixe));
    dire(ligne('VM Q — préfixe délivré', q.prefixe));
    dire(ligne('longueur des deux préfixes', [p.prefixe.length, q.prefixe.length]));
    dire(ligne('P — jeton (corps masqué)', jetonAbrege(p.jeton)));
    dire(ligne('Q — jeton (corps masqué)', jetonAbrege(q.jeton)));
    juger('préfixes DISTINCTS', p.prefixe !== q.prefixe);
    juger('jetons DISTINCTS', p.jeton !== q.jeton);
    dire();

    // Chaque agent ouvre SA session de contrôle et SA première fenêtre.
    // 🔴 C'est ici que le binaire de P2 se casse : sans préfixe, les deux
    // nomment leur session de contrôle `bureau`, et le second est refusé.
    const sessions: Record<string, Awaited<ReturnType<typeof tenterSession>>> = {};
    for (const [nom, ident] of [
        ['P', p],
        ['Q', q],
    ] as const) {
        for (const local of ['bureau', 'w-1']) {
            const session = `${ident.prefixe}:${local}`;
            const r = await tenterSession(port, session, ident.jeton);
            sessions[`${nom}:${local}`] = r;
            dire(ligne(`agent ${nom} -> ${session}`, r.acceptee ? 'ACCEPTÉ' : `REFUSÉ (${r.refus})`));
            juger(`agent ${nom} sert ${local}`, r.acceptee);
        }
    }
    dire();
    dire(ligne('aucun refus « déjà connecté »', Object.values(sessions).every((s) => s.refus === undefined)));
    // 🔴 LE TÉMOIN POSITIF DE L'ASSERTION SUIVANTE. Sans TURN_URL/TURN_SECRET,
    // le relais n'envoie AUCUNE `ice-config` à PERSONNE, et « l'intrus n'en a
    // pas reçu » deviendrait vacueux — vrai sur un service dont la garde
    // aurait été entièrement retirée. Cette ligne établit que la sonde tourne
    // avec un TURN configuré, donc que l'assertion d'en dessous PEUT échouer.
    juger(
        'un agent ACCEPTÉ reçoit bien une ice-config (témoin)',
        Object.values(sessions).every((s) => s.iceConfig),
    );

    // Le second volet du critère : chacun ne voit QUE sa session.
    //
    // 🔴 LA SESSION VISÉE EST VIERGE, ET C'EST TOUTE LA DIFFÉRENCE. Viser
    // `<Q>:bureau` — que l'agent de Q occupe déjà — donnerait un refus même
    // sans garde du tout : l'appariement s'en chargerait, avec « un agent est
    // déjà connecté ». L'assertion « P est refusé » serait alors VRAIE sur un
    // service dont la comparaison de préfixe aurait été retirée, c'est-à-dire
    // incapable d'échouer. MESURÉ : c'est ce que la première rédaction de
    // cette sonde a produit, et la rouge ①B l'a démasquée. On vise donc
    // `<Q>:w-9`, que PERSONNE n'occupe — le seul refus possible y est celui de
    // la garde.
    dire();
    const vierge = `${q.prefixe}:w-9`;
    const intrusion = await tenterSession(port, vierge, p.jeton);
    dire(ligne('agent P -> <Q>:w-9 (session VIERGE)', intrusion.acceptee ? 'ACCEPTÉ' : `REFUSÉ (${intrusion.refus})`));
    dire(ligne('trames reçues par l\'intrus', intrusion.brut));
    juger('agent P REFUSÉ sur une session vierge de Q', !intrusion.acceptee);
    juger("l'intrus n'a reçu AUCUNE ice-config", !intrusion.iceConfig);
    juger(
        'le refus ne dit ni à qui ni si elle existe',
        intrusion.refus === 'accès refusé à la session demandée',
    );

    // Et sur la session OCCUPÉE, la garde tranche AVANT l'appariement : le
    // motif le prouve, puisque ce n'est pas « un agent est déjà connecté ».
    const occupee = await tenterSession(port, `${q.prefixe}:bureau`, p.jeton);
    dire();
    dire(ligne('agent P -> <Q>:bureau (OCCUPÉE)', occupee.refus ?? 'ACCEPTÉ'));
    juger(
        'la garde tranche AVANT l\'appariement',
        occupee.refus === 'accès refusé à la session demandée',
    );

    p.canal.fermer();
    q.canal.fermer();
}

async function critere2(): Promise<void> {
    const { vmId, secret } = await enrolerLaVm(service.base, 'vm-connue', '192.168.3.2', Date.now());
    dire(ligne('VM enrôlée (identifiant)', vmId));
    dire(ligne('longueur du secret tiré', secret.length));
    dire();

    // Cas A — la VM n'existe pas.
    const inconnue = await Pair.ouvrir(`ws://127.0.0.1:${port}/agent`);
    inconnue.envoyer(encodeEnroler('11111111-2222-3333-4444-555555555555', secret));
    await inconnue.attendre(1);
    const refusA = inconnue.recues[0]?.brut ?? '<AUCUNE RÉPONSE>';

    // Cas B — la VM existe, le secret est faux.
    const mauvais = await Pair.ouvrir(`ws://127.0.0.1:${port}/agent`);
    mauvais.envoyer(encodeEnroler(vmId, `${secret}-faux`));
    await mauvais.attendre(1);
    const refusB = mauvais.recues[0]?.brut ?? '<AUCUNE RÉPONSE>';

    dire(ligne('A — VM inconnue, refus BRUT', refusA));
    dire(ligne('B — secret faux,  refus BRUT', refusB));
    dire(ligne('longueurs (A, B)', [refusA.length, refusB.length]));
    // La comparaison CARACTÈRE POUR CARACTÈRE que le critère exige.
    const premierEcart = [...refusA].findIndex((c, i) => c !== refusB[i]);
    dire(ligne('premier caractère divergent', premierEcart === -1 ? 'aucun' : premierEcart));
    juger('les deux refus sont IDENTIQUES', refusA === refusB);
    dire();
    dire(ligne('A — fermeture', inconnue.fermeture ?? 'socket ouvert'));
    dire(ligne('B — fermeture', mauvais.fermeture ?? 'socket ouvert'));
    juger(
        'les deux fermetures sont IDENTIQUES',
        JSON.stringify(inconnue.fermeture) === JSON.stringify(mauvais.fermeture),
    );
    dire();

    // Et le bon secret passe : sans cette ligne, un service qui refuserait
    // TOUT passerait le critère ② sans rien authentifier.
    const bonne = await Pair.ouvrir(`ws://127.0.0.1:${port}/agent`);
    bonne.envoyer(encodeEnroler(vmId, secret));
    await bonne.attendre(1);
    const accepte = JSON.parse(bonne.recues[0]?.brut ?? '{}') as { type?: string };
    dire(ligne('C — bon secret, type de réponse', accepte.type ?? '<AUCUNE>'));
    juger('le BON secret est accepté (témoin)', accepte.type === 'enrole');
    inconnue.fermer();
    mauvais.fermer();
    bonne.fermer();
}

async function critere3(): Promise<void> {
    const suivante = PLATEFORME_VERSION + 1;
    dire(ligne('PLATEFORME_VERSION', PLATEFORME_VERSION));
    dire(ligne('version du vecteur éprouvé', suivante));
    dire();

    // ① Le parseur TypeScript du sens AGENT -> PLATEFORME.
    const vers = parseVersLaPlateforme(
        JSON.stringify({ type: 'enroler', v: suivante, vm: 'w1', secret: 'chut' }),
    );
    dire(ligne('TS parseVersLaPlateforme', vers));
    juger('TS (vers) refuse pour motif « version »', !vers.ok && vers.motif === 'version');

    // ② Le parseur TypeScript du sens PLATEFORME -> AGENT.
    let leve: string | undefined;
    try {
        parseDepuisLaPlateforme(JSON.stringify({ type: 'refus', v: suivante, motif: 'version' }));
    } catch (cause) {
        leve = String(cause);
    }
    dire(ligne('TS parseDepuisLaPlateforme', leve ?? '🔴 N’A PAS LEVÉ'));
    juger('TS (depuis) LÈVE sur la version suivante', leve?.includes('version de plateforme non supportée') === true);
    dire();

    // ③ Le SERVICE lui-même, sur le fil : un message de version future doit
    // recevoir `refus/version` PUIS voir son socket fermé — le motif `version`
    // est fermant (`agents/canal.ts`), et ce n'est pas la même chose que
    // `forme`, qui laisse le pair se reprendre.
    const pair = await Pair.ouvrir(`ws://127.0.0.1:${port}/agent`);
    pair.envoyer(JSON.stringify({ type: 'enroler', v: suivante, vm: 'w1', secret: 'chut' }));
    await pair.attendre(1);
    dire(ligne('service — réponse BRUTE', pair.recues[0]?.brut ?? '<AUCUNE>'));
    dire(ligne('service — fermeture', pair.fermeture ?? 'socket ouvert'));
    juger(
        'le service répond refus/version',
        pair.recues[0]?.brut === JSON.stringify({ type: 'refus', v: PLATEFORME_VERSION, motif: 'version' }),
    );
    juger('le socket est FERMÉ (1008)', pair.fermeture?.code === 1008);
    dire();
    dire('# Le côté RUST est éprouvé par `cargo test -p proto plateforme` — voir');
    dire('# critere-3-rust-*.log : cinq tests `rejette_la_version_suivante_sur_*`,');
    dire('# un par variante, plus la vérification de la clé `version` du fichier');
    dire('# de vecteurs, que seul le TypeScript contrôlait avant P3.');
    pair.fermer();
}

async function critere4(): Promise<void> {
    const p = await enrolerEtOuvrir(port, service.base, 'vm-muette');
    // `vu_a` est posé par l'ENRÔLEMENT lui-même : il est le premier signe de
    // vie. On l'attend plutôt que de le supposer — l'écriture part par un
    // `void … .catch()` dans `canal.ts`, donc elle n'est pas synchrone.
    const vuApresEnrolement = await attendreVuA(p.vmId);
    dire(ligne("vu_a après l'enrôlement", vuApresEnrolement));
    juger("l'enrôlement pose vu_a", vuApresEnrolement !== null);

    // Un battement, et `vu_a` DOIT avancer.
    await new Promise((r) => setTimeout(r, 25));
    p.canal.envoyer(encodeBattement());
    await p.canal.attendre(2);
    const battement = parseDepuisLaPlateforme(p.canal.recues[1].brut);
    dire(ligne('réponse au battement', battement.type));
    const vuApresBattement = await attendreVuA(p.vmId, vuApresEnrolement ?? 0);
    dire(ligne('vu_a après un battement', vuApresBattement));
    juger(
        'le battement AVANCE vu_a',
        Number(vuApresBattement ?? 0) > Number(vuApresEnrolement ?? 0),
    );
    juger('le battement rend un jeton FRAIS', battement.type === 'battement-recu');
    dire();

    // 🔴 L'AGENT SE TAIT. On ne dort pas 90 s : l'horloge est un PARAMÈTRE
    // (`agents/fraicheur.ts`), et c'est précisément ce qui rend la transition
    // OBSERVABLE. Le relevé ci-dessous fait VARIER l'instant et assiège le
    // seuil des deux côtés — figer l'horloge est la rouge de ce critère.
    // 🔴 DIVERGENCE ENTRE LES DEUX MOTEURS, TROUVÉE PAR CETTE RECETTE ET
    // RELEVÉE PLUTÔT QUE CONTOURNÉE EN SILENCE. `LigneAgent.vu_a` est déclaré
    // `number | null` (`depot/agent.ts:29`), et il l'est bien sur SQLite ; sur
    // Postgres, `pg` rend les colonnes BIGINT (OID 20) sous forme de CHAÎNE
    // pour ne pas perdre de précision, et `pilote-postgres.ts` n'enregistre
    // aucun `setTypeParser`. Le type déclaré est donc faux sur l'un des deux
    // moteurs. `etatDe` n'en souffre pas — son `maintenant - vuA` force la
    // conversion numérique —, mais tout `+`, tout `===` ou tout `>` sur cette
    // valeur se comporterait différemment selon le moteur. La sonde CONVERTIT
    // explicitement, et le relevé ci-dessous nomme le type reçu.
    dire(ligne('typeof vu_a rendu par le dépôt', typeof vuApresBattement));
    const vuA = vuApresBattement === null ? null : Number(vuApresBattement);
    dire(ligne('SEUIL_INJOIGNABLE_MS', SEUIL_INJOIGNABLE_MS));
    const instants = [
        ['t = vu_a', 0],
        ['t = vu_a + seuil/2', SEUIL_INJOIGNABLE_MS / 2],
        ['t = vu_a + seuil (borne)', SEUIL_INJOIGNABLE_MS],
        ['t = vu_a + seuil + 1 ms', SEUIL_INJOIGNABLE_MS + 1],
        ['t = vu_a + 10 × seuil', SEUIL_INJOIGNABLE_MS * 10],
    ] as const;
    const etats: string[] = [];
    for (const [nom, delta] of instants) {
        const etat = etatDe(vuA, (vuA ?? 0) + delta);
        etats.push(etat);
        dire(ligne(nom, etat));
    }
    juger('la borne exacte est encore « prete »', etats[2] === 'prete');
    juger('une milliseconde de plus bascule', etats[3] === 'injoignable');
    // 🔴 LA TRANSITION EST VUE, PAS DÉDUITE : deux états distincts se
    // succèdent dans le même relevé.
    juger('le relevé VOIT la transition prete -> injoignable', new Set(etats).size === 2);
    dire();
    dire(ligne('une VM jamais vue (vu_a = null)', etatDe(null, Date.now())));
    juger('une VM jamais vue est « injoignable »', etatDe(null, Date.now()) === 'injoignable');
    p.canal.fermer();
}

/// Relit `vu_a` jusqu'à ce qu'il dépasse `plancher`, ou expiration.
/// ⚠️ Rend la valeur TELLE QU'ELLE EST à l'expiration, sans lever : c'est
/// l'assertion appelante qui doit rougir, pas la sonde qui doit planter.
async function attendreVuA(vmId: string, plancher = -1): Promise<number | null> {
    const fin = Date.now() + 2000;
    let vu: number | null = null;
    while (Date.now() < fin) {
        const l = await lireParVm(service.base, vmId);
        vu = l?.vu_a ?? null;
        if (vu !== null && vu > plancher) return vu;
        await new Promise((r) => setTimeout(r, 20));
    }
    return vu;
}

async function e2Ferme(): Promise<void> {
    dire('# La MÊME sonde que journaux-plateforme-p2/e2-role-agent-toujours-anonyme.log,');
    dire('# à lire ligne à ligne contre elle : un pair qui déclare');
    dire('# {"role":"agent","session":"x"} sans rien présenter.');
    dire();
    const r = await tenterSession(port, 'x', undefined, 'agent');
    dire(ligne('service', `ws://127.0.0.1:${port}/`));
    dire(ligne('messages reçus', r.brut));
    dire(ligne('a reçu ice-config', r.iceConfig));
    dire(ligne('a été refusé', r.refus !== undefined));
    dire(ligne('motif du refus', r.refus ?? '<aucun>'));
    juger("le pair anonyme est REFUSÉ", r.refus !== undefined);
    juger("il ne reçoit AUCUNE ice-config", !r.iceConfig);
    juger(
        'le message est celui de la garde',
        r.refus === 'authentification requise',
    );
    dire();
    dire('# En P2, ce même relevé donnait : a reçu ice-config = true, a été');
    dire('# refusé = false, identifiants TURN présents et valables 86 400 s.');
}

/// 🔴 L'HYPOTHÈSE QUE P3 A LAISSÉE NON MESURÉE, et elle est éliminatoire pour
/// le multi-fenêtres. Depuis la tâche 19, `agent/src/main.rs` ouvre un canal
/// `/agent` dans CHAQUE processus qui porte `AGENT_VM`/`AGENT_SECRET` — donc
/// dans le superviseur ET dans chacun de ses enfants, qui en héritent
/// (`superviseur/lanceur.rs` ne les retire pas). À N fenêtres, ce sont N+1
/// enrôlements concurrents pour la MÊME VM. L'implémenteur l'a déduit d'une
/// LECTURE de `canal.ts` ; ce qui suit le MESURE.
///
/// La sonde n'ouvre pas seulement N canaux : elle vérifie ensuite que le
/// PREMIER est toujours vivant et servi, car une plateforme qui n'accepterait
/// qu'un enrôlement à la fois pourrait tout aussi bien couper le précédent —
/// ce qui se lirait comme un succès si l'on ne regardait que le dernier.
async function enrolementsConcurrents(): Promise<void> {
    const N = 4;
    const { vmId, secret, prefixe } = await enrolerLaVm(
        service.base,
        'vm-multifenetre',
        '192.168.3.2',
        Date.now(),
    );
    dire(ligne('VM unique — préfixe attendu', prefixe));
    dire(ligne('canaux /agent ouverts de front', N));
    dire('# N = 4 : le superviseur, plus trois enfants — la configuration du');
    dire('# chantier D à trois fenêtres.');
    dire();

    // OUVERTS DE FRONT, pas l'un après l'autre : c'est la concurrence qui est
    // en cause, et une ouverture séquentielle ne l'éprouverait pas.
    const canaux = await Promise.all(
        Array.from({ length: N }, () => Pair.ouvrir(`ws://127.0.0.1:${port}/agent`)),
    );
    for (const c of canaux) c.envoyer(encodeEnroler(vmId, secret));
    await Promise.all(canaux.map((c) => c.attendre(1)));

    const reponses = canaux.map((c) => c.recues[0]?.brut ?? '<AUCUNE>');
    const types = reponses.map((r) => {
        try {
            return (JSON.parse(r) as { type?: string }).type ?? '<sans type>';
        } catch {
            return '<illisible>';
        }
    });
    dire(ligne('types de réponse, canal par canal', types));
    juger('les N enrôlements sont ACCEPTÉS', types.every((t) => t === 'enrole'));

    const identites = reponses.map((r) => parseDepuisLaPlateforme(r)).filter((m) => m.type === 'enrole');
    const prefixes = new Set(identites.map((m) => (m as { prefixe: string }).prefixe));
    const jetons = identites.map((m) => (m as { jeton: string }).jeton);
    dire(ligne('préfixes distincts délivrés', [...prefixes]));
    juger('tous reçoivent LE MÊME préfixe', prefixes.size === 1 && prefixes.has(prefixe));
    juger('aucun socket n’a été fermé', canaux.every((c) => c.fermeture === undefined));
    dire(ligne('longueurs de jeton', jetons.map((j) => j.length)));
    dire();

    // Le PREMIER canal est-il toujours servi une fois les trois autres
    // enrôlés ? Un battement le dit — et `sequence` serait la réponse d'un
    // service qui l'aurait désenrôlé.
    canaux[0].envoyer(encodeBattement());
    await canaux[0].attendre(2);
    const suite = canaux[0].recues[1]?.brut ?? '<AUCUNE>';
    const recu = JSON.parse(suite) as { type?: string; jeton?: string; expire_a?: number };
    dire(ligne('battement du PREMIER canal', recu.type ?? '<sans type>'));
    dire(ligne('jeton rendu (corps masqué)', jetonAbrege(recu.jeton ?? '..')));
    // Les revendications du jeton sont LISIBLES : elles ne portent aucun
    // secret, et c'est ici qu'on voit `sty:agent` et le sujet = préfixe, les
    // deux choses que la garde exige (`identite/garde.ts`).
    dire(ligne('revendications', decoderCharge(recu.jeton ?? '')));
    juger('le premier canal est toujours enrôlé', recu.type === 'battement-recu');
    dire();

    // Et les N jetons ouvrent-ils N sessions distinctes de la même VM ?
    const locales = ['bureau', 'w-1', 'w-2', 'w-3'];
    const issues: string[] = [];
    for (let i = 0; i < N; i += 1) {
        const r = await tenterSession(port, `${prefixe}:${locales[i]}`, jetons[i]);
        issues.push(r.acceptee ? 'ACCEPTÉ' : `REFUSÉ (${r.refus})`);
        dire(ligne(`jeton n°${i} -> ${prefixe}:${locales[i]}`, issues[i]));
    }
    juger('les N sessions de la MÊME VM sont servies', issues.every((i) => i === 'ACCEPTÉ'));
    for (const c of canaux) c.fermer();
}
