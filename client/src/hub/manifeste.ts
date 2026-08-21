// La RÈGLE du manifeste PWA par application. PURE : aucun DOM, aucun `fetch`,
// aucune horloge. Elle prend un nom, un identifiant et les octets d'une icône,
// et rend l'objet de manifeste. C'est tout, et c'est ce qui la rend éprouvable
// sur l'hôte.
//
// 🔴 POURQUOI CE MANIFESTE EST CONSTRUIT ICI PLUTÔT QUE SERVI PAR UNE ROUTE.
// Un `<link rel="manifest">` est allé chercher par le navigateur SANS en-tête
// `Authorization`, exactement comme les icônes qu'il nomme — et le sous-projet
// ⑤ ne pose AUCUN cookie, son porteur vivant dans `localStorage`, qui ne
// voyage sur aucune requête que le navigateur émet de lui-même. Servir ce
// manifeste demanderait donc d'ouvrir une route aujourd'hui authentifiée,
// c'est-à-dire une DÉCISION DE SÉCURITÉ que `routes-icone.ts:20-26` laisse
// explicitement au propriétaire du dépôt. La page, elle, est authentifiée :
// elle lit tout par `fetch`, et publie ce qu'elle a lu. **Aucune route ne
// change, aucun contrôle de porteur ne saute.**
//
// 🔴 DEUX CONTRAINTES MESURÉES PAR LA PORTE P0, ET QUE RIEN N'ANNONÇAIT.
// Journaux : `docs/superpowers/plans/journaux-gestion-apps-g5/`.
//
//   ① LES URL DOIVENT ÊTRE ABSOLUES. Sous un manifeste publié en `blob:`, la
//      base de résolution est l'URL `blob:` elle-même, qui n'est pas une base
//      utilisable. Un `start_url: '/shell.html'` — la forme de n'importe quel
//      manifeste servi par HTTP — se fait REFUSER :
//        « property 'start_url' ignored, URL is invalid. »
//        « property 'scope' ignored, URL is invalid. »
//      plus `start-url-not-valid` à l'installabilité, et
//      `beforeinstallprompt` qui ne se déclenche pas. Mesuré 2 exécutions, et
//      REJOUABLE : c'est la sonde `f` de `instrument/porte-p0.mjs`, conservée
//      pour cela. **C'est la différence entre un manifeste installable et un
//      manifeste refusé, pas un détail d'écriture.**
//
//   ② LE SEUIL D'ICÔNE EST 144, ET NON 192. Chromium le NOMME dans son
//      relevé : `minimum-icon-size-in-pixels: 144`. La conception de ④ écrit
//      « en dessous de 192 px » pour la ROUGE du critère ①. Sans conséquence
//      sur cette ROUGE, dont l'icône témoin fait 128 — sous les DEUX seuils —,
//      mais un successeur qui poserait 160 en se croyant sous le seuil
//      obtiendrait un VERT et lirait sa rouge ratée comme un produit correct.

/// Ce qu'il faut savoir d'une application pour lui bâtir un manifeste.
export interface Sujet {
    /// L'identifiant de l'application — un UUID engendré par la plateforme.
    id: string;
    nom: string;
    /// Les octets du PNG de l'icône, ou `undefined` s'il n'y en a pas.
    ///
    /// 🔴 SA TAILLE EST LUE DANS SES OCTETS, JAMAIS DÉCLARÉE PAR L'APPELANT.
    /// Un premier jet prenait un `coteIcone` optionnel qui valait 256 par
    /// défaut — la seule taille que le magasin connaisse
    /// (`agent/src/apps/icone/extraction.rs:45`, `const COTE: i32 = 256;`).
    /// **La recette a montré que c'était FAUX** : l'application témoin, dont
    /// l'icône fait 128, publiait un manifeste annonçant `256x256`. Chromium
    /// l'a attrapé — il DÉCODE l'image, et rend `no-acceptable-icon` — mais
    /// **un manifeste qui ment sur ce qu'il porte est un défaut même quand le
    /// navigateur le rattrape** : sur une icône de 200 px annoncée 256, il
    /// aurait accepté, et le système aurait mis à l'échelle une image qu'il
    /// croyait plus grande.
    icone?: Uint8Array;
    /// La couleur d'accent, en `#rrggbb`.
    ///
    /// ⚠️ ELLE N'ARRIVE JAMAIS À CE JOUR, et c'est DÉCLARÉ : la couleur
    /// d'accent par application n'existe nulle part dans ④ — aucune colonne,
    /// aucun champ de protocole, aucun calcul (divergence E1 du plan de G5).
    /// Celle du sous-projet ① est **par FENÊTRE**, arrive en cours de session
    /// sur le canal WebRTC, et ne décrit pas la même chose. Le membre est
    /// OPTIONNEL, et son absence fait OMETTRE `theme_color` — jamais poser une
    /// valeur inventée.
    ///
    /// Comme `fond`, elle vient du thème vivant ou de la base — jamais d'une
    /// constante de ce fichier (voir l'encadré §7.2 ci-dessous).
    accent?: string;
}

/// Le manifeste, dans la forme que `JSON.stringify` publiera.
export interface Manifeste {
    name: string;
    short_name: string;
    id: string;
    start_url: string;
    scope: string;
    display: string;
    display_override: string[];
    background_color: string;
    theme_color?: string;
    icons: { src: string; sizes: string; type: string; purpose: string }[];
}

/// 🔴 IL N'Y A AUCUNE COULEUR ÉCRITE DANS CE FICHIER, ET CE N'EST PAS UN GOÛT :
/// LE CONTRÔLE §7.2 DE ⑥ BALAIE LES `.ts` AUTANT QUE LES `.css`. Un premier
/// jet posait ici `const FOND_MANIFESTE = '#…'` et le contrôle l'a relevé —
/// avec deux littérales du test. **Élargir son exclusion aurait satisfait le
/// contrôle en le VIDANT**, ce que ce dépôt combat ; les couleurs sont donc
/// devenues des PARAMÈTRES, que la page lit sur le thème vivant par
/// `getComputedStyle` (la voie que la spec §4.1 de ⑥ sanctionne, et que
/// `design/galerie.ts` emploie déjà). **Il n'existe qu'une source de vérité
/// pour une couleur, et c'est `tokens.css`.**

/// Le PNG en `data:`, sans dépendance.
///
/// ⚠️ `btoa` ET NON `Buffer` : `client/` n'a pas `@types/node`, et un module
/// écrit avec `Buffer` passe sous Vitest puis CASSE `npm run typecheck` —
/// piège mesuré par le sous-bloc P2 (`TS2580 Cannot find name 'Buffer'`).
export function versDataUrl(octets: Uint8Array): string {
    let binaire = '';
    // Par paquets : `String.fromCharCode(...tres_long)` déborde la pile
    // d'appels, et une icône de 256×256 se compte en dizaines de milliers
    // d'octets.
    const PAQUET = 0x2000;
    for (let i = 0; i < octets.length; i += PAQUET) {
        binaire += String.fromCharCode(...octets.subarray(i, i + PAQUET));
    }
    return `data:image/png;base64,${btoa(binaire)}`;
}

/// Le côté d'un PNG, lu dans son en-tête IHDR — ou `undefined` si ces octets
/// n'en sont pas un.
///
/// La structure est fixe et le restera : signature de 8 octets, puis la
/// longueur (4) et le type (4) du premier morceau, qui DOIT être `IHDR`, puis
/// la largeur (4) et la hauteur (4), en gros-boutiste.
///
/// ⚠️ IL REND LA LARGEUR, et le manifeste s'en sert pour les DEUX dimensions :
/// une icône non carrée y serait mal décrite. Le magasin n'en produit que des
/// carrées, et c'est une propriété de l'AGENT, pas de ce module — d'où le
/// contrôle explicite plutôt qu'une hypothèse tacite.
export function cotePng(octets: Uint8Array): number | undefined {
    const SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    if (octets.length < 24) return undefined;
    for (let i = 0; i < SIGNATURE.length; i += 1) {
        if (octets[i] !== SIGNATURE[i]) return undefined;
    }
    // 'I','H','D','R' aux octets 12 à 15.
    if (octets[12] !== 0x49 || octets[13] !== 0x48 || octets[14] !== 0x44 || octets[15] !== 0x52) {
        return undefined;
    }
    const entier = (d: number): number =>
        octets[d] * 0x1000000 + octets[d + 1] * 0x10000 + octets[d + 2] * 0x100 + octets[d + 3];
    const largeur = entier(16);
    const hauteur = entier(20);
    if (largeur <= 0 || largeur !== hauteur) return undefined;
    return largeur;
}

/// Bâtit le manifeste d'une application.
///
/// `origine` est `location.origin` — SANS barre oblique finale. `fond` est la
/// valeur de `--fond-0` LUE SUR LE THÈME VIVANT, jamais une constante d'ici.
///
/// 🔴 `start_url` POINTE LA PAGE-SHELL, ET NON LE HUB (décision D9 du plan).
/// C'est le seul des trois candidats où **la fenêtre de session** tourne dans
/// une fenêtre de PWA, donc le seul où le legs de S4 — « c'est à SA recette de
/// regarder LA FENÊTRE DE SESSION sous une barre superposée » — puisse être
/// exercé. ⚠️ **Le coût est déclaré** : la session s'ouvre par `window.open`,
/// donc dans une SECONDE fenêtre de la PWA. L'alternative — héberger la
/// session DANS la page-shell — est une refonte du client, hors périmètre.
///
/// 🔴 `scope` EST PARTAGÉ, DONC C'EST `id` QUI PORTE L'IDENTITÉ. Deux
/// applications ne peuvent pas avoir deux `scope` disjoints — elles vivent
/// toutes deux sous `/shell.html`. ⚠️ Que deux manifestes de même `scope` et
/// d'`id` distincts produisent deux applications DISTINCTES **n'est pas
/// mesurable par le montage de G5** : il faudrait en installer deux, et l'hôte
/// n'a pas d'interface graphique.
export function batirManifeste(sujet: Sujet, origine: string, fond: string): Manifeste {
    const base = origine.replace(/\/$/, '');
    const app = encodeURIComponent(sujet.id);
    const manifeste: Manifeste = {
        name: sujet.nom,
        short_name: sujet.nom,
        // L'identité de l'application, distincte pour chacune.
        id: `${base}/shell.html?app=${app}`,
        start_url: `${base}/shell.html?app=${app}`,
        scope: `${base}/`,
        display: 'standalone',
        // La DÉCLARATION du Window Controls Overlay. Ce que G5 apporte est
        // qu'elle soit posée, et il ne prétend pas à plus : le WCO n'existe
        // que dans une fenêtre de PWA installée, et un Chromium sans interface
        // n'en installe aucune. Repli explicite sur `standalone`.
        display_override: ['window-controls-overlay', 'standalone'],
        background_color: fond,
        icons: [],
    };
    if (sujet.accent !== undefined) manifeste.theme_color = sujet.accent;
    // 🔴 UNE ICÔNE DONT ON NE SAIT PAS LIRE LA TAILLE N'EST PAS DÉCLARÉE.
    //    Poser `256x256` par défaut serait affirmer ce qu'on ne sait pas, et
    //    c'est exactement le défaut que la recette a trouvé.
    const cote = sujet.icone === undefined ? undefined : cotePng(sujet.icone);
    if (sujet.icone !== undefined && cote !== undefined) {
        manifeste.icons.push({
            src: versDataUrl(sujet.icone),
            sizes: `${cote}x${cote}`,
            type: 'image/png',
            // `any maskable` ferait rogner l'icône par le système sur les
            // plateformes qui l'appliquent ; le magasin ne garantit aucune
            // zone de sûreté, donc `any` seul.
            purpose: 'any',
        });
    }
    return manifeste;
}
