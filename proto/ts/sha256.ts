// SHA-256 (FIPS 180-4 §6.2) **INCRÉMENTAL**, en JavaScript pur — le jumeau de
// `agent/src/apps/sha256.rs`, avec une raison d'exister de plus.
//
// 🔴 POURQUOI CE MODULE EXISTE ALORS QUE LE NAVIGATEUR SAIT DÉJÀ CONDENSER :
// l'API Web Crypto n'expose que `crypto.subtle.digest(algo, tampon)`, qui prend
// le message **COMPLET** en une fois. Elle n'a ni `update`, ni `digest`
// incrémental, ni rien qui s'en approche — c'est une lacune de l'API, pas un
// oubli de notre part. Empreindre un installeur de 800 Mo par cette voie
// exigerait donc de tenir 800 Mo dans un onglet, en plus de ce que la lecture
// du fichier consomme déjà.
//
// 🔵 LE PRIX DU REMÈDE EST MESURÉ, PAS SUPPOSÉ (Node 24.9.0) :
//
//   | voie                                  | débit         |
//   | ------------------------------------- | ------------- |
//   | ce module, JS pur                     |   74,7 Mo/s   |
//   | `crypto.subtle.digest`, natif         | 1160,6 Mo/s   |
//
// Soit **≈ 11 s pour 800 Mo**. C'est l'arbitrage, écrit plutôt que subi :
// **11 s d'attente sont payables, 800 Mo en mémoire ne le sont pas.** Le jour
// où un navigateur exposerait un condensat incrémental, ce module devrait
// céder la place — et cette phrase est là pour qu'on le sache.
//
// ⚠️ CE TABLEAU EST LE RELEVÉ DU PLAN, ET UNE SECONDE MESURE N'EN CONFIRME
// QU'UNE LIGNE. Rejoué sur ce module une fois, à sa rédaction : **73,9 Mo/s**
// ici (donc 10,8 s pour 800 Mo — la ligne qui porte la décision tient), mais
// **462,4 Mo/s** seulement pour `crypto.subtle`, contre 1160,6 annoncés. Une
// exécution chacun, sans échauffement : aucun taux, et l'écart n'est PAS
// expliqué. **Il ne change rien à l'arbitrage** — c'est la mémoire qui décide,
// pas le rapport des débits —, mais il est déclaré plutôt que lissé.
//
// 🔴 AUCUNE DÉPENDANCE, AUCUN `node:`, AUCUN DOM : c'est un invariant que les
// sous-blocs G1 et G2 tiennent tous deux, et ce module ne l'entame pas. Il
// charge à l'identique dans un navigateur et sous Node — ce qui est aussi la
// condition pour que la comparaison à `crypto.subtle` du fichier de test soit
// un oracle INDÉPENDANT, et non notre propre code relu deux fois.
//
// ⚠️ CE N'EST PAS UNE PRIMITIVE DE SÉCURITÉ ICI : l'empreinte sert d'identité
// stable pour un contenu téléversé, jamais à authentifier quoi que ce soit. Le
// jour où quelque chose d'authentifiant en dépendrait, ce module doit céder la
// place à une implémentation auditée.
//
// La preuve tient aux vecteurs de réponse connue de FIPS 180-4 ET à la
// confrontation à `crypto.subtle` sur des découpages IRRÉGULIERS : un
// algorithme faux rate les premiers, un tampon résiduel faux rate la seconde.

/**
 * Les 64 constantes de ronde, §4.2.3.
 *
 * ⚠️ `Int32Array` et non un tableau ordinaire, et ce n'est pas une coquetterie :
 * il force chaque constante en entier signé 32 bits, ce qui rend l'arithmétique
 * du reste du fichier homogène. Un tableau ordinaire garderait `0x428a2f98`
 * comme un flottant positif, et le mélange des deux représentations est
 * exactement là où naissent les fautes d'un SHA-256 écrit à la main.
 */
const K = new Int32Array([
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);

/** L'état initial, §5.3.3. */
const ETAT_INITIAL = new Int32Array([
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
]);

/**
 * La plus grande taille de message que ce module accepte.
 *
 * ⚠️ La longueur voyage dans le bourrage **en BITS**, donc `octets * 8`. Au-delà
 * de `MAX_SAFE_INTEGER / 8` (2^50, soit 1 Pio) cette multiplication cesse
 * d'être exacte en virgule flottante et le bourrage porterait une longueur
 * FAUSSE — sans que rien ne le dise. La borne est donc GARDÉE, pas seulement
 * nommée : `absorber` lève. Aucun téléversement de navigateur n'en approche.
 */
const OCTETS_MAX = Math.floor(Number.MAX_SAFE_INTEGER / 8);

/** La rotation vers la droite de §3.2, sur 32 bits. */
function rotr(x: number, n: number): number {
    return (x >>> n) | (x << (32 - n));
}

/**
 * Une passe de compression sur les 64 octets à `decalage` dans `octets`.
 *
 * ⚠️ Le bloc est lu **en place**, par décalage, plutôt que découpé en
 * `subarray` : à 74,7 Mo/s ce sont plus d'un million d'objets par seconde
 * évités sur un gros fichier. `w` est fourni par l'appelant pour la même
 * raison — le réallouer par bloc dominerait le coût.
 */
function comprimer(etat: Int32Array, w: Int32Array, octets: Uint8Array, decalage: number): void {
    for (let i = 0; i < 16; i += 1) {
        const d = decalage + i * 4;
        w[i] = (octets[d] << 24) | (octets[d + 1] << 16) | (octets[d + 2] << 8) | octets[d + 3];
    }
    for (let i = 16; i < 64; i += 1) {
        const x = w[i - 15];
        const y = w[i - 2];
        const s0 = rotr(x, 7) ^ rotr(x, 18) ^ (x >>> 3);
        const s1 = rotr(y, 17) ^ rotr(y, 19) ^ (y >>> 10);
        w[i] = (w[i - 16] + s0 + w[i - 7] + s1) | 0;
    }

    let a = etat[0];
    let b = etat[1];
    let c = etat[2];
    let d = etat[3];
    let e = etat[4];
    let f = etat[5];
    let g = etat[6];
    let h = etat[7];
    for (let i = 0; i < 64; i += 1) {
        const s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
        const ch = (e & f) ^ (~e & g);
        // Cinq entiers signés 32 bits : leur somme reste sous 2^34, donc exacte
        // en virgule flottante, et le `| 0` la ramène modulo 2^32 — c'est
        // l'équivalent du `wrapping_add` du jumeau Rust.
        const t1 = (h + s1 + ch + K[i] + w[i]) | 0;
        const s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
        const maj = (a & b) ^ (a & c) ^ (b & c);
        const t2 = (s0 + maj) | 0;
        h = g;
        g = f;
        f = e;
        e = (d + t1) | 0;
        d = c;
        c = b;
        b = a;
        a = (t1 + t2) | 0;
    }

    etat[0] = (etat[0] + a) | 0;
    etat[1] = (etat[1] + b) | 0;
    etat[2] = (etat[2] + c) | 0;
    etat[3] = (etat[3] + d) | 0;
    etat[4] = (etat[4] + e) | 0;
    etat[5] = (etat[5] + f) | 0;
    etat[6] = (etat[6] + g) | 0;
    etat[7] = (etat[7] + h) | 0;
}

/**
 * Un condensat SHA-256 que l'on alimente morceau par morceau.
 *
 * ```ts
 * const empreinte = new Sha256();
 * for await (const morceau of flux) empreinte.absorber(morceau);
 * const hex = empreinte.terminer();
 * ```
 *
 * 🔴 LES MORCEAUX N'ONT AUCUNE TAILLE IMPOSÉE, et c'est tout l'intérêt : un
 * `ReadableStream` de fichier rend ce qu'il veut, jamais des multiples de 64.
 * Ce qui l'absorbe est le tampon résiduel ci-dessous, et c'est la seule partie
 * de ce module que les vecteurs de FIPS n'éprouvent PAS — d'où la confrontation
 * à `crypto.subtle` sur des découpages irréguliers, côté test.
 */
export class Sha256 {
    private readonly etat = Int32Array.from(ETAT_INITIAL);
    /** Le message d'expansion, alloué une fois pour toute la vie de l'objet. */
    private readonly w = new Int32Array(64);
    /** Les octets reçus qui n'ont pas encore complété un bloc de 64. */
    private readonly residu = new Uint8Array(64);
    private residuLongueur = 0;
    /** Le nombre TOTAL d'octets absorbés — c'est lui que le bourrage inscrit. */
    private octets = 0;
    /** L'empreinte, une fois `terminer` appelée. `null` tant qu'elle ne l'est pas. */
    private empreinte: string | null = null;

    /**
     * Absorbe un morceau, de n'importe quelle taille, y compris vide.
     *
     * ⚠️ **Lève si `terminer` a déjà été appelée.** L'état interne a été détruit
     * par le bourrage : continuer rendrait une empreinte silencieusement fausse,
     * et une empreinte fausse qui ne se signale pas est le pire des deux maux
     * pour une identité de contenu.
     */
    absorber(bloc: Uint8Array): void {
        if (this.empreinte !== null) {
            throw new Error('Sha256.absorber après Sha256.terminer : le condensat est clos');
        }
        if (this.octets + bloc.length > OCTETS_MAX) {
            throw new Error(`Sha256 : message de plus de ${OCTETS_MAX} octets, longueur non représentable`);
        }
        this.octets += bloc.length;

        let i = 0;
        // D'abord compléter le résidu, s'il y en a un : tant qu'il n'est pas
        // plein, aucun bloc de l'entrée n'est aligné sur une frontière de 64.
        if (this.residuLongueur > 0) {
            const pris = Math.min(64 - this.residuLongueur, bloc.length);
            this.residu.set(bloc.subarray(0, pris), this.residuLongueur);
            this.residuLongueur += pris;
            i = pris;
            if (this.residuLongueur < 64) return;
            comprimer(this.etat, this.w, this.residu, 0);
            this.residuLongueur = 0;
        }
        // Puis les blocs pleins, lus directement dans l'entrée : aucune copie.
        for (; i + 64 <= bloc.length; i += 64) {
            comprimer(this.etat, this.w, bloc, i);
        }
        // Ce qui reste attend le morceau suivant, ou le bourrage.
        if (i < bloc.length) {
            this.residu.set(bloc.subarray(i), 0);
            this.residuLongueur = bloc.length - i;
        }
    }

    /**
     * Clôt le condensat et rend les 64 caractères hexadécimaux **minuscules**.
     *
     * ⚠️ **Idempotente** : l'empreinte est retenue, et un second appel rend la
     * même valeur plutôt que de lever. C'est l'asymétrie voulue avec `absorber`
     * — relire un résultat est inoffensif, poursuivre un calcul clos ne l'est
     * pas.
     */
    terminer(): string {
        if (this.empreinte !== null) return this.empreinte;

        // Bourrage FIPS 180-4 §5.1.1 : l'octet 0x80, des zéros, puis la
        // longueur en BITS sur 64 bits gros-boutiens. Si le résidu atteint
        // 56 octets, la longueur ne tient plus dans ce bloc et il en faut un
        // SECOND — c'est le cas que le troisième vecteur de FIPS exerce, et le
        // seul endroit de ce fichier où une erreur de borne (`<` contre `<=`)
        // resterait invisible sur des messages courts.
        const queue = new Uint8Array(128);
        queue.set(this.residu.subarray(0, this.residuLongueur), 0);
        queue[this.residuLongueur] = 0x80;
        const taille = this.residuLongueur < 56 ? 64 : 128;

        const bits = this.octets * 8;
        const haut = Math.floor(bits / 4294967296);
        const bas = bits - haut * 4294967296;
        for (let i = 0; i < 4; i += 1) {
            queue[taille - 8 + i] = (haut >>> ((3 - i) * 8)) & 0xff;
            queue[taille - 4 + i] = (bas >>> ((3 - i) * 8)) & 0xff;
        }

        for (let debut = 0; debut < taille; debut += 64) {
            comprimer(this.etat, this.w, queue, debut);
        }

        let sortie = '';
        for (let i = 0; i < 8; i += 1) {
            sortie += (this.etat[i] >>> 0).toString(16).padStart(8, '0');
        }
        this.empreinte = sortie;
        return sortie;
    }
}

/**
 * L'empreinte d'un message tenu en entier, en 64 caractères hexadécimaux
 * minuscules — la commodité qui correspond à `hex` du jumeau Rust.
 *
 * ⚠️ À n'employer que sur ce qui tient déjà en mémoire. Pour un fichier, c'est
 * `Sha256` morceau par morceau, faute de quoi ce module perd sa raison d'être.
 */
export function condenserHex(message: Uint8Array): string {
    const empreinte = new Sha256();
    empreinte.absorber(message);
    return empreinte.terminer();
}
