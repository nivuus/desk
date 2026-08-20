import { describe, expect, it } from 'vitest';
import { lireConfig } from './config';

/// 42 caractères : au-dessus de `LONGUEUR_SECRET_MIN`, et jamais `''` — un
/// secret de test explicite, comme l'exige la tâche 3.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

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
    const BASE = { PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_SECRET_JETON: SECRET };

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
