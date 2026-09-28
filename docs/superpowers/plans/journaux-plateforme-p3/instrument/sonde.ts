// The probes of sub-block P3's acceptance run. One subcommand per criterion.
//
//     tsx sonde.ts <critere-1|critere-2|critere-3|critere-4|e2-ferme> <sqlite|postgres> <commit>
//
// 🔴 EACH PROBE WRITES ITS READING ON STANDARD OUTPUT, and returns a NON-ZERO
// exit code if its criterion is not met. A log that merely
// printed without judging could not turn red — it is the pattern this repository
// paid for four times in sub-block D10.
//
// ⚠️ NO SECRET IS WRITTEN IN CLEAR. The enrolment secrets are drawn at
// random by `enrolerLaVm` and appear in no filed log: only
// their LENGTH and their equality/difference are recorded. Signed tokens are
// only shown by their header prefix and their length.

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

/// The title each log's header carries.
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

/// Returns a token's claims — they carry no secret, and it is
/// they that show the `sty` type claim and the subject = prefix.
function decoderCharge(jeton: string): unknown {
    const charge = jeton.split('.')[1];
    if (charge === undefined) return '<illisible>';
    try {
        return JSON.parse(Buffer.from(charge, 'base64url').toString('utf8'));
    } catch {
        return '<illisible>';
    }
}

/// Hides a token's body: its presence and its shape are enough for the reading.
function jetonAbrege(jeton: string): string {
    const parts = jeton.split('.');
    return `${parts[0]}.<charge:${parts[1]?.length ?? 0}>.<sig:${parts[2]?.length ?? 0}>`;
}

/// Enrols a VM and opens its `/agent` channel. Returns the delivered identity.
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

/// Opens an `agent`-role handshake on the relay and records what
/// it obtains. ⚠️ WE RECORD `ice-config` SEPARATELY from the refusal: a service
/// refusing AFTER having sent the TURN configuration would already have given
/// everything away (that is what `garde-fil.test.ts` holds on its side).
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
    // ⚠️ "ACCEPTED" CANNOT BE READ FROM THE MERE PRESENCE OF `ice-config`:
    // without TURN_URL, the relay sends NONE and the accepted peer receives
    // nothing at all. Acceptance is therefore read from the ABSENCE of a refusal, socket
    // still open — and `ice-config` is recorded separately.
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

    // Each agent opens ITS control session and ITS first window.
    // 🔴 It is here that P2's binary breaks: without a prefix, both
    // name their control session `bureau`, and the second is refused.
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
    // 🔴 THE POSITIVE CONTROL OF THE NEXT ASSERTION. Without TURN_URL/TURN_SECRET,
    // the relay sends NO `ice-config` to ANYONE, and "the intruder did not
    // receive one" would become vacuous — true on a service whose guard
    // had been entirely removed. This line establishes that the probe runs
    // with a configured TURN, hence that the assertion below CAN fail.
    juger(
        'un agent ACCEPTÉ reçoit bien une ice-config (témoin)',
        Object.values(sessions).every((s) => s.iceConfig),
    );

    // The second part of the criterion: each one sees ONLY its own session.
    //
    // 🔴 THE TARGETED SESSION IS PRISTINE, AND THAT MAKES ALL THE DIFFERENCE. Targeting
    // `<Q>:bureau` — which Q's agent already occupies — would give a refusal even
    // without any guard at all: pairing would take care of it, with "an agent is
    // already connected". The assertion "P is refused" would then be TRUE on a
    // service whose prefix comparison had been removed, that is,
    // unable to fail. MEASURED: it is what the first wording of
    // this probe produced, and red run ①B unmasked it. We therefore target
    // `<Q>:w-9`, which NO ONE occupies — the only possible refusal there is the
    // guard's.
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

    // And on the OCCUPIED session, the guard decides BEFORE pairing: the
    // reason proves it, since it is not "an agent is already connected".
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

    // Case A — the VM does not exist.
    const inconnue = await Pair.ouvrir(`ws://127.0.0.1:${port}/agent`);
    inconnue.envoyer(encodeEnroler('11111111-2222-3333-4444-555555555555', secret));
    await inconnue.attendre(1);
    const refusA = inconnue.recues[0]?.brut ?? '<AUCUNE RÉPONSE>';

    // Case B — the VM exists, the secret is wrong.
    const mauvais = await Pair.ouvrir(`ws://127.0.0.1:${port}/agent`);
    mauvais.envoyer(encodeEnroler(vmId, `${secret}-faux`));
    await mauvais.attendre(1);
    const refusB = mauvais.recues[0]?.brut ?? '<AUCUNE RÉPONSE>';

    dire(ligne('A — VM inconnue, refus BRUT', refusA));
    dire(ligne('B — secret faux,  refus BRUT', refusB));
    dire(ligne('longueurs (A, B)', [refusA.length, refusB.length]));
    // The CHARACTER FOR CHARACTER comparison the criterion requires.
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

    // And the right secret passes: without this line, a service refusing
    // EVERYTHING would pass criterion ② without authenticating anything.
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

    // ③ The SERVICE itself, on the wire: a message of a future version must
    // receive `refus/version` THEN see its socket closed — the `version` reason
    // is closing (`agents/canal.ts`), and it is not the same thing as
    // `forme`, which lets the peer recover.
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
    // `vu_a` is set by the ENROLMENT itself: it is the first sign of
    // life. We wait for it rather than assume it — the write goes out through a
    // `void … .catch()` in `canal.ts`, so it is not synchronous.
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

    // 🔴 THE AGENT GOES QUIET. We do not sleep 90 s: the clock is a PARAMETER
    // (`agents/fraicheur.ts`), and that is precisely what makes the transition
    // OBSERVABLE. The reading below VARIES the instant and besieges the
    // threshold from both sides — freezing the clock is this criterion's red run.
    // 🔴 DIVERGENCE BETWEEN THE TWO ENGINES, FOUND BY THIS ACCEPTANCE RUN AND
    // RECORDED RATHER THAN SILENTLY WORKED AROUND. `LigneAgent.vu_a` is declared
    // `number | null` (`depot/agent.ts:29`), and it is so on SQLite; on
    // Postgres, `pg` returns BIGINT columns (OID 20) as a STRING
    // so as not to lose precision, and `pilote-postgres.ts` registers
    // no `setTypeParser`. The declared type is therefore wrong on one of the two
    // engines. `etatDe` does not suffer from it — its `maintenant - vuA` forces the
    // numeric conversion —, but any `+`, any `===` or any `>` on this
    // value would behave differently depending on the engine. The probe CONVERTS
    // explicitly, and the reading below names the type received.
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
    // 🔴 THE TRANSITION IS SEEN, NOT DEDUCED: two distinct states
    // follow each other in the same reading.
    juger('le relevé VOIT la transition prete -> injoignable', new Set(etats).size === 2);
    dire();
    dire(ligne('une VM jamais vue (vu_a = null)', etatDe(null, Date.now())));
    juger('une VM jamais vue est « injoignable »', etatDe(null, Date.now()) === 'injoignable');
    p.canal.fermer();
}

/// Rereads `vu_a` until it exceeds `plancher`, or expiry.
/// ⚠️ Returns the value AS IT IS at expiry, without throwing: it is
/// the calling assertion that must turn red, not the probe that must crash.
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

/// 🔴 THE HYPOTHESIS P3 LEFT UNMEASURED, and it is eliminating for
/// multi-window. Since task 19, `agent/src/main.rs` opens an `/agent`
/// channel in EACH process carrying `AGENT_VM`/`AGENT_SECRET` — hence
/// in the supervisor AND in each of its children, which inherit them
/// (`superviseur/lanceur.rs` does not remove them). With N windows, that makes N+1
/// concurrent enrolments for the SAME VM. The implementer deduced it from a
/// READING of `canal.ts`; what follows MEASURES it.
///
/// The probe does not only open N channels: it then checks that the
/// FIRST is still alive and served, because a platform that only accepted
/// one enrolment at a time could just as well cut the previous one —
/// which would read as a success if one only looked at the last.
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

    // OPENED SIDE BY SIDE, not one after the other: it is concurrency that is
    // at stake, and a sequential opening would not test it.
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

    // Is the FIRST channel still served once the three others are
    // enrolled? A heartbeat says so — and `sequence` would be the answer of a
    // service that had unenrolled it.
    canaux[0].envoyer(encodeBattement());
    await canaux[0].attendre(2);
    const suite = canaux[0].recues[1]?.brut ?? '<AUCUNE>';
    const recu = JSON.parse(suite) as { type?: string; jeton?: string; expire_a?: number };
    dire(ligne('battement du PREMIER canal', recu.type ?? '<sans type>'));
    dire(ligne('jeton rendu (corps masqué)', jetonAbrege(recu.jeton ?? '..')));
    // The token's claims are READABLE: they carry no
    // secret, and it is here that one sees `sty:agent` and the subject = prefix, the
    // two things the guard requires (`identite/garde.ts`).
    dire(ligne('revendications', decoderCharge(recu.jeton ?? '')));
    juger('le premier canal est toujours enrôlé', recu.type === 'battement-recu');
    dire();

    // And do the N tokens open N distinct sessions of the same VM?
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
