// Les tests de l'orchestration du téléversement — SUR L'HÔTE, SANS DOM.
//
// 🔴 CE FICHIER N'EXISTE QUE PARCE QUE LES TROIS DÉPENDANCES SONT INJECTÉES.
// `fetch`, l'horloge et l'`AbortSignal` étant des paramètres, la séquence
// entière s'éprouve sans navigateur ni serveur — et c'est la même propriété qui
// permettra au pilote de recette (D14) de lancer LE CODE DU PRODUIT sous Node.
//
// ⚠️ `Buffer` EST INTERDIT ICI : `client/` n'a pas `@types/node`, et un test qui
// l'emploierait passerait sous Vitest en CASSANT `npm run typecheck`
// (`TS2580`). Piège déjà payé par ce dépôt — d'où `Uint8Array` partout.
import { describe, expect, it } from 'vitest';
import { condenserHex } from '../../../proto/ts/sha256';
import { televerser, type DepsTeleversement, type Fetch, type Issue } from './televersement';

/// Un contenu déterministe — un générateur congruentiel, jamais `Math.random` :
/// un test dont les octets changent d'une exécution à l'autre ne se rejoue pas.
function octetsDe(taille: number, graine = 7) {
    const sortie = new Uint8Array(taille);
    let x = graine;
    for (let i = 0; i < taille; i += 1) {
        x = (x * 1103515245 + 12345) & 0x7fffffff;
        sortie[i] = (x >>> 16) & 0xff;
    }
    return sortie;
}

interface Appel {
    methode: string;
    url: string;
    octets: number;
    entetes: Record<string, string>;
    corps?: string;
}

interface Reponse {
    status: number;
    corps?: unknown;
}

type Route = 'creation' | 'etat' | 'tranche' | 'sceller';

function routeDe(url: string, methode: string): Route {
    if (url.endsWith('/sceller')) return 'sceller';
    if (url.includes('/tranche/')) return 'tranche';
    return methode === 'POST' ? 'creation' : 'etat';
}

/// Un service factice qui ENREGISTRE ce qu'on lui envoie. C'est l'enregistrement
/// qui compte : le critère ③ de la spec se juge aux OCTETS RÉELLEMENT ÉMIS, pas
/// au verdict final — « recommencer à zéro passerait un test qui ne regarde que
/// le résultat ».
function serveur(reponses: Partial<Record<Route, Reponse>>) {
    const appels: Appel[] = [];
    const fetch: Fetch = async (url, init) => {
        const methode = init?.method ?? 'GET';
        const corpsEmis = init?.body;
        appels.push({
            methode,
            url,
            octets: corpsEmis instanceof Uint8Array ? corpsEmis.byteLength : 0,
            entetes: init?.headers ?? {},
            corps: typeof corpsEmis === 'string' ? corpsEmis : undefined,
        });
        const r = reponses[routeDe(url, methode)] ?? { status: 200, corps: {} };
        return { ok: r.status < 400, status: r.status, json: async () => r.corps };
    };
    const puts = (): Appel[] => appels.filter((a) => a.methode === 'PUT');
    return {
        fetch,
        appels,
        puts,
        octetsEmis: (): number => puts().reduce((s, a) => s + a.octets, 0),
        rangs: (): number[] => puts().map((a) => Number(a.url.split('/tranche/')[1])),
    };
}

function deps(fetch: Fetch, extra: Partial<DepsTeleversement> = {}): DepsTeleversement {
    return { base: 'https://p', jeton: 'j-1', fetch, maintenant: () => 0, ...extra };
}

/// Un fichier de 16 octets découpé par 4 : quatre tranches, le cas de la rouge n°1.
const OCTETS = octetsDe(16);
const SHA = condenserHex(OCTETS);
const fichier = (o: BlobPart = OCTETS, nom = 'setup.exe'): File => new File([o], nom);
const creation = { status: 200, corps: { id: 't-1', taille_tranche: 4, tranches_presentes: [] } };
/// 🔴 LA FORME QUE LA ROUTE REND, ET LA SEULE : `{n, octets}`.
///
/// ⚠️ CES CAS PORTAIENT DES RANGS NUS pendant que `routes-televersement.ts`
/// s'écrivait en parallèle, et le module les tolérait. La route étant arrêtée —
/// elle rend le LISTAGE du magasin —, la tolérance est retirée : elle faisait
/// **croire le service sur une taille qu'il n'avait jamais annoncée**, si bien
/// qu'un rang présent à la MAUVAISE taille serait passé pour conforme et que le
/// scellement aurait refusé plus tard, ailleurs, sans que rien ne relie les deux.
const presente = (n: number, octets = 4) => ({ n, octets });

const etatDe = (presentes: unknown, extra: Record<string, unknown> = {}): Reponse => ({
    status: 200,
    corps: { taille: 16, sha256: SHA, taille_tranche: 4, tranches_presentes: presentes, ...extra },
});

describe('le chemin nominal', () => {
    it('empreint, crée, dépose les quatre tranches et scelle', async () => {
        const s = serveur({ creation });
        const issue = await televerser(fichier(), deps(s.fetch));
        expect(issue).toEqual({ etat: 'scelle', id: 't-1', taille: 16, sha256: SHA, deposees: [0, 1, 2, 3] });
        expect(s.rangs()).toEqual([0, 1, 2, 3]);
        expect(s.octetsEmis()).toBe(16);
        // L'ordre du protocole : créer, déposer, sceller — et une seule création.
        expect(s.appels.map((a) => routeDe(a.url, a.methode))).toEqual([
            'creation', 'tranche', 'tranche', 'tranche', 'tranche', 'sceller',
        ]);
    });

    it("annonce dès la CRÉATION le nom, la taille et l'empreinte (D5)", async () => {
        // 🔴 C'est cette annonce, et elle seule, qui rendra une reprise
        // vérifiable : sans `{taille, sha256}` posés d'avance, rien ne dira que
        // le fichier re-choisi après un rechargement d'onglet est LE MÊME.
        const s = serveur({ creation });
        await televerser(fichier(OCTETS, 'programme.msi'), deps(s.fetch));
        expect(s.appels[0].url).toBe('https://p/televersement');
        expect(s.appels[0].entetes.authorization).toBe('Bearer j-1');
        expect(JSON.parse(s.appels[0].corps ?? '{}')).toEqual({ nom: 'programme.msi', taille: 16, sha256: SHA });
    });

    it("la `fetch` de la plateforme d'exécution satisfait le type `Fetch`", () => {
        // 🔵 LE CONTRÔLE QUI EMPÊCHE `Fetch` DE DÉRIVER. Il est déclaré à la main
        // — une `Response` complète serait infabricable dans un test —, donc rien
        // ne garantirait qu'une VRAIE `fetch` le satisfasse encore après une
        // retouche. Cette affectation est vérifiée par `tsc --noEmit`, et elle
        // rougirait à la COMPILATION le jour où les deux formes divergeraient.
        const preuve: Fetch = globalThis.fetch;
        expect(typeof preuve).toBe('function');
    });

    it('porte le jeton et le type binaire sur chaque tranche déposée', async () => {
        const s = serveur({ creation });
        await televerser(fichier(), deps(s.fetch));
        for (const p of s.puts()) {
            expect(p.entetes.authorization).toBe('Bearer j-1');
            expect(p.entetes['content-type']).toBe('application/octet-stream');
        }
    });

    it("d'un fichier VIDE, ne dépose aucune tranche et scelle quand même", async () => {
        const s = serveur({ creation: { status: 200, corps: { id: 't-0', taille_tranche: 4, tranches_presentes: [] } } });
        const issue = await televerser(fichier(new Uint8Array(0)), deps(s.fetch));
        expect(issue).toMatchObject({ etat: 'scelle', taille: 0, sha256: condenserHex(new Uint8Array(0)) });
        expect(s.puts()).toHaveLength(0);
        expect(s.appels.at(-1)?.url).toBe('https://p/televersement/t-0/sceller');
    });
});

describe('🔴 la reprise ne redépose QUE les tranches manquantes', () => {
    it('sur quatre tranches dont deux présentes, en émet DEUX — pas quatre', async () => {
        // 🔴 C'est la rouge du critère ③ : un produit qui recommencerait à zéro
        // rendrait le MÊME verdict `scelle`. Seul le COMPTE D'OCTETS le sépare
        // d'un produit correct — d'où l'assertion sur `octetsEmis`, et non sur
        // la seule issue.
        const s = serveur({ etat: etatDe([presente(0), presente(1)]) });
        const issue = await televerser(fichier(), deps(s.fetch, { reprise: 't-1' }));
        // ⚠️ LE COMPTE D'OCTETS PASSE EN PREMIER, ET CE N'EST PAS COSMÉTIQUE :
        // `deposees` est un champ que le PRODUIT déclare, donc falsifiable par un
        // produit qui redéposerait tout en annonçant le contraire. Les octets
        // réellement émis, eux, ne se déclarent pas.
        expect(s.octetsEmis()).toBe(8);
        expect(s.rangs()).toEqual([2, 3]);
        expect(s.puts()).toHaveLength(2);
        expect(issue).toMatchObject({ etat: 'scelle', id: 't-1', deposees: [2, 3] });
    });

    it('accepte AUSSI la forme riche `{n, octets}` du listage', async () => {
        const s = serveur({ etat: etatDe([{ n: 0, octets: 4 }, { n: 2, octets: 4 }]) });
        const issue = await televerser(fichier(), deps(s.fetch, { reprise: 't-1' }));
        expect(s.octetsEmis()).toBe(8);
        expect(issue).toMatchObject({ etat: 'scelle', deposees: [1, 3] });
    });

    it("d'un dépôt COMPLET, ne redépose rien et va droit au scellement", async () => {
        const s = serveur({ etat: etatDe([presente(0), presente(1), presente(2), presente(3)]) });
        const issue = await televerser(fichier(), deps(s.fetch, { reprise: 't-1' }));
        expect(s.puts()).toHaveLength(0);
        expect(issue).toMatchObject({ etat: 'scelle', deposees: [] });
        expect(s.appels.map((a) => a.methode)).toEqual(['GET', 'POST']);
    });

    it('refuse une tranche mal TAILLÉE au lieu de la redemander sans fin', async () => {
        // `incoherentes` n'est pas `manquantes` : redéposer ne réparerait rien,
        // et boucler serait le vrai défaut.
        const s = serveur({ etat: etatDe([{ n: 0, octets: 3 }]) });
        const issue = await televerser(fichier(), deps(s.fetch, { reprise: 't-1' }));
        expect(issue).toEqual({
            etat: 'refus',
            id: 't-1',
            refus: { source: 'client', motif: 'tranches-incoherentes', detail: 'rangs 0' },
        });
        expect(s.puts()).toHaveLength(0);
    });
});

describe("🔴 la reprise vérifie l'identité du fichier AVANT de reprendre", () => {
    it("refuse une TAILLE différente AVANT d'empreindre, et n'émet AUCUN PUT", async () => {
        // 🔴 LA PREMIÈRE ASSERTION EST LA SEULE QUI DISTINGUE LE GARDE DE TAILLE
        // DU GARDE D'EMPREINTE, et elle a été ajoutée APRÈS avoir vu la rouge
        // correspondante rester VERTE : un fichier de 12 octets a de toute façon
        // une empreinte différente, donc le verdict `fichier-different` sortait
        // même sans le garde de taille. Ce que ce garde achète n'est pas le
        // verdict — c'est de ne PAS payer la passe de lecture complète, onze
        // secondes pour 800 Mo. Une phase `empreinte` observée prouve qu'elle a
        // été payée.
        const phases: string[] = [];
        const s = serveur({ etat: etatDe([presente(0), presente(1)]) });
        const issue = await televerser(
            fichier(octetsDe(12)),
            deps(s.fetch, { reprise: 't-1', progression: (p) => void phases.push(p.phase) }),
        );
        expect(phases).toEqual([]);
        expect(issue.etat).toBe('refus');
        expect((issue as Extract<Issue, { etat: 'refus' }>).refus).toMatchObject({
            source: 'client',
            motif: 'fichier-different',
        });
        expect(s.puts()).toHaveLength(0);
    });

    it("refuse un CONTENU différent à taille égale, et n'émet AUCUN PUT", async () => {
        const s = serveur({ etat: etatDe([presente(0), presente(1)]) });
        const issue = await televerser(fichier(octetsDe(16, 99)), deps(s.fetch, { reprise: 't-1' }));
        expect(issue).toMatchObject({ etat: 'refus', id: 't-1', refus: { motif: 'fichier-different' } });
        expect(s.puts()).toHaveLength(0);
    });

    it("refuse de reprendre un état qui n'annonce NI taille NI empreinte", async () => {
        // Reprendre à l'aveugle mélangerait les tranches de deux fichiers, et le
        // scellement échouerait sans que rien ne dise pourquoi.
        const s = serveur({ etat: { status: 200, corps: { taille_tranche: 4, tranches_presentes: [] } } });
        const issue = await televerser(fichier(), deps(s.fetch, { reprise: 't-1' }));
        expect(issue).toMatchObject({ etat: 'refus', refus: { motif: 'etat-illisible' } });
        expect(s.puts()).toHaveLength(0);
    });
});

describe('🔴 les refus du service remontent TYPÉS', () => {
    it("d'un scellement en 409 `empreinte`, rend le refus et ne réessaie PAS", async () => {
        const s = serveur({ creation, sceller: { status: 409, corps: { refus: 'empreinte' } } });
        const issue = await televerser(fichier(), deps(s.fetch));
        expect(issue).toEqual({
            etat: 'refus',
            id: 't-1',
            refus: { source: 'service', etape: 'scellement', statut: 409, motif: 'empreinte' },
        });
        // ⚠️ LES DEUX ASSERTIONS QUI SUIVENT SONT LE CŒUR DE CETTE ROUGE : une
        // boucle de réessai téléverserait sans fin, et un `id` perdu obligerait
        // l'appelant à tout redéposer.
        expect(s.appels.filter((a) => a.url.endsWith('/sceller'))).toHaveLength(1);
        expect(s.octetsEmis()).toBe(16);
    });

    it("d'un dépôt de tranche refusé, s'arrête à la PREMIÈRE et nomme l'étape", async () => {
        const s = serveur({ creation, tranche: { status: 413, corps: { refus: 'tranche-trop-grande' } } });
        const issue = await televerser(fichier(), deps(s.fetch));
        expect(issue).toMatchObject({
            etat: 'refus',
            refus: { source: 'service', etape: 'tranche', statut: 413, motif: 'tranche-trop-grande' },
        });
        expect(s.puts()).toHaveLength(1);
    });

    it("d'un corps sans `refus`, rend le statut plutôt que d'inventer un motif", async () => {
        const s = serveur({ creation: { status: 503, corps: 'indisponible' } });
        const issue = await televerser(fichier(), deps(s.fetch));
        expect(issue).toEqual({
            etat: 'refus',
            refus: { source: 'service', etape: 'creation', statut: 503, motif: 'http-503' },
        });
    });

    it("d'une création sans identifiant, refuse au lieu de déposer dans le vide", async () => {
        const s = serveur({ creation: { status: 200, corps: { taille_tranche: 4 } } });
        const issue = await televerser(fichier(), deps(s.fetch));
        expect(issue).toMatchObject({ etat: 'refus', refus: { motif: 'etat-illisible' } });
        expect(s.puts()).toHaveLength(0);
    });

    it("d'une `taille_tranche` absurde, refuse au lieu de LEVER", async () => {
        // `plan` lève sur un pas nul — sa garde vise un défaut de programme, pas
        // une donnée de fil. La valider ici est ce qui rend un refus nommé.
        const s = serveur({ creation: { status: 200, corps: { id: 't-1', taille_tranche: 0 } } });
        const issue = await televerser(fichier(), deps(s.fetch));
        expect(issue).toMatchObject({ etat: 'refus', id: 't-1', refus: { motif: 'etat-illisible' } });
    });

    it("de `tranches_presentes` de forme inconnue, refuse", async () => {
        const s = serveur({ creation: { status: 200, corps: { id: 't-1', taille_tranche: 4, tranches_presentes: 'deux' } } });
        const issue = await televerser(fichier(), deps(s.fetch));
        expect(issue).toMatchObject({ etat: 'refus', refus: { motif: 'etat-illisible' } });
    });
});

describe("l'interruption est un refus, jamais une panne", () => {
    it('arrête le dépôt entre deux tranches et rend `interrompu` avec son `id`', async () => {
        const s = serveur({ creation });
        const arret = new AbortController();
        // ⚠️ L'HORLOGE DOIT AVANCER : figée, le cadenceur ne laisserait passer que
        // le premier événement de chaque phase — celui qui est forcé — et
        // l'abandon ne serait jamais déclenché. Ce test a été VU VERT À TORT
        // pour cette raison avant d'être corrigé.
        let horloge = 0;
        const issue = await televerser(
            fichier(),
            deps(s.fetch, {
                signal: arret.signal,
                maintenant: () => (horloge += 1000),
                progression: (p) => {
                    if (p.phase === 'transfert' && p.octets > 0) arret.abort();
                },
            }),
        );
        expect(issue).toMatchObject({ etat: 'refus', id: 't-1', refus: { motif: 'interrompu' } });
        expect(s.puts().length).toBeLessThan(4);
    });

    it("convertit le rejet d'un `fetch` COUPÉ en refus, mais laisse passer les autres", async () => {
        const arret = new AbortController();
        const casse: Fetch = async () => {
            throw new Error('coupé');
        };
        arret.abort();
        const issue = await televerser(fichier(), deps(casse, { signal: arret.signal }));
        expect(issue).toMatchObject({ etat: 'refus', refus: { motif: 'interrompu' } });
        // Sans signal levé, la même panne REMONTE : ce module n'a rien d'utile à
        // dire d'une panne d'environnement, et la déguiser en refus la ferait
        // passer pour une décision de protocole.
        await expect(televerser(fichier(), deps(casse))).rejects.toThrow('coupé');
    });
});

describe("la progression, cadencée par l'horloge injectée", () => {
    it('annonce les trois phases, et ne dépend que du paramètre `maintenant`', async () => {
        const s = serveur({ creation });
        const vues: string[] = [];
        let horloge = 0;
        await televerser(
            fichier(),
            deps(s.fetch, {
                maintenant: () => (horloge += 1000),
                progression: (p) => void vues.push(`${p.phase}:${p.octets}/${p.total}`),
            }),
        );
        expect(vues[0]).toBe('empreinte:0/16');
        expect(vues.at(-1)).toBe('scellement:16/16');
        expect(new Set(vues.map((v) => v.split(':')[0]))).toEqual(
            new Set(['empreinte', 'transfert', 'scellement']),
        );
    });

    it('retient les événements NON forcés quand la période n\'est pas écoulée', async () => {
        // L'horloge figée : seuls les changements de phase, qui passent toujours,
        // doivent sortir. Sans cadence, empreindre 800 Mo en émettrait des milliers.
        const s = serveur({ creation });
        const vues: string[] = [];
        await televerser(fichier(), deps(s.fetch, { progression: (p) => void vues.push(p.phase) }));
        expect(vues.filter((v) => v === 'transfert')).toHaveLength(1);
    });
});
