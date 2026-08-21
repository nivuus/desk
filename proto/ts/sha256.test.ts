import { describe, expect, it } from 'vitest';
import { Sha256, condenserHex } from './sha256';

/** L'octet-à-hexadécimal de l'ORACLE — jamais celui du module sous test. */
function hexDe(tampon: ArrayBuffer): string {
    return Array.from(new Uint8Array(tampon))
        .map((octet) => octet.toString(16).padStart(2, '0'))
        .join('');
}

/** Ce que le NAVIGATEUR répond, en une fois. C'est l'oracle indépendant. */
async function empreinteDeReference(message: Uint8Array): Promise<string> {
    // ⚠️ La copie n'est pas superflue : `crypto.subtle.digest` exige un
    // `BufferSource` adossé à un `ArrayBuffer`, quand un `Uint8Array` quelconque
    // peut l'être à un `SharedArrayBuffer` — que `tsc` refuse d'écarter. Une
    // assertion de type le tairait ; la copie le règle, et l'oracle reste un
    // oracle.
    const copie = new Uint8Array(new ArrayBuffer(message.length));
    copie.set(message);
    return hexDe(await crypto.subtle.digest('SHA-256', copie));
}

/**
 * Un générateur déterministe (xorshift32), et il n'est pas décoratif.
 *
 * ⚠️ Un message de N octets tous identiques laisserait passer une faute qui
 * mélange deux positions du bloc — le contenu étant le même partout, l'échange
 * serait invisible. Le contenu doit donc VARIER, et il doit être REPRODUCTIBLE
 * pour qu'un échec se rejoue à l'identique : d'où une graine fixe plutôt que
 * `Math.random`.
 */
function messageDeTaille(taille: number): Uint8Array {
    const octets = new Uint8Array(taille);
    let etat = 0x9e3779b9 ^ taille;
    for (let i = 0; i < taille; i += 1) {
        etat ^= etat << 13;
        etat ^= etat >>> 17;
        etat ^= etat << 5;
        octets[i] = etat & 0xff;
    }
    return octets;
}

function texte(chaine: string): Uint8Array {
    return new TextEncoder().encode(chaine);
}

/** Absorbe `message` en morceaux de `taille` octets, puis clôt. */
function empreinteParMorceaux(message: Uint8Array, taille: number): string {
    const empreinte = new Sha256();
    for (let i = 0; i < message.length; i += taille) {
        empreinte.absorber(message.subarray(i, Math.min(i + taille, message.length)));
    }
    return empreinte.terminer();
}

describe('Sha256, les vecteurs de réponse connue', () => {
    /**
     * 🔴 CE SONT EUX QUI FONT DE CE MODULE AUTRE CHOSE QU'UNE PROMESSE, et ils
     * sont recopiés de FIPS 180-4, **pas** produits par notre code. Une
     * constante mal transcrite, une rotation à l'envers, un bourrage qui oublie
     * son second bloc : les trois tombent.
     *
     * ⚠️ Ils sont identiques, caractère pour caractère, à ceux du jumeau Rust
     * (`agent/src/apps/sha256.rs`) — c'est ce qui rattraperait une divergence
     * entre les deux implémentations, qui doivent rendre la même identité pour
     * le même contenu.
     */
    it('rend les trois empreintes de FIPS 180-4', () => {
        // §D.1 : le message vide, dont tout le bloc est du bourrage.
        expect(condenserHex(texte(''))).toBe(
            'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
        );
        // §B.1 : « abc », un seul bloc.
        expect(condenserHex(texte('abc'))).toBe(
            'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
        );
        // §B.2 : 448 bits — 56 octets, donc la longueur ne tient PAS dans le
        // bloc de bourrage et il en faut un SECOND. C'est le seul vecteur de la
        // norme qui exerce cette branche.
        expect(
            condenserHex(texte('abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq')),
        ).toBe('248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1');
    });
});

/**
 * Les tailles de message éprouvées, choisies sur les frontières du bourrage.
 *
 * 55 est le dernier octet où la longueur tient encore dans le bloc, 56 le
 * premier qui en exige un second, 64 un bloc plein dont le bourrage occupe
 * tout un bloc de plus. Les grandes tailles, elles, éprouvent l'enchaînement.
 */
const TAILLES = [0, 1, 3, 55, 56, 57, 63, 64, 65, 127, 128, 129, 1000, 4096, 100_000];

/**
 * 🔴 LES DÉCOUPAGES, ET C'EST LE CŒUR DE CE FICHIER.
 *
 * Le tampon résiduel de `Sha256` est la seule partie du module que les vecteurs
 * de FIPS n'éprouvent PAS : `condenserHex` absorbe tout d'un coup, donc ne
 * laisse jamais de résidu à reporter d'un morceau au suivant. Or c'est
 * exactement là que vivent les fautes d'un condensat incrémental — et c'est
 * exactement ce qu'un flux de fichier produira, puisqu'un `ReadableStream` rend
 * ce qu'il veut et jamais des multiples de 64.
 *
 * Un test qui n'absorberait qu'en blocs de 64 ne prouverait donc presque rien :
 * il court-circuiterait le résidu à chaque morceau. Les tailles ci-dessous sont
 * choisies pour qu'il soit franchi dans tous ses régimes — plus petit qu'un
 * bloc (1, 63), pile un bloc (64), à cheval (65), bien plus grand (1000).
 */
const DECOUPES = [1, 63, 64, 65, 1000];

describe('Sha256 confronté à crypto.subtle', () => {
    /**
     * 🔵 L'ORACLE EST INDÉPENDANT DE NOTRE CODE, et c'est ce qui donne son poids
     * à ce test : `crypto.subtle.digest` est l'implémentation du moteur, écrite
     * par d'autres, en natif. Comparer notre condensat au sien ne peut pas être
     * satisfait par une faute que nous aurions commise deux fois — à la
     * différence d'un aller-retour de notre code contre lui-même.
     */
    it.each(TAILLES)('concorde sur un message de %i octets absorbé d’un coup', async (taille) => {
        const message = messageDeTaille(taille);
        expect(condenserHex(message)).toBe(await empreinteDeReference(message));
    });

    it.each(DECOUPES)('concorde sur tous les messages absorbés par morceaux de %i octets', async (decoupe) => {
        for (const taille of TAILLES) {
            const message = messageDeTaille(taille);
            expect(empreinteParMorceaux(message, decoupe), `taille ${taille}, morceaux de ${decoupe}`).toBe(
                await empreinteDeReference(message),
            );
        }
    });

    /**
     * Le découpage IRRÉGULIER, celui qu'aucune taille fixe ne reproduit : les
     * morceaux changent de longueur d'un appel au suivant, comme le ferait un
     * flux réel. C'est le seul cas où le résidu est repris à des décalages
     * chaque fois différents.
     */
    it('concorde sur un découpage aux longueurs variables', async () => {
        const longueurs = [1, 7, 64, 2, 63, 65, 128, 3, 55, 56, 1, 200, 9];
        for (const taille of TAILLES) {
            const message = messageDeTaille(taille);
            const empreinte = new Sha256();
            let i = 0;
            let n = 0;
            while (i < message.length) {
                const pris = Math.min(longueurs[n % longueurs.length], message.length - i);
                empreinte.absorber(message.subarray(i, i + pris));
                i += pris;
                n += 1;
            }
            expect(empreinte.terminer(), `taille ${taille}`).toBe(await empreinteDeReference(message));
        }
    });

    it('absorbe un morceau vide sans rien changer', async () => {
        const message = messageDeTaille(200);
        const empreinte = new Sha256();
        empreinte.absorber(new Uint8Array(0));
        empreinte.absorber(message.subarray(0, 70));
        empreinte.absorber(new Uint8Array(0));
        empreinte.absorber(message.subarray(70));
        empreinte.absorber(new Uint8Array(0));
        expect(empreinte.terminer()).toBe(await empreinteDeReference(message));
    });
});

describe('Sha256, le contrat de l’objet', () => {
    it('lève si l’on absorbe après avoir terminé', () => {
        const empreinte = new Sha256();
        empreinte.absorber(texte('abc'));
        empreinte.terminer();
        // Poursuivre rendrait une empreinte silencieusement fausse : elle doit
        // lever, jamais mentir.
        expect(() => empreinte.absorber(texte('def'))).toThrow(/clos/);
    });

    it('rend la même empreinte à chaque appel de terminer', () => {
        const empreinte = new Sha256();
        empreinte.absorber(texte('abc'));
        const premier = empreinte.terminer();
        expect(empreinte.terminer()).toBe(premier);
        expect(premier).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
    });

    it('rend 64 caractères hexadécimaux minuscules', () => {
        for (const taille of TAILLES) {
            expect(condenserHex(messageDeTaille(taille))).toMatch(/^[0-9a-f]{64}$/);
        }
    });

    it('rend des empreintes distinctes pour 130 longueurs distinctes', () => {
        // Un bourrage cassé — un `<` pour un `<=`, une longueur écrite au
        // mauvais décalage — ferait collisionner deux tailles voisines.
        const toutes = new Set<string>();
        for (let n = 0; n < 130; n += 1) toutes.add(condenserHex(new Uint8Array(n).fill(0x61)));
        expect(toutes.size).toBe(130);
    });
});
