import { describe, expect, it } from 'vitest';
import {
    BUDGET_ADRESSE,
    BUDGET_REQUETES,
    ECHECS_MAX_ADRESSE,
    ECHECS_MAX_COMPTE,
    ENTREES_MAX,
    FENETRE_MS,
    FENETRE_REQUETES_MS,
    REQUETES_MAX_ADRESSE,
    Frein,
    cleAdresse as cleAdresseDuModule,
    cleRequetes,
    type Budget,
} from './frein';

/// Une époque réelle, sur le patron de `base/harnais.ts:32` : les petites
/// valeurs ne mesurent rien (leçon de P1), et un frein qui soustrait des
/// horodatages doit être éprouvé sur des horodatages plausibles.
const T0 = 1_787_000_000_000;

/// Des adresses de documentation (RFC 5737), jamais `a`/`b`/`c` : un frein qui
/// normalise les adresses ne peut pas être éprouvé sur des étiquettes.
const ADR = '203.0.113.7';
const AUTRE_ADR = '198.51.100.4';

const COMPTE: Budget = { max: ECHECS_MAX_COMPTE, fenetreMs: FENETRE_MS };
const ADRESSE: Budget = { max: ECHECS_MAX_ADRESSE, fenetreMs: FENETRE_MS };

function cleCompte(email: string): [string, Budget] {
    return [`compte:${email}`, COMPTE];
}
function cleAdresse(adresse: string): [string, Budget] {
    return [`adr:${adresse}`, ADRESSE];
}

describe('Frein', () => {
    it('(a) sous le budget, ne freine pas', () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        // `max - 1` échecs : il reste exactement un essai.
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) f.echec(cles, T0 + i);
        expect(f.consulter(cles, T0 + ECHECS_MAX_COMPTE)).toEqual({
            freine: false,
            retryApresS: 0,
        });
    });

    it('(b) au budget exactement, freine — la comparaison est `>=`, jamais `>`', () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) f.echec(cles, T0 + i);
        expect(f.consulter(cles, T0 + ECHECS_MAX_COMPTE).freine).toBe(true);
    });

    it('(c) après la fenêtre, le budget est rendu', () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) f.echec(cles, T0 + i);
        expect(f.consulter(cles, T0 + FENETRE_MS - 1).freine).toBe(true);
        // L'horloge est un PARAMÈTRE : le test avance, il n'attend pas.
        expect(f.consulter(cles, T0 + FENETRE_MS + 1).freine).toBe(false);
    });

    it('(d) `succes` efface la clé donnée, et elle seule', () => {
        const f = new Frein();
        const compte = cleCompte('alice@exemple.test');
        const adresse = cleAdresse(ADR);
        // Les DEUX clés sont épuisées, pour que l'effacement d'une seule soit
        // observable sur l'autre.
        for (let i = 0; i < ECHECS_MAX_ADRESSE; i++) f.echec([compte, adresse], T0 + i);
        f.succes(compte[0]);
        expect(f.consulter([compte], T0 + ECHECS_MAX_ADRESSE).freine).toBe(false);
        expect(f.consulter([adresse], T0 + ECHECS_MAX_ADRESSE).freine).toBe(true);
    });

    it("(e) `consulter` n'enregistre RIEN", () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        // `max + 1` consultations : si `consulter` comptait, la dernière
        // freinerait. Un frein auto-entretenu tiendrait un compte bloqué sans
        // qu'aucun mot de passe ne soit jamais essayé.
        for (let i = 0; i <= ECHECS_MAX_COMPTE; i++) {
            expect(f.consulter(cles, T0 + i).freine).toBe(false);
        }
        expect(f.taille()).toBe(0);
    });

    it('(f) 🔴 la table ne dépasse JAMAIS son plafond d’entrées', () => {
        // `entreesMax` est un paramètre du constructeur PRÉCISÉMENT pour que
        // le test puisse en poser un petit : insérer dix mille clés ne
        // mesurerait rien de plus, et coûterait le temps de la suite.
        const f = new Frein(8);
        for (let i = 0; i < 9; i++) f.echec([cleCompte(`n${i}@exemple.test`)], T0 + i);
        expect(f.taille()).toBeLessThanOrEqual(8);
    });

    it('(g) une éviction est COMPTÉE, jamais silencieuse', () => {
        const f = new Frein(8);
        expect(f.evictions()).toBe(0);
        for (let i = 0; i < 9; i++) f.echec([cleCompte(`n${i}@exemple.test`)], T0 + i);
        expect(f.evictions()).toBeGreaterThanOrEqual(1);
    });

    it('(g bis) une entrée EXPIRÉE est purgée avant qu’on évince une vivante', () => {
        const f = new Frein(2);
        f.echec([cleCompte('vieille@exemple.test')], T0);
        f.echec([cleCompte('recente@exemple.test')], T0 + FENETRE_MS + 1);
        // La table est pleine, et la plus vieille est EXPIRÉE : la purge doit
        // suffire, sans qu'aucune entrée vivante ne soit évincée.
        f.echec([cleCompte('neuve@exemple.test')], T0 + FENETRE_MS + 2);
        expect(f.taille()).toBeLessThanOrEqual(2);
        expect(f.evictions()).toBe(0);
    });

    it('(h) `retryApresS` est le temps RESTANT, arrondi vers le haut', () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) f.echec(cles, T0);
        // À la moitié de la fenêtre, il reste la moitié — pas la fenêtre
        // entière, qui est ce qu'une constante en dur rendrait.
        const moitie = FENETRE_MS / 2;
        expect(f.consulter(cles, T0 + moitie).retryApresS).toBe(moitie / 1000);
        // 1 ms restante s'arrondit à 1 s, jamais à 0 : un `Retry-After: 0`
        // inviterait le demandeur à revenir immédiatement.
        expect(f.consulter(cles, T0 + FENETRE_MS - 1).retryApresS).toBe(1);
    });

    it('(i) une seule clé freinée suffit, et le verdict porte SON temps', () => {
        const f = new Frein();
        const compte = cleCompte('alice@exemple.test');
        const adresse = cleAdresse(AUTRE_ADR);
        // Seule la clé de COMPTE est épuisée ; la clé d'adresse est loin de
        // son budget. Le couple doit tout de même être freiné.
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) f.echec([compte, adresse], T0);
        const v = f.consulter([compte, adresse], T0 + 1000);
        expect(v.freine).toBe(true);
        expect(v.retryApresS).toBe(FENETRE_MS / 1000 - 1);
    });

    it('(j) les constantes sont celles que le plan fixe, et elles sont exportées', () => {
        // Elles sont NON CALIBRÉES (voir l'en-tête du module) : ce test les
        // fige pour qu'un changement soit un geste délibéré, pas une dérive.
        expect(FENETRE_MS).toBe(15 * 60_000);
        expect(ECHECS_MAX_COMPTE).toBe(5);
        expect(ECHECS_MAX_ADRESSE).toBe(50);
        expect(ENTREES_MAX).toBe(10_000);
        // ⚠️ NON CALIBRÉES NON PLUS — voir `BUDGET_REQUETES` dans le module.
        expect(FENETRE_REQUETES_MS).toBe(60_000);
        expect(REQUETES_MAX_ADRESSE).toBe(120);
    });

    // 🔴 LE TEST QUI COMPTE LE PLUS DE CE LOT. `Frein` COMPTE DES ÉCHECS —
    // `echec()`, `succes()`, `BUDGET_COMPTE`, `BUDGET_ADRESSE` — et
    // `BUDGET_REQUETES` réutilise la MÉTHODE `echec()` pour compter un
    // VOLUME, jamais un échec. Si `cleRequetes` et `cleAdresse` produisaient
    // la MÊME clé pour la MÊME adresse, un utilisateur actif épuiserait son
    // propre budget d'ÉCHECS D'AUTHENTIFICATION en faisant simplement du
    // trafic légitime sur `/vm` ou `/session` — ou l'inverse, et un
    // attaquant s'offrirait des essais de mot de passe gratuits en générant
    // du volume. Les deux DOIVENT donc vivre sous des préfixes disjoints
    // (`req:` contre `adr:`), et c'est ce que ce test éprouve directement sur
    // les fonctions RÉELLEMENT exportées — pas sur des clones locaux au
    // fichier, comme les tests (a) à (i) ci-dessus : eux éprouvent la
    // MÉCANIQUE générique de `Frein`, celui-ci éprouve le NAMESPACING réel de
    // `securite/frein.ts`.
    it("(k) le budget de requêtes n'entame PAS le budget d'échecs de la même adresse", () => {
        // 🔴 CORRECTIF (round de correction 1, critique ①) : le brief d'origine
        // prescrivait `i < 10` contre un budget d'échecs de 50 — l'assertion
        // NE POUVAIT PAS tomber, fusion des clés ou pas (10 < 50 dans les DEUX
        // cas). MESURÉ : en fusionnant `cleRequetes` et `cleAdresse` sur le
        // même préfixe, ce test restait VERT. `ECHECS_MAX_ADRESSE + 1` est le
        // plus PETIT nombre qui rende la fusion détectable : au budget
        // exactement, `Frein` freine (comparaison `>=`, jamais `>`).
        const frein = new Frein();
        for (let i = 0; i < ECHECS_MAX_ADRESSE + 1; i++) {
            frein.echec([[cleRequetes(ADR), BUDGET_REQUETES]], T0);
        }
        expect(frein.consulter([[cleAdresseDuModule(ADR), BUDGET_ADRESSE]], T0).freine).toBe(
            false,
        );
    });

    it('(k bis) — et RÉCIPROQUEMENT : des échecs sur une adresse n’entament PAS son budget de requêtes', () => {
        // Symétrique de (k) : un attaquant qui épuise le budget D'ÉCHECS
        // d'une adresse (en se trompant de mot de passe, par exemple) ne
        // doit PAS voir son budget de VOLUME entamé pour autant — les deux
        // sont des ressources distinctes, protégeant des abus distincts.
        //
        // 🔴 MÊME CORRECTIF QUE (k) : `ECHECS_MAX_ADRESSE` (50) échecs contre
        // un budget de requêtes de `REQUETES_MAX_ADRESSE` (120) ne pouvait PAS
        // tomber, fusion ou pas (50 < 120 dans les DEUX cas). `REQUETES_MAX_
        // ADRESSE + 1` est le plus petit nombre qui rende la fusion détectable
        // de CE côté-ci.
        const frein = new Frein();
        for (let i = 0; i < REQUETES_MAX_ADRESSE + 1; i++) {
            frein.echec([[cleAdresseDuModule(ADR), BUDGET_ADRESSE]], T0 + i);
        }
        expect(
            frein.consulter(
                [[cleRequetes(ADR), BUDGET_REQUETES]],
                T0 + REQUETES_MAX_ADRESSE + 1,
            ).freine,
        ).toBe(false);
    });

    // 🔵 LA PROPRIÉTÉ FAVORABLE QUE LA REVUE A TROUVÉE (round de correction 1) :
    // l'éviction croisée ne mord pas parce que `req:` (une minute) expire
    // TOUJOURS avant `compte:`/`adr:` (quinze minutes). Elle ne tient QUE si
    // cette inégalité tient — voir `BUDGET_REQUETES` dans le module.
    it('(l) 🔴 FENETRE_REQUETES_MS reste PLUS COURTE que FENETRE_MS — sans quoi la propriété favorable de purge tombe', () => {
        expect(FENETRE_REQUETES_MS).toBeLessThan(FENETRE_MS);
    });

    // 🔴 LA ROUGE DEMANDÉE PAR LA REVUE (round de correction 1, critique ③) :
    // « un pair refusé retente en boucle et ne verrouille pas son adresse ».
    // Reproduit ici, EN TS, le calendrier EXACT de
    // `agent/src/plateforme/repli.rs::delai_de_repli` — le remède que le
    // correctif agent réutilise pour `/signal` — et montre que, contrairement
    // au martèlement à plat de 500 ms d'AVANT ce correctif, un pair qui recule
    // ainsi entre deux tentatives ne consomme jamais qu'une fraction infime du
    // budget partagé : le reste demeure ouvert à toute autre requête de la
    // même adresse (session de contrôle du superviseur, autres fenêtres).
    it("(m) 🔴 un pair refusé qui retente selon delai_de_repli (repli.rs) NE VERROUILLE PAS son adresse", () => {
        // Copie fidèle de `agent/src/plateforme/repli.rs::delai_de_repli` —
        // mêmes constantes (`REPLI_MIN_MS`/`REPLI_MAX_MS`), même formule.
        // Si l'une des deux dérive un jour sans que l'autre suive, ce test
        // ne le verra pas — c'est un COMPARATIF, pas un miroir garanti.
        const REPLI_MIN_MS = 500;
        const REPLI_MAX_MS = 30_000;
        function delaiDeRepli(tentative: number): number {
            const facteur = tentative >= 63 ? Number.MAX_SAFE_INTEGER : 2 ** tentative;
            return Math.min(REPLI_MIN_MS * facteur, REPLI_MAX_MS);
        }

        const frein = new Frein();
        const cle: readonly [string, Budget] = [cleRequetes(ADR), BUDGET_REQUETES];
        const CINQ_MINUTES_MS = 5 * 60_000;
        let instant = T0;
        let tentative = 0;
        let admises = 0;
        // ⚠️ LE DERNIER INSTANT SIMULÉ, PAS UN INSTANT NEUF choisi après coup :
        // un instant neuf pourrait tomber exactement sur une frontière de
        // fenêtre (`restant === 0`, cas limite documenté dans `consulter`) et
        // rendrait le test dépendant du hasard de cette coïncidence plutôt que
        // du comportement qu'il éprouve.
        let dernierInstant = instant;
        while (instant < T0 + CINQ_MINUTES_MS) {
            const verdict = frein.consulter([cle], instant);
            if (!verdict.freine) {
                frein.echec([cle], instant);
                admises++;
            }
            dernierInstant = instant;
            instant += delaiDeRepli(tentative);
            tentative++;
        }
        // Loin, très loin du plafond de `REQUETES_MAX_ADRESSE` : la preuve
        // que le remède agent laisse le budget quasi entier disponible pour
        // les AUTRES pairs de la même adresse.
        expect(admises).toBeLessThan(REQUETES_MAX_ADRESSE / 4);
        // 🔴 CETTE SECONDE ASSERTION EST INCAPABLE DE ROUGIR, ET C'EST DIT ICI
        // PLUTÔT QUE LAISSÉ IMPLICITE (revue, round de correction 2) : sous
        // TOUT espacement qui respecte ne serait-ce que le plancher documenté
        // (≥ 500 ms), `admises` ne peut structurellement PAS approcher
        // `REQUETES_MAX_ADRESSE` sur une fenêtre de `FENETRE_REQUETES_MS`
        // (60 s) — l'assertion ci-dessus l'établit déjà. `freine` ne peut donc
        // JAMAIS devenir vrai ici, quelle que soit la qualité RÉELLE du repli
        // exponentiel simulé : cette ligne ne fait que reformuler la même
        // propriété sous une autre forme, elle ne l'éprouve pas
        // indépendamment. **C'est `(m bis)`, seul, qui porte la preuve que le
        // frein sait encore verrouiller** — en retentant plus vite que le
        // plancher documenté, il fait bien rougir cette même assertion
        // (`freine === true` là-bas). Conservée ici pour la LISIBILITÉ du
        // scénario (« et donc, un pair légitime n'est pas bloqué »), jamais
        // comme un contrôle à part entière.
        expect(frein.consulter([cle], dernierInstant).freine).toBe(false);
    });

    // 🔵 TÉMOIN NÉGATIF DE (m) : sans le correctif agent, un pair qui retente
    // À PLAT toutes les 500 ms — le comportement MESURÉ avant ce round —
    // VERROUILLE bel et bien l'adresse. Sans ce témoin, (m) ne prouverait
    // rien : un contrôle qu'on n'a jamais vu rougir n'est pas un contrôle.
    it('(m bis) — témoin négatif : un pair qui retente À PLAT, plus vite que le plancher documenté, VERROUILLE son adresse', () => {
        // ⚠️ PAS 500 ms : c'est PRÉCISÉMENT `PERIODE_RELANCE_PONT_MIN`, le
        // PLANCHER documenté de `surveillance_pont.rs`, et il coïncide —
        // c'est un hasard d'arrondi — avec `REQUETES_MAX_ADRESSE` (120) sur
        // une fenêtre de 60 s : 60000/500 = 120 exactement, si bien qu'un
        // martèlement à EXACTEMENT 500 ms n'atteint JAMAIS le seuil `>=`
        // (la 121ᵉ requête arrive PILE quand la fenêtre expire). Ce témoin
        // retente à 200 ms — plus vite que le plancher, ce que ce lot ne
        // permet PAS en pratique (voir `surveillance_pont.rs`), mais qui
        // reste la meilleure façon d'éprouver que LE FREIN LUI-MÊME sait
        // encore verrouiller une adresse en l'absence de tout repli.
        const frein = new Frein();
        const cle: readonly [string, Budget] = [cleRequetes(ADR), BUDGET_REQUETES];
        const CINQ_MINUTES_MS = 5 * 60_000;
        let instant = T0;
        let admises = 0;
        let dernierInstant = instant;
        while (instant < T0 + CINQ_MINUTES_MS) {
            const verdict = frein.consulter([cle], instant);
            if (!verdict.freine) {
                frein.echec([cle], instant);
                admises++;
            }
            dernierInstant = instant;
            instant += 200;
        }
        // Le patron du bug MESURÉ, à une cadence plus agressive que le
        // plancher : le budget entier est consommé par CE SEUL pair, encore
        // et encore.
        expect(admises).toBeGreaterThanOrEqual(REQUETES_MAX_ADRESSE);
        // Et un pair LÉGITIME arrivant juste après cette rafale reste freiné
        // — c'est exactement « une fenêtre neuve ne peut plus s'attacher ».
        expect(frein.consulter([cle], dernierInstant).freine).toBe(true);
    });
});
