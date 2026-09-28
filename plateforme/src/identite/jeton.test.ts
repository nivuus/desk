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
    MIN_SECRET_LENGTH,
    signer,
    verifyToken,
} from './jeton';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const T0 = 1_787_000_000_000;

function b64(value: unknown): string {
    return Buffer.from(JSON.stringify(value), 'utf8').toString('base64url');
}

/// Forge un jeton avec l'en-tête et la charge voulus. Si `signature` est
/// omise, elle est calculée en HS256 avec le secret — un jeton dont SEUL
/// l'`alg` est mensonger.
function forger(entete: unknown, charge: unknown, signature?: string): string {
    const tete = `${b64(entete)}.${b64(charge)}`;
    return `${tete}.${signature ?? createHmac('sha256', SECRET).update(tete).digest('base64url')}`;
}

describe('signer and verifierJeton', () => {
    it('re-reads the subject of a signed token', () => {
        const jeton = signer('user-42', SECRET, T0);
        expect(verifyToken(jeton, SECRET, T0))
            .toEqual({ ok: true, sujet: 'user-42', type: 'utilisateur' });
    });

    it('REFUSES alg:none, even with an empty signature', () => {
        // La forge classique : l'attaquant met `none` et retire la signature.
        const jeton = forger({ alg: 'none', typ: 'JWT' }, { sub: 'intrus', exp: T0 + 10_000 }, '');
        expect(verifyToken(jeton, SECRET, T0)).toEqual({ ok: false, motif: 'algorithme' });
    });

    it('REFUSES alg:RS256, whose HS256 signature is nevertheless VALID', () => {
        // 🔴 Ce jeton-ci porte une signature HMAC correcte : SEUL le contrôle
        // de l'`alg` peut le refuser. Un vérificateur qui ne comparerait pas
        // l'algorithme l'accepterait, et ce test est le seul à le voir.
        const jeton = forger({ alg: 'RS256', typ: 'JWT' }, { sub: 'intrus', exp: T0 + 10_000 });
        expect(verifyToken(jeton, SECRET, T0)).toEqual({ ok: false, motif: 'algorithme' });
    });

    it('REFUSES a signature with one changed character', () => {
        const jeton = signer('user-42', SECRET, T0);
        const [tete, charge, signature] = jeton.split('.');
        const abimee = (signature[0] === 'A' ? 'B' : 'A') + signature.slice(1);
        expect(verifyToken(`${tete}.${charge}.${abimee}`, SECRET, T0))
            .toEqual({ ok: false, motif: 'signature' });
    });

    it('expires on a clock that VARIES: accepted at t0+d/2, refused at t0+d and beyond', () => {
        // 🔴 TROIS instants distincts, et c'est le point : une horloge figée
        // rendrait ce test inerte, ce qui est exactement ce que la colonne
        // ROUGE du critère ② interdit.
        const duree = 60_000;
        const jeton = signer('user-42', SECRET, T0, duree);
        expect(verifyToken(jeton, SECRET, T0 + duree / 2)).toEqual({
            ok: true,
            sujet: 'user-42',
            type: 'utilisateur',
        });
        // La borne est FRANCHE : `maintenant >= exp` refuse.
        expect(verifyToken(jeton, SECRET, T0 + duree)).toEqual({ ok: false, motif: 'expire' });
        expect(verifyToken(jeton, SECRET, T0 + duree + 1)).toEqual({ ok: false, motif: 'expire' });
    });

    it('REFUSES an invalid shape without ever THROWING', () => {
        // Un `JSON.parse` qui lève ici ferait répondre 500 à l'appelant HTTP,
        // là où il doit répondre 401 — et l'écart serait à lui seul un oracle.
        const attendu = { ok: false, motif: 'forme' };
        expect(verifyToken('deux.segments', SECRET, T0)).toEqual(attendu);
        expect(verifyToken('###.###.###', SECRET, T0)).toEqual(attendu);
        // Une charge qui est un nombre, pas un objet : `JSON.parse` réussit.
        expect(verifyToken(forger({ alg: 'HS256', typ: 'JWT' }, 42), SECRET, T0)).toEqual(attendu);
        // Et tout ce qui n'est même pas une chaîne.
        expect(verifyToken(undefined, SECRET, T0)).toEqual(attendu);
        expect(verifyToken(null, SECRET, T0)).toEqual(attendu);
        expect(verifyToken({ jeton: 'x' }, SECRET, T0)).toEqual(attendu);
    });

    it('signer THROWS on a secret shorter than LONGUEUR_SECRET_MIN', () => {
        // Sans ce refus, une plateforme se déploierait avec un secret
        // devinable, et rien ne le dirait.
        expect(MIN_SECRET_LENGTH).toBe(32);
        expect(() => signer('u', 'trop-court', T0)).toThrow(/32/);
        expect(DUREE_JETON_ACCES_MS).toBeGreaterThan(0);
    });
    it('a token WITHOUT a type claim counts as « utilisateur » — P2 stays in flight', () => {
        // 🔴 La rouge : rendre `undefined`. Tout jeton emis par P2 et encore en
        // vol deviendrait indecidable, ce qu'aucune exigence ne reclame — la
        // seule chose que P3 ajoute est la capacite de DIRE `agent`, pas celle
        // d'invalider ce qui existe.
        const jeton = signer('u1', SECRET, T0);
        const v = verifyToken(jeton, SECRET, T0);
        expect(v).toEqual({ ok: true, sujet: 'u1', type: 'utilisateur' });
    });

    it('a token signed with the « agent » type is re-read as such', () => {
        const jeton = signer('RhH1x2QmTz9kLpVbNc7dAw', SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        expect(verifyToken(jeton, SECRET, T0))
            .toEqual({ ok: true, sujet: 'RhH1x2QmTz9kLpVbNc7dAw', type: 'agent' });
    });

    it('REFUSES a FORGED type claim, with the original signature kept', () => {
        // ⚠️ CE TEST EST FAIBLE, ET IL FAUT LE DIRE PLUTOT QUE DE LE DECOUVRIR.
        // Il passe DEJA sur le code d'avant P3, ou aucun claim n'existe : ce
        // qu'il eprouve reellement est que toute retouche de la charge casse la
        // signature — propriete que P2 avait deja. Le plan de P3 l'annonce
        // comme rouge ; il ne l'est pas, et la mutation qu'il nomme (« porter
        // le claim hors de la charge signee ») N'EST PAS REALISABLE ICI : la
        // signature couvre `entete.charge`, donc l'en-tete AUSSI. Il n'existe
        // aucune position non signee dans ce jeton ou loger un claim.
        //
        // 🔴 CE QUI EPINGLE REELLEMENT L'EMPLACEMENT DU CLAIM est le test
        // suivant, celui du type INCONNU : deplacer le claim vers l'en-tete le
        // rend rouge (« expected { ok: true, sujet: 'u1', …(1) } to deeply
        // equal { ok: false, motif: 'forme' } »), MESURE. Celui-ci reste comme
        // garde de non-regression, a sa juste valeur et pas au-dela.
        const legitime = signer('u1', SECRET, T0);
        const [, , signatureDOrigine] = legitime.split('.');
        const charge = JSON.parse(
            Buffer.from(legitime.split('.')[1], 'base64url').toString('utf8'),
        );
        const promu = forger(
            { alg: 'HS256', typ: 'JWT' },
            { ...charge, sty: 'agent' },
            signatureDOrigine,
        );
        expect(verifyToken(promu, SECRET, T0)).toEqual({ ok: false, motif: 'signature' });
    });

    it('🔴 REFUSES a type of UNKNOWN value, rather than bringing it back to « utilisateur »', () => {
        // 🔴 La rouge : le laisser passer, ou le ramener a `user`. Un
        // jeton de type inconnu deviendrait un jeton humain -- et le jour ou
        // un troisieme type existera, un service ancien l'accepterait comme
        // humain au lieu de le refuser. Le jeton est ici VALIDEMENT SIGNE :
        // seule la valeur du claim est hors du domaine.
        const jeton = forger({ alg: 'HS256', typ: 'JWT' }, {
            sub: 'u1',
            exp: T0 + 10_000,
            sty: 'administrateur',
        });
        expect(verifyToken(jeton, SECRET, T0)).toEqual({ ok: false, motif: 'forme' });
    });
});
