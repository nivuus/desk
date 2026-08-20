// 🔴 CE FICHIER EXISTE POUR UNE PANNE QU'AUCUN TEST NODE NE PEUT VOIR.
//
// Une page servie en `https://` par le proxy qui ouvrirait `ws://…:8080` est du
// CONTENU MIXTE : le navigateur refuse la connexion, en silence pour tout ce
// qui n'est pas la console. Le déploiement de P5 serait donc livré non
// fonctionnel ET VERT. Ce module rend la chose décidable en Node en faisant de
// `location` un PARAMÈTRE — patron de `resize.ts` et `prefixe.ts`.

import { describe, expect, it } from 'vitest';
import { adressePlateforme, adresseSignaling, type Emplacement } from './adresse-plateforme';

/// Une page servie par le proxy, en TLS, sur le port par défaut.
const TLS: Emplacement = { protocol: 'https:', host: 'plateforme.exemple.fr' };
/// La même en clair — l'amorce d'un déploiement, ou un essai local.
const CLAIR: Emplacement = { protocol: 'http:', host: 'plateforme.exemple.fr' };
/// Le serveur de développement de vite.
const ESSAI: Emplacement = { protocol: 'http:', host: 'localhost:5173' };

describe('adressePlateforme et adresseSignaling', () => {
    it("(a) une page en https: donne https: et wss:", () => {
        expect(adressePlateforme(TLS)).toBe('https://plateforme.exemple.fr');
        expect(adresseSignaling(TLS)).toBe('wss://plateforme.exemple.fr');
    });

    it("(b) une page en http: donne http: et ws:", () => {
        expect(adressePlateforme(CLAIR)).toBe('http://plateforme.exemple.fr');
        expect(adresseSignaling(CLAIR)).toBe('ws://plateforme.exemple.fr');
    });

    it("🔴 (c) le paramètre EXPLICITE l'emporte, sur les deux", () => {
        // 🔴 LA ROUGE : dériver toujours de la page. Le mode d'essai local
        // disparaîtrait, et P3 a déjà payé la disparition d'un mode d'essai.
        expect(adressePlateforme(TLS, 'http://192.168.3.2:8080'))
            .toBe('http://192.168.3.2:8080');
        expect(adresseSignaling(TLS, 'ws://192.168.3.2:8080'))
            .toBe('ws://192.168.3.2:8080');
    });

    it("🔴 (d) AUCUN :8080 n'apparaît quand la page est sur le port par défaut", () => {
        // 🔴 C'EST TOUT L'OBJET DE CE MODULE, et la rouge est le code d'avant :
        // `ws://${location.hostname}:8080`. Sous TLS c'est du contenu mixte, et
        // le navigateur refuse — sans qu'aucun test Node ne puisse le voir.
        expect(adressePlateforme(TLS)).not.toContain(':8080');
        expect(adresseSignaling(TLS)).not.toContain(':8080');
        expect(adressePlateforme(CLAIR)).not.toContain(':8080');
        expect(adresseSignaling(CLAIR)).not.toContain(':8080');
    });

    it("🔴 (d bis) sous TLS, JAMAIS de ws: ni de http: — c'est le contenu mixte", () => {
        // 🔴 LA ROUGE la plus directe : garder `ws://` en dur. Cette assertion
        // tombe, et elle est SÉPARÉE de (a) parce qu'`expect` interrompt un
        // test à la première assertion fausse — la leçon que P2 a payée.
        expect(adresseSignaling(TLS).startsWith('wss://')).toBe(true);
        expect(adressePlateforme(TLS).startsWith('https://')).toBe(true);
    });

    it("(e) le PORT de la page est conservé, pas remplacé par 8080", () => {
        // Le serveur de développement de vite sert sur 5173 : l'adresse rendue
        // est celle de la page, port compris.
        expect(adressePlateforme(ESSAI)).toBe('http://localhost:5173');
        expect(adresseSignaling(ESSAI)).toBe('ws://localhost:5173');
    });

    it("(f) un port NON standard sous TLS est conservé lui aussi", () => {
        const p: Emplacement = { protocol: 'https:', host: 'exemple.fr:8443' };
        expect(adressePlateforme(p)).toBe('https://exemple.fr:8443');
        expect(adresseSignaling(p)).toBe('wss://exemple.fr:8443');
    });

    it("(g) un paramètre VIDE ou absent ne l'emporte pas", () => {
        // Une chaîne vide est ce que rend `URLSearchParams.get` sur `?x=` : la
        // traiter comme explicite produirait une adresse vide, donc une panne
        // sans message. `null` est ce qu'il rend sur un paramètre absent.
        expect(adresseSignaling(TLS, '')).toBe('wss://plateforme.exemple.fr');
        expect(adresseSignaling(TLS, null)).toBe('wss://plateforme.exemple.fr');
        expect(adressePlateforme(TLS, '')).toBe('https://plateforme.exemple.fr');
        expect(adressePlateforme(TLS, null)).toBe('https://plateforme.exemple.fr');
    });

    it("(h) un protocole inconnu ne fabrique pas une adresse absurde", () => {
        // `file:` arrive quand on ouvre le HTML depuis le disque. Rien ne peut
        // en être déduit : le module retombe sur le clair plutôt que de rendre
        // `filews://`, qu'aucun navigateur ne comprendrait et dont le message
        // d'erreur ne désignerait pas la cause.
        const f: Emplacement = { protocol: 'file:', host: '' };
        expect(adresseSignaling(f)).toBe('ws://');
        expect(adressePlateforme(f)).toBe('http://');
    });
});
