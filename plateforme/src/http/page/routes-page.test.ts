import { afterEach, describe, expect, it } from 'vitest';
import { mkdirSync, mkdtempSync, readdirSync, readlinkSync, symlinkSync, writeFileSync } from 'node:fs';
import { request as httpRequest } from 'node:http';
import type { ServerResponse } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable, Writable } from 'node:stream';
import { baseNeuve } from '../../base/harnais';
import type { Pilote } from '../../base/pilote';
import { startServer, type ServicePlateforme } from '../serveur';
import type { Config } from '../../config';
import { serveWithStream } from './routes-page';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'page-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'page-tranches-')), 'televersements'),
    auth: 'pomerium',
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

// Une racine temporaire, bâtie à la main : le test ne dépend pas de
// `client/dist`, qui peut ne pas être bâti.
function racineJetable(): string {
    const racine = mkdtempSync(join(tmpdir(), 'page-'));
    mkdirSync(join(racine, 'assets'));
    // 🔴 `hub.html` PORTE UN CORPS DISTINCT D'`index.html` — délibérément :
    // c'est ce qui rend le test « GET / rend le corps EXACT de hub.html »
    // discriminant. Depuis la décision « sers le hub à la racine » (30 août
    // 2026), `resolution.ts::PAGE` résout `hub.html` pour `/` ; `index.html`
    // (la page de session) reste servi, mais à SON PROPRE chemin explicite —
    // voir le test dédié « GET /index.html sert encore la page de session ».
    writeFileSync(join(racine, 'hub.html'), '<!doctype html><title>hub</title>');
    writeFileSync(join(racine, 'index.html'), '<!doctype html><title>page</title>');
    writeFileSync(join(racine, 'assets', 'index-a1b2c3.js'), 'export const x = 1;\n');
    // ⚠️ UN NOM QUE VITE N'EMPREINTE JAMAIS, à la RACINE : c'est le cas que
    // l'`immutable` d'un an rendait non révisable. `client/dist` en porte un
    // vrai (`ls client/dist` -> `hub.webmanifest`), et ce fichier-ci le
    // reproduit sans dépendre d'un build.
    writeFileSync(join(racine, 'hub.webmanifest'), '{"name":"hub"}');
    // Le PIÈGE que le critère ⑥ éprouve : un fichier homonyme d'une route.
    writeFileSync(join(racine, 'sante'), 'this must NEVER be served');
    writeFileSync(join(racine, 'secret.env'), 'MOT_DE_PASSE=x');
    return racine;
}

/// Une racine qui porte DEUX liens symboliques qui en SORTENT — le trou de la
/// Critique 3 (round 2). `dessus/` vit à CÔTÉ de la racine, jamais dedans.
function rootWithSymlinks(): string {
    const parent = mkdtempSync(join(tmpdir(), 'page-parent-'));
    const dessus = join(parent, 'dessus');
    mkdirSync(dessus);
    writeFileSync(join(dessus, 'vole.json'), '"SECRET_DU_DESSUS"');
    writeFileSync(join(dessus, 'vole.html'), '<!doctype html>SECRET_DU_DESSUS_HTML');

    const racine = join(parent, 'racine');
    mkdirSync(racine);
    writeFileSync(join(racine, 'index.html'), '<!doctype html><title>page</title>');
    // Un lien FICHIER qui sort par un chemin RELATIF.
    symlinkSync(join('..', 'dessus', 'vole.json'), join(racine, 'lien.json'));
    // Un lien RÉPERTOIRE qui sort — aucun `..` n'apparaît jamais dans l'URL
    // qui l'atteint (`/lien-rep/vole.html`).
    symlinkSync(join('..', 'dessus'), join(racine, 'lien-rep'));
    return racine;
}

/// Compte, PARMI les descripteurs ouverts du PROCESSUS DE TEST (le serveur
/// tourne dans ce même processus — `startServer` ne fork rien), ceux qui
/// pointent EXACTEMENT vers `chemin`.
///
/// 🔴 CIBLÉ, PAS UN TOTAL AVEC TOLÉRANCE (round 3, Mineur) : un premier jet
/// comptait le total des descripteurs du processus avec une marge de `N/2` —
/// un contrôle qu'une fuite RÉELLE peut laisser VERT, noyée dans le bruit des
/// sockets clientes qui s'ouvrent et se ferment. `readlink` sur le fichier
/// précis — celui qu'a employé la revue — rend `0` contre `210` : exactement
/// discriminant.
function comptesFdPour(chemin: string): number {
    let n = 0;
    for (const entree of readdirSync('/proc/self/fd')) {
        try {
            if (readlinkSync(`/proc/self/fd/${entree}`) === chemin) n++;
        } catch {
            // Le descripteur a pu se fermer entre le listage et la lecture —
            // ce n'est pas une fuite, on l'ignore.
        }
    }
    return n;
}

/// Toutes les requêtes du fichier passent par elle, jamais par `fetch()` nu.
///
/// ⚠️ NEUF 4 (round 3) : ce fichier est passé de 221 ms à 4–10 s, avec des
/// blocages REPRODUCTIBLES de 3,0 s sur 1 à 3 tests par exécution — absents
/// avant ce lot. DIX-HUIT serveurs ÉPHÉMÈRES successifs, un par test, et
/// `fetch()` GARDE la connexion ouverte par défaut (HTTP/1.1 keep-alive) :
/// `service.close()`, dans `afterEach`, attend qu'une connexion gardée
/// ouverte se referme — elle ne le fait pas tant que rien ne le lui demande.
/// C'est le CORPS DU TEST qui bloque (établissement/fermeture de connexion
/// côté client), jamais le produit : un banc à `http.request` neuf ne l'a
/// jamais vu. `Connection: close` fait fermer le socket immédiatement après
/// la réponse — vérifié que Node l'honore (le serveur le reçoit et ferme).
function requeteFermee(url: string, options: RequestInit = {}): Promise<Response> {
    return fetch(url, { ...options, headers: { ...options.headers, Connection: 'close' } });
}

describe('GET /', () => {
    // 🔴 LE TÉMOIN NÉGATIF. Sans lui, le 200 du test suivant ne prouve pas que
    // PLATEFORME_PAGE a servi à quelque chose.
    it("without PLATEFORME_PAGE, GET / returns yesterday's 404, word for word", async () => {
        base = await baseNeuve('page-temoin-negatif');
        service = await startServer({ ...CONFIG, racinePage: undefined }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('not found\n');
    });

    // ⚠️ MINEUR PROMU (round 2) : `racinePage: ''` doit se comporter comme
    // l'ABSENCE, jamais comme `resolve('')` (le répertoire courant du
    // processus). `process.chdir` vers un répertoire CONTRÔLÉ qui porte un
    // `index.html` rend la garde discriminante : sans elle, ce test verrait
    // `200` et le corps piégé.
    it("an EMPTY root ('') also removes the server, never the current directory", async () => {
        const cwdBefore = process.cwd();
        const piege = mkdtempSync(join(tmpdir(), 'page-cwd-piege-'));
        writeFileSync(join(piege, 'index.html'), 'THIS MUST NEVER BE SERVED');
        process.chdir(piege);
        try {
            base = await baseNeuve('page-racine-vide');
            service = await startServer({ ...CONFIG, racinePage: '' }, base);
            const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
            expect(r.status).toBe(404);
        } finally {
            process.chdir(cwdBefore);
        }
    });

    it('with PLATEFORME_PAGE, GET / returns 200', async () => {
        base = await baseNeuve('page-index-statut');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        expect(r.status).toBe(200);
    });

    // 🔴 LA ROUGE DE « SERS LE HUB À LA RACINE » (30 août 2026) : avant ce
    // lot, `GET /` rendait le corps d'`index.html` — la page de SESSION,
    // mesurée en PRODUCTION comme la panne (voir `resolution.ts::PAGE`).
    it('with PLATEFORME_PAGE, GET / returns the EXACT body of hub.html', async () => {
        base = await baseNeuve('page-index-corps');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        // Comparer les OCTETS, jamais le seul code 200.
        expect(await r.text()).toBe('<!doctype html><title>hub</title>');
    });

    // ⚠️ AUCUN CHEMIN EXISTANT NE CASSE : `index.html` reste servi, à SON
    // PROPRE chemin explicite — c'est CE test qui le garantit, distinct du
    // repli SPA de `/` ci-dessus.
    it('GET /index.html still serves the session page, explicitly', async () => {
        base = await baseNeuve('page-index-explicite');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/index.html`);
        expect(r.status).toBe(200);
        expect(await r.text()).toBe('<!doctype html><title>page</title>');
    });

    it('the document carries the CSP', async () => {
        base = await baseNeuve('page-index-csp');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        expect(r.headers.get('content-security-policy')).toContain("default-src 'self'");
    });

    it('the document carries cache-control no-store', async () => {
        base = await baseNeuve('page-index-cache');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        expect(r.headers.get('cache-control')).toBe('no-store');
    });

    it('a FINGERPRINTED asset carries immutable, NEVER no-store', async () => {
        base = await baseNeuve('page-actif');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/assets/index-a1b2c3.js`);
        expect(r.headers.get('cache-control')).toBe('public, max-age=31536000, immutable');
    });

    // 🔴 LE DÉFAUT MESURÉ SUR LE VRAI `client/dist` : `/hub.webmanifest`
    // rendait `public, max-age=31536000, immutable`, donc **le manifeste PWA
    // du hub non révisable pendant un an** chez tout navigateur l'ayant vu.
    // La classification se faisait par EXTENSION, et la liste MIME admet
    // `webmanifest`, `json`, `ico`, `png` — que Vite n'empreinte jamais à la
    // racine. Sous nginx, `location /` n'émet AUCUN `Cache-Control` : c'était
    // une régression que le seul montage Pomerium introduisait.
    //
    // 🔴 ANCRÉ SUR L'ABSENCE D'`immutable`, PAS SUR LA VALEUR EXACTE : c'est
    // la propriété qui compte, et elle rougirait quelle que soit la forme
    // qu'une régression prendrait pour revenir à un an.
    it("a NON-fingerprinted resource NEVER gets immutable", async () => {
        base = await baseNeuve('page-manifeste-immutable');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/hub.webmanifest`);
        expect(r.headers.get('cache-control')).not.toContain('immutable');
    });

    // 🔴 SÉPARÉ, ET C'EST L'AUTRE EXTRÊME : `no-store` referait payer le
    // transfert entier à chaque visite. Une seule assertion groupée
    // n'éprouverait que la première.
    it("a NON-fingerprinted resource does not get no-store either", async () => {
        base = await baseNeuve('page-manifeste-nostore');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/hub.webmanifest`);
        expect(r.headers.get('cache-control')).not.toContain('no-store');
    });

    it('a NON-fingerprinted resource is REVALIDATABLE', async () => {
        base = await baseNeuve('page-manifeste-revalidable');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/hub.webmanifest`);
        expect(r.headers.get('cache-control')).toContain('must-revalidate');
    });

    // 🔴 LA ROUGE DE L'ORDRE DE CHAÎNAGE. Un fichier nommé `sante` déposé dans
    // la racine ne doit JAMAIS supplanter le routeur de santé — la panne la
    // plus discrète possible, le service répondant 200 avec un corps
    // plausible. SÉPARÉE EN DEUX TESTS (round 2, Important 1) : dans la
    // mutation de l'étape 6, `content-type` rougit et arrête le test AVANT
    // que le corps ne soit jamais vérifié — la seconde assertion, groupée,
    // n'était donc jamais éprouvée.
    it('/sante stays served by ITS router — json content-type, despite a file of the same name', async () => {
        base = await baseNeuve('page-homonyme-type');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/sante`);
        expect(r.headers.get('content-type')).toContain('application/json');
    });

    it('/sante stays served by ITS router — body, despite a file of the same name', async () => {
        base = await baseNeuve('page-homonyme-corps');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/sante`);
        expect(await r.text()).not.toContain('JAMAIS');
    });

    // 🔴 SÉPARÉS (round 3, Neuf 2) : groupés, `expect(r.status).toBe(404)`
    // courait EN PREMIER — si le servant avait fuité le secret dans un
    // `200`, cette assertion aurait échoué et le test se serait arrêté LÀ,
    // sans jamais atteindre celle qui vérifie l'absence du secret. C'est
    // exactement le patron dénoncé par l'Important 1 du round 2, réintroduit
    // ici en corrigeant celui-là — chacune des deux DOIT pouvoir rougir
    // indépendamment de l'autre.
    it('refuses a file outside the MIME list', async () => {
        base = await baseNeuve('page-mime-refuse-statut');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/secret.env`);
        expect(r.status).toBe(404);
    });

    it("a file outside the MIME list never leaks its content, EVEN if the status regressed", async () => {
        // ⚠️ ANCRÉ SUR LE CORPS (round 2, Important 3) : un `404` seul ne dit
        // pas SI c'est parce que la liste MIME est close, ou parce que rien
        // ne répond — les deux rendent le même statut. Cette assertion-ci ne
        // dépend PAS du statut : elle rougirait même si un futur défaut
        // faisait passer la réponse à `200`.
        base = await baseNeuve('page-mime-refuse-corps');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/secret.env`);
        expect(await r.text()).not.toContain('MOT_DE_PASSE');
    });

    // ⚠️ IMPORTANT 2 (round 2) : le trou que l'addendum précédent fermait
    // (`resolution.ts`, motif `nom-vide`) n'était éprouvé qu'à la règle PURE,
    // jamais avec un VRAI fichier `.json` nu sur le disque — la seule tâche
    // du lot qui en expose.
    it('refuses a bare `.json` file (empty name), even one really present on the disk', async () => {
        const racine = racineJetable();
        writeFileSync(join(racine, '.json'), '"must never be served"');
        base = await baseNeuve('page-json-nu-disque');
        service = await startServer({ ...CONFIG, racinePage: racine }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/.json`);
        expect(r.status).toBe(404);
    });

    // ⚠️ IMPORTANT 2 (round 2) : la traversée, elle aussi, au niveau DISQUE.
    // 🔴 FORME ENCODÉE, PAS LA FORME EN CLAIR : `new URL(req.url, …)`, dans
    // `routes-page.ts`, COLLAPSE `/assets/../../etc/passwd` en `/etc/passwd`
    // AVANT même que `resoudre()` ne le voie — la forme en clair ne teste
    // donc RIEN de la garde de `resolution.ts`, elle est neutralisée un cran
    // plus tôt. `%2e%2e%2f` SURVIT à cette normalisation (vérifié :
    // `new URL('/%2e%2e%2f…', base).pathname` la restitue verbatim), et
    // c'est ce que `resoudre()` décode et refuse LUI-MÊME.
    it('refuses an ENCODED traversal escaping the root, through a real HTTP request', async () => {
        base = await baseNeuve('page-traversee-disque');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(
            `http://127.0.0.1:${service.port}/assets/..%2f..%2f..%2f..%2f..%2f..%2fetc%2fpasswd`,
        );
        expect(r.status).toBe(404);
    });

    // 🔴 CRITIQUE 3 (round 2), MESURÉE : un lien FICHIER qui sort de la
    // racine par un chemin relatif rendait `200` avec le contenu volé avant
    // le `realpath` de `routes-page.ts`.
    it('refuses a file reached through a FILE symbolic link escaping the root', async () => {
        base = await baseNeuve('page-link-file');
        service = await startServer({ ...CONFIG, racinePage: rootWithSymlinks() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/lien.json`);
        expect(r.status).toBe(404);
    });

    // 🔴 CRITIQUE 3 (round 2), MESURÉE : un lien RÉPERTOIRE qui sort de la
    // racine — aucun `..` n'apparaît jamais dans l'URL qui l'atteint.
    it('refuses a file reached through a DIRECTORY symbolic link escaping the root', async () => {
        base = await baseNeuve('page-lien-repertoire');
        service = await startServer({ ...CONFIG, racinePage: rootWithSymlinks() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/lien-rep/vole.html`);
        expect(r.status).toBe(404);
    });

    // 🔴 CRITIQUE 1 (round 2), MESURÉE : `.pipe()` ne détruit pas la source
    // quand la destination meurt. Un client qui abandonne — ici, un socket
    // détruit dès la réception des en-têtes — laissait le descripteur du
    // fichier ouvert INDÉFINIMENT, de façon MONOTONE.
    it("a download ABANDONED by the client does not leak a descriptor", async () => {
        const racine = racineJetable();
        // Un fichier assez gros pour laisser le temps d'abandonner AVANT que
        // le flux n'ait fini de couler.
        const cheminGros = join(racine, 'assets', 'gros.js');
        writeFileSync(cheminGros, 'x'.repeat(8 * 1024 * 1024));
        base = await baseNeuve('page-fd-abandon');
        service = await startServer({ ...CONFIG, racinePage: racine }, base);

        const before = comptesFdPour(cheminGros);
        const N = 20;
        for (let i = 0; i < N; i++) {
            await new Promise<void>((resolvePromesse, reject) => {
                const req = httpRequest(
                    { hostname: '127.0.0.1', port: service!.port, path: '/assets/gros.js', method: 'GET' },
                    (res) => {
                        // Abandon dès la réception des en-têtes — avant que le
                        // corps n'ait fini de couler.
                        res.destroy();
                        resolvePromesse();
                    },
                );
                req.on('error', () => resolvePromesse());
                req.end();
            });
        }
        // Laisse le temps aux gestionnaires 'close' asynchrones de courir —
        // la fermeture d'un descripteur de fichier n'est pas synchrone.
        await new Promise((r) => setTimeout(r, 300));
        const apres = comptesFdPour(cheminGros);
        // 🔴 EXACTEMENT DISCRIMINANT (round 3, Mineur) : ciblé sur LE fichier
        // précis, une fuite rendrait `apres` proche de `before + N` (un fd par
        // abandon, monotone, mesuré). Aucune tolérance : `before` et `apres`
        // DOIVENT être égaux.
        expect(apres).toBe(before);
    });

    // 🔴 CRITIQUE 2 (round 2), MESURÉE : une lecture qui casse APRÈS que les
    // en-têtes sont partis n'a AUCUN gestionnaire d'erreur sous `.pipe()` —
    // un `EventEmitter` qui émet `error` sans écouteur LÈVE, et la revue a
    // reproduit le service ENTIER mourant (`EMFILE`). Le flux est ICI
    // INJECTÉ pour reproduire une erreur de lecture MI-FLUX SANS provoquer
    // un vrai épuisement de descripteurs — ce qui ferait courir le risque
    // exact que ce test existe pour éprouver, sur le processus de test
    // lui-même.
    //
    // ⚠️ MONTAGE UNITAIRE, SANS SERVEUR HTTP RÉEL (round 3, Neuf 3) : ceci
    // appelle `serveWithStream` directement, avec un `req`/`rep` FABRIQUÉS —
    // il n'y a ni socket, ni port, ni processus serveur. Le titre précédent
    // (« … ne fait PAS mourir LE SERVICE ») promettait une propriété que ce
    // montage n'établit PAS ; la propriété reste vraie (voir le commentaire
    // de `serveWithStream` sur ce que ce choix coûte et ce qu'il n'établit
    // pas), mais CE test-ci n'éprouve que le comportement de la FONCTION.
    //
    // Fabrique un couple `(racine, rep, req, flux)` NEUF à chaque appel :
    // le flux factice est à USAGE UNIQUE (`envoyee`), et `rep` porte son
    // propre état de destruction — les deux tests ci-dessous ne peuvent PAS
    // partager une seule fabrication sans que l'un des deux devienne
    // vacueux.
    function unitCallWithFaultyStream(): { result: Promise<boolean>; rep: Writable } {
        const racine = racineJetable();
        // 🔴 UN VRAI `Writable`, PAS UN OBJET FACTICE À MÉTHODES MUETTES :
        // `pipeline()` s'appuie sur de VRAIS évènements (`'close'`,
        // `'finish'`, `'error'`) pour savoir quand la destination a fini ou
        // est morte. Un premier essai avec un objet dont `on()`/`once()` ne
        // faisaient qu'ignorer le rappel laissait `pipeline()` attendre un
        // signal qui ne viendrait jamais — le test restait bloqué jusqu'au
        // délai d'expiration. `writeHead` est simplement ajoutée par-dessus,
        // seule méthode qu'un `Writable` nu n'a pas.
        const rep = new Writable({
            write(_chunk, _enc, cb) {
                cb();
            },
        });
        (rep as unknown as { writeHead: (...a: unknown[]) => unknown }).writeHead = () => rep;

        // Un flux qui émet un premier morceau PUIS échoue — reproduisant une
        // lecture qui casse alors que la réponse est déjà en cours.
        //
        // ⚠️ `envoyee` EST OBLIGATOIRE : sans elle, `_read()` — rappelé en
        // boucle SYNCHRONE tant que le consommateur signale qu'il est prêt —
        // repousserait indéfiniment avant même que le `process.nextTick` de
        // l'erreur n'ait sa chance de courir, remplissant le tampon interne
        // sans borne. Payé une fois pendant l'écriture de ce test même :
        // `JavaScript heap out of memory` sur le worker Vitest.
        let envoyee = false;
        const fluxFautif = new Readable({
            read() {
                if (envoyee) return;
                envoyee = true;
                this.push('debut ');
                process.nextTick(() => this.emit('error', new Error('EMFILE (simulated)')));
            },
        });

        const req = { url: '/assets/index-a1b2c3.js', method: 'GET', headers: {} };
        const result = serveWithStream(
            req as never,
            rep as unknown as ServerResponse,
            { racinePage: racine },
            () => fluxFautif,
        );
        return { result, rep };
    }

    // 🔴 SÉPARÉS (round 3, Neuf 2), même raison que le test MIME plus haut :
    // groupées, `expect(result).toBe(true)` courait en premier et aurait
    // masqué un échec de la seconde assertion si elle avait rougi.
    it('a stream error injected AFTER the headers is handled (route not fallen back to 404)', async () => {
        const { result } = unitCallWithFaultyStream();
        // La route EST prise en charge (les en-têtes sont partis) : rendre
        // `false` ferait tomber la chaîne sur un 404 générique par-dessus une
        // réponse déjà commencée.
        expect(await result).toBe(true);
    });

    it('a stream error injected AFTER the headers destroys the response, rather than leaving it hanging', async () => {
        const { result, rep } = unitCallWithFaultyStream();
        await result;
        expect(rep.destroyed).toBe(true);
    });

    // 🔴 HORS GET/HEAD, LE COMPORTEMENT EST CELUI D'HIER À L'OCTET PRÈS. Rendre
    // 405 ferait qu'un POST sur un chemin mal orthographié — le repli SPA résout
    // n'importe quoi — obtiendrait « méthode » au lieu du 404 qui le désigne.
    it('a POST on an unknown path always returns 404, never 405', async () => {
        base = await baseNeuve('page-post-inconnu');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/aplication/x`, { method: 'POST' });
        expect(r.status).toBe(404);
    });

    it('serves a HEAD without a body', async () => {
        base = await baseNeuve('page-head');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`, { method: 'HEAD' });
        expect(r.status).toBe(200);
        expect(await r.text()).toBe('');
    });
});
