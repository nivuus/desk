import { describe, expect, it } from 'vitest';
import { lireConfig } from './config';

/// 42 caractères : au-dessus de `LONGUEUR_SECRET_MIN`, et jamais `''` — un
/// secret de test explicite, comme l'exige la tâche 3.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

/// Le montage minimal — réutilisé dans plusieurs `describe`.
/// ⚠️ Le positionnement ici, avant tous les tests, rend `BASE` disponible
/// partout sans redondance.
const BASE = { PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: SECRET };

describe('lireConfig', () => {
    it("refuse de démarrer sans PLATEFORME_HOTE — il n'y a pas de défaut", () => {
        // Le défaut DOIT être l'absence de défaut (spec §4, critère ④).
        // Poser '0.0.0.0' par défaut ferait passer un test d'écoute sans rien
        // garantir : c'est exactement la panne muette que ce dépôt combat.
        expect(() => lireConfig({})).toThrow(/PLATEFORME_HOTE/);
    });

    it("n'invente pas d'adresse quand la variable est vide", () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '' })).toThrow(/PLATEFORME_HOTE/);
    });

    it('lit les champs, avec leurs défauts non permissifs', () => {
        // `PLATEFORME_SECRET_JETON` est fourni parce qu'il n'a AUCUN défaut :
        // c'est le sujet des trois tests suivants.
        const c = lireConfig({ PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: SECRET });
        expect(c).toEqual({
            hote: '127.0.0.1',
            port: 8080,
            base: 'sqlite',
            urlBase: ':memory:',
            secretJeton: SECRET,
            // Absente, donc le défaut : voir la description ci-dessous. Ce
            // `toEqual` compare l'objet ENTIER — un champ ajouté à `Config`
            // sans être ajouté ici le rendrait rouge, et c'est voulu.
            auth: 'pomerium',
            // Absente, donc `undefined` : aucun en-tête CORS ne sera émis, et
            // le refus est le défaut.
            origineClient: undefined,
            // Absente, donc ensemble VIDE : on ne croit l'en-tête
            // `X-Forwarded-For` d'aucune source. Voir les quatre tests dédiés
            // en fin de fichier.
            proxyDeConfiance: new Set(),
            // Absente, donc le défaut. ⚠️ L'ASYMÉTRIE AVEC `PLATEFORME_HOTE`
            // est assumée : un mauvais répertoire coûte un retéléversement
            // borné et automatique, là où une mauvaise adresse d'écoute
            // exposerait le service.
            repertoireIcones: 'donnees/icones',
            // Absente, donc le défaut — même asymétrie assumée, et une
            // conséquence PLUS lourde : un magasin d'icônes perdu se
            // reconstruit tout seul, un téléversement perdu exige qu'un humain
            // redépose. Voir `config.ts`.
            repertoireTeleversements: 'donnees/televersements',
            // Absente, donc `undefined` : la plateforme ne sert aucun fichier
            // statique, et le comportement est celui d'avant le lot à l'octet
            // près. C'est ce qui rend l'ajout strictement additif et sûr.
            racinePage: undefined,
        });
    });

    it('retient le répertoire d’icônes qu’on lui NOMME', () => {
        // 🔴 LA ROUGE : la variable posée et IGNORÉE. Le magasin se
        // reconstruirait ailleurs, en silence, en retéléversant tout.
        expect(lireConfig({ ...BASE, PLATEFORME_ICONES: '/var/lib/guac/ic' }).repertoireIcones)
            .toBe('/var/lib/guac/ic');
    });

    it('🔴 un PLATEFORME_ICONES VIDE retombe sur le défaut, pas sur le répertoire courant', () => {
        // `env.X ?? 'defaut'` ne rattrape PAS la chaîne vide — P1 a payé cette
        // erreur exacte, où un des deux rouges annoncés était en réalité vert.
        expect(lireConfig({ ...BASE, PLATEFORME_ICONES: '' }).repertoireIcones)
            .toBe('donnees/icones');
    });

    it('refuse un PLATEFORME_BASE inconnu, plutôt que de retomber sur sqlite', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_SECRET_JETON: SECRET, PLATEFORME_BASE: 'mysql' }))
            .toThrow(/PLATEFORME_BASE/);
    });

    it('refuse un port qui n’est pas un entier', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_SECRET_JETON: SECRET, PLATEFORME_PORT: 'huit-mille' }))
            .toThrow(/PLATEFORME_PORT/);
    });

    it("refuse de démarrer sans PLATEFORME_SECRET_JETON — il n'y a pas de défaut", () => {
        // 🔴 Le défaut DOIT être l'absence de défaut. Un secret tiré au hasard
        // au démarrage passerait ce test ET invaliderait tous les jetons à
        // chaque redémarrage, sans que rien ne le dise.
        expect(() => lireConfig({ PLATEFORME_HOTE: '127.0.0.1' })).toThrow(/PLATEFORME_SECRET_JETON/);
    });

    it("n'invente pas de secret quand la variable est vide", () => {
        // ⚠️ P1 a payé exactement cette erreur : `env.X ?? 'defaut'` ne
        // rattrape pas la chaîne vide, et le test annoncé rouge était vert.
        expect(() => lireConfig({ PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: '' }))
            .toThrow(/PLATEFORME_SECRET_JETON/);
    });

    it('refuse un secret trop court, plutôt que de signer avec', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: 'trop-court' }))
            .toThrow(/32/);
    });

    it("lit PLATEFORME_ORIGINE_CLIENT, qui est FACULTATIVE et ne lève jamais", () => {
        // Elle est facultative là où PLATEFORME_HOTE ne l'est pas, et
        // l'asymétrie tient aux conséquences : une origine absente produit un
        // refus BRUYANT du navigateur, qu'un opérateur voit ; une adresse
        // d'écoute absente produirait une écoute universelle SILENCIEUSE.
        // Refuser de démarrer pour elle casserait le déploiement de P5, où le
        // proxy inverse met les deux sur la même origine.
        const sans = lireConfig({ PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: SECRET });
        expect(sans.origineClient).toBeUndefined();
        const avec = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_ORIGINE_CLIENT: 'http://127.0.0.1:5173',
        });
        expect(avec.origineClient).toBe('http://127.0.0.1:5173');
        // Vide vaut absente, jamais la chaîne vide : un `Origin: ` vide ne
        // correspondrait à aucune origine réelle.
        const vide = lireConfig({
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_SECRET_JETON: SECRET,
            PLATEFORME_ORIGINE_CLIENT: '',
        });
        expect(vide.origineClient).toBeUndefined();
    });

    /// ⚠️ AUCUN DE CES TESTS NE LIT `process.env`, ET C'EST LA PROPRIÉTÉ DU
    /// MODULE, PAS UNE PRÉCAUTION DU TEST : `lireConfig` reçoit son
    /// environnement en PARAMÈTRE, et son en-tête l'écrit en toutes lettres —
    /// « la lecture d'environnement se fait ICI et nulle part ailleurs ». Il
    /// n'y a donc RIEN à poser ni à restaurer, contrairement à
    /// `signaling/turn-harnais.ts`, dont le harnais existe précisément parce
    /// que `relais.ts` lit `process.env` en douce. Ajouter une variable à
    /// `Config` n'a pas rendu un seul test dépendant du shell qui le lance.

    it("(a) PLATEFORME_PROXY_DE_CONFIANCE absente ⇒ on ne croit PERSONNE", () => {
        // 🔴 Le défaut est de ne rien croire, jamais de tout croire. Un défaut
        // permissif ici rendrait l'adresse du client FORGEABLE par le client
        // lui-même, donc le frein par adresse contournable en une ligne
        // d'en-tête.
        expect(lireConfig(BASE).proxyDeConfiance.size).toBe(0);
    });

    it("(b) chaîne VIDE ⇒ ensemble vide, et non une entrée vide", () => {
        // ⚠️ `env.X ?? 'defaut'` ne rattrape pas `''` — P1 a payé cette erreur
        // exacte à sa tâche 1, où un des deux rouges annoncés était vert.
        expect(lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '' }).proxyDeConfiance.size)
            .toBe(0);
    });

    it("(c) une liste séparée par des virgules, espaces RETIRÉES", () => {
        const c = lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5, 10.0.0.1' });
        expect(c.proxyDeConfiance.size).toBe(2);
        // Sans le `trim`, la seconde entrée serait ` 10.0.0.1` et ne
        // correspondrait JAMAIS à une adresse de pair — la confiance
        // échouerait en silence, et le frein par adresse dégénérerait en
        // frein global sans qu'aucune ligne ne le dise.
        expect(c.proxyDeConfiance.has('172.18.0.5')).toBe(true);
        expect(c.proxyDeConfiance.has('10.0.0.1')).toBe(true);
    });

    it("(d) une entrée vide entre deux virgules est IGNORÉE", () => {
        const c = lireConfig({ ...BASE, PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5,,10.0.0.1' });
        expect(c.proxyDeConfiance.size).toBe(2);
        // Une entrée vide dans l'ensemble de confiance rendrait de confiance
        // tout pair dont l'adresse est vide — c'est-à-dire `ADRESSE_INCONNUE`
        // s'il venait à valoir `''`.
        expect(c.proxyDeConfiance.has('')).toBe(false);
    });
});

describe('PLATEFORME_PAGE', () => {
    // 🔴 AUCUN DÉFAUT, à la différence de PLATEFORME_ICONES : un défaut comme
    // `client/dist` ferait servir un répertoire au hasard du répertoire
    // courant, et ferait passer le montage nginx — où la plateforme ne doit
    // RIEN servir — d'un 404 franc à un 200 sur des fichiers non voulus.
    it("est ABSENTE par défaut, et le service ne sert alors aucun fichier", () => {
        expect(lireConfig(BASE).racinePage).toBeUndefined();
    });

    // ⚠️ Le test de la chaîne VIDE est DISTINCT de celui de l'absence :
    // `env.X ?? 'defaut'` ne rattrape pas `''`. P1 a payé cette erreur exacte.
    it('traite la chaîne VIDE comme une absence', () => {
        expect(lireConfig({ ...BASE, PLATEFORME_PAGE: '' }).racinePage).toBeUndefined();
    });

    it('retient le chemin posé', () => {
        expect(lireConfig({ ...BASE, PLATEFORME_PAGE: '/srv/page' }).racinePage).toBe(
            '/srv/page',
        );
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

    it('vaut pomerium par défaut', () => {
        expect(lireConfig({ ...base }).auth).toBe('pomerium');
    });

    it('accepte motdepasse', () => {
        expect(lireConfig({ ...base, PLATEFORME_AUTH: 'motdepasse' }).auth).toBe('motdepasse');
    });

    it('retombe sur le défaut quand la valeur est VIDE', () => {
        expect(lireConfig({ ...base, PLATEFORME_AUTH: '' }).auth).toBe('pomerium');
    });

    // 🔴 LA ROUGE DU CRITÈRE ④ : une coquille de casse ne doit PAS retomber
    // sur le défaut, sinon le mode mot de passe tournerait sous le nom du
    // mode Pomerium — et le second sens est une OUVERTURE.
    it('LÈVE sur une valeur inconnue, jamais un repli', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_AUTH: 'Pomerium' })).toThrow(
            /PLATEFORME_AUTH/,
        );
    });
});

describe("la garde d'écoute du mode pomerium", () => {
    // Même écart assumé que ci-dessus : `SECRET` plutôt que le littéral du
    // brief, pour ne pas déclencher `securite/secrets.test.ts`.
    const base = { PLATEFORME_SECRET_JETON: SECRET };

    // 🔴 LA ROUGE DU CRITÈRE ⑤, et elle décrit le montage RÉEL du
    // 21 août 2026 : le service tourne aujourd'hui sur 0.0.0.0:8080.
    it('REFUSE 0.0.0.0 en mode pomerium', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '0.0.0.0' })).toThrow(
            /écoute universelle/,
        );
    });

    it('REFUSE :: en mode pomerium', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '::' })).toThrow(
            /écoute universelle/,
        );
    });

    // ⚠️ ARBITRAGE : le brief propose `ECOUTES_UNIVERSELLES` avec QUATRE
    // membres (`0.0.0.0`, `::`, `[::]`, `*`) mais ne fait tester que les deux
    // ci-dessus. Un membre non éprouvé est exactement « un contrôle qu'on n'a
    // jamais vu rouge » (§ méthode de mesure, CLAUDE.md) : les deux tests
    // suivants ferment ce trou plutôt que de retirer les membres du set.
    it('REFUSE [::] en mode pomerium', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '[::]' })).toThrow(
            /écoute universelle/,
        );
    });

    it('REFUSE * en mode pomerium', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '*' })).toThrow(
            /écoute universelle/,
        );
    });

    // La garde est liée au MODE, pas universelle : sans ce test, on ne saurait
    // pas si elle refuse 0.0.0.0 ou si elle refuse toujours.
    it('LAISSE PASSER 0.0.0.0 en mode motdepasse', () => {
        const c = lireConfig({ ...base, PLATEFORME_HOTE: '0.0.0.0', PLATEFORME_AUTH: 'motdepasse' });
        expect(c.hote).toBe('0.0.0.0');
    });

    // Le déploiement Docker pose un NOM DE SERVICE, pas une adresse : la garde
    // doit le laisser passer, sinon elle casse le montage le plus sûr des trois.
    it('LAISSE PASSER un nom de service de réseau interne', () => {
        const c = lireConfig({ ...base, PLATEFORME_HOTE: 'plateforme' });
        expect(c.hote).toBe('plateforme');
    });

    // Le montage retenu par la spec § 7.1.
    it("LAISSE PASSER l'adresse du pont libvirt", () => {
        expect(lireConfig({ ...base, PLATEFORME_HOTE: '192.168.3.1' }).hote).toBe('192.168.3.1');
    });
});
