// Reading the catalogue and its icons, browser side.
//
// 🔴 NO DOM, AND `fetch` IS INJECTED — the discipline of `televersement.ts`,
// for the same reason: it is what makes the rule testable on the host without a
// browser, and what lets an acceptance run execute THE PRODUCT'S CODE
// rather than a `curl` reimplementation that would only test itself.
//
// 🔴 ❌ ~~THE ICON IS READ BY AN AUTHENTICATED `fetch`, AND IT IS THE ONLY WAY.
// `routes-icone.ts` wrote it out in full: an `<img src>` carries no
// `Authorization` header.~~ **NO LONGER TRUE SINCE AUGUST 30TH, 2026**, by
// DECISION OF THE REPOSITORY OWNER — not for convenience: the icon route
// is now reached through a **SIGNED URL**, minted by the catalogue
// (hence under a bearer token, and after the VM ownership check) and
// returned in the `icone_url` field. An `<img src>` can load it as
// is. See `plateforme/src/apps/url-icone.ts` for the derived key, what
// the signature covers and the retained duration.
//
// ⚠️ WHAT REMAINS TRUE OF G5, AND EXPLAINS WHY `lireIcone` SURVIVES:
// a `<link rel="manifest">` is fetched WITHOUT a cookie, and the per-application
// manifest therefore keeps carrying its icon as `data:` — a form G5
// measured installable. Building that `data:` requires the PNG's BYTES, which
// `lireIcone` fetches. **What changes is that it no longer sends a
// header**: the signed URL is self-sufficient.
//
// ⚠️ AN EXPECTED REFUSAL IS AN OUTCOME, NEVER AN EXCEPTION — the arbitration of
// `plateforme/src/orchestration/refus.ts`, already held by `televersement.ts`.
// An ENVIRONMENT failure (the `fetch` that rejects) propagates as is:
// disguising it as a refusal would pass it off as a protocol decision.

/* ── THE DEPENDENCIES, ALL INJECTED ────────────────────────────────── */

/// The response shape this module needs, and nothing more. DECLARED
/// rather than borrowed from `Response` — a fake `fetch` has no chance
/// of satisfying its thirty members. That the REAL `fetch` satisfies it is
/// checked by typing, in the test.
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
    /// The platform's origin, WITHOUT a trailing slash.
    base: string;
    /// The bearer token, as `client/src/jeton.ts` returns it.
    jeton: string;
    fetch: Fetch;
}

/* ── WHAT CROSSES ──────────────────────────────────────────────────── */

/// An application, as `GET /applications` returns it — and NOTHING more.
///
/// ⚠️ NEITHER `cible`, NOR `arguments`, NOR `repertoire`, NOR `chemin`: the platform
/// withholds them deliberately (`routes-applications.ts:250-255`), because they are
/// paths on the VM's disk. **Do not add them here believing you are
/// completing the type**: they will never arrive, and launching goes through
/// the identifier, never through a path the client would supply.
export interface ApplicationListee {
    id: string;
    nom: string;
    /// The sha256 fingerprint of the icon, or `null` if there is none.
    icone: string | null;
    /// The SIGNED URL of the icon — relative, hence to resolve against the page's
    /// origin —, or `null` if there is no icon.
    ///
    /// 🔴 IT IS MINTED BY THE PLATFORM, AND NEVER REBUILT HERE: the
    /// client does not have the key, and a URL it made up would be refused.
    /// It is also what prevents a page from extending a capability's
    /// lifetime by itself.
    ///
    /// ⚠️ IT EXPIRES — 5 to 6 minutes (`apps/url-icone.ts`). A page that
    /// kept it for hours would see its images fail; rereading it means
    /// rereading the catalogue.
    icone_url: string | null;
    source_max: string;
    /// The dominant colour of the icon, as `#rrggbb`, or `null`.
    ///
    /// ⚠️ `null` MEANS "NO ACCENT", NEVER "NOT MEASURED YET": an
    /// icon too pale, too dark or too transparent has no dominant colour.
    /// The manifest then OMITS `theme_color` rather than inventing one.
    accent: string | null;
    /// The extensions this application opens — lowercase, with the dot.
    ///
    /// ⚠️ EMPTY IS THE MOST FREQUENT CASE, not a failure.
    associations: string[];
}

export type Refus =
    | { source: 'client'; motif: 'reponse-illisible'; detail: string }
    | { source: 'service'; statut: number; motif: string };

export type Issue<T> = { etat: 'ok'; value: T } | { etat: 'refus'; refus: Refus };

/* ── L'INTERNE ────────────────────────────────────────────────────────── */

function entetes(deps: DepsCatalogue): Record<string, string> {
    return { authorization: `Bearer ${deps.jeton}` };
}

/// The reason the service returned, or its code alone if it returns none.
///
/// 🔴 THE REASON IS A `string`, NOT A UNION, AND IT IS DELIBERATE — the same
/// arbitration as `televersement.ts`. The vocabulary of refusals belongs to
/// `plateforme/`, which `client/` cannot import; copying it into a union
/// would be the copy no type confronts with its source, silently
/// wrong on renaming. It is the defect `connexion.ts` declares about
/// `aucune-vm`, and which P4 bequeathed without closing it.
async function motifDuService(r: ReponseHttp): Promise<string> {
    try {
        const corps = await r.json();
        if (typeof corps === 'object' && corps !== null && 'refus' in corps) {
            const refus = (corps as { refus: unknown }).refus;
            if (typeof refus === 'string') return refus;
        }
    } catch {
        // An unreadable body is no more informative than an absent body.
    }
    return `statut ${r.status}`;
}

/* ── READING THE CATALOGUE ──────────────────────────────────────────── */

/// `GET /applications?vm=<id>` — the list, or a typed refusal.
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
    const list = (corps as { applications: unknown }).applications;
    if (!Array.isArray(list)) return illisible("'applications' n'est pas un tableau");
    const applications: ApplicationListee[] = [];
    for (const entree of list) {
        if (typeof entree !== 'object' || entree === null) return illisible('une entrée n\'est pas un objet');
        const e = entree as Record<string, unknown>;
        if (typeof e.id !== 'string' || typeof e.nom !== 'string') {
            return illisible("une entrée n'a ni `id` ni `nom` utilisables");
        }
        applications.push({
            id: e.id,
            nom: e.nom,
            icone: typeof e.icone === 'string' ? e.icone : null,
            icone_url: typeof e.icone_url === 'string' ? e.icone_url : null,
            source_max: typeof e.source_max === 'string' ? e.source_max : 'non-mesuree',
            accent: typeof e.accent === 'string' ? e.accent : null,
            // ⚠️ WE FILTER THE ELEMENTS, AND DO NOT MERELY CHECK
            // THAT IT IS AN ARRAY: a non-textual entry would land in
            // a manifest's `accept`, where the browser would reject it without
            // anyone knowing where it came from.
            associations: Array.isArray(e.associations)
                ? e.associations.filter((x): x is string => typeof x === 'string')
                : [],
        });
    }
    return { etat: 'ok', value: applications };
}

function illisible<T>(detail: string): Issue<T> {
    return { etat: 'refus', refus: { source: 'client', motif: 'reponse-illisible', detail } };
}

/* ── LA VM ────────────────────────────────────────────────────────────── */

/// A VM, as `GET /vm` returns it — and NOTHING more.
///
/// ⚠️ NEITHER `adresse`, NOR `userId`: the platform withholds them deliberately
/// (`routes-vm.ts:177-180`) — the first is internal topology, the
/// second is the requester's, which they already know.
export interface VmListee {
    id: string;
    nom: string;
    etat: string;
    prefixe: string | null;
}

/// `GET /vm` — the user's VMs.
///
/// ⚠️ THERE IS AT MOST ONE TO DATE, and it is a property of the DATABASE, not
/// of this module: the partial index `vm_un_utilisateur` of `0001-socle.sql`
/// guarantees it. `routes-vm.ts` writes that the day this invariant fell, its
/// `sessions_ouvertes` field would become wrong. **This module therefore returns a
/// LIST**, so as to have nothing to undo that day.
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
    const list = (corps as { vms: unknown }).vms;
    if (!Array.isArray(list)) return illisible("'vms' n'est pas un tableau");
    const vms: VmListee[] = [];
    for (const entree of list) {
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
    return { etat: 'ok', value: vms };
}

/* ── READING AN ICON ───────────────────────────────────────────── */

/// The PNG's BYTES, through the signed URL the catalogue returned.
///
/// 🔴 NO HEADER IS SENT, AND IT IS THE POINT OF THE BATCH OF AUGUST 30TH, 2026:
/// the same URL, set as is in a `src`, loads identically.
/// Sending the bearer anyway would keep alive a second authorisation
/// path the platform precisely removed.
///
/// 🔴 IT IS NOT REBUILT HERE. The earlier version built
/// `/application/:id/icone?e=…` by its own hands; that path is
/// now REFUSED (`400 signature-absente`), and rebuilding it would be a
/// check one would never otherwise see red.
export async function lireIcone(
    application: ApplicationListee,
    deps: DepsCatalogue,
): Promise<Issue<Uint8Array>> {
    if (application.icone_url === null) {
        return illisible(`l'application ${application.id} n'a pas d'icône`);
    }
    const r = await deps.fetch(`${deps.base}${application.icone_url}`, { method: 'GET' });
    if (!r.ok) return { etat: 'refus', refus: { source: 'service', statut: r.status, motif: await motifDuService(r) } };
    return { etat: 'ok', value: new Uint8Array(await r.arrayBuffer()) };
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
    return { etat: 'ok', value: null };
}
