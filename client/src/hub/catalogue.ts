// La lecture du catalogue et de ses icônes, côté navigateur.
//
// 🔴 AUCUN DOM, ET `fetch` EST INJECTÉ — la discipline de `televersement.ts`,
// pour la même raison : c'est ce qui rend la règle éprouvable sur l'hôte sans
// navigateur, et ce qui permet à une recette d'exécuter LE CODE DU PRODUIT
// plutôt qu'une réimplémentation `curl` qui n'éprouverait qu'elle-même.
//
// 🔴 L'ICÔNE EST LUE PAR UN `fetch` AUTHENTIFIÉ, ET C'EST LA SEULE VOIE.
// `plateforme/src/http/routes-icone.ts:20-26` l'a écrit en toutes lettres :
// **un `<img src>` ne porte pas d'en-tête `Authorization`**. G5 mesure que
// l'obstacle est plus large encore — un `<link rel="manifest">` non plus, et
// ⑤ ne pose AUCUN cookie, son porteur vivant dans `localStorage`. D'où la
// voie V1, reçue par la porte P0 : la page lit tout elle-même, et publie ce
// qu'elle a lu en `data:` et en `blob:`.
//
// ⚠️ UN REFUS ATTENDU EST UNE ISSUE, JAMAIS UNE EXCEPTION — l'arbitrage de
// `plateforme/src/orchestration/refus.ts`, déjà tenu par `televersement.ts`.
// Une panne d'ENVIRONNEMENT (le `fetch` qui rejette) remonte telle quelle :
// la déguiser en refus la ferait passer pour une décision de protocole.

/* ── LES DÉPENDANCES, TOUTES INJECTÉES ────────────────────────────────── */

/// La forme de réponse dont ce module a besoin, et rien de plus. DÉCLARÉE
/// plutôt qu'empruntée à `Response` — un `fetch` factice n'a aucune chance
/// d'en satisfaire les trente membres. Que la VRAIE `fetch` la satisfasse est
/// vérifié par le typage, au test.
export interface ReponseHttp {
    ok: boolean;
    status: number;
    json(): Promise<unknown>;
    arrayBuffer(): Promise<ArrayBuffer>;
}
export interface InitHttp {
    method?: string;
    headers?: Record<string, string>;
}
export type Fetch = (url: string, init?: InitHttp) => Promise<ReponseHttp>;

export interface DepsCatalogue {
    /// L'origine de la plateforme, SANS barre oblique finale.
    base: string;
    /// Le jeton porteur, tel que `client/src/jeton.ts` le rend.
    jeton: string;
    fetch: Fetch;
}

/* ── CE QUI TRAVERSE ──────────────────────────────────────────────────── */

/// Une application, telle que `GET /applications` la rend — et RIEN de plus.
///
/// ⚠️ NI `cible`, NI `arguments`, NI `repertoire`, NI `chemin` : la plateforme
/// les tait délibérément (`routes-applications.ts:250-255`), parce que ce sont
/// des chemins du disque de la VM. **Ne pas les ajouter ici en croyant
/// compléter le type** : ils n'arriveront jamais, et le lancement se fait par
/// l'identifiant, jamais par un chemin que le client fournirait.
export interface ApplicationListee {
    id: string;
    nom: string;
    /// L'empreinte sha256 de l'icône, ou `null` s'il n'y en a pas.
    icone: string | null;
    source_max: string;
    /// La couleur dominante de l'icône, en `#rrggbb`, ou `null`.
    ///
    /// ⚠️ `null` VEUT DIRE « PAS D'ACCENT », JAMAIS « PAS ENCORE MESURÉ » : une
    /// icône trop pâle, trop sombre ou trop transparente n'a aucune dominante.
    /// Le manifeste OMET alors `theme_color` plutôt que d'en inventer un.
    accent: string | null;
    /// Les extensions que cette application ouvre — minuscules, avec le point.
    ///
    /// ⚠️ VIDE EST LE CAS LE PLUS FRÉQUENT, pas une panne.
    associations: string[];
}

export type Refus =
    | { source: 'client'; motif: 'reponse-illisible'; detail: string }
    | { source: 'service'; statut: number; motif: string };

export type Issue<T> = { etat: 'ok'; valeur: T } | { etat: 'refus'; refus: Refus };

/* ── L'INTERNE ────────────────────────────────────────────────────────── */

function entetes(deps: DepsCatalogue): Record<string, string> {
    return { authorization: `Bearer ${deps.jeton}` };
}

/// Le motif que le service a rendu, ou son code seul s'il n'en rend aucun.
///
/// 🔴 LE MOTIF EST UNE `string`, PAS UNE UNION, ET C'EST DÉLIBÉRÉ — le même
/// arbitrage que `televersement.ts`. Le vocabulaire des refus appartient à
/// `plateforme/`, que `client/` ne peut pas importer ; le recopier en union
/// serait la copie qu'aucun type ne confronte à sa source, silencieusement
/// fausse au renommage. C'est le défaut que `connexion.ts` déclare sur
/// `aucune-vm`, et que P4 a légué sans le fermer.
async function motifDuService(r: ReponseHttp): Promise<string> {
    try {
        const corps = await r.json();
        if (typeof corps === 'object' && corps !== null && 'refus' in corps) {
            const refus = (corps as { refus: unknown }).refus;
            if (typeof refus === 'string') return refus;
        }
    } catch {
        // Un corps illisible n'est pas plus informatif qu'une absence de corps.
    }
    return `statut ${r.status}`;
}

/* ── LA LECTURE DU CATALOGUE ──────────────────────────────────────────── */

/// `GET /applications?vm=<id>` — la liste, ou un refus typé.
export async function listerApplications(
    vm: string,
    deps: DepsCatalogue,
): Promise<Issue<ApplicationListee[]>> {
    const url = `${deps.base}/applications?vm=${encodeURIComponent(vm)}`;
    const r = await deps.fetch(url, { method: 'GET', headers: entetes(deps) });
    if (!r.ok) return { etat: 'refus', refus: { source: 'service', statut: r.status, motif: await motifDuService(r) } };
    let corps: unknown;
    try {
        corps = await r.json();
    } catch (e) {
        return illisible(`corps non JSON : ${(e as Error).message}`);
    }
    if (typeof corps !== 'object' || corps === null || !('applications' in corps)) {
        return illisible("le corps ne porte pas de champ 'applications'");
    }
    const liste = (corps as { applications: unknown }).applications;
    if (!Array.isArray(liste)) return illisible("'applications' n'est pas un tableau");
    const applications: ApplicationListee[] = [];
    for (const entree of liste) {
        if (typeof entree !== 'object' || entree === null) return illisible('une entrée n\'est pas un objet');
        const e = entree as Record<string, unknown>;
        if (typeof e.id !== 'string' || typeof e.nom !== 'string') {
            return illisible("une entrée n'a ni `id` ni `nom` utilisables");
        }
        applications.push({
            id: e.id,
            nom: e.nom,
            icone: typeof e.icone === 'string' ? e.icone : null,
            source_max: typeof e.source_max === 'string' ? e.source_max : 'non-mesuree',
            accent: typeof e.accent === 'string' ? e.accent : null,
            // ⚠️ ON FILTRE LES ÉLÉMENTS, ET ON NE SE CONTENTE PAS DE VÉRIFIER
            // QUE C'EST UN TABLEAU : une entrée non textuelle atterrirait dans
            // un `accept` de manifeste, où le navigateur la rejetterait sans
            // qu'on sache d'où elle vient.
            associations: Array.isArray(e.associations)
                ? e.associations.filter((x): x is string => typeof x === 'string')
                : [],
        });
    }
    return { etat: 'ok', valeur: applications };
}

function illisible<T>(detail: string): Issue<T> {
    return { etat: 'refus', refus: { source: 'client', motif: 'reponse-illisible', detail } };
}

/* ── LA VM ────────────────────────────────────────────────────────────── */

/// Une VM, telle que `GET /vm` la rend — et RIEN de plus.
///
/// ⚠️ NI `adresse`, NI `utilisateurId` : la plateforme les tait délibérément
/// (`routes-vm.ts:177-180`) — la première est de la topologie interne, la
/// seconde est celle du demandeur, qu'il connaît déjà.
export interface VmListee {
    id: string;
    nom: string;
    etat: string;
    prefixe: string | null;
}

/// `GET /vm` — les VMs de l'utilisateur.
///
/// ⚠️ IL Y EN A AU PLUS UNE À CE JOUR, et c'est une propriété de la BASE, pas
/// de ce module : l'index partiel `vm_un_utilisateur` de `0001-socle.sql` la
/// garantit. `routes-vm.ts` écrit que le jour où cet invariant tomberait, son
/// champ `sessions_ouvertes` deviendrait faux. **Ce module rend donc une
/// LISTE**, pour n'avoir rien à défaire ce jour-là.
export async function listerVms(deps: DepsCatalogue): Promise<Issue<VmListee[]>> {
    const r = await deps.fetch(`${deps.base}/vm`, { method: 'GET', headers: entetes(deps) });
    if (!r.ok) return { etat: 'refus', refus: { source: 'service', statut: r.status, motif: await motifDuService(r) } };
    let corps: unknown;
    try {
        corps = await r.json();
    } catch (e) {
        return illisible(`corps non JSON : ${(e as Error).message}`);
    }
    if (typeof corps !== 'object' || corps === null || !('vms' in corps)) {
        return illisible("le corps ne porte pas de champ 'vms'");
    }
    const liste = (corps as { vms: unknown }).vms;
    if (!Array.isArray(liste)) return illisible("'vms' n'est pas un tableau");
    const vms: VmListee[] = [];
    for (const entree of liste) {
        if (typeof entree !== 'object' || entree === null) return illisible("une entrée n'est pas un objet");
        const e = entree as Record<string, unknown>;
        if (typeof e.id !== 'string' || typeof e.nom !== 'string') {
            return illisible("une entrée n'a ni `id` ni `nom` utilisables");
        }
        vms.push({
            id: e.id,
            nom: e.nom,
            etat: typeof e.etat === 'string' ? e.etat : 'inconnu',
            prefixe: typeof e.prefixe === 'string' ? e.prefixe : null,
        });
    }
    return { etat: 'ok', valeur: vms };
}

/* ── LA LECTURE D'UNE ICÔNE ───────────────────────────────────────────── */

/// `GET /application/:id/icone?e=<empreinte>` — les octets du PNG.
///
/// ⚠️ L'EMPREINTE EST OBLIGATOIRE côté service : elle est ce qui rend le cache
/// immuable. La passer est donc une propriété du protocole, pas une option.
export async function lireIcone(
    application: ApplicationListee,
    deps: DepsCatalogue,
): Promise<Issue<Uint8Array>> {
    if (application.icone === null) {
        return illisible(`l'application ${application.id} n'a pas d'icône`);
    }
    const url = `${deps.base}/application/${encodeURIComponent(application.id)}/icone?e=${encodeURIComponent(application.icone)}`;
    const r = await deps.fetch(url, { method: 'GET', headers: entetes(deps) });
    if (!r.ok) return { etat: 'refus', refus: { source: 'service', statut: r.status, motif: await motifDuService(r) } };
    return { etat: 'ok', valeur: new Uint8Array(await r.arrayBuffer()) };
}

/* ── LE LANCEMENT ─────────────────────────────────────────────────────── */

/// `POST /application/:id/lancer`.
export async function lancerApplication(
    id: string,
    deps: DepsCatalogue,
): Promise<Issue<null>> {
    const url = `${deps.base}/application/${encodeURIComponent(id)}/lancer`;
    const r = await deps.fetch(url, { method: 'POST', headers: entetes(deps) });
    if (!r.ok) return { etat: 'refus', refus: { source: 'service', statut: r.status, motif: await motifDuService(r) } };
    return { etat: 'ok', valeur: null };
}
