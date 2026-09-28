import { describe, expect, it } from 'vitest';
import { lireConfig } from './config';

/// 42 caractères : au-dessus de `MIN_SECRET_LENGTH`, et jamais `''` — un
/// secret de test explicite, comme l'exige la tâche 3.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

/// Le montage minimal — réutilisé dans plusieurs `describe`.
/// ⚠️ Le positionnement ici, avant tous les tests, rend `BASE` disponible
/// partout sans redondance. AUCUN DE CES TESTS NE LIT `process.env` : `lireConfig`
/// reçoit son environnement en PARAMÈTRE (voir `config.ts`), donc rien à poser ni
/// restaurer. Ajouter une variable à `Config` n'a rendu aucun test dépendant du
/// shell qui le lance.
const BASE = { PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: SECRET };

describe('lireConfig', () => {
    it("refuses to start without PLATEFORME_HOTE — there is no default", () => {
        // Le défaut DOIT être l'absence de défaut (spec §4, critère ④).
        // Poser '0.0.0.0' par défaut ferait passer un test d'écoute sans rien
        // garantir : c'est exactement la panne muette que ce dépôt combat.
        expect(() => lireConfig({})).toThrow(/PLATEFORME_HOTE/);
    });

    it("does not invent an address when the variable is empty", () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '' })).toThrow(/PLATEFORME_HOTE/);
    });

    it('reads the fields, with their non-permissive defaults', () => {
        // `PLATEFORME_SECRET_JETON` est fourni parce qu'il n'a AUCUN défaut :
        // c'est le sujet des trois tests suivants.
        //
        // 🔴 `PLATEFORME_PROXY_DE_CONFIANCE` EST FOURNIE, ET C'EST DEVENU
        // OBLIGATOIRE (tâche 6, garde du refus de démarrer) : sans elle, le
        // mode `pomerium` par défaut de ce test lèverait avant même
        // d'atteindre l'assertion. Le sujet de CE test n'est pas cette garde
        // — elle a son propre `describe` plus bas —, donc on la satisfait
        // sans la questionner.
        const c = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        // 🔴 PASSER À toStrictEqual, PAS toEqual : `toEqual` ignore les
        // propriétés `undefined`, donc un champ facultatif ajouté à `Config`
        // et retourné comme `undefined` ne causerait PAS de rouge. `toStrictEqual`
        // exige que les deux objets aient exactement les mêmes clés — c'est le
        // seul garde qui tienne contre la divergence. Preuve :
        // `expect({a:1, u: undefined}).toEqual({a:1})` PASSE,
        // `toStrictEqual` échoue.
        expect(c).toStrictEqual({
            hote: '127.0.0.1',
            port: 8080,
            base: 'sqlite',
            urlBase: ':memory:',
            secretJeton: SECRET,
            auth: 'pomerium',
            origineClient: undefined,
            proxyDeConfiance: new Set(['172.18.0.5']),
            repertoireIcones: 'donnees/icones',
            repertoireTeleversements: 'donnees/televersements',
            racinePage: undefined,
        });
    });

    it('keeps the icon directory it is GIVEN', () => {
        // 🔴 LA ROUGE : la variable posée et IGNORÉE. Le magasin se
        // reconstruirait ailleurs, en silence, en retéléversant tout.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` est posée pour satisfaire la garde
        // du refus de démarrer (tâche 6) — ce n'est pas le sujet de ce test.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_ICONES: '/var/lib/guac/ic',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).repertoireIcones,
        ).toBe('/var/lib/guac/ic');
    });

    it('🔴 an EMPTY PLATEFORME_ICONES falls back to the default, not to the current directory', () => {
        // `env.X ?? 'defaut'` ne rattrape PAS la chaîne vide — P1 a payé cette
        // erreur exacte, où un des deux rouges annoncés était en réalité vert.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` est posée pour satisfaire la garde
        // du refus de démarrer (tâche 6) — ce n'est pas le sujet de ce test.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_ICONES: '',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).repertoireIcones,
        ).toBe('donnees/icones');
    });

    it('refuses an unknown PLATEFORME_BASE, rather than falling back to sqlite', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_SECRET_JETON: SECRET, PLATEFORME_BASE: 'mysql' }))
            .toThrow(/PLATEFORME_BASE/);
    });

    it('refuses a port that is not an integer', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_SECRET_JETON: SECRET, PLATEFORME_PORT: 'huit-mille' }))
            .toThrow(/PLATEFORME_PORT/);
    });

    it("refuses to start without PLATEFORME_SECRET_JETON — there is no default", () => {
        // 🔴 Le défaut DOIT être l'absence de défaut. Un secret tiré au hasard
        // au démarrage passerait ce test ET invaliderait tous les jetons à
        // chaque redémarrage, sans que rien ne le dise.
        expect(() => lireConfig({ PLATEFORME_HOTE: '127.0.0.1' })).toThrow(/PLATEFORME_SECRET_JETON/);
    });

    it("does not invent a secret when the variable is empty", () => {
        // ⚠️ P1 a payé exactement cette erreur : `env.X ?? 'defaut'` ne
        // rattrape pas la chaîne vide, et le test annoncé rouge était vert.
        expect(() => lireConfig({ PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: '' }))
            .toThrow(/PLATEFORME_SECRET_JETON/);
    });

    it('refuses a secret that is too short, rather than signing with it', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: 'trop-court' }))
            .toThrow(/32/);
    });

    it("reads PLATEFORME_ORIGINE_CLIENT, which is OPTIONAL and never throws", () => {
        // Elle est facultative là où PLATEFORME_HOTE ne l'est pas, et
        // l'asymétrie tient aux conséquences : une origine absente produit un
        // refus BRUYANT du navigateur, qu'un opérateur voit ; une adresse
        // d'écoute absente produirait une écoute universelle SILENCIEUSE.
        // Refuser de démarrer pour elle casserait le déploiement de P5, où le
        // proxy inverse met les deux sur la même origine.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` est posée sur les trois appels pour
        // satisfaire la garde du refus de démarrer (tâche 6) — ce n'est pas
        // le sujet de ce test.
        const sans = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        expect(sans.origineClient).toBeUndefined();
        const withIt = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_ORIGINE_CLIENT: 'http://127.0.0.1:5173',
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        expect(withIt.origineClient).toBe('http://127.0.0.1:5173');
        // Vide vaut absente, jamais la chaîne vide : un `Origin: ` vide ne
        // correspondrait à aucune origine réelle.
        const vide = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_ORIGINE_CLIENT: '',
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        expect(vide.origineClient).toBeUndefined();
    });

    it("(a) PLATEFORME_PROXY_DE_CONFIANCE absent ⇒ we trust NOBODY", () => {
        // 🔴 Le défaut est de ne rien croire, jamais de tout croire. Un défaut
        // permissif ici rendrait l'adresse du client FORGEABLE par le client
        // lui-même, donc le frein par adresse contournable en une ligne
        // d'en-tête.
        //
        // 🔴 `PLATEFORME_AUTH: 'motdepasse'` EST POSÉE, ET C'EST LE POINT :
        // depuis la garde du refus de démarrer (tâche 6), l'ensemble VIDE
        // n'est atteignable qu'en mode `motdepasse` — en `pomerium`, ce même
        // montage LÈVE désormais (voir le describe dédié). C'est précisément
        // ce que la garde signifie : un ensemble de confiance vide n'est plus
        // un état qu'on documente en `pomerium`, il est refusé au démarrage.
        expect(lireConfig({ ...BASE, PLATEFORME_AUTH: 'motdepasse' }).proxyDeConfiance.size).toBe(0);
    });

    it("(b) EMPTY string ⇒ empty set, and not an empty entry", () => {
        // ⚠️ `env.X ?? 'defaut'` ne rattrape pas `''` — P1 a payé cette erreur
        // exacte à sa tâche 1, où un des deux rouges annoncés était vert.
        //
        // 🔴 `PLATEFORME_AUTH: 'motdepasse'` EST POSÉE — même raison que (a) :
        // depuis la garde du refus de démarrer (tâche 6), l'ensemble VIDE
        // n'est atteignable qu'en mode `motdepasse`.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_AUTH: 'motdepasse',
                PLATEFORME_PROXY_DE_CONFIANCE: '',
            }).proxyDeConfiance.size,
        ).toBe(0);
    });

    it("(c) a comma-separated list, spaces REMOVED", () => {
        const c = lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5, 10.0.0.1' });
        expect(c.proxyDeConfiance.size).toBe(2);
        // Sans le `trim`, la seconde entrée serait ` 10.0.0.1` et ne
        // correspondrait JAMAIS à une adresse de pair — la confiance
        // échouerait en silence, et le frein par adresse dégénérerait en
        // frein global sans qu'aucune ligne ne le dise.
        expect(c.proxyDeConfiance.has('172.18.0.5')).toBe(true);
        expect(c.proxyDeConfiance.has('10.0.0.1')).toBe(true);
    });

    it("(d) an empty entry between two commas is IGNORED", () => {
        const c = lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5,,10.0.0.1' });
        expect(c.proxyDeConfiance.size).toBe(2);
        // Une entrée vide dans l'ensemble de confiance rendrait de confiance
        // tout pair dont l'adresse est vide — c'est-à-dire `ADRESSE_INCONNUE`
        // s'il venait à valoir `''`.
        expect(c.proxyDeConfiance.has('')).toBe(false);
    });
});

describe('the refusal to start in pomerium mode without a trusted proxy', () => {
    // 🔴 UN REFUS DE DÉMARRER SE LIT AVANT D'AGIR. Un 401 silencieux pour
    // tout le monde se lirait APRÈS, sur un service qui répond, écoute et
    // sert les dix autres routeurs — la panne la plus discrète possible.
    it('THROWS in pomerium mode without PLATEFORME_PROXY_DE_CONFIANCE', () => {
        expect(() => lireConfig({ ...BASE, PLATEFORME_AUTH: 'pomerium' })).toThrow(
            /PLATEFORME_PROXY_DE_CONFIANCE/,
        );
    });

    // ⚠️ LA GARDE EST LIÉE AU MODE, comme celle de PLATEFORME_HOTE : en
    // `motdepasse`, le service s'authentifie lui-même et l'en-tête n'est lu
    // par personne.
    it('does NOT throw in motdepasse mode', () => {
        expect(() => lireConfig({ ...BASE, PLATEFORME_AUTH: 'motdepasse' })).not.toThrow();
    });
});

describe('PLATEFORME_PAGE', () => {
    // 🔴 AUCUN DÉFAUT, à la différence de PLATEFORME_ICONES : un défaut comme
    // `client/dist` ferait servir un répertoire au hasard du répertoire
    // courant, et ferait passer le montage nginx — où la plateforme ne doit
    // RIEN servir — d'un 404 franc à un 200 sur des fichiers non voulus.
    // `PLATEFORME_PROXY_DE_CONFIANCE` est posée sur les quatre tests de ce
    // bloc pour satisfaire la garde du refus de démarrer (tâche 6) — ce
    // n'est pas leur sujet, qui reste `racinePage`.
    it("is ABSENT by default, and the service then serves no file", () => {
        expect(
            lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5' }).racinePage,
        ).toBeUndefined();
    });

    // ⚠️ Le test de la chaîne VIDE est DISTINCT de celui de l'absence :
    // `env.X ?? 'defaut'` ne rattrape pas `''`. P1 a payé cette erreur exacte.
    it('treats the EMPTY string as an absence', () => {
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_PAGE: '',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).racinePage,
        ).toBeUndefined();
    });

    it('keeps the path that was set', () => {
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_PAGE: '/srv/page',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).racinePage,
        ).toBe('/srv/page');
    });

    it('includes the racinePage field in the object, even when absent', () => {
        // 🔴 CE TEST FERME LE TROU : retirer `racinePage,` de l'objet que
        // lireConfig rend fait échouer ce test, alors que les trois tests
        // ci-dessus passent (puisqu'on peut lire `.racinePage` sur undefined).
        // C'est le seul qui détecte la perte pure et simple du champ.
        expect(
            Object.hasOwn(
                lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5' }),
                'racinePage',
            ),
        ).toBe(true);
    });
});

describe('PLATEFORME_AUTH', () => {
    /// Le montage minimal — copié du haut de ce fichier, jamais réinventé.
    ///
    /// ⚠️ ÉCART ASSUMÉ AVEC LE BRIEF : celui-ci portait `'x'.repeat(32)` en
    /// littéral, ce que `securite/secrets.test.ts` dénonce — il balaie toute
    /// AFFECTATION LITTÉRALE de `PLATEFORME_SECRET_JETON` dans un fichier
    /// versionné, et une chaîne citée en est une, peu importe qu'elle soit
    /// triviale. `SECRET`, la constante déjà déclarée en tête de ce fichier,
    /// est un IDENTIFIANT — la même convention que `BASE` juste plus bas,
    /// exemptée nommément par `inoffensive()`.
    const base = {
        PLATEFORME_HOTE: '127.0.0.1',
        PLATEFORME_SECRET_JETON: SECRET,
    };

    it('defaults to pomerium', () => {
        // `PLATEFORME_PROXY_DE_CONFIANCE` est posée pour satisfaire la garde
        // du refus de démarrer (tâche 6) — le sujet de ce test est le mode
        // `pomerium` lui-même, pas cette garde.
        expect(lireConfig({ ...base, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5' }).auth).toBe(
            'pomerium',
        );
    });

    it('accepts motdepasse', () => {
        expect(lireConfig({ ...base, PLATEFORME_AUTH: 'motdepasse' }).auth).toBe('motdepasse');
    });

    it('falls back to the default when the value is EMPTY', () => {
        // `PLATEFORME_PROXY_DE_CONFIANCE` est posée pour satisfaire la garde
        // du refus de démarrer (tâche 6) : une valeur VIDE retombe sur
        // `pomerium`, qui exige la variable.
        expect(
            lireConfig({
                ...base,
                PLATEFORME_AUTH: '',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).auth,
        ).toBe('pomerium');
    });

    // 🔴 LA ROUGE DU CRITÈRE ④ : une coquille de casse ne doit PAS retomber
    // sur le défaut, sinon le mode mot de passe tournerait sous le nom du
    // mode Pomerium — et le second sens est une OUVERTURE.
    it('THROWS on an unknown value, never a fallback', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_AUTH: 'Pomerium' })).toThrow(
            /PLATEFORME_AUTH/,
        );
    });
});

describe("the listen guard of pomerium mode", () => {
    // Même écart assumé que ci-dessus : `SECRET` plutôt que le littéral du
    // brief, pour ne pas déclencher `securite/secrets.test.ts`.
    const base = { PLATEFORME_SECRET_JETON: SECRET };

    // 🔴 LA ROUGE DU CRITÈRE ⑤, et elle décrit le montage RÉEL du
    // 21 août 2026 : le service tourne aujourd'hui sur 0.0.0.0:8080.
    it('REFUSES 0.0.0.0 in pomerium mode', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '0.0.0.0' })).toThrow(
            /universal listen/,
        );
    });

    it('REFUSES :: in pomerium mode', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '::' })).toThrow(
            /universal listen/,
        );
    });

    // ⚠️ ARBITRAGE : le brief propose `ECOUTES_UNIVERSELLES` avec QUATRE
    // membres (`0.0.0.0`, `::`, `[::]`, `*`) mais ne fait tester que les deux
    // ci-dessus. Un membre non éprouvé est exactement « un contrôle qu'on n'a
    // jamais vu rouge » (§ méthode de mesure, CLAUDE.md) : les deux tests
    // suivants ferment ce trou plutôt que de retirer les membres du set.
    it('REFUSES [::] in pomerium mode', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '[::]' })).toThrow(
            /universal listen/,
        );
    });

    it('REFUSES * in pomerium mode', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '*' })).toThrow(
            /universal listen/,
        );
    });

    // La garde est liée au MODE, pas universelle : sans ce test, on ne saurait
    // pas si elle refuse 0.0.0.0 ou si elle refuse toujours.
    it('LETS THROUGH 0.0.0.0 in motdepasse mode', () => {
        const c = lireConfig({ ...base, PLATEFORME_HOTE: '0.0.0.0', PLATEFORME_AUTH: 'motdepasse' });
        expect(c.hote).toBe('0.0.0.0');
    });

    // Le déploiement Docker pose un NOM DE SERVICE, pas une adresse : la garde
    // doit le laisser passer, sinon elle casse le montage le plus sûr des trois.
    //
    // `PLATEFORME_PROXY_DE_CONFIANCE` est posée sur ces deux tests pour
    // satisfaire la garde du refus de démarrer (tâche 6) — le mode reste
    // `pomerium` par défaut, c'est le sujet de ce describe.
    it('LETS THROUGH an internal network service name', () => {
        const c = lireConfig({
            ...base,
            PLATEFORME_HOTE: 'plateforme',
            PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
        });
        expect(c.hote).toBe('plateforme');
    });

    // Le montage retenu par la spec § 7.1.
    it("LETS THROUGH the address of the libvirt bridge", () => {
        expect(
            lireConfig({
                ...base,
                PLATEFORME_HOTE: '192.168.3.1',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).hote,
        ).toBe('192.168.3.1');
    });
});
