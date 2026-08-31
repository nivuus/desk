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
    it('sans API de verrou, l onglet devient porteur : le repli est OPTIMISTE', () => {
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

    it('avec l API, le porteur n est proclame QUE lorsque le verrou est obtenu', () => {
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
    it('reconnait le motif TYPE', () => {
        expect(estPlacePrise({ type: 'error', reason: 'peu importe', motif: 'role-occupe' })).toBe(true);
    });

    it('ne reconnait PAS un refus d une autre cause', () => {
        // 🔴 CE CAS EST LE POINT : un frein de volume doit rester VISIBLE.
        // L avaler ferait de ce lot la panne muette qu il pretend eviter.
        expect(estPlacePrise({ type: 'error', reason: 'trop de requetes', motif: 'trop-de-requetes' })).toBe(false);
    });

    it('ne reconnait PAS un refus sans motif, meme si sa phrase le dit', () => {
        // ⚠️ Le piege de F1 : une phrase francaise se reformule. On ne
        // devine pas, on lit le motif -- ou on affiche.
        expect(estPlacePrise({ type: 'error', reason: 'un client est deja connecte a la session s' })).toBe(false);
    });

    it('ne leve pas sur une entree qui n est pas un objet', () => {
        expect(estPlacePrise(undefined)).toBe(false);
        expect(estPlacePrise('role-occupe')).toBe(false);
    });
});

describe('lireEtat', () => {
    it('rend la liste d un etat bien forme', () => {
        const etat = batirEtat([{ session: 's', titre: 'Bloc-notes', ouverte: true }]);
        expect(lireEtat(etat)?.[0]?.titre).toBe('Bloc-notes');
    });

    it('rend undefined sur un message d un AUTRE emetteur', () => {
        // Un `BroadcastChannel` est partage par origine : tout ce qui y passe
        // n est pas forcement de nous.
        expect(lireEtat({ type: 'autre-chose', fenetres: [] })).toBeUndefined();
    });

    it('rend undefined quand `fenetres` n est pas un tableau', () => {
        expect(lireEtat({ type: 'etat-bureau', fenetres: 'trois' })).toBeUndefined();
    });

    it('ECARTE une entree mal formee au lieu de la laisser passer', () => {
        const lu = lireEtat({
            type: 'etat-bureau',
            fenetres: [{ session: 's', titre: 'bon', ouverte: false }, { session: 42 }],
        });
        expect(lu?.length).toBe(1);
    });
});

describe('fenetresAPeindre', () => {
    it('un suiveur qui n a encore rien recu ne peint rien', () => {
        expect(fenetresAPeindre('suiveur', [], undefined)).toEqual([]);
    });

    it(
        'un suiveur qui a recu N fenetres les garde au tour de minuterie suivant, ' +
            'MEME QUAND SA PROPRE LISTE EST VIDE',
        () => {
            // 🔴 C EST CE CAS QUI ATTRAPE LE DEFAUT DU ROUND 1 (critique ①) :
            // la minuterie repeint a 1 Hz depuis `bureau.liste()`, qui est
            // STRUCTURELLEMENT VIDE chez un suiveur -- aucun socket, donc
            // aucun `fenetreOuverte` ne l alimente jamais. Une regle qui
            // peindrait `listePropre` chez un suiveur effacerait donc, au
            // tour SUIVANT une diffusion, ce qu elle venait de montrer.
            const recues = [{ session: 's', titre: 'Bloc-notes', ouverte: true }];
            expect(fenetresAPeindre('suiveur', [], recues)).toEqual(recues);
        },
    );

    it('le porteur peint TOUJOURS sa propre liste, jamais un etat recu perime', () => {
        const propre = [{ session: 's', titre: 'Bloc-notes', ouverte: false }];
        const recuPerime = [{ session: 's', titre: 'Bloc-notes', ouverte: true }];
        expect(fenetresAPeindre('porteur', propre, recuPerime)).toEqual(propre);
    });
});

/* ══ CE QUE LA REVUE FINALE DU 31 AOUT 2026 A AJOUTE ═════════════════════ */

describe('promouvoir : le cycle de promotion', () => {
    it(
        'redemande un jeton FRAIS, puis installe le pont, PUIS ouvre le socket',
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
            expect(demandes, 'le jeton doit etre REDEMANDE a la promotion').toBe(1);
            expect(ordre).toEqual(['pont', 'socket:frais-a-la-promotion']);
        },
    );

    it('sans jeton obtenable, n ouvre AUCUN socket et le DIT', async () => {
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

describe('elire : le verrou se RELACHE', () => {
    it('un porteur demis rend son verrou, et sa partition peut en elire un autre', async () => {
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
        expect(rendu, 'le verrou ne se rend PAS de lui-meme').toBe(false);
        election.relacher();
        // ⚠️ ON N ATTEND PAS `tenue` : un `await` sur une promesse qui pourrait
        // ne JAMAIS se regler rougirait par EXPIRATION, et une expiration ne
        // dit pas QUELLE assertion a echoue. On laisse courir les microtaches
        // du `then`, puis on ASSERTE.
        await Promise.resolve();
        await Promise.resolve();
        expect(rendu, 'le verrou doit etre RENDU quand on le relache').toBe(true);
    });

    it('relacher est IDEMPOTENTE, et inerte sans verrou detenu', () => {
        // Le repli sans `navigator.locks` ne detient aucun verrou : il n y a
        // rien a rendre, et le dire ne doit pas lever.
        const election = elire('v', { devenirPorteur: () => {}, devenirSuiveur: () => {} });
        expect(() => { election.relacher(); election.relacher(); }).not.toThrow();
    });
});

describe('estDemandeEtat', () => {
    it('reconnait la demande qu un onglet neuf pose au montage', () => {
        // 🔴 IMPORTANT ① : `diffuserSiChange` ne poste que sur CHANGEMENT. En
        // regime -- trois fenetres, rien qui bouge -- un onglet qui rejoint
        // montrait une liste VIDE POUR TOUJOURS.
        expect(estDemandeEtat(batirDemande())).toBe(true);
    });

    it('ne confond PAS une diffusion d etat avec une demande', () => {
        expect(estDemandeEtat(batirEtat([]))).toBe(false);
    });

    it('ne leve pas sur une entree qui n est pas un objet', () => {
        expect(estDemandeEtat(undefined)).toBe(false);
        expect(estDemandeEtat('demande-etat')).toBe(false);
    });
});

describe('lireTrame', () => {
    it('rend l objet d une trame bien formee', () => {
        expect(lireTrame('{"type":"fenetre-ouverte","session":"s"}')?.type).toBe('fenetre-ouverte');
    });

    it('rend undefined sur une trame NON-JSON, au lieu de lever', () => {
        // 🔴 MINOR ③ : `JSON.parse(evenement.data)` etait NU dans l ecouteur
        // du socket de controle.
        expect(lireTrame('pas du json')).toBeUndefined();
    });

    it('rend undefined sur `null`, `42` et un TABLEAU -- que JSON.parse accepte', () => {
        // `null.type` leve une `TypeError` ; un tableau et un nombre ont bien
        // un `.type` `undefined`, mais ne sont pas des trames.
        expect(lireTrame('null')).toBeUndefined();
        expect(lireTrame('42')).toBeUndefined();
        expect(lireTrame('[1,2]')).toBeUndefined();
    });

    it('rend undefined sur ce qui n est meme pas une chaine', () => {
        expect(lireTrame(new ArrayBuffer(4))).toBeUndefined();
    });
});

describe('ouvertureParLeBureau', () => {
    it('le porteur passe par le bureau, qui MEMORISE le handle', () => {
        expect(ouvertureParLeBureau('porteur')).toBe(true);
    });

    it('un suiveur ouvre directement : son `bureau` n est alimente par rien', () => {
        expect(ouvertureParLeBureau('suiveur')).toBe(false);
    });
});
