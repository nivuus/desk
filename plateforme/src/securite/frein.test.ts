import { describe, expect, it } from 'vitest';
import {
    ECHECS_MAX_ADRESSE,
    ECHECS_MAX_COMPTE,
    ENTREES_MAX,
    FENETRE_MS,
    Frein,
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
    });
});
