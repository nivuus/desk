import { describe, expect, it, vi } from 'vitest';
import { accesDeReponse, CLE_ACCES, CLE_RAFRAICHISSEMENT, expireAvant, jetonAcces, poser, poserAcces, rafraichirSiNecessaire, vider } from './jeton';
import type { Coffre } from './jeton';

/// Un coffre factice, en mémoire. Il n'y a AUCUN `localStorage` dans
/// l'environnement de test (relevé : `client/` n'a aucun `vitest.config.*`,
/// donc l'environnement est le Node par défaut) — c'est précisément pourquoi
/// le `Coffre` est un paramètre et non un global.
function coffreFactice(initial: Record<string, string> = {}): Coffre & { contenu: Map<string, string> } {
    const contenu = new Map(Object.entries(initial));
    return {
        contenu,
        getItem: (c) => contenu.get(c) ?? null,
        setItem: (c, v) => void contenu.set(c, v),
        removeItem: (c) => void contenu.delete(c),
    };
}

/// Fabrique un jeton de la FORME d'un JWT, avec l'`exp` demandé — en
/// MILLISECONDES, comme le service (`plateforme/src/identite/jeton.ts` déclare
/// cette divergence avec la RFC 7519). La signature est du remplissage : ce
/// module ne la vérifie jamais, et c'est ce que le test `signature` éprouve.
function jetonFactice(expMs: number, signature = 'peu-importe'): string {
    return `${b64url({ alg: 'HS256', typ: 'JWT' })}.${b64url({ sub: 'u-1', exp: expMs })}.${signature}`;
}

/// Encode en base64url avec `btoa`, jamais avec `Buffer` : `client/` n'a pas
/// `@types/node` (relevé : `npm run typecheck` rend `TS2580 Cannot find name
/// 'Buffer'`), et le code testé tourne de toute façon dans un navigateur.
function b64url(valeur: unknown): string {
    const octets = new TextEncoder().encode(JSON.stringify(valeur));
    const binaire = Array.from(octets, (o) => String.fromCharCode(o)).join('');
    return btoa(binaire).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

describe('le coffre à jetons du navigateur', () => {
    it("rend l'accès qui vient d'être posé", () => {
        const coffre = coffreFactice();
        poser(coffre, { acces: 'a-1', rafraichissement: 'r-1' });
        expect(jetonAcces(coffre)).toBe('a-1');
    });

    it("rend `undefined` quand aucun jeton n'a été posé", () => {
        expect(jetonAcces(coffreFactice())).toBeUndefined();
    });

    it('`vider` efface LES DEUX clés', () => {
        // 🔴 Le rouge de ce test est de n'en effacer qu'une : une déconnexion
        // qui laisserait le rafraîchissement derrière elle laisserait un
        // moyen de se reconnecter sans mot de passe, dans un stockage que
        // tout script de la page lit.
        const coffre = coffreFactice();
        poser(coffre, { acces: 'a-1', rafraichissement: 'r-1' });
        vider(coffre);
        expect(coffre.contenu.has(CLE_ACCES)).toBe(false);
        expect(coffre.contenu.has(CLE_RAFRAICHISSEMENT)).toBe(false);
    });
});

describe('poserAcces', () => {
    it("pose l'accès", () => {
        const c = coffreFactice();
        poserAcces(c, 'J');
        expect(c.getItem(CLE_ACCES)).toBe('J');
    });

    // 🔴 CE TEST EST LA RAISON D'ÊTRE DE LA FONCTION. Un jeton de
    // rafraîchissement laissé par un montage `motdepasse` antérieur survivrait
    // au changement de mode et serait présenté à une route qui rend désormais
    // 404 — une panne dont le symptôme serait une déconnexion inexpliquée dix
    // minutes après chaque ouverture de page.
    it('EFFACE le jeton de rafraîchissement laissé par un montage antérieur', () => {
        const c = coffreFactice();
        c.setItem(CLE_RAFRAICHISSEMENT, 'vieux');
        poserAcces(c, 'J');
        expect(c.getItem(CLE_RAFRAICHISSEMENT)).toBeNull();
    });
});

/* ── LA VALIDATION DU CORPS DE `GET /auth/moi` ─────────────────────────────
   🔴 CES TESTS EXISTENT PARCE QUE LA RÈGLE VIVAIT DANS `connexion.ts`, QUI
   N'EST PAS TESTÉ. L'en-tête de ce fichier-là pose le critère qui départage
   une règle d'un câblage — « une condition est une règle si la changer change
   ce que le PRODUIT décide » —, et la garde sur `corps.acces` le franchit :
   sans elle, le produit écrit la chaîne `"undefined"` au coffre, envoie
   `Bearer undefined`, montre une erreur de session au lieu du formulaire, et
   **laisse le coffre empoisonné**. Elle a donc été FAITE DESCENDRE ici, où
   les tests la tiennent.

   🔴 LA ROUGE, JOUÉE — TROIS MUTATIONS, ET ELLES NE ROUGISSENT PAS PAREIL.
   Le premier jet de ce commentaire annonçait « les QUATRE `it()` de refus
   tombent » pour une seule mutation : **c'était faux, et la mesure l'a dit**.
   Relevé le 21 août 2026, `cd client && npx vitest run src/jeton.test.ts`,
   en remplaçant le corps d'`accesDeReponse` par :

     A. `return (corps as {acces?: string} | undefined | null)?.acces;`
        -> **2 échecs** (chaîne vide, non-chaîne). Les cas `{}`, `undefined`,
           `null` et `'J'` restent VERTS : le chaînage optionnel rend déjà
           `undefined` pour eux, donc ces tests-là ne discriminent pas CETTE
           mutation.
     B. `return (corps as {acces?: string}).acces;` — le retrait littéral, tel
        que `connexion.ts` portait la garde
        -> **3 échecs**, le troisième par `TypeError: Cannot read properties
           of undefined`.
     C. `return String((corps as {acces?: string} | undefined | null)?.acces);`
        — **la reproduction du défaut RÉEL du produit**, celui qui écrit la
        chaîne `"undefined"` au coffre
        -> **4 échecs**, les quatre `it()` de refus.

   🔴 CE QUE CETTE DISPERSION ENSEIGNE, ET POURQUOI ELLE EST ÉCRITE ICI PLUTÔT
   QUE LISSÉE : le test du corps `{}` **ne peut pas** rougir sur un simple
   retrait de garde — un accès à une clé absente rend `undefined` de toute
   façon. Il ne gagne sa valeur que contre la mutation C, c'est-à-dire contre
   le défaut qu'on cherche réellement à empêcher. Annoncer « les quatre
   tombent » sans dire SOUS QUELLE mutation aurait été exactement le patron
   que `CLAUDE.md` appelle « un contrôle qu'on n'a jamais vu rouge ».

   ⚠️ DANS LES TROIS CAS, LE PREMIER `it()` — celui qui éprouve l'ACCEPTATION
   — reste VERT. C'est ce qui rend chaque rouge discriminante : elle ne dénonce
   pas un module débranché. */
describe('le corps de `GET /auth/moi`', () => {
    it("rend le jeton quand le corps en porte un", () => {
        expect(accesDeReponse({ acces: 'J' })).toBe('J');
    });

    // Le cas EXACT que le mode `motdepasse` produirait si le 404 ne portait
    // pas le mode : un corps sans `acces`.
    it("rend `undefined` sur un corps SANS `acces` — sinon le coffre reçoit la chaîne « undefined »", () => {
        expect(accesDeReponse({})).toBeUndefined();
    });

    // ⚠️ DISTINCT DU CAS CI-DESSUS, ET NON REDONDANT : `typeof '' === 'string'`.
    // Un `''` posé au coffre serait un jeton qu'aucun `Authorization` ne peut
    // porter, et `jetonAcces` le rendrait comme s'il valait quelque chose.
    it('rend `undefined` sur une chaîne VIDE', () => {
        expect(accesDeReponse({ acces: '' })).toBeUndefined();
    });

    it("rend `undefined` quand `acces` n'est pas une chaîne", () => {
        expect(accesDeReponse({ acces: 42 })).toBeUndefined();
        expect(accesDeReponse({ acces: null })).toBeUndefined();
    });

    // `reponse.json().catch(() => undefined)` rend `undefined` sur un corps
    // illisible, et `null` est un JSON parfaitement valable : les deux
    // atteignent cette fonction, et ni l'un ni l'autre ne doit la faire lever.
    it('rend `undefined` sur `undefined`, `null` et un corps qui n’est pas un objet', () => {
        expect(accesDeReponse(undefined)).toBeUndefined();
        expect(accesDeReponse(null)).toBeUndefined();
        expect(accesDeReponse('J')).toBeUndefined();
    });
});

describe('la fraîcheur, lue SANS vérifier la signature', () => {
    it("lit `exp` d'un jeton dont la SIGNATURE est fausse, et ne le refuse pas", () => {
        // 🔴 C'est la moitié décidable de « le navigateur ne vérifie jamais » :
        // ce jeton porte une signature qui n'est celle de personne, et
        // `expireAvant` rend quand même `false` parce que son `exp` est loin.
        // Le rouge est de prétendre vérifier — le client n'a pas le secret, et
        // croire qu'il vérifie serait pire que savoir qu'il ne le fait pas.
        const jeton = jetonFactice(10_000, 'signature-qui-n-est-celle-de-personne');
        expect(expireAvant(jeton, 5_000)).toBe(false);
    });

    it('rend `true` quand `exp` est déjà passé', () => {
        expect(expireAvant(jetonFactice(1_000), 5_000)).toBe(true);
    });

    it('rend `true` sur un jeton MAL FORMÉ', () => {
        // Le rouge est de rendre `false` : un jeton illisible serait alors cru
        // valable, et la session échouerait plus tard, ailleurs, sur un refus
        // du service que rien ne relierait à cette lecture.
        expect(expireAvant('pas-un-jeton', 0)).toBe(true);
        expect(expireAvant('a.b.c', 0)).toBe(true);
        expect(expireAvant(`${b64url({})}.${b64url({})}.x`, 0)).toBe(true);
    });
});

describe('le rafraîchissement', () => {
    it("N'APPELLE PAS le réseau quand le jeton est frais au-delà de la marge", async () => {
        // 🔴 Le rouge est d'appeler toujours : un aller-retour réseau par
        // ouverture de fenêtre, sur un chemin qui n'a rien à faire.
        const coffre = coffreFactice({
            [CLE_ACCES]: jetonFactice(100_000),
            [CLE_RAFRAICHISSEMENT]: 'r-1',
        });
        const appel = vi.fn();
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, appel)).toBe(true);
        expect(appel).not.toHaveBeenCalled();
    });

    it('appelle, pose la paire neuve et rend `true` quand la marge est franchie', async () => {
        const coffre = coffreFactice({
            [CLE_ACCES]: jetonFactice(10_000),
            [CLE_RAFRAICHISSEMENT]: 'r-1',
        });
        // L'appel est INJECTÉ, jamais `fetch` global : avec `fetch`, ce test
        // exigerait un réseau et cesserait d'être un test.
        const appel = vi.fn(async () => ({ acces: 'a-2', rafraichissement: 'r-2' }));
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, appel)).toBe(true);
        expect(appel).toHaveBeenCalledWith({ rafraichissement: 'r-1' });
        expect(coffre.contenu.get(CLE_ACCES)).toBe('a-2');
        expect(coffre.contenu.get(CLE_RAFRAICHISSEMENT)).toBe('r-2');
    });

    it("un refus de l'appel VIDE le coffre et rend `false`", async () => {
        // Le rouge est de garder la paire morte : l'utilisateur boucle alors
        // sur un refus sans jamais revoir l'écran de connexion.
        const coffre = coffreFactice({
            [CLE_ACCES]: jetonFactice(10_000),
            [CLE_RAFRAICHISSEMENT]: 'r-1',
        });
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, async () => undefined)).toBe(false);
        expect(coffre.contenu.size).toBe(0);
    });

    it('rend `false` sans appeler quand aucun rafraîchissement n’est stocké', async () => {
        const coffre = coffreFactice({ [CLE_ACCES]: jetonFactice(10_000) });
        const appel = vi.fn();
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, appel)).toBe(false);
        expect(appel).not.toHaveBeenCalled();
    });
});
