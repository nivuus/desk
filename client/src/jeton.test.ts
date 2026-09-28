import { describe, expect, it, vi } from 'vitest';
import { accesDeReponse, accesParPomerium, assurerAccesFrais, CLE_ACCES, CLE_RAFRAICHISSEMENT, expiresBefore, jetonAcces, paireDeReponse, poser, poserAcces, rafraichirSiNecessaire, drain } from './jeton';
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
function b64url(value: unknown): string {
    const octets = new TextEncoder().encode(JSON.stringify(value));
    const binaire = Array.from(octets, (o) => String.fromCharCode(o)).join('');
    return btoa(binaire).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

describe('the browser token store', () => {
    it("returns the access that was just set", () => {
        const coffre = coffreFactice();
        poser(coffre, { acces: 'a-1', rafraichissement: 'r-1' });
        expect(jetonAcces(coffre)).toBe('a-1');
    });

    it("returns `undefined` when no token was set", () => {
        expect(jetonAcces(coffreFactice())).toBeUndefined();
    });

    it('`vider` erases BOTH keys', () => {
        // 🔴 Le rouge de ce test est de n'en effacer qu'une : une déconnexion
        // qui laisserait le rafraîchissement derrière elle laisserait un
        // moyen de se reconnecter sans mot de passe, dans un stockage que
        // tout script de la page lit.
        const coffre = coffreFactice();
        poser(coffre, { acces: 'a-1', rafraichissement: 'r-1' });
        drain(coffre);
        expect(coffre.contenu.has(CLE_ACCES)).toBe(false);
        expect(coffre.contenu.has(CLE_RAFRAICHISSEMENT)).toBe(false);
    });
});

describe('poserAcces', () => {
    it("sets the access", () => {
        const c = coffreFactice();
        poserAcces(c, 'J');
        expect(c.getItem(CLE_ACCES)).toBe('J');
    });

    // 🔴 CE TEST EST LA RAISON D'ÊTRE DE LA FONCTION. Un jeton de
    // rafraîchissement laissé par un montage `motdepasse` antérieur survivrait
    // au changement de mode et serait présenté à une route qui rend désormais
    // 404 — une panne dont le symptôme serait une déconnexion inexpliquée dix
    // minutes après chaque ouverture de page.
    it('ERASES the refresh token left by an earlier mount', () => {
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
describe('the body of `GET /auth/moi`', () => {
    it("returns the token when the body carries one", () => {
        expect(accesDeReponse({ acces: 'J' })).toBe('J');
    });

    // Le cas EXACT que le mode `motdepasse` produirait si le 404 ne portait
    // pas le mode : un corps sans `acces`.
    it("returns `undefined` on a body WITHOUT `acces` — otherwise the store receives the string « undefined »", () => {
        expect(accesDeReponse({})).toBeUndefined();
    });

    // ⚠️ DISTINCT DU CAS CI-DESSUS, ET NON REDONDANT : `typeof '' === 'string'`.
    // Un `''` posé au coffre serait un jeton qu'aucun `Authorization` ne peut
    // porter, et `jetonAcces` le rendrait comme s'il valait quelque chose.
    it('returns `undefined` on an EMPTY string', () => {
        expect(accesDeReponse({ acces: '' })).toBeUndefined();
    });

    it("returns `undefined` when `acces` is not a string", () => {
        expect(accesDeReponse({ acces: 42 })).toBeUndefined();
        expect(accesDeReponse({ acces: null })).toBeUndefined();
    });

    // `reponse.json().catch(() => undefined)` rend `undefined` sur un corps
    // illisible, et `null` est un JSON parfaitement valable : les deux
    // atteignent cette fonction, et ni l'un ni l'autre ne doit la faire lever.
    it('returns `undefined` on `undefined`, `null` and a body that is not an object', () => {
        expect(accesDeReponse(undefined)).toBeUndefined();
        expect(accesDeReponse(null)).toBeUndefined();
        expect(accesDeReponse('J')).toBeUndefined();
    });
});

/// 🔴 AJOUTÉ EN CORRECTION DE REVUE (round 1) : `paireDeReponse` réutilise
/// `accesDeReponse` pour la moitié `acces` — ses cas sont donc redémontrés
/// une seule fois ici, pas répétés en entier — et applique le MÊME critère
/// (chaîne, non vide) à `rafraichissement`, sur le modèle exact des tests
/// ci-dessus.
describe('the body of `POST /auth/rafraichir`', () => {
    it('returns the pair when the body carries a complete one', () => {
        expect(paireDeReponse({ acces: 'A', rafraichissement: 'R' })).toEqual({
            acces: 'A',
            rafraichissement: 'R',
        });
    });

    it('returns `undefined` when `acces` is absent, empty or not a string — through `accesDeReponse`', () => {
        expect(paireDeReponse({ rafraichissement: 'R' })).toBeUndefined();
        expect(paireDeReponse({ acces: '', rafraichissement: 'R' })).toBeUndefined();
        expect(paireDeReponse({ acces: 42, rafraichissement: 'R' })).toBeUndefined();
    });

    it("returns `undefined` when `rafraichissement` is ABSENT — the case the review found missing", () => {
        expect(paireDeReponse({ acces: 'A' })).toBeUndefined();
    });

    // ⚠️ DISTINCT DU CAS CI-DESSUS, ET NON REDONDANT : `typeof '' === 'string'`,
    // le même piège que celui d'`accesDeReponse`, appliqué ici au second champ.
    it('returns `undefined` on an EMPTY `rafraichissement`', () => {
        expect(paireDeReponse({ acces: 'A', rafraichissement: '' })).toBeUndefined();
    });

    it("returns `undefined` when `rafraichissement` is not a string", () => {
        expect(paireDeReponse({ acces: 'A', rafraichissement: 42 })).toBeUndefined();
        expect(paireDeReponse({ acces: 'A', rafraichissement: null })).toBeUndefined();
    });

    it('returns `undefined` on `undefined`, `null` and a body that is not an object', () => {
        expect(paireDeReponse(undefined)).toBeUndefined();
        expect(paireDeReponse(null)).toBeUndefined();
        expect(paireDeReponse('J')).toBeUndefined();
    });
});

describe('freshness, read WITHOUT checking the signature', () => {
    it("reads `exp` from a token whose SIGNATURE is wrong, and does not refuse it", () => {
        // 🔴 C'est la moitié décidable de « le navigateur ne vérifie jamais » :
        // ce jeton porte une signature qui n'est celle de personne, et
        // `expiresBefore` rend quand même `false` parce que son `exp` est loin.
        // Le rouge est de prétendre vérifier — le client n'a pas le secret, et
        // croire qu'il vérifie serait pire que savoir qu'il ne le fait pas.
        const jeton = jetonFactice(10_000, 'signature-that-is-no-one-s');
        expect(expiresBefore(jeton, 5_000)).toBe(false);
    });

    it('returns `true` when `exp` has already passed', () => {
        expect(expiresBefore(jetonFactice(1_000), 5_000)).toBe(true);
    });

    it('returns `true` on a MALFORMED token', () => {
        // Le rouge est de rendre `false` : un jeton illisible serait alors cru
        // valable, et la session échouerait plus tard, ailleurs, sur un refus
        // du service que rien ne relierait à cette lecture.
        expect(expiresBefore('not-a-token', 0)).toBe(true);
        expect(expiresBefore('a.b.c', 0)).toBe(true);
        expect(expiresBefore(`${b64url({})}.${b64url({})}.x`, 0)).toBe(true);
    });
});

describe('the refresh', () => {
    it("DOES NOT CALL the network when the token is fresh beyond the margin", async () => {
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

    it('calls, sets the new pair and returns `true` when the margin is crossed', async () => {
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

    it("a refusal of the call EMPTIES the store and returns `false`", async () => {
        // Le rouge est de garder la paire morte : l'utilisateur boucle alors
        // sur un refus sans jamais revoir l'écran de connexion.
        const coffre = coffreFactice({
            [CLE_ACCES]: jetonFactice(10_000),
            [CLE_RAFRAICHISSEMENT]: 'r-1',
        });
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, async () => undefined)).toBe(false);
        expect(coffre.contenu.size).toBe(0);
    });

    it('returns `false` without calling when no refresh is stored', async () => {
        const coffre = coffreFactice({ [CLE_ACCES]: jetonFactice(10_000) });
        const appel = vi.fn();
        expect(await rafraichirSiNecessaire(coffre, 0, 30_000, appel)).toBe(false);
        expect(appel).not.toHaveBeenCalled();
    });
});

/* ── L'ACCÈS AUTOMATIQUE — CE QUI MANQUAIT, TROUVÉ EN PRODUCTION LE 30 AOÛT
   2026 ────────────────────────────────────────────────────────────────────
   Le hub (`hub/page.ts`) se contentait de LIRE le coffre et de se plaindre
   s'il était vide ("Aucun jeton : connectez-vous d'abord.") ; le seul code
   qui savait obtenir un jeton par Pomerium (`connexion.ts::tenterPomerium`)
   ne courait qu'au CHARGEMENT DE LA PAGE DE CONNEXION. Tant que la racine
   servait la page de session, personne n'avait vu un visiteur atterrir
   DIRECTEMENT sur le hub sans être passé par cet écran — le lot qui a mis le
   hub à la racine avait vérifié que `/` SERT le hub, jamais qu'un visiteur
   SANS JETON puisse s'en servir. `accesParPomerium` et `assurerAcces`
   n'existaient pas : c'est CE QUE ce bloc rougit, avant toute implémentation
   — `accesParPomerium` et `assurerAcces` sont absents de l'export de
   `./jeton` sur le produit d'aujourd'hui, donc cet `import`, à lui seul,
   fait échouer TOUT le fichier (voir le rapport de tâche pour la sortie
   réelle de cette rougeur). */
function appelFactice(
    reponses: { ok?: boolean; corps?: unknown; leve?: boolean }[],
): (url: string) => Promise<{ ok: boolean; json(): Promise<unknown> }> {
    let i = 0;
    return vi.fn(async () => {
        const r = reponses[Math.min(i, reponses.length - 1)];
        i += 1;
        if (r.leve === true) throw new Error('network unreachable');
        return { ok: r.ok ?? false, json: async () => r.corps };
    });
}

describe('accesParPomerium — the `tenterPomerium` path, extracted', () => {
    it("returns the token when `/auth/moi` answers 200 with a valid body", async () => {
        const appel = appelFactice([{ ok: true, corps: { acces: 'J' } }]);
        expect(await accesParPomerium('https://h', appel)).toBe('J');
        expect(appel).toHaveBeenCalledWith('https://h/auth/moi');
    });

    it("returns `undefined` on the 404 THAT `routes-identite.ts` RETURNS IN motdepasse MODE", async () => {
        // 🔴 CE CAS PORTE LE MODE JUSQU'ICI (en-tête de `connexion.ts`) : un
        // 404 n'est pas une panne, c'est le service qui dit « ce montage
        // authentifie par mot de passe ». Le prendre pour une panne serait
        // déjà correct ICI (les deux rendent `undefined`) ; c'est le SENS
        // qui diffère, et il n'a besoin d'aucune branche de plus.
        const appel = appelFactice([{ ok: false }]);
        expect(await accesParPomerium('https://h', appel)).toBeUndefined();
    });

    it('returns `undefined` on an unreachable network, without throwing', async () => {
        const appel = appelFactice([{ leve: true }]);
        expect(await accesParPomerium('https://h', appel)).toBeUndefined();
    });

    it("returns `undefined` on a body without a usable `acces`", async () => {
        const appel = appelFactice([{ ok: true, corps: {} }]);
        expect(await accesParPomerium('https://h', appel)).toBeUndefined();
    });
});

describe('assurerAccesFrais', () => {
    /// Un jeton dont `exp` vaut `expMs`. La signature n'est pas vérifiée par
    /// le navigateur (voir `expiresBefore`), donc un en-tête et une signature
    /// factices suffisent — c'est ce que font déjà les tests d'`expiresBefore`.
    function jetonExpirantA(expMs: number): string {
        const charge = btoa(JSON.stringify({ exp: expMs })).replace(/=+$/, '');
        return `x.${charge}.y`;
    }

    function vaultWith(entrees: Record<string, string>): Coffre {
        const carte = new Map(Object.entries(entrees));
        return {
            getItem: (c) => carte.get(c) ?? null,
            setItem: (c, v) => void carte.set(c, v),
            removeItem: (c) => void carte.delete(c),
        };
    }

    it('a fresh token is returned WITHOUT any network call', async () => {
        const coffre = vaultWith({ [CLE_ACCES]: jetonExpirantA(100_000) });
        let appels = 0;
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => { appels += 1; return { ok: false, json: async () => ({}) }; },
            0,
            async () => { appels += 1; return undefined; },
        );
        // 🔴 LE ZERO D'APPELS EST LE SUJET DU TEST, ET IL EST SEUL :
        // `expect` s'arrete au premier echec, donc une assertion qui compte
        // ne se place jamais en seconde position.
        expect(appels).toBe(0);
        expect(acces).toBe(jetonExpirantA(100_000));
    });

    it('a token expiring WITHIN THE MARGIN is treated as expired', async () => {
        // `exp` = 20 s, marge = 30 s, maintenant = 0 : encore valide a
        // l'instant meme, deja perime au sens de la marge.
        const coffre = vaultWith({ [CLE_ACCES]: jetonExpirantA(20_000) });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            0,
            async () => undefined,
        );
        expect(acces).toBe('FRAIS');
    });

    it('an expired token with a refresh goes through the refresh, NOT through Pomerium', async () => {
        const coffre = vaultWith({
            [CLE_ACCES]: jetonExpirantA(0),
            [CLE_RAFRAICHISSEMENT]: 'R',
        });
        let pomerium = 0;
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => { pomerium += 1; return { ok: true, json: async () => ({ acces: 'PAR-POMERIUM' }) }; },
            10_000,
            async () => ({ acces: jetonExpirantA(999_000), rafraichissement: 'R2' }),
        );
        expect(pomerium).toBe(0);
        expect(acces).toBe(jetonExpirantA(999_000));
    });

    it('without a refresh token, Pomerium takes over and the token is SET', async () => {
        const coffre = vaultWith({ [CLE_ACCES]: jetonExpirantA(0) });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            10_000,
            async () => undefined,
        );
        expect(acces).toBe('FRAIS');
        // Pose, sinon le rechargement suivant repaierait l'aller-retour.
        expect(coffre.getItem(CLE_ACCES)).toBe('FRAIS');
    });

    it('both ways fail: returns undefined AND empties the store', async () => {
        const coffre = vaultWith({ [CLE_ACCES]: jetonExpirantA(0) });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: false, json: async () => ({}) }),
            10_000,
            async () => undefined,
        );
        expect(acces).toBeUndefined();
        // 🔴 LE COFFRE EST VIDE, ET C'EST LE POINT : un acces perime laisse
        // en place ferait echouer la poignee de main plus tard, ailleurs, sur
        // un refus que rien ne relierait a ici.
        expect(coffre.getItem(CLE_ACCES)).toBeNull();
    });

    it('an EMPTY store goes straight to Pomerium', async () => {
        const coffre = vaultWith({});
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            0,
            async () => undefined,
        );
        expect(acces).toBe('FRAIS');
    });

    // 🔴 AJOUTES EN CORRECTION DE REVUE (round 1) : le chemin de
    // rafraichissement etait le PREMIER appelant de production de
    // `rafraichirSiNecessaire`, et rien n'eprouvait ni une exception ni un
    // corps malforme avant cette ronde.
    it('a refresh callback that THROWS does NOT make assurerAccesFrais throw: goes through Pomerium', async () => {
        // Le rouge serait l'exception qui remonte non rattrapee, au lieu de
        // retomber sur l'etape suivante (Pomerium), et la page resterait
        // bloquee sur "identification...".
        const coffre = vaultWith({
            [CLE_ACCES]: jetonExpirantA(0),
            [CLE_RAFRAICHISSEMENT]: 'R',
        });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            10_000,
            async () => { throw new Error('network unreachable'); },
        );
        expect(acces).toBe('FRAIS');
    });

    // Le callback est cable comme `hub/page.ts` le fait reellement : il passe
    // le corps recu par `paireDeReponse` avant de le rendre. Un corps
    // incomplet (`ok: true` mais sans `rafraichissement`, ou avec une chaine
    // vide) doit donc etre rejete PAR LE CALLBACK, jamais ecrit tel quel au
    // coffre par `rafraichirSiNecessaire::poser`.
    it("a MALFORMED refresh body (ok:true, without `rafraichissement`) does NOT poison the store", async () => {
        const coffre = vaultWith({
            [CLE_ACCES]: jetonExpirantA(0),
            [CLE_RAFRAICHISSEMENT]: 'R',
        });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            10_000,
            async () => paireDeReponse({ acces: 'X' }),
        );
        expect(acces).toBe('FRAIS');
        // 🔴 LE POINT DU TEST : le coffre ne doit JAMAIS avoir vu 'X'.
        expect(coffre.getItem(CLE_ACCES)).toBe('FRAIS');
    });

    it("a MALFORMED refresh body (empty `acces`) does NOT poison the store", async () => {
        const coffre = vaultWith({
            [CLE_ACCES]: jetonExpirantA(0),
            [CLE_RAFRAICHISSEMENT]: 'R',
        });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            10_000,
            async () => paireDeReponse({ acces: '', rafraichissement: 'R2' }),
        );
        expect(acces).toBe('FRAIS');
        expect(coffre.getItem(CLE_ACCES)).toBe('FRAIS');
    });
});
