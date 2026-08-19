// Le JWT HS256, sans dépendance et avec une horloge en paramètre.
//
// 🔴 Deux tests de ce fichier forgent un jeton À LA MAIN — c'est le seul moyen
// d'éprouver la vulnérabilité JWT la plus classique, la confusion
// d'algorithme : un vérificateur qui LIT `alg` dans l'en-tête et s'y fie
// accepte un jeton `alg:'none'` sans signature. L'en-tête n'est pas signé :
// on ne dérive jamais un comportement d'une donnée non signée.

import { createHmac } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import {
    DUREE_JETON_ACCES_MS,
    LONGUEUR_SECRET_MIN,
    signer,
    verifierJeton,
} from './jeton';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const T0 = 1_787_000_000_000;

function b64(valeur: unknown): string {
    return Buffer.from(JSON.stringify(valeur), 'utf8').toString('base64url');
}

/// Forge un jeton avec l'en-tête et la charge voulus. Si `signature` est
/// omise, elle est calculée en HS256 avec le secret — un jeton dont SEUL
/// l'`alg` est mensonger.
function forger(entete: unknown, charge: unknown, signature?: string): string {
    const tete = `${b64(entete)}.${b64(charge)}`;
    return `${tete}.${signature ?? createHmac('sha256', SECRET).update(tete).digest('base64url')}`;
}

describe('signer et verifierJeton', () => {
    it('relit le sujet d’un jeton signé', () => {
        const jeton = signer('utilisateur-42', SECRET, T0);
        expect(verifierJeton(jeton, SECRET, T0)).toEqual({ ok: true, sujet: 'utilisateur-42' });
    });

    it('REFUSE alg:none, même avec une signature vide', () => {
        // La forge classique : l'attaquant met `none` et retire la signature.
        const jeton = forger({ alg: 'none', typ: 'JWT' }, { sub: 'intrus', exp: T0 + 10_000 }, '');
        expect(verifierJeton(jeton, SECRET, T0)).toEqual({ ok: false, motif: 'algorithme' });
    });

    it('REFUSE alg:RS256, dont la signature HS256 est pourtant VALIDE', () => {
        // 🔴 Ce jeton-ci porte une signature HMAC correcte : SEUL le contrôle
        // de l'`alg` peut le refuser. Un vérificateur qui ne comparerait pas
        // l'algorithme l'accepterait, et ce test est le seul à le voir.
        const jeton = forger({ alg: 'RS256', typ: 'JWT' }, { sub: 'intrus', exp: T0 + 10_000 });
        expect(verifierJeton(jeton, SECRET, T0)).toEqual({ ok: false, motif: 'algorithme' });
    });

    it('REFUSE une signature dont un caractère a changé', () => {
        const jeton = signer('utilisateur-42', SECRET, T0);
        const [tete, charge, signature] = jeton.split('.');
        const abimee = (signature[0] === 'A' ? 'B' : 'A') + signature.slice(1);
        expect(verifierJeton(`${tete}.${charge}.${abimee}`, SECRET, T0))
            .toEqual({ ok: false, motif: 'signature' });
    });

    it('expire sur une horloge qui VARIE : accepté à t0+d/2, refusé à t0+d et au-delà', () => {
        // 🔴 TROIS instants distincts, et c'est le point : une horloge figée
        // rendrait ce test inerte, ce qui est exactement ce que la colonne
        // ROUGE du critère ② interdit.
        const duree = 60_000;
        const jeton = signer('utilisateur-42', SECRET, T0, duree);
        expect(verifierJeton(jeton, SECRET, T0 + duree / 2)).toEqual({
            ok: true,
            sujet: 'utilisateur-42',
        });
        // La borne est FRANCHE : `maintenant >= exp` refuse.
        expect(verifierJeton(jeton, SECRET, T0 + duree)).toEqual({ ok: false, motif: 'expire' });
        expect(verifierJeton(jeton, SECRET, T0 + duree + 1)).toEqual({ ok: false, motif: 'expire' });
    });

    it('REFUSE une forme invalide sans jamais LEVER', () => {
        // Un `JSON.parse` qui lève ici ferait répondre 500 à l'appelant HTTP,
        // là où il doit répondre 401 — et l'écart serait à lui seul un oracle.
        const attendu = { ok: false, motif: 'forme' };
        expect(verifierJeton('deux.segments', SECRET, T0)).toEqual(attendu);
        expect(verifierJeton('###.###.###', SECRET, T0)).toEqual(attendu);
        // Une charge qui est un nombre, pas un objet : `JSON.parse` réussit.
        expect(verifierJeton(forger({ alg: 'HS256', typ: 'JWT' }, 42), SECRET, T0)).toEqual(attendu);
        // Et tout ce qui n'est même pas une chaîne.
        expect(verifierJeton(undefined, SECRET, T0)).toEqual(attendu);
        expect(verifierJeton(null, SECRET, T0)).toEqual(attendu);
        expect(verifierJeton({ jeton: 'x' }, SECRET, T0)).toEqual(attendu);
    });

    it('signer LÈVE sur un secret plus court que LONGUEUR_SECRET_MIN', () => {
        // Sans ce refus, une plateforme se déploierait avec un secret
        // devinable, et rien ne le dirait.
        expect(LONGUEUR_SECRET_MIN).toBe(32);
        expect(() => signer('u', 'trop-court', T0)).toThrow(/32/);
        expect(DUREE_JETON_ACCES_MS).toBeGreaterThan(0);
    });
});
