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
        });
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
});
