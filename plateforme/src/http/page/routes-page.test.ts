import { afterEach, describe, expect, it } from 'vitest';
import { mkdirSync, mkdtempSync, readdirSync, symlinkSync, writeFileSync } from 'node:fs';
import { request as httpRequest } from 'node:http';
import type { ServerResponse } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable, Writable } from 'node:stream';
import { baseNeuve } from '../../base/harnais';
import type { Pilote } from '../../base/pilote';
import { demarrerServeur, type ServicePlateforme } from '../serveur';
import type { Config } from '../../config';
import { servirAvecFlux } from './routes-page';

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
    writeFileSync(join(racine, 'index.html'), '<!doctype html><title>page</title>');
    writeFileSync(join(racine, 'assets', 'index-a1b2c3.js'), 'export const x = 1;\n');
    // Le PIÈGE que le critère ⑥ éprouve : un fichier homonyme d'une route.
    writeFileSync(join(racine, 'sante'), 'ceci ne doit JAMAIS etre servi');
    writeFileSync(join(racine, 'secret.env'), 'MOT_DE_PASSE=x');
    return racine;
}

/// Une racine qui porte DEUX liens symboliques qui en SORTENT — le trou de la
/// Critique 3 (round 2). `dessus/` vit à CÔTÉ de la racine, jamais dedans.
function racineAvecLiensSymboliques(): string {
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

/// Compte les descripteurs ouverts du PROCESSUS DE TEST — le serveur tourne
/// dans ce même processus (`demarrerServeur` ne fork rien), donc c'est un
/// témoin direct de ce que `servirPage` laisse ouvert.
function comptesFd(): number {
    return readdirSync('/proc/self/fd').length;
}

describe('GET /', () => {
    // 🔴 LE TÉMOIN NÉGATIF. Sans lui, le 200 du test suivant ne prouve pas que
    // PLATEFORME_PAGE a servi à quelque chose.
    it("sans PLATEFORME_PAGE, GET / rend le 404 d'hier, mot pour mot", async () => {
        base = await baseNeuve('page-temoin-negatif');
        service = await demarrerServeur({ ...CONFIG, racinePage: undefined }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/`);
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('introuvable\n');
    });

    // ⚠️ MINEUR PROMU (round 2) : `racinePage: ''` doit se comporter comme
    // l'ABSENCE, jamais comme `resolve('')` (le répertoire courant du
    // processus). `process.chdir` vers un répertoire CONTRÔLÉ qui porte un
    // `index.html` rend la garde discriminante : sans elle, ce test verrait
    // `200` et le corps piégé.
    it("une racine VIDE ('') retire aussi le servant, jamais le répertoire courant", async () => {
        const cwdAvant = process.cwd();
        const piege = mkdtempSync(join(tmpdir(), 'page-cwd-piege-'));
        writeFileSync(join(piege, 'index.html'), 'CECI NE DOIT JAMAIS ETRE SERVI');
        process.chdir(piege);
        try {
            base = await baseNeuve('page-racine-vide');
            service = await demarrerServeur({ ...CONFIG, racinePage: '' }, base);
            const r = await fetch(`http://127.0.0.1:${service.port}/`);
            expect(r.status).toBe(404);
        } finally {
            process.chdir(cwdAvant);
        }
    });

    it('avec PLATEFORME_PAGE, GET / rend 200', async () => {
        base = await baseNeuve('page-index-statut');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/`);
        expect(r.status).toBe(200);
    });

    it('avec PLATEFORME_PAGE, GET / rend le corps EXACT de index.html', async () => {
        base = await baseNeuve('page-index-corps');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/`);
        // Comparer les OCTETS, jamais le seul code 200.
        expect(await r.text()).toBe('<!doctype html><title>page</title>');
    });

    it('le document porte la CSP', async () => {
        base = await baseNeuve('page-index-csp');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/`);
        expect(r.headers.get('content-security-policy')).toContain("default-src 'self'");
    });

    it('le document porte cache-control no-store', async () => {
        base = await baseNeuve('page-index-cache');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/`);
        expect(r.headers.get('cache-control')).toBe('no-store');
    });

    it('un actif porte immutable, JAMAIS no-store', async () => {
        base = await baseNeuve('page-actif');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/assets/index-a1b2c3.js`);
        expect(r.headers.get('cache-control')).toBe('public, max-age=31536000, immutable');
    });

    // 🔴 LA ROUGE DE L'ORDRE DE CHAÎNAGE. Un fichier nommé `sante` déposé dans
    // la racine ne doit JAMAIS supplanter le routeur de santé — la panne la
    // plus discrète possible, le service répondant 200 avec un corps
    // plausible. SÉPARÉE EN DEUX TESTS (round 2, Important 1) : dans la
    // mutation de l'étape 6, `content-type` rougit et arrête le test AVANT
    // que le corps ne soit jamais vérifié — la seconde assertion, groupée,
    // n'était donc jamais éprouvée.
    it('/sante reste servi par SON routeur — content-type json, malgré un fichier homonyme', async () => {
        base = await baseNeuve('page-homonyme-type');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
        expect(r.headers.get('content-type')).toContain('application/json');
    });

    it('/sante reste servi par SON routeur — corps, malgré un fichier homonyme', async () => {
        base = await baseNeuve('page-homonyme-corps');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
        expect(await r.text()).not.toContain('JAMAIS');
    });

    it('refuse un fichier hors de la liste MIME, sans jamais en fuiter le contenu', async () => {
        base = await baseNeuve('page-mime-refuse');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/secret.env`);
        expect(r.status).toBe(404);
        // ⚠️ ANCRÉ SUR LE CORPS (round 2, Important 3) : un `404` seul ne dit
        // pas SI c'est parce que la liste MIME est close, ou parce que rien
        // ne répond — les deux rendent le même statut. Un contenu qui
        // fuirait le secret le distinguerait, lui, à coup sûr.
        expect(await r.text()).not.toContain('MOT_DE_PASSE');
    });

    // ⚠️ IMPORTANT 2 (round 2) : le trou que l'addendum précédent fermait
    // (`resolution.ts`, motif `nom-vide`) n'était éprouvé qu'à la règle PURE,
    // jamais avec un VRAI fichier `.json` nu sur le disque — la seule tâche
    // du lot qui en expose.
    it('refuse un fichier `.json` nu (nom vide), même réellement présent sur le disque', async () => {
        const racine = racineJetable();
        writeFileSync(join(racine, '.json'), '"ne doit jamais etre servi"');
        base = await baseNeuve('page-json-nu-disque');
        service = await demarrerServeur({ ...CONFIG, racinePage: racine }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/.json`);
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
    it('refuse une traversée ENCODÉE qui sort de la racine, via une vraie requête HTTP', async () => {
        base = await baseNeuve('page-traversee-disque');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(
            `http://127.0.0.1:${service.port}/assets/..%2f..%2f..%2f..%2f..%2f..%2fetc%2fpasswd`,
        );
        expect(r.status).toBe(404);
    });

    // 🔴 CRITIQUE 3 (round 2), MESURÉE : un lien FICHIER qui sort de la
    // racine par un chemin relatif rendait `200` avec le contenu volé avant
    // le `realpath` de `routes-page.ts`.
    it('refuse un fichier atteint via un lien symbolique FICHIER qui sort de la racine', async () => {
        base = await baseNeuve('page-lien-fichier');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineAvecLiensSymboliques() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/lien.json`);
        expect(r.status).toBe(404);
    });

    // 🔴 CRITIQUE 3 (round 2), MESURÉE : un lien RÉPERTOIRE qui sort de la
    // racine — aucun `..` n'apparaît jamais dans l'URL qui l'atteint.
    it('refuse un fichier atteint via un lien symbolique RÉPERTOIRE qui sort de la racine', async () => {
        base = await baseNeuve('page-lien-repertoire');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineAvecLiensSymboliques() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/lien-rep/vole.html`);
        expect(r.status).toBe(404);
    });

    // 🔴 CRITIQUE 1 (round 2), MESURÉE : `.pipe()` ne détruit pas la source
    // quand la destination meurt. Un client qui abandonne — ici, un socket
    // détruit dès la réception des en-têtes — laissait le descripteur du
    // fichier ouvert INDÉFINIMENT, de façon MONOTONE.
    it("un téléchargement ABANDONNÉ par le client ne fuit pas de descripteur", async () => {
        const racine = racineJetable();
        // Un fichier assez gros pour laisser le temps d'abandonner AVANT que
        // le flux n'ait fini de couler.
        writeFileSync(join(racine, 'assets', 'gros.js'), 'x'.repeat(8 * 1024 * 1024));
        base = await baseNeuve('page-fd-abandon');
        service = await demarrerServeur({ ...CONFIG, racinePage: racine }, base);

        const avant = comptesFd();
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
        const apres = comptesFd();
        // Une fuite RENDRAIT `apres - avant` proche de N (un fd par abandon,
        // monotone, mesuré). La marge tolère la variation des sockets
        // clientes elles-mêmes, sans jamais s'approcher de N.
        expect(apres - avant).toBeLessThan(N / 2);
    });

    // 🔴 CRITIQUE 2 (round 2), MESURÉE : une lecture qui casse APRÈS que les
    // en-têtes sont partis n'a AUCUN gestionnaire d'erreur sous `.pipe()` —
    // un `EventEmitter` qui émet `error` sans écouteur LÈVE, et la revue a
    // reproduit le service ENTIER mourant (`EMFILE`). Le flux est ICI
    // INJECTÉ pour reproduire une erreur de lecture MI-FLUX SANS provoquer
    // un vrai épuisement de descripteurs — ce qui ferait courir le risque
    // exact que ce test existe pour éprouver, sur le processus de test
    // lui-même.
    it('une erreur de lecture APRÈS les en-têtes ne fait PAS mourir le service, et détruit la réponse', async () => {
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
        }) as unknown as ServerResponse;
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
                process.nextTick(() => this.emit('error', new Error('EMFILE (simulé)')));
            },
        });

        const req = { url: '/assets/index-a1b2c3.js', method: 'GET', headers: {} };
        const resultat = await servirAvecFlux(req as never, rep, { racinePage: racine }, () => fluxFautif);

        // La route EST prise en charge (les en-têtes sont partis) : rendre
        // `false` ferait tomber la chaîne sur un 404 générique par-dessus une
        // réponse déjà commencée.
        expect(resultat).toBe(true);
        expect((rep as unknown as Writable).destroyed).toBe(true);
    });

    // 🔴 HORS GET/HEAD, LE COMPORTEMENT EST CELUI D'HIER À L'OCTET PRÈS. Rendre
    // 405 ferait qu'un POST sur un chemin mal orthographié — le repli SPA résout
    // n'importe quoi — obtiendrait « méthode » au lieu du 404 qui le désigne.
    it('un POST sur un chemin inconnu rend toujours 404, jamais 405', async () => {
        base = await baseNeuve('page-post-inconnu');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/aplication/x`, { method: 'POST' });
        expect(r.status).toBe(404);
    });

    it('sert un HEAD sans corps', async () => {
        base = await baseNeuve('page-head');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/`, { method: 'HEAD' });
        expect(r.status).toBe(200);
        expect(await r.text()).toBe('');
    });
});
