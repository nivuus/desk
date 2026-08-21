// L'orchestration du téléversement d'un installeur, côté navigateur :
// empreindre, créer, déposer les tranches MANQUANTES, sceller.
//
// 🔴 AUCUN DOM, ET AUCUNE DÉPENDANCE IMPLICITE. `fetch`, l'horloge et
// l'`AbortSignal` sont des PARAMÈTRES — aucun n'est lu ici, ni au chargement ni
// à l'appel. C'est ce qui rend la séquence éprouvable sur l'hôte sans
// navigateur, et surtout ce qui permet au pilote de recette (D14) d'exécuter LE
// CODE DU PRODUIT plutôt qu'une réimplémentation `curl` qui n'éprouverait
// qu'elle-même.
//
// 🔴 L'ARITHMÉTIQUE DES TRANCHES ET LE CONDENSAT VIENNENT DE `proto/ts/`, parce
// que la plateforme les emploie AUSSI : deux découpages divergents produiraient
// un scellement qui refuse sans qu'on sache lequel des deux bouts a tort (D6).
//
// ⚠️ UN REFUS ATTENDU EST UNE `Issue`, JAMAIS UNE EXCEPTION — l'arbitrage de
// `plateforme/src/orchestration/refus.ts`. Une panne d'ENVIRONNEMENT (le `fetch`
// qui rejette hors interruption) remonte telle quelle : la déguiser en refus la
// ferait passer pour une décision de protocole.
import { plan, verdict, type Tranche } from '../../../proto/ts/tranches';
import { Sha256 } from '../../../proto/ts/sha256';
/* ── LES DÉPENDANCES, TOUTES INJECTÉES ────────────────────────────────── */

/// La forme de réponse dont ce module a besoin, et rien de plus. DÉCLARÉE
/// plutôt qu'empruntée à `Response` : un `fetch` factice n'a aucune chance d'en
/// satisfaire les trente membres, et l'exiger rendrait ce fichier intestable.
/// Que la VRAIE `fetch` satisfasse `Fetch` est vérifié par le typage, au test.
export interface ReponseHttp {
    ok: boolean;
    status: number;
    json(): Promise<unknown>;
}

/// Ce que ce module passe à `fetch` — un sous-ensemble strict de `RequestInit`.
export interface InitHttp {
    method?: string;
    headers?: Record<string, string>;
    /// 🔵 `BufferSource` ET NON `Uint8Array` : ce dernier vaut désormais
    /// `Uint8Array<ArrayBufferLike>` — possiblement adossé à un `SharedArrayBuffer` —
    /// et n'est PAS un `BodyInit`, si bien que la vraie `fetch` cessait de satisfaire
    /// `Fetch`. Trouvé par le contrôle de forme du test, À LA COMPILATION.
    body?: string | BufferSource;
    signal?: AbortSignal;
}
export type Fetch = (url: string, init?: InitHttp) => Promise<ReponseHttp>;
export type Phase = 'empreinte' | 'transfert' | 'scellement';
export interface Progression {
    phase: Phase;
    /// ⚠️ EN PHASE `scellement`, `octets` VAUT `total` : la plateforme relit tout en
    /// flux et n'annonce rien en chemin — une progression figée n'est pas un blocage.
    octets: number;
    total: number;
}
export interface DepsTeleversement {
    /// L'origine de la plateforme, SANS barre oblique finale.
    base: string;
    /// Le jeton porteur, tel que `client/src/jeton.ts` le rend.
    jeton: string;
    fetch: Fetch;
    /// L'horloge, en millisecondes. Elle CADENCE la progression, et c'est son seul
    /// emploi : sans elle, empreindre 800 Mo émettrait des milliers d'événements.
    maintenant: () => number;
    signal?: AbortSignal;
    progression?: (p: Progression) => void;
    /// L'identifiant d'un téléversement à REPRENDRE. Absent, on crée.
    reprise?: string;
}
/* ── LES ISSUES ───────────────────────────────────────────────────────── */

export type MotifLocal = 'fichier-different' | 'tranches-incoherentes' | 'etat-illisible' | 'interrompu';
export type Etape = 'creation' | 'etat' | 'tranche' | 'scellement';
/// 🔴 LE MOTIF DU SERVICE EST UNE `string`, PAS UNE UNION, ET C'EST DÉLIBÉRÉ.
/// Son vocabulaire lui appartient et vit dans `plateforme/`, que `client/` ne
/// peut pas importer. Le recopier en union serait EXACTEMENT le défaut que
/// `client/src/connexion.ts` déclare sur `aucune-vm` : une copie qu'aucun type
/// ne confronte à sa source, silencieusement fausse au renommage.
export type Refus =
    | { source: 'client'; motif: MotifLocal; detail: string }
    | { source: 'service'; etape: Etape; statut: number; motif: string };

/// 🔴 UNE UNION DISCRIMINÉE, NI UN BOOLÉEN NI UNE EXCEPTION. Le compilateur
/// interdit de lire `id` sans avoir regardé `etat` : un appelant ne peut pas
/// prendre un refus pour un succès en oubliant un `if`, ce qu'un booléen ignoré
/// permettrait — l'argument de `prefixe.ts::poserPrefixe`, tenu ici par le typage.
export type Issue =
    | { etat: 'scelle'; id: string; taille: number; sha256: string; deposees: number[] }
    | { etat: 'refus'; refus: Refus; id?: string };

/// La taille d'un morceau lu pour empreindre. NON CALIBRÉE. ⚠️ AUCUN RAPPORT
/// AVEC `taille_tranche`, qui vient du service et n'est pas encore connue quand
/// on empreint : leur donner la même valeur laisserait croire à une dérivation,
/// alors qu'elles se recalibreraient séparément. Elle échange de la mémoire
/// d'onglet contre la durée pendant laquelle le fil est bloqué à condenser
/// (≈ 54 ms par morceau, au débit mesuré de 74,7 Mo/s de `proto/ts/sha256.ts`).
const OCTETS_LECTURE = 4 * 1024 * 1024;
/// La cadence de la progression. NON CALIBRÉE : c'est un confort d'affichage.
const PERIODE_PROGRESSION_MS = 250;
/* ── LA SÉQUENCE ──────────────────────────────────────────────────────── */

function entetes(deps: DepsTeleversement, type?: string): Record<string, string> {
    const en: Record<string, string> = { authorization: `Bearer ${deps.jeton}` };
    if (type !== undefined) en['content-type'] = type;
    return en;
}

/// Appelle le service. Rend `null` — et seulement `null` — QUAND LE SIGNAL DIT
/// QU'ON A ÉTÉ INTERROMPU : une interruption est un geste de l'utilisateur, donc
/// un refus, jamais une panne. La condition porte sur `signal.aborted`, non sur
/// la forme de l'exception : sinon une panne réseau se déguiserait en annulation.
async function appeler(deps: DepsTeleversement, url: string, init: InitHttp): Promise<ReponseHttp | null> {
    try {
        return await deps.fetch(url, { ...init, signal: deps.signal });
    } catch (cause) {
        if (deps.signal?.aborted === true) return null;
        throw cause;
    }
}

async function motifDuService(reponse: ReponseHttp): Promise<string> {
    const corps = (await reponse.json().catch(() => undefined)) as { refus?: unknown } | undefined;
    return typeof corps?.refus === 'string' ? corps.refus : `http-${reponse.status}`;
}
type Emettre = (p: Phase, octets: number, total: number, force: boolean) => void;
/// Le cadenceur de progression — le seul lecteur de l'horloge.
function cadenceur(deps: DepsTeleversement): Emettre {
    let dernier = Number.NEGATIVE_INFINITY;
    return (phase, octets, total, force) => {
        if (deps.progression === undefined) return;
        const instant = deps.maintenant();
        // Un changement de phase passe TOUJOURS : sans quoi un petit fichier
        // n'afficherait qu'une phase sur trois, et la passe de lecture complète
        // resterait invisible.
        if (!force && instant - dernier < PERIODE_PROGRESSION_MS) return;
        dernier = instant;
        deps.progression({ phase, octets, total });
    };
}

/// L'empreinte du fichier entier. Rend `null` si le signal a coupé.
///
/// 🔴 `File.slice` PLUTÔT QUE TOUT LE FICHIER : on ne tient jamais plus
/// d'`OCTETS_LECTURE` en mémoire, là où `crypto.subtle.digest` exigerait de
/// tenir les 800 Mo — la lacune d'API que `proto/ts/sha256.ts` contourne.
/// ⚠️ L'`await` DE `arrayBuffer()` EST LA CESSION, et il faut être exact sur ce
/// qu'elle achète : l'onglet reprend la main ENTRE deux morceaux, il reste bloqué
/// PENDANT la condensation de chacun — une suite de pauses courtes, pas un gel
/// silencieux, et la phase est affichée.
async function empreindre(fichier: File, deps: DepsTeleversement, emettre: Emettre): Promise<string | null> {
    const condensat = new Sha256();
    for (let debut = 0; debut < fichier.size; debut += OCTETS_LECTURE) {
        if (deps.signal?.aborted === true) return null;
        const fin = Math.min(debut + OCTETS_LECTURE, fichier.size);
        condensat.absorber(new Uint8Array(await fichier.slice(debut, fin).arrayBuffer()));
        emettre('empreinte', fin, fichier.size, false);
    }
    // Un fichier vide n'entre pas dans la boucle et rend l'empreinte du message vide :
    // la valeur juste, pas un cas particulier ajouté à la main.
    return condensat.terminer();
}

/// Met les tranches annoncées par le service sous la forme que `verdict` attend.
///
/// 🔴 DEUX FORMES SONT ACCEPTÉES, ET C'EST DÉCLARÉ PLUTÔT QUE DEVINÉ. Le plan
/// écrit `tranches_presentes: []` (D4) et confie au listage de rendre
/// `(n, octets)` (Task 25) : les deux lectures sont compatibles avec un tableau
/// vide, et ce module ne peut trancher pour une route qui n'est pas encore
/// écrite. La forme riche `{n, octets}` est la seule qui permette de DÉTECTER une
/// tranche mal taillée ; la forme nue `n` fait CROIRE le service sur la taille —
/// le dire, c'est nommer ce qu'on perd. Une troisième forme devient
/// `etat-illisible`, jamais un silence.
/// 🔴 UNE SEULE FORME EST ACCEPTÉE : `{n, octets}`, celle que la route rend.
///
/// ⚠️ ELLE EN TOLÉRAIT DEUX pendant l'écriture — un rang NU était accepté, et
/// sa taille était alors **empruntée au plan local**. La route étant arrêtée
/// (`routes-televersement.ts` rend le LISTAGE du magasin, donc des
/// `{n, octets}`), la tolérance est retirée, et pas seulement parce qu'elle est
/// devenue morte : **elle faisait croire le service sur une taille qu'il n'avait
/// jamais annoncée.** Un rang présent dont la taille aurait dérivé serait passé
/// pour conforme, et le scellement aurait refusé plus tard, ailleurs, sans que
/// rien ne relie les deux — alors que `verdict` sait dire `incoherentes`.
///
/// ⚠️ TOUTE AUTRE FORME EST UN REFUS TYPÉ, JAMAIS UN SILENCE : c'est ce qui
/// distingue « le service parle une autre version » de « il n'y a rien à
/// reprendre », et les deux appellent des gestes opposés.
function normaliserPresentes(brut: unknown): Tranche[] | null {
    if (!Array.isArray(brut)) return null;
    const sortie: Tranche[] = [];
    for (const entree of brut) {
        const t = entree as { n?: unknown; octets?: unknown };
        if (typeof t?.n !== 'number' || typeof t?.octets !== 'number') return null;
        sortie.push({ n: t.n, octets: t.octets });
    }
    return sortie;
}

export async function televerser(fichier: File, deps: DepsTeleversement): Promise<Issue> {
    const taille = fichier.size;
    const emettre = cadenceur(deps);
    // 🔴 L'IDENTIFIANT EST CAPTURÉ, PAS PASSÉ À CHAQUE REFUS : c'est ce qui garantit
    // qu'aucun refus ne l'oublie — un appelant qui perdrait l'`id` sur un scellement
    // refusé ne pourrait plus reprendre, et redéposerait tout. `''` = pas encore créé.
    let id = deps.reprise ?? '';
    const refuse = (refus: Refus): Issue => ({ etat: 'refus', refus, id: id === '' ? undefined : id });
    const nonLocal = (motif: MotifLocal, detail: string): Issue => refuse({ source: 'client', motif, detail });
    const nonService = async (etape: Etape, r: ReponseHttp): Promise<Issue> =>
        refuse({ source: 'service', etape, statut: r.status, motif: await motifDuService(r) });

    // ① RELIRE L'ÉTAT, ET COMPARER LA TAILLE AVANT D'EMPREINDRE (D5). L'ordre est
    // un gain réel : un fichier manifestement différent est refusé sans payer la
    // passe de lecture complète — onze secondes pour 800 Mo — ni déposer un octet.
    let etat: Record<string, unknown> | undefined;
    if (deps.reprise !== undefined) {
        const url = `${deps.base}/televersement/${encodeURIComponent(id)}`;
        const r = await appeler(deps, url, { method: 'GET', headers: entetes(deps) });
        if (r === null) return nonLocal('interrompu', 'pendant la relecture');
        if (!r.ok) return await nonService('etat', r);
        etat = (await r.json().catch(() => undefined)) as Record<string, unknown> | undefined;
        // 🔴 UN ÉTAT QUI N'ANNONCE PAS `{taille, sha256}` FAIT REFUSER LA REPRISE, il
        // ne la fait pas reprendre à l'aveugle : sans ces deux valeurs, rien ne dit
        // que le fichier re-choisi est LE MÊME, les tranches de deux fichiers se
        // mélangeraient, et le scellement échouerait sans que rien ne dise pourquoi.
        if (typeof etat?.taille !== 'number' || typeof etat.sha256 !== 'string') {
            return nonLocal('etat-illisible', 'état sans taille ni empreinte : reprise invérifiable');
        }
        if (etat.taille !== taille) {
            return nonLocal('fichier-different', `taille ${taille} contre ${etat.taille} au téléversement`);
        }
    }

    // ② L'EMPREINTE, À LA CRÉATION ET NON AU SCELLEMENT (D5) : c'est elle qui
    // rend la reprise sûre, et la seule valeur que les trois étages comparent.
    emettre('empreinte', 0, taille, true);
    const sha256 = await empreindre(fichier, deps, emettre);
    if (sha256 === null) return nonLocal('interrompu', "pendant l'empreinte");
    emettre('empreinte', taille, taille, true);
    if (etat !== undefined && etat.sha256 !== sha256) {
        return nonLocal('fichier-different', `empreinte ${sha256} contre ${String(etat.sha256)} retenue`);
    }

    // ③ CRÉER, SI L'ON NE REPREND PAS.
    let tailleTranche: unknown;
    let presentesBrut: unknown;
    if (etat !== undefined) {
        tailleTranche = etat.taille_tranche;
        presentesBrut = etat.tranches_presentes;
    } else {
        const r = await appeler(deps, `${deps.base}/televersement`, {
            method: 'POST',
            headers: entetes(deps, 'application/json'),
            body: JSON.stringify({ nom: fichier.name, taille, sha256 }),
        });
        if (r === null) return nonLocal('interrompu', 'pendant la création');
        if (!r.ok) return await nonService('creation', r);
        const c = (await r.json().catch(() => undefined)) as Record<string, unknown> | undefined;
        if (typeof c?.id !== 'string') return nonLocal('etat-illisible', 'création sans identifiant');
        id = c.id;
        tailleTranche = c.taille_tranche;
        presentesBrut = c.tranches_presentes;
    }

    // ④ LE DÉCOUPAGE. `plan` et `verdict` LÈVENT sur un contrat absurde — leur garde
    // vise un défaut de programme. Or `taille_tranche` vient du FIL : la valider ici
    // empêche une réponse déréglée de faire tomber le tout par une exception.
    if (!Number.isInteger(tailleTranche) || (tailleTranche as number) <= 0) {
        return nonLocal('etat-illisible', `taille_tranche ${String(tailleTranche)}`);
    }
    const pas = tailleTranche as number;
    const attendu = plan(taille, pas);
    const presentes = normaliserPresentes(presentesBrut);
    if (presentes === null) return nonLocal('etat-illisible', 'tranches_presentes de forme inconnue');

    // 🔴 `incoherentes` NE SE RECOMPLÈTE PAS : les deux bouts ne s'accordent plus
    // sur le découpage, et redéposer rendrait la même chose indéfiniment. On
    // refuse, on nomme les rangs, l'appelant décide.
    const v = verdict(taille, pas, presentes);
    if (v.etat === 'incoherentes') return nonLocal('tranches-incoherentes', `rangs ${v.n.join(', ')}`);
    const aDeposer = v.etat === 'manquantes' ? v.n : [];

    // ⑤ LE DÉPÔT DES SEULES MANQUANTES. Recommencer à zéro passerait un test qui
    // ne regarde que le résultat : c'est le critère ③ de la spec, et c'est
    // pourquoi le test COMPTE LES OCTETS ENVOYÉS.
    let envoyes = taille - aDeposer.reduce((s, n) => s + attendu[n].octets, 0);
    emettre('transfert', envoyes, taille, true);
    for (const n of aDeposer) {
        if (deps.signal?.aborted === true) return nonLocal('interrompu', `avant la tranche ${n}`);
        const debut = n * pas;
        const corps = new Uint8Array(await fichier.slice(debut, debut + attendu[n].octets).arrayBuffer());
        const url = `${deps.base}/televersement/${encodeURIComponent(id)}/tranche/${n}`;
        const r = await appeler(deps, url, {
            method: 'PUT',
            headers: entetes(deps, 'application/octet-stream'),
            body: corps,
        });
        if (r === null) return nonLocal('interrompu', `pendant la tranche ${n}`);
        if (!r.ok) return await nonService('tranche', r);
        envoyes += attendu[n].octets;
        emettre('transfert', envoyes, taille, false);
    }

    // ⑥ LE SCELLEMENT. La plateforme RECALCULE l'empreinte et refuse si elle diffère
    // (D4). Ce refus remonte tel quel : ni avalé, ni réessayé — une empreinte qui
    // diverge ne converge pas, et une boucle de réessai téléverserait sans fin.
    emettre('scellement', taille, taille, true);
    const url = `${deps.base}/televersement/${encodeURIComponent(id)}/sceller`;
    const r = await appeler(deps, url, { method: 'POST', headers: entetes(deps) });
    if (r === null) return nonLocal('interrompu', 'pendant le scellement');
    if (!r.ok) return await nonService('scellement', r);
    return { etat: 'scelle', id, taille, sha256, deposees: aDeposer };
}
