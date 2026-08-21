// Les types de charge utile de la GESTION D'APPLICATIONS, côté TypeScript.
//
// 🔴 EXTRAIT AVANT L'ADDITION, JAMAIS APRÈS, et c'est le miroir exact de ce que
// le sous-bloc G2 a fait côté Rust (`proto/src/plateforme/apps.rs`) — à ceci
// près qu'il l'y a fait APRÈS avoir franchi 500 lignes, et qu'il l'a déclaré.
// `plateforme.ts` était à 475 lignes, marge 25, et le sous-bloc G3 y ajoute
// trois messages, deux énumérations et leurs encodeurs : il aurait franchi.
// La doctrine de `CLAUDE.md` est de rendre la marge par une extraction jouée
// D'AVANCE, jamais par une compression.
//
// 🔴 AUCUNE LIGNE DE COMPORTEMENT N'A CHANGÉ. Les trois types sont transposés
// mot pour mot, et `plateforme.ts` les RÉEXPORTE — de sorte que les dix
// importateurs relevés dans `plateforme/` et `proto/` n'ont pas eu à bouger
// d'un caractère. C'est la même figure que le `pub use apps::{…}` du parent
// Rust.
//
// ⚠️ CE FICHIER NE DOIT IMPORTER NI `node:` NI AUCUN DOM : il est chargé par le
// service ET par le navigateur.

/**
 * Une application telle que l'agent la découvre sur le disque de la VM.
 *
 * ⚠️ `arguments` est BRUT et SENSIBLE À LA CASSE, contrairement à `cible` et
 * `repertoire` qui sont normalisés. Deux chemins Windows qui ne diffèrent que
 * par la casse désignent le même fichier ; deux lignes de commande qui ne
 * diffèrent que par la casse d'un argument sont deux invocations distinctes.
 */
export interface Application {
    /** Empreinte du triplet `(cible, arguments, repertoire)` — l'identité. */
    cle: string;
    /** Le nom du `.lnk`, sans son extension. */
    nom: string;
    /** Le chemin du `.lnk` LUI-MÊME, et c'est lui qu'on lance. */
    chemin: string;
    cible: string;
    /** BRUTS (voir ci-dessus). Vide = `''`, jamais absent. */
    arguments: string;
    repertoire: string;
    /**
     * L'empreinte SHA-256 du PNG de l'icône, en hexadécimal minuscule — ou
     * `null` quand l'extraction a échoué.
     *
     * ⚠️ UNE APPLICATION SANS ICÔNE VAUT MIEUX QU'UNE APPLICATION ABSENTE.
     * `null` n'est pas une erreur, et le champ reste PRÉSENT sur le fil.
     */
    icone: string | null;
    /** Toujours présent. Vaut `'non-mesuree'` quand `icone` est `null`. */
    source_max: SourceMax;
    /**
     * La couleur DOMINANTE de l'icône, en `#rrggbb`, ou `null`.
     *
     * 🔴 C'est la « couleur d'accent » que la conception de ④ demande au §G5,
     * et elle est **PAR APPLICATION** — à ne pas confondre avec celle du
     * sous-projet ①, qui est **par FENÊTRE** et arrive sur le canal WebRTC.
     * Les deux se calculent par la même règle pure ; c'est leur SUJET qui
     * diffère.
     *
     * ⚠️ `null` N'EST PAS UNE ERREUR : une icône trop pâle, trop sombre ou
     * trop transparente n'a pas de dominante. Le manifeste OMET alors
     * `theme_color` plutôt que d'en inventer un.
     */
    accent: string | null;
    /**
     * Les extensions que cette application ouvre — minuscules, **avec** le
     * point, triées et dédupliquées par l'agent.
     *
     * 🔴 DES EXTENSIONS, JAMAIS DES TYPES MIME (décision D13 du plan de G5) :
     * un MIME n'est pas une propriété de la VM, c'est une convention du Web.
     * La carte extension → MIME vit **une seule fois**, côté plateforme, à
     * l'endroit qui écrit le manifeste — la faire voyager doublerait une table
     * en Rust **et** en TypeScript.
     *
     * ⚠️ VIDE EST UN ÉTAT NORMAL : la plupart des applications n'ouvrent aucun
     * type de fichier. Le champ reste PRÉSENT sur le fil.
     */
    associations: string[];
}

/**
 * D'où vient l'image : la plus grande entrée réellement PRÉSENTE dans le
 * répertoire d'icônes de la source.
 *
 * 🔴 CE N'EST PAS LA TAILLE RENDUE. Mesuré le 20 août 2026 sur deux témoins
 * fabriqués (`agent/testdata/g2-temoin-{48,256}.ico`) : un `.ico` ne contenant
 * QU'UNE entrée 48×48, interrogé à 256, rend 256×256 32bpp — par
 * `IShellItemImageFactory` comme par `PrivateExtractIconsW`, sans
 * `SIIGBF_SCALEUP` et MÊME avec `SIIGBF_BIGGERSIZEOK`. Un critère qui
 * comparerait la taille rendue à 256 NE PEUT PAS ÉCHOUER.
 *
 * 🔵 `'non-mesuree'` s'écrit avec un TIRET, jamais un tiret bas : c'est le
 * `rename_all = "kebab-case"` du Rust sur une variante à DEUX MOTS, donc la
 * seule du module dont la convention soit observable.
 */
export type SourceMax = { pixels: number } | 'non-mesuree';


/**
 * Ce qu'un ordre de lancement a réellement fait.
 *
 * 🔴 `raccourci` CONTRE `cible` EST CE QUI REND LE CRITÈRE DE RECETTE
 * DÉCIDABLE : lancer par la cible reconstruite au lieu du `.lnk` passerait un
 * critère qui ne dirait que « quelque chose s'est lancé ».
 *
 * ⚠️ CE N'EST PAS UN `MotifCanal` : deux valeurs de `MotifCanal` FERMENT le
 * socket, et un lancement raté ne doit fermer aucun canal.
 */
export type IssueLancement = 'raccourci' | 'cible' | 'inconnue' | 'echec';

