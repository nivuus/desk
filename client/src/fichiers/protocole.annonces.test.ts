import { describe, expect, it } from 'vitest';
import { TYPE_DUES, TYPE_FAIT, encoder } from '../../../proto/ts/fichiers';
import type { Adaptateur } from './adaptateur';
import { creerServeur } from './protocole';

/**
 * La famille des ANNONCES du protocole fichiers, extraite de
 * `protocole.test.ts`.
 *
 * # Ce que cette extraction EST, et ce qu'elle n'est PAS
 *
 * **Elle n'ajoute aucun test.** Le `describe` est transposé **VERBATIM**
 * (l. 324-358 du parent), et le compte de `npx vitest run` est **inchangé**.
 * Elle vient **AVANT** l'addition qu'elle accueille : `protocole.test.ts`
 * était à **484** lignes, marge **16**, et F5 doit y ajouter les tests des
 * DEUX annonces neuves — `Bonjour` et `Rafraichir`, la **quatrième famille**
 * du protocole, celle qui va du navigateur vers le pont. C'est le geste que
 * D9 a inventé et que D10 a joué trois fois : **jamais une compression**.
 *
 * ⚠️ **`fauxAdaptateur` est recopié plutôt qu'importé**, et c'est délibéré :
 * l'exporter depuis le parent ferait du fichier de tests un module que deux
 * fichiers se partagent, et un `describe` du parent pourrait alors dépendre
 * d'une modification faite ici sans que rien ne le dise. Un doublet de six
 * lignes coûte moins qu'un couplage entre deux suites.
 */

/** Un adaptateur factice : le protocole ne connaît AUCUN système de fichiers. */
function fauxAdaptateur(surcharge: Partial<Adaptateur> = {}): Adaptateur {
    return {
        lister: async () => [{ nom: 'a.txt', repertoire: false, taille: 7, modifie: 42 }],
        attributs: async () => ({
            nom: 'Nom Stocké.txt',
            repertoire: false,
            taille: 1234,
            modifie: 1_690_000_000_000,
        }),
        lire: async () => new Uint8Array([9, 8, 7]),
        ...surcharge,
    };
}

describe('l’ANNONCE des écritures dues', () => {
    it('🔴 ne répond RIEN, et appelle le rappel injecté', async () => {
        // Rendre une trame ferait recevoir au pont une réponse à une
        // corrélation qu'il ne connaît pas, et il la jetterait en `debug!` —
        // SILENCIEUSEMENT. C'est le bras catch-all payé quatre fois sur
        // `capteur/pont_media.rs`.
        const vues: unknown[] = [];
        const serveur = creerServeur(fauxAdaptateur(), () => {}, {
            onDues: (dues) => vues.push(dues),
        });
        const reponse = await serveur.traiter(
            encoder(TYPE_DUES, 0, { dues: [{ chemin: 'note.txt', octets: 12 }] }),
        );
        expect(reponse).toBeNull();
        expect(vues).toEqual([[{ chemin: 'note.txt', octets: 12 }]]);
    });

    it('une annonce illisible est journalisée, jamais fatale', async () => {
        const messages: string[] = [];
        const serveur = creerServeur(fauxAdaptateur(), (m) => messages.push(m), {
            onDues: () => {
                throw new Error('jamais atteint');
            },
        });
        expect(await serveur.traiter(encoder(TYPE_DUES, 0, { dues: 'pas un tableau' }))).toBeNull();
        expect(messages.join(' ')).toMatch(/dues/);
    });

    it('un FAIT reçu par le navigateur est IGNORÉ : il ne demande rien', async () => {
        const messages: string[] = [];
        const serveur = creerServeur(fauxAdaptateur(), (m) => messages.push(m));
        expect(await serveur.traiter(encoder(TYPE_FAIT, 3, {}))).toBeNull();
        expect(messages.join(' ')).toMatch(/ne demande rien/);
    });
});
