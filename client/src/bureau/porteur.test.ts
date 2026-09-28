import { describe, expect, it } from 'vitest';
import {
    batirDemande,
    batirEtat,
    elire,
    estDemandeEtat,
    estPlacePrise,
    fenetresAPeindre,
    lireEtat,
    lireTrame,
    ouvertureParLeBureau,
    promouvoir,
} from './porteur';

describe('elire', () => {
    it('without a lock API, the tab becomes holder: the fallback is OPTIMISTIC', () => {
        // ⚠️ Optimiste et non pessimiste : sans verrou, se declarer suiveur
        // ferait qu AUCUN onglet n ouvrirait jamais la session. La plateforme
        // tranchera, et `estPlacePrise` rattrapera le perdant.
        let role = '';
        elire('v', {
            devenirPorteur: () => { role = 'porteur'; },
            devenirSuiveur: () => { role = 'suiveur'; },
        });
        expect(role).toBe('porteur');
    });

    it('with the API, the holder is proclaimed ONLY once the lock is obtained', () => {
        let role = '';
        let relacher: (() => void) | undefined;
        elire('v', {
            // Un verrou qui n appelle JAMAIS `pendant` : le verrou n est pas
            // obtenu, donc cet onglet n est pas porteur.
            verrou: (_nom, pendant) => { relacher = () => void pendant(); },
            devenirPorteur: () => { role = 'porteur'; },
            devenirSuiveur: () => { role = 'suiveur'; },
        });
        expect(role).toBe('suiveur');
        // Puis le verrou se libere : l onglet en attente est promu.
        relacher!();
        expect(role).toBe('porteur');
    });
});

describe('estPlacePrise', () => {
    it('recognises the TYPED reason', () => {
        expect(estPlacePrise({ type: 'error', reason: 'whatever', motif: 'role-occupe' })).toBe(true);
    });

    it('does NOT recognise a refusal for another cause', () => {
        // 🔴 CE CAS EST LE POINT : un frein de volume doit rester VISIBLE.
        // L avaler ferait de ce lot la panne muette qu il pretend eviter.
        expect(estPlacePrise({ type: 'error', reason: 'too many requests', motif: 'trop-de-requetes' })).toBe(false);
    });

    it('does NOT recognise a refusal without a reason, even if its sentence says so', () => {
        // ⚠️ Le piege de F1 : une phrase francaise se reformule. On ne
        // devine pas, on lit le motif -- ou on affiche.
        expect(estPlacePrise({ type: 'error', reason: 'a client is already connected to session s' })).toBe(false);
    });

    it('does not throw on an entry that is not an object', () => {
        expect(estPlacePrise(undefined)).toBe(false);
        expect(estPlacePrise('role-occupe')).toBe(false);
    });
});

describe('lireEtat', () => {
    it('returns the list of a well-formed state', () => {
        const etat = batirEtat([{ session: 's', titre: 'Bloc-notes', ouverte: true }]);
        expect(lireEtat(etat)?.[0]?.titre).toBe('Bloc-notes');
    });

    it('returns undefined on a message from ANOTHER sender', () => {
        // Un `BroadcastChannel` est partage par origine : tout ce qui y passe
        // n est pas forcement de nous.
        expect(lireEtat({ type: 'autre-chose', fenetres: [] })).toBeUndefined();
    });

    it('returns undefined when `fenetres` is not an array', () => {
        expect(lireEtat({ type: 'etat-bureau', fenetres: 'trois' })).toBeUndefined();
    });

    it('DISCARDS a malformed entry instead of letting it through', () => {
        const lu = lireEtat({
            type: 'etat-bureau',
            fenetres: [{ session: 's', titre: 'bon', ouverte: false }, { session: 42 }],
        });
        expect(lu?.length).toBe(1);
    });
});

describe('fenetresAPeindre', () => {
    it('a follower that has received nothing yet paints nothing', () => {
        expect(fenetresAPeindre('suiveur', [], undefined)).toEqual([]);
    });

    it(
        'a follower that received N windows keeps them on the next timer tick, ' +
            'EVEN WHEN ITS OWN LIST IS EMPTY',
        () => {
            // 🔴 C EST CE CAS QUI ATTRAPE LE DEFAUT DU ROUND 1 (critique ①) :
            // la minuterie repeint a 1 Hz depuis `bureau.list()`, qui est
            // STRUCTURELLEMENT VIDE chez un suiveur -- aucun socket, donc
            // aucun `fenetreOuverte` ne l alimente jamais. Une regle qui
            // peindrait `ownList` chez un suiveur effacerait donc, au
            // tour SUIVANT une diffusion, ce qu elle venait de montrer.
            const recues = [{ session: 's', titre: 'Bloc-notes', ouverte: true }];
            expect(fenetresAPeindre('suiveur', [], recues)).toEqual(recues);
        },
    );

    it('the holder ALWAYS paints its own list, never a stale received state', () => {
        const propre = [{ session: 's', titre: 'Bloc-notes', ouverte: false }];
        const recuPerime = [{ session: 's', titre: 'Bloc-notes', ouverte: true }];
        expect(fenetresAPeindre('porteur', propre, recuPerime)).toEqual(propre);
    });
});

/* ══ CE QUE LA REVUE FINALE DU 31 AOUT 2026 A AJOUTE ═════════════════════ */

describe('promote: the promotion cycle', () => {
    it(
        'requests a FRESH token again, then installs the bridge, THEN opens the socket',
        async () => {
            // 🔴 C EST LA JONCTION QUI ETAIT FAUSSE, PAS LA REGLE DE
            // FRAICHEUR. Le socket etait ouvert avec `deps.jeton`, une chaine
            // FIGEE AU CHARGEMENT ; or un suiveur n est promu qu a la mort du
            // porteur, potentiellement des heures plus tard, et un jeton d
            // acces vit DIX MINUTES. Un test d `assurerAccesFrais` ne pouvait
            // pas voir ce defaut : il ne vit qu ici.
            const ordre: string[] = [];
            let demandes = 0;
            await promouvoir({
                jetonFrais: () => { demandes += 1; return Promise.resolve('frais-a-la-promotion'); },
                installerPont: () => { ordre.push('pont'); },
                ouvrirSocket: (jeton) => { ordre.push(`socket:${jeton}`); },
                sansJeton: () => { ordre.push('sans-jeton'); },
            });
            expect(demandes, 'the token must be REQUESTED AGAIN on promotion').toBe(1);
            expect(ordre).toEqual(['pont', 'socket:frais-a-la-promotion']);
        },
    );

    it('without an obtainable token, opens NO socket and SAYS so', async () => {
        // ⚠️ Ouvrir un socket voue au refus afficherait un refus que
        // `canalDeControlePerdu` ecraserait aussitot par « Rechargez la
        // page » : l utilisateur ne saurait pas que sa session a expire.
        const ordre: string[] = [];
        await promouvoir({
            jetonFrais: () => Promise.resolve(undefined),
            installerPont: () => { ordre.push('pont'); },
            ouvrirSocket: () => { ordre.push('socket'); },
            sansJeton: () => { ordre.push('sans-jeton'); },
        });
        expect(ordre).toEqual(['sans-jeton']);
    });
});

describe('elire: the lock is RELEASED', () => {
    it('a dismissed holder gives back its lock, and its partition can elect another one', async () => {
        // 🔴 IMPORTANT ③ : la promesse tenue etait un `Promise<never>` que
        // rien ne resolvait -- un porteur demis par `estPlacePrise` gardait
        // le verrou POUR TOUJOURS, et sa partition n aurait PLUS JAMAIS eu de
        // porteur.
        let rendu = false;
        let tenue: Promise<void> | undefined;
        const election = elire('v', {
            verrou: (_nom, pendant) => { tenue = pendant(); },
            devenirPorteur: () => {},
            devenirSuiveur: () => {},
        });
        void tenue!.then(() => { rendu = true; });
        // Tant que personne ne relache, la promesse ne se regle pas.
        await Promise.resolve();
        expect(rendu, 'the lock is NOT released by itself').toBe(false);
        election.relacher();
        // ⚠️ ON N ATTEND PAS `tenue` : un `await` sur une promesse qui pourrait
        // ne JAMAIS se regler rougirait par EXPIRATION, et une expiration ne
        // dit pas QUELLE assertion a echoue. On laisse courir les microtaches
        // du `then`, puis on ASSERTE.
        await Promise.resolve();
        await Promise.resolve();
        expect(rendu, 'the lock must be RELEASED when it is let go').toBe(true);
    });

    it('release is IDEMPOTENT, and inert without a held lock', () => {
        // Le repli sans `navigator.locks` ne detient aucun verrou : il n y a
        // rien a rendre, et le dire ne doit pas lever.
        const election = elire('v', { devenirPorteur: () => {}, devenirSuiveur: () => {} });
        expect(() => { election.relacher(); election.relacher(); }).not.toThrow();
    });
});

describe('estDemandeEtat', () => {
    it('recognises the request a new tab makes on mount', () => {
        // 🔴 IMPORTANT ① : `diffuserSiChange` ne poste que sur CHANGEMENT. En
        // regime -- trois fenetres, rien qui bouge -- un onglet qui rejoint
        // montrait une liste VIDE POUR TOUJOURS.
        expect(estDemandeEtat(batirDemande())).toBe(true);
    });

    it('does NOT mistake a state broadcast for a request', () => {
        expect(estDemandeEtat(batirEtat([]))).toBe(false);
    });

    it('does not throw on an entry that is not an object', () => {
        expect(estDemandeEtat(undefined)).toBe(false);
        expect(estDemandeEtat('demande-etat')).toBe(false);
    });
});

describe('lireTrame', () => {
    it('returns the object of a well-formed frame', () => {
        expect(lireTrame('{"type":"fenetre-ouverte","session":"s"}')?.type).toBe('fenetre-ouverte');
    });

    it('returns undefined on a NON-JSON frame, instead of throwing', () => {
        // 🔴 MINOR ③ : `JSON.parse(evenement.data)` etait NU dans l ecouteur
        // du socket de controle.
        expect(lireTrame('not json')).toBeUndefined();
    });

    it('returns undefined on `null`, `42` and an ARRAY -- which JSON.parse accepts', () => {
        // `null.type` leve une `TypeError` ; un tableau et un nombre ont bien
        // un `.type` `undefined`, mais ne sont pas des trames.
        expect(lireTrame('null')).toBeUndefined();
        expect(lireTrame('42')).toBeUndefined();
        expect(lireTrame('[1,2]')).toBeUndefined();
    });

    it('returns undefined on what is not even a string', () => {
        expect(lireTrame(new ArrayBuffer(4))).toBeUndefined();
    });
});

describe('ouvertureParLeBureau', () => {
    it('the holder goes through the desktop, which REMEMBERS the handle', () => {
        expect(ouvertureParLeBureau('porteur')).toBe(true);
    });

    it('a follower opens directly: its `bureau` is fed by nothing', () => {
        expect(ouvertureParLeBureau('suiveur')).toBe(false);
    });
});
